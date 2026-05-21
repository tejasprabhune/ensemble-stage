#![allow(unused_variables)]

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{auth::RequireUser, AppError, AppState};

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
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateApiKeyBody {
    pub name: String,
    pub scope: String,
    pub expires_at: Option<String>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub key: String,
}

pub async fn get_me(
    State(_state): State<AppState>,
    RequireUser(user): RequireUser,
) -> Result<Json<MeResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn list_api_keys(
    State(_state): State<AppState>,
    RequireUser(user): RequireUser,
) -> Result<Json<Vec<ApiKeyResponse>>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create_api_key(
    State(_state): State<AppState>,
    RequireUser(user): RequireUser,
    Json(body): Json<CreateApiKeyBody>,
) -> Result<Json<CreateApiKeyResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn revoke_api_key(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}
