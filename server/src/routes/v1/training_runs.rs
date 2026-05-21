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
pub struct TrainingRunResponse {
    pub id: Uuid,
    pub project_id: i64,
    pub persona_name: String,
    pub base_model: String,
    pub status: String,
    pub hyperparameters: Option<Value>,
    pub final_metrics: Option<Value>,
    pub artifact_uri: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
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
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingRunResponse>, AppError> {
    type TrRow = (
        Uuid,
        i64,
        String,
        String,
        String,
        Option<Value>,
        Option<Value>,
        Option<String>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        bool,
    );

    let row = sqlx::query_as::<_, TrRow>(
        r#"
        SELECT tr.id, tr.project_id, tr.persona_name, tr.base_model, tr.status::text,
               tr.hyperparameters, tr.final_metrics, tr.artifact_uri,
               tr.started_at, tr.ended_at, tr.created_at, p.public
        FROM training_runs tr
        JOIN projects p ON p.id = tr.project_id
        WHERE tr.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    if !row.11 && maybe_user.0.is_none() && maybe_key.0.is_none() {
        return Err(AppError::Forbidden);
    }

    Ok(Json(TrainingRunResponse {
        id: row.0,
        project_id: row.1,
        persona_name: row.2,
        base_model: row.3,
        status: row.4,
        hyperparameters: row.5,
        final_metrics: row.6,
        artifact_uri: row.7,
        started_at: row.8,
        ended_at: row.9,
        created_at: row.10,
    }))
}

pub async fn append_metrics(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<AppendMetricsBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.metrics.len() > 1000 {
        return Err(AppError::BadRequest(
            "a maximum of 1000 metric points may be sent per request".into(),
        ));
    }

    let project_id: i64 = sqlx::query_scalar("SELECT project_id FROM training_runs WHERE id = $1")
        .bind(id)
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
    let mut accepted = 0i64;

    for metric in &body.metrics {
        let result = sqlx::query(
            r#"
            INSERT INTO training_metrics (training_run_id, step, metric_name, value)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (training_run_id, step, metric_name) DO NOTHING
            "#,
        )
        .bind(id)
        .bind(metric.step)
        .bind(&metric.metric_name)
        .bind(metric.value)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        if result.rows_affected() == 1 {
            accepted += 1;
        }
    }

    tx.commit().await.map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({ "accepted": accepted })))
}

pub async fn update_status(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let valid_statuses = ["running", "completed", "failed", "cancelled"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "status must be one of: {}",
            valid_statuses.join(", ")
        )));
    }

    let project_id: i64 = sqlx::query_scalar("SELECT project_id FROM training_runs WHERE id = $1")
        .bind(id)
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
            UPDATE training_runs
            SET status        = $2::training_run_status,
                final_metrics = COALESCE($3, final_metrics),
                artifact_uri  = COALESCE($4, artifact_uri),
                ended_at      = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&body.status)
        .bind(&body.final_metrics)
        .bind(&body.artifact_uri)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    } else {
        sqlx::query(
            r#"
            UPDATE training_runs
            SET status     = $2::training_run_status,
                started_at = COALESCE(started_at, NOW())
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&body.status)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
