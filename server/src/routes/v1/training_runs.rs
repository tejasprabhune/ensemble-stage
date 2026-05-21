use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{AppError, AppState, auth::{ApiKeyAuth, MaybeUser}};

#[derive(Serialize)]
pub struct TrainingRunResponse {
    pub id: Uuid,
    pub persona_name: String,
    pub base_model: String,
    pub status: String,
    pub hyperparameters: Option<Value>,
    pub final_metrics: Option<Value>,
    pub artifact_uri: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

#[derive(Deserialize)]
pub struct AppendMetricsBody {
    pub metrics: Vec<MetricPoint>,
}

#[derive(Deserialize)]
pub struct MetricPoint {
    pub step: i64,
    pub metric_name: String,
    pub value: f64,
}

#[derive(Deserialize)]
pub struct UpdateStatusBody {
    pub status: String,
    pub final_metrics: Option<Value>,
    pub artifact_uri: Option<String>,
}

pub async fn get_training_run(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingRunResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn append_metrics(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<AppendMetricsBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotImplemented)
}
