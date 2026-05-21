use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use axum::http::StatusCode;

use crate::{
    auth::{ApiKeyAuth, MaybeApiKey, MaybeUser, RequireUser},
    AppError, AppState,
};

#[derive(Serialize)]
pub struct RunResponse {
    pub id: Uuid,
    pub project_id: i64,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: String,
    pub outcome: Option<Value>,
    pub wall_time_ms: Option<i64>,
    pub total_cost: Option<Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub sweep_id: Option<Uuid>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdateStatusBody {
    pub status: String,
    pub outcome: Option<Value>,
    pub total_cost: Option<Value>,
    pub wall_time_ms: Option<i64>,
}

pub async fn get_run(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunResponse>, AppError> {
    type RunRow = (
        Uuid,
        i64,
        String,
        String,
        String,
        String,
        Option<Value>,
        Option<i64>,
        Option<Value>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<Uuid>,
        Option<Value>,
        DateTime<Utc>,
        bool,
    );

    let row = sqlx::query_as::<_, RunRow>(
        r#"
        SELECT r.id, r.project_id, r.scenario, r.world, r.backend, r.status::text,
               r.outcome, r.wall_time_ms, r.total_cost,
               r.started_at, r.ended_at, r.sweep_id, r.metadata, r.created_at,
               p.public
        FROM runs r
        JOIN projects p ON p.id = r.project_id
        WHERE r.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let public = row.14;
    if !public && maybe_user.0.is_none() && maybe_key.0.is_none() {
        return Err(AppError::Forbidden);
    }

    Ok(Json(RunResponse {
        id: row.0,
        project_id: row.1,
        scenario: row.2,
        world: row.3,
        backend: row.4,
        status: row.5,
        outcome: row.6,
        wall_time_ms: row.7,
        total_cost: row.8,
        started_at: row.9,
        ended_at: row.10,
        sweep_id: row.11,
        metadata: row.12,
        created_at: row.13,
    }))
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(run_id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let valid_statuses = ["running", "completed", "failed", "cancelled"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "status must be one of: {}",
            valid_statuses.join(", ")
        )));
    }

    // Verify the run exists and the caller has write access
    let (project_id,): (i64,) = sqlx::query_as("SELECT r.project_id FROM runs r WHERE r.id = $1")
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

    let is_terminal = matches!(body.status.as_str(), "completed" | "failed" | "cancelled");

    if is_terminal {
        sqlx::query(
            r#"
            UPDATE runs
            SET status       = $2::run_status,
                outcome      = $3,
                total_cost   = $4,
                wall_time_ms = $5,
                ended_at     = NOW()
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .bind(&body.status)
        .bind(&body.outcome)
        .bind(&body.total_cost)
        .bind(body.wall_time_ms)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    } else {
        sqlx::query(
            r#"
            UPDATE runs
            SET status     = $2::run_status,
                started_at = CASE WHEN status = 'queued' THEN NOW() ELSE started_at END
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .bind(&body.status)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_run(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path(run_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let (project_id,): (i64,) = sqlx::query_as("SELECT project_id FROM runs WHERE id = $1")
        .bind(run_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM org_members om JOIN projects p ON p.org_id = om.org_id WHERE p.id = $1 AND om.user_id = $2)",
    )
    .bind(project_id)
    .bind(user.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if !is_member {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM runs WHERE id = $1")
        .bind(run_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok(StatusCode::NO_CONTENT)
}
