#![allow(unused_variables)]

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{AppError, AppState, auth::{ApiKeyAuth, MaybeUser}};

#[derive(Deserialize)]
pub struct ListRunsQuery {
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub org_slug: String,
    pub project_slug: String,
    pub name: String,
    pub public: bool,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct RunSummary {
    pub id: Uuid,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub wall_time_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct ListRunsResponse {
    pub runs: Vec<RunSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRunBody {
    pub id: Option<Uuid>,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub sweep_id: Option<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Serialize)]
pub struct CreateRunResponse {
    pub id: Uuid,
    pub url: String,
}

#[derive(Deserialize)]
pub struct CreateSweepBody {
    pub config: Value,
}

#[derive(Serialize)]
pub struct CreateSweepResponse {
    pub id: Uuid,
    pub url: String,
}

#[derive(Deserialize)]
pub struct CreateTrainingRunBody {
    pub persona_name: String,
    pub base_model: String,
    pub hyperparameters: Option<Value>,
}

#[derive(Serialize)]
pub struct CreateTrainingRunResponse {
    pub id: Uuid,
    pub url: String,
}

pub async fn get_project(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<ProjectResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn list_runs(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<ListRunsQuery>,
) -> Result<Json<ListRunsResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateRunBody>,
) -> Result<Json<CreateRunResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create_sweep(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateSweepBody>,
) -> Result<Json<CreateSweepResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create_training_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateTrainingRunBody>,
) -> Result<Json<CreateTrainingRunResponse>, AppError> {
    Err(AppError::NotImplemented)
}
