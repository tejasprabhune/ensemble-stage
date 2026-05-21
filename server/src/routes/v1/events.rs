use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{ApiKeyAuth, MaybeApiKey, MaybeUser},
    AppError, AppState,
};

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
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(run_id): Path<Uuid>,
    Query(q): Query<ListEventsQuery>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let public: bool = sqlx::query_scalar(
        "SELECT p.public FROM runs r JOIN projects p ON p.id = r.project_id WHERE r.id = $1",
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    if !public && maybe_user.0.is_none() && maybe_key.0.is_none() {
        return Err(AppError::Forbidden);
    }

    let since = q.since.unwrap_or(-1);

    type EventRow = (Uuid, i64, String, Value, Uuid, Option<i64>);

    let rows = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT run_id, sequence_number, kind, payload, event_id, wall_time_ms
        FROM run_events
        WHERE run_id = $1 AND sequence_number > $2
        ORDER BY sequence_number ASC
        "#,
    )
    .bind(run_id)
    .bind(since)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let events = rows
        .into_iter()
        .map(|r| EventResponse {
            run_id: r.0,
            sequence_number: r.1,
            kind: r.2,
            payload: r.3,
            event_id: r.4,
            wall_time_ms: r.5,
        })
        .collect();

    Ok(Json(events))
}

pub async fn append_events(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(run_id): Path<Uuid>,
    Json(body): Json<AppendEventsBody>,
) -> Result<Json<AppendEventsResponse>, AppError> {
    if body.events.len() > 500 {
        return Err(AppError::BadRequest(
            "a maximum of 500 events may be sent per request".into(),
        ));
    }

    // Verify run exists and caller has access
    let project_id: i64 = sqlx::query_scalar("SELECT project_id FROM runs WHERE id = $1")
        .bind(run_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM org_members om JOIN projects p ON p.org_id = om.org_id WHERE p.id = $1 AND om.user_id = $2)",
    )
    .bind(project_id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if !is_member {
        return Err(AppError::Forbidden);
    }

    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;
    let mut accepted = 0usize;

    for event in &body.events {
        let result = sqlx::query(
            r#"
            INSERT INTO run_events (run_id, sequence_number, kind, payload, event_id, wall_time_ms)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (run_id, event_id) DO NOTHING
            "#,
        )
        .bind(run_id)
        .bind(event.sequence_number)
        .bind(&event.kind)
        .bind(&event.payload)
        .bind(event.event_id)
        .bind(event.wall_time_ms)
        .execute(&mut *tx)
        .await;

        match result {
            Ok(r) if r.rows_affected() == 1 => accepted += 1,
            Ok(_) => {}
            Err(sqlx::Error::Database(ref e)) if e.constraint() == Some("run_events_pkey") => {
                return Err(AppError::Conflict(format!(
                    "duplicate sequence_number {} in this run",
                    event.sequence_number
                )));
            }
            Err(e) => return Err(AppError::Database(e)),
        }
    }

    tx.commit().await.map_err(AppError::Database)?;

    Ok(Json(AppendEventsResponse { accepted }))
}
