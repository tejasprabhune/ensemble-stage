use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{ApiKeyAuth, MaybeApiKey, MaybeUser},
    AppError, AppState,
};

#[derive(Serialize)]
pub struct SweepResponse {
    pub id: Uuid,
    pub project_id: i64,
    pub config: Value,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
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
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(sweep_id): Path<Uuid>,
) -> Result<Json<SweepResponse>, AppError> {
    type SweepRow = (
        Uuid,
        i64,
        Value,
        String,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        bool,
    );

    let row = sqlx::query_as::<_, SweepRow>(
        r#"
        SELECT s.id, s.project_id, s.config, s.status::text,
               s.started_at, s.ended_at, s.created_at, p.public
        FROM sweeps s
        JOIN projects p ON p.id = s.project_id
        WHERE s.id = $1
        "#,
    )
    .bind(sweep_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    if !row.7 && maybe_user.0.is_none() && maybe_key.0.is_none() {
        return Err(AppError::Forbidden);
    }

    Ok(Json(SweepResponse {
        id: row.0,
        project_id: row.1,
        config: row.2,
        status: row.3,
        started_at: row.4,
        ended_at: row.5,
        created_at: row.6,
    }))
}

pub async fn register_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(sweep_id): Path<Uuid>,
    Json(body): Json<RegisterRunBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify sweep and run exist in the same project, and caller has access
    let sweep_project_id: i64 =
        sqlx::query_scalar("SELECT project_id FROM sweeps WHERE id = $1")
            .bind(sweep_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;

    let run_project_id: i64 =
        sqlx::query_scalar("SELECT project_id FROM runs WHERE id = $1")
            .bind(body.run_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;

    if sweep_project_id != run_project_id {
        return Err(AppError::BadRequest(
            "run and sweep must belong to the same project".into(),
        ));
    }

    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM org_members om JOIN projects p ON p.org_id = om.org_id WHERE p.id = $1 AND om.user_id = $2)",
    )
    .bind(sweep_project_id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if !is_member {
        return Err(AppError::Forbidden);
    }

    sqlx::query("UPDATE runs SET sweep_id = $1 WHERE id = $2")
        .bind(sweep_id)
        .bind(body.run_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(sweep_id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let valid_statuses = ["running", "completed", "failed", "cancelled"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "status must be one of: {}",
            valid_statuses.join(", ")
        )));
    }

    let project_id: i64 = sqlx::query_scalar("SELECT project_id FROM sweeps WHERE id = $1")
        .bind(sweep_id)
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
            "UPDATE sweeps SET status = $2::sweep_status, ended_at = NOW() WHERE id = $1",
        )
        .bind(sweep_id)
        .bind(&body.status)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    } else {
        sqlx::query(
            "UPDATE sweeps SET status = $2::sweep_status, started_at = COALESCE(started_at, NOW()) WHERE id = $1",
        )
        .bind(sweep_id)
        .bind(&body.status)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
