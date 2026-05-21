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
pub struct SweepResponse {
    pub id: Uuid,
    pub config: Value,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterRunBody {
    pub run_id: Uuid,
}

#[derive(Deserialize)]
pub struct UpdateStatusBody {
    pub status: String,
}

pub async fn get_sweep(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path(sweep_id): Path<Uuid>,
) -> Result<Json<SweepResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn register_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(sweep_id): Path<Uuid>,
    Json(body): Json<RegisterRunBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(sweep_id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}
