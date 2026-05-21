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
pub struct ListEventsQuery {
    pub since: Option<i64>,
}

#[derive(Serialize)]
pub struct EventResponse {
    pub run_id: Uuid,
    pub sequence_number: i64,
    pub kind: String,
    pub payload: Value,
    pub event_id: Uuid,
    pub wall_time_ms: Option<i64>,
}

#[derive(Deserialize)]
pub struct AppendEventsBody {
    pub events: Vec<AppendEventItem>,
}

#[derive(Deserialize)]
pub struct AppendEventItem {
    pub sequence_number: i64,
    pub kind: String,
    pub payload: Value,
    pub event_id: Uuid,
    pub wall_time_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct AppendEventsResponse {
    pub accepted: usize,
}

pub async fn list_events(
    State(_state): State<AppState>,
    maybe_user: MaybeUser,
    Path(run_id): Path<Uuid>,
    Query(q): Query<ListEventsQuery>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn append_events(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(run_id): Path<Uuid>,
    Json(body): Json<AppendEventsBody>,
) -> Result<Json<AppendEventsResponse>, AppError> {
    Err(AppError::NotImplemented)
}
