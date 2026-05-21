#![allow(unused_variables)]

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{ApiKeyAuth, MaybeUser},
    AppError, AppState,
};

#[derive(Serialize)]
pub struct RunResponse {
    pub id: Uuid,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: String,
    pub outcome: Option<Value>,
    pub wall_time_ms: Option<i64>,
    pub total_cost: Option<Value>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Deserialize)]
pub struct UpdateStatusBody {
    pub status: String,
    pub outcome: Option<Value>,
    pub total_cost: Option<Value>,
    pub wall_time_ms: Option<i64>,
}

pub async fn get_run(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(run_id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}
