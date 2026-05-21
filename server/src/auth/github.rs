use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;

use crate::{auth::middleware::hash_api_key, auth::middleware::SessionClaims, AppError, AppState};

fn github_auth_url(state: &AppState, csrf: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user+user:email&state={}",
        state.config.github_client_id,
        urlencoding::encode(&state.config.github_callback_url()),
        csrf,
    )
}

pub async fn login(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let csrf = uuid::Uuid::new_v4().to_string();
    let mut csrf_cookie = Cookie::new("stage_csrf", csrf.clone());
    csrf_cookie.set_http_only(true);
    csrf_cookie.set_same_site(SameSite::Lax);
    csrf_cookie.set_path("/auth/github/callback");
    csrf_cookie.set_max_age(time::Duration::minutes(10));
    (
        jar.add(csrf_cookie),
        Redirect::to(&github_auth_url(&state, &csrf)),
    )
}

#[derive(Deserialize)]
pub struct CliLoginQuery {
    pub callback: String,
}

pub async fn cli_login(
    State(state): State<AppState>,
    Query(query): Query<CliLoginQuery>,
    jar: CookieJar,
) -> impl IntoResponse {
    let csrf = uuid::Uuid::new_v4().to_string();

    let mut csrf_cookie = Cookie::new("stage_csrf", csrf.clone());
    csrf_cookie.set_http_only(true);
    csrf_cookie.set_same_site(SameSite::Lax);
    csrf_cookie.set_path("/auth/github/callback");
    csrf_cookie.set_max_age(time::Duration::minutes(10));

    let mut cli_cookie = Cookie::new("stage_cli_callback", query.callback);
    cli_cookie.set_http_only(true);
    cli_cookie.set_same_site(SameSite::Lax);
    cli_cookie.set_path("/auth/github/callback");
    cli_cookie.set_max_age(time::Duration::minutes(10));

    (
        jar.add(csrf_cookie).add(cli_cookie),
        Redirect::to(&github_auth_url(&state, &csrf)),
    )
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
    email: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
    jar: CookieJar,
) -> Result<axum::response::Response, AppError> {
    let csrf_cookie = jar.get("stage_csrf").map(|c| c.value().to_string());
    let csrf_state = query.state.as_deref().unwrap_or("");
    if csrf_cookie.as_deref() != Some(csrf_state) || csrf_state.is_empty() {
        return Err(AppError::BadRequest("invalid csrf state".into()));
    }

    let http = reqwest::Client::new();

    let token_res: GithubTokenResponse = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "ensemble-stage/1.0")
        .json(&serde_json::json!({
            "client_id": state.config.github_client_id,
            "client_secret": state.config.github_client_secret,
            "code": query.code,
        }))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("token exchange: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("token parse: {e}")))?;

    let github_user: GithubUser = http
        .get("https://api.github.com/user")
        .bearer_auth(&token_res.access_token)
        .header("User-Agent", "ensemble-stage/1.0")
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("user fetch: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("user parse: {e}")))?;

    let user_id = upsert_user(&state, &github_user).await?;

    let exp = (Utc::now() + chrono::Duration::days(30)).timestamp();
    let login = github_user.login.clone();
    let claims = SessionClaims {
        sub: user_id.to_string(),
        login: github_user.login,
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt encode: {e}")))?;

    let mut session_cookie = Cookie::new("stage_session", token);
    session_cookie.set_http_only(true);
    session_cookie.set_same_site(SameSite::Lax);
    session_cookie.set_path("/");
    session_cookie.set_max_age(time::Duration::days(30));

    let remove_csrf = Cookie::build(("stage_csrf", ""))
        .path("/auth/github/callback")
        .build();
    let remove_cli = Cookie::build(("stage_cli_callback", ""))
        .path("/auth/github/callback")
        .build();

    let cli_callback = jar.get("stage_cli_callback").map(|c| c.value().to_string());

    if let Some(callback_url) = cli_callback {
        let raw_key = format!(
            "stage_sk_{}{}",
            uuid::Uuid::new_v4().as_simple(),
            uuid::Uuid::new_v4().as_simple(),
        );
        let key_hash = hash_api_key(&raw_key);
        sqlx::query(
            "INSERT INTO api_keys (user_id, scope, name, key_hash) VALUES ($1, 'push', 'cli-login', $2)",
        )
        .bind(user_id)
        .bind(&key_hash)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

        let redirect_url = format!("{}?api_key={}", callback_url, urlencoding::encode(&raw_key),);
        return Ok((
            jar.remove(remove_csrf).remove(remove_cli),
            Redirect::to(&redirect_url),
        )
            .into_response());
    }

    let dest = format!("/{login}");

    Ok((
        jar.add(session_cookie)
            .remove(remove_csrf)
            .remove(remove_cli),
        Redirect::to(&dest),
    )
        .into_response())
}

async fn upsert_user(state: &AppState, github_user: &GithubUser) -> Result<i64, AppError> {
    let org_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO orgs (slug, name)
        VALUES ($1, $1)
        ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
    )
    .bind(&github_user.login)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let user_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO users (github_id, github_login, email, default_org_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (github_id) DO UPDATE
          SET github_login   = EXCLUDED.github_login,
              email          = COALESCE(EXCLUDED.email, users.email),
              default_org_id = EXCLUDED.default_org_id
        RETURNING id
        "#,
    )
    .bind(github_user.id)
    .bind(&github_user.login)
    .bind(&github_user.email)
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::Database)?;

    Ok(user_id)
}

pub async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("stage_session", "")).path("/").build();
    (jar.remove(cookie), Redirect::to("/"))
}
