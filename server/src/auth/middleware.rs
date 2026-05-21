use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use axum_extra::extract::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::{AppError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: String,
    pub login: String,
    pub exp: i64,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub github_login: String,
}

#[derive(Debug, Clone)]
pub struct RequireUser(pub AuthUser);

#[derive(Debug, Clone)]
pub struct MaybeUser(pub Option<AuthUser>);

#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    pub user_id: i64,
    pub scope: crate::models::user::ApiKeyScope,
}

fn extract_user_from_jar(jar: &CookieJar, secret: &str) -> Option<AuthUser> {
    let token = jar.get("stage_session")?.value();
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<SessionClaims>(token, &key, &validation).ok()?;
    let user_id = data.claims.sub.parse().ok()?;
    Some(AuthUser {
        user_id,
        github_login: data.claims.login,
    })
}

#[async_trait]
impl<S> FromRequestParts<S> for RequireUser
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let app_state = AppState::from_ref(state);
        let jar = CookieJar::from_request_parts(parts, state).await.unwrap();
        let user = extract_user_from_jar(&jar, &app_state.config.jwt_secret)
            .ok_or(AppError::Unauthorized)?;
        Ok(RequireUser(user))
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for MaybeUser
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let app_state = AppState::from_ref(state);
        let jar = CookieJar::from_request_parts(parts, state).await.unwrap();
        let user = extract_user_from_jar(&jar, &app_state.config.jwt_secret);
        Ok(MaybeUser(user))
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let app_state = AppState::from_ref(state);
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let raw_key = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        let key_hash = hash_api_key(raw_key);

        let row = sqlx::query_as::<_, (i64, String)>(
            "SELECT user_id, scope FROM api_keys WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > NOW())"
        )
        .bind(&key_hash)
        .fetch_optional(&app_state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::Unauthorized)?;

        sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE key_hash = $1")
            .bind(&key_hash)
            .execute(&app_state.pool)
            .await
            .ok();

        let scope = match row.1.as_str() {
            "push" => crate::models::user::ApiKeyScope::Push,
            "admin" => crate::models::user::ApiKeyScope::Admin,
            _ => return Err(AppError::Unauthorized),
        };

        Ok(ApiKeyAuth {
            user_id: row.0,
            scope,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MaybeApiKey(pub Option<ApiKeyAuth>);

#[async_trait]
impl<S> FromRequestParts<S> for MaybeApiKey
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match ApiKeyAuth::from_request_parts(parts, state).await {
            Ok(auth) => Ok(MaybeApiKey(Some(auth))),
            Err(_) => Ok(MaybeApiKey(None)),
        }
    }
}

pub fn hash_api_key(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let result = hasher.finalize();
    let mut s = String::new();
    for byte in result {
        write!(s, "{byte:02x}").unwrap();
    }
    s
}
