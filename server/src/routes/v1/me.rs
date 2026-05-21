use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{middleware::hash_api_key, MaybeApiKey, MaybeUser},
    models::user::ApiKeyScope,
    AppError, AppState,
};

#[derive(Serialize)]
pub struct MeResponse {
    pub id: i64,
    pub github_login: String,
    pub email: Option<String>,
    pub default_org_slug: String,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct CreateApiKeyBody {
    pub name: String,
    pub scope: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub key: String,
}

fn resolve_caller(maybe_user: &MaybeUser, maybe_key: &MaybeApiKey) -> Result<i64, AppError> {
    if let Some(user) = &maybe_user.0 {
        return Ok(user.user_id);
    }
    if let Some(key) = &maybe_key.0 {
        return Ok(key.user_id);
    }
    Err(AppError::Unauthorized)
}

fn resolve_admin_caller(maybe_user: &MaybeUser, maybe_key: &MaybeApiKey) -> Result<i64, AppError> {
    if let Some(user) = &maybe_user.0 {
        return Ok(user.user_id);
    }
    if let Some(key) = &maybe_key.0 {
        if key.scope == ApiKeyScope::Admin {
            return Ok(key.user_id);
        }
        return Err(AppError::Forbidden);
    }
    Err(AppError::Unauthorized)
}

pub async fn get_me(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
) -> Result<Json<MeResponse>, AppError> {
    let user_id = resolve_caller(&maybe_user, &maybe_key)?;

    let row = sqlx::query_as::<_, (i64, String, Option<String>, String)>(
        r#"
        SELECT u.id, u.github_login, u.email, o.slug
        FROM users u
        JOIN orgs o ON o.id = u.default_org_id
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    Ok(Json(MeResponse {
        id: row.0,
        github_login: row.1,
        email: row.2,
        default_org_slug: row.3,
    }))
}

pub async fn list_api_keys(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
) -> Result<Json<Vec<ApiKeyResponse>>, AppError> {
    let user_id = resolve_admin_caller(&maybe_user, &maybe_key)?;

    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            Option<DateTime<Utc>>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT id, name, scope::text, last_used_at, created_at, expires_at
        FROM api_keys
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let keys = rows
        .into_iter()
        .map(|r| ApiKeyResponse {
            id: r.0,
            name: r.1,
            scope: r.2,
            last_used_at: r.3,
            created_at: r.4,
            expires_at: r.5,
        })
        .collect();

    Ok(Json(keys))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    _maybe_key: MaybeApiKey,
    Json(body): Json<CreateApiKeyBody>,
) -> Result<impl IntoResponse, AppError> {
    // POST /v1/me/api_keys requires a session cookie, not an API key
    let user = maybe_user.0.ok_or(AppError::Unauthorized)?;

    let scope = match body.scope.as_str() {
        "push" => ApiKeyScope::Push,
        "admin" => ApiKeyScope::Admin,
        _ => {
            return Err(AppError::BadRequest(
                "scope must be 'push' or 'admin'".into(),
            ))
        }
    };

    let prefix = match scope {
        ApiKeyScope::Push => "stage_sk_",
        ApiKeyScope::Admin => "stage_ak_",
    };

    let raw_key = format!(
        "{}{}{}",
        prefix,
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple(),
    );
    let key_hash = hash_api_key(&raw_key);

    let scope_str = match scope {
        ApiKeyScope::Push => "push",
        ApiKeyScope::Admin => "admin",
    };

    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO api_keys (user_id, scope, name, key_hash, expires_at)
        VALUES ($1, $2::api_key_scope, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user.user_id)
    .bind(scope_str)
    .bind(&body.name)
    .bind(&key_hash)
    .bind(body.expires_at)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id,
            name: body.name,
            scope: scope_str.to_string(),
            key: raw_key,
        }),
    ))
}

pub async fn revoke_api_key(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    _maybe_key: MaybeApiKey,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = maybe_user.0.ok_or(AppError::Unauthorized)?;

    sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.user_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
