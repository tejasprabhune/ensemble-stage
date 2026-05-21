use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;

use crate::{auth::middleware::SessionClaims, AppError, AppState};

fn github_auth_url(state: &AppState, csrf: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user+user:email&state={}",
        state.config.github_client_id,
        urlencoding::encode(&state.config.github_callback_url()),
        csrf,
    )
}

pub async fn login(State(state): State<AppState>) -> impl IntoResponse {
    let csrf = uuid::Uuid::new_v4().to_string();
    Redirect::to(&github_auth_url(&state, &csrf))
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
) -> Result<impl IntoResponse, AppError> {
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
        .map_err(|e| AppError::Internal(anyhow::anyhow!("token exchange: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("token parse: {}", e)))?;

    let github_user: GithubUser = http
        .get("https://api.github.com/user")
        .bearer_auth(&token_res.access_token)
        .header("User-Agent", "ensemble-stage/1.0")
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("user fetch: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("user parse: {}", e)))?;

    let user_id = upsert_user(&state, &github_user).await?;

    let exp = (Utc::now() + chrono::Duration::days(30)).timestamp();
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt encode: {}", e)))?;

    let mut cookie = Cookie::new("stage_session", token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::days(30));

    Ok((jar.add(cookie), Redirect::to("/")))
}

async fn upsert_user(state: &AppState, github_user: &GithubUser) -> Result<i64, AppError> {
    // Create the personal org first so the FK on users.default_org_id is
    // satisfied when we insert the user row.
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
          SET github_login  = EXCLUDED.github_login,
              email         = COALESCE(EXCLUDED.email, users.email),
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
