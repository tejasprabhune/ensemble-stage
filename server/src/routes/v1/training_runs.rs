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

pub async fn get_past_runs(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    type TrMeta = (String, i64, bool);
    let (persona_name, project_id, public) = sqlx::query_as::<_, TrMeta>(
        "SELECT tr.persona_name, tr.project_id, p.public FROM training_runs tr JOIN projects p ON p.id = tr.project_id WHERE tr.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let caller_id = maybe_user.0.as_ref().map(|u| u.user_id)
        .or_else(|| maybe_key.0.as_ref().map(|k| k.user_id));
    if !public {
        let uid = caller_id.ok_or(AppError::Forbidden)?;
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM org_members om JOIN projects p ON p.org_id = om.org_id WHERE p.id = $1 AND om.user_id = $2)",
        )
        .bind(project_id)
        .bind(uid)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;
        if !is_member { return Err(AppError::Forbidden); }
    }

    type PastRow = (Uuid, String, String, Option<Value>, Option<Value>, Option<String>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, DateTime<Utc>);
    let past = sqlx::query_as::<_, PastRow>(
        r#"
        SELECT id, base_model, status::text, hyperparameters, final_metrics, artifact_uri,
               started_at, ended_at, created_at
        FROM training_runs
        WHERE persona_name = $1 AND project_id = $2 AND id != $3
          AND status = 'completed'::training_run_status
        ORDER BY created_at DESC
        LIMIT 5
        "#,
    )
    .bind(&persona_name)
    .bind(project_id)
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    type MetricRow = (i64, String, f64);
    let mut runs = Vec::new();
    for row in past {
        let metrics = sqlx::query_as::<_, MetricRow>(
            "SELECT step, metric_name, value FROM training_metrics WHERE training_run_id = $1 ORDER BY step ASC",
        )
        .bind(row.0)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

        runs.push(serde_json::json!({
            "id": row.0,
            "base_model": row.1,
            "status": row.2,
            "hyperparameters": row.3,
            "final_metrics": row.4,
            "artifact_uri": row.5,
            "started_at": row.6,
            "ended_at": row.7,
            "created_at": row.8,
            "metrics": metrics.into_iter().map(|m| serde_json::json!({
                "step": m.0,
                "metric_name": m.1,
                "value": m.2,
            })).collect::<Vec<_>>(),
        }));
    }

    Ok(Json(serde_json::json!({ "past_runs": runs, "persona_name": persona_name })))
}

pub async fn baseline_comparison(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    type TrMeta = (Option<String>, String, i64, bool);
    let (artifact_uri, persona_name, project_id, public) = sqlx::query_as::<_, TrMeta>(
        "SELECT tr.artifact_uri, tr.persona_name, tr.project_id, p.public FROM training_runs tr JOIN projects p ON p.id = tr.project_id WHERE tr.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let caller_id = maybe_user.0.as_ref().map(|u| u.user_id)
        .or_else(|| maybe_key.0.as_ref().map(|k| k.user_id));
    if !public {
        let uid = caller_id.ok_or(AppError::Forbidden)?;
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM org_members om JOIN projects p ON p.org_id = om.org_id WHERE p.id = $1 AND om.user_id = $2)",
        )
        .bind(project_id)
        .bind(uid)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;
        if !is_member { return Err(AppError::Forbidden); }
    }

    let artifact_uri = match artifact_uri {
        Some(uri) if !uri.is_empty() => uri,
        _ => return Ok(Json(serde_json::json!({
            "linked": false,
            "reason": "no_artifact",
            "entries": [],
        }))),
    };

    type RunRow = (Uuid, String, String, Option<Value>);
    let trained_runs = sqlx::query_as::<_, RunRow>(
        r#"
        SELECT id, scenario, backend, outcome
        FROM runs
        WHERE project_id = $1
          AND metadata->>'adapter_uri' = $2
          AND status = 'completed'
        "#,
    )
    .bind(project_id)
    .bind(&artifact_uri)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if trained_runs.is_empty() {
        return Ok(Json(serde_json::json!({
            "linked": false,
            "reason": "no_trained_runs",
            "entries": [],
        })));
    }

    use std::collections::HashMap;
    let mut groups: HashMap<(String, String), Vec<(Uuid, Option<Value>)>> = HashMap::new();
    for (run_id, scenario, backend, outcome) in &trained_runs {
        groups.entry((scenario.clone(), backend.clone()))
            .or_default()
            .push((*run_id, outcome.clone()));
    }

    let mut entries = Vec::new();
    for ((scenario, backend), t_runs) in &groups {
        let baseline_runs = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT id, scenario, backend, outcome
            FROM runs
            WHERE project_id = $1
              AND scenario = $2
              AND backend = $3
              AND status = 'completed'
              AND (metadata->>'adapter_uri' IS NULL OR metadata->>'adapter_uri' = '')
              AND metadata->>'persona' = $4
            "#,
        )
        .bind(project_id)
        .bind(scenario)
        .bind(backend)
        .bind(&persona_name)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

        let trained_scores: Vec<f64> = t_runs.iter()
            .filter_map(|(_, outcome)| {
                outcome.as_ref()
                    .and_then(|o| o.as_object())
                    .and_then(|m| m.values().next())
                    .and_then(|v| v.as_f64())
            })
            .collect();

        let baseline_scores: Vec<f64> = baseline_runs.iter()
            .filter_map(|(_, _, _, outcome)| {
                outcome.as_ref()
                    .and_then(|o| o.as_object())
                    .and_then(|m| m.values().next())
                    .and_then(|v| v.as_f64())
            })
            .collect();

        let trained_mean = if trained_scores.is_empty() { None } else {
            Some(trained_scores.iter().sum::<f64>() / trained_scores.len() as f64)
        };
        let baseline_mean = if baseline_scores.is_empty() { None } else {
            Some(baseline_scores.iter().sum::<f64>() / baseline_scores.len() as f64)
        };
        let delta = match (trained_mean, baseline_mean) {
            (Some(t), Some(b)) => Some(t - b),
            _ => None,
        };

        let baseline_run_ids: Vec<Uuid> = baseline_runs.iter().map(|(id, _, _, _)| *id).collect();
        let trained_run_ids: Vec<Uuid> = t_runs.iter().map(|(id, _)| *id).collect();
        let no_baseline = baseline_runs.is_empty();

        entries.push(serde_json::json!({
            "scenario": scenario,
            "model": backend,
            "trained_mean": trained_mean,
            "baseline_mean": baseline_mean,
            "delta": delta,
            "trained_run_count": trained_run_ids.len(),
            "baseline_run_count": baseline_run_ids.len(),
            "no_baseline": no_baseline,
        }));
    }

    entries.sort_by(|a, b| {
        let ad = a["delta"].as_f64().map(|v| v.abs()).unwrap_or(0.0);
        let bd = b["delta"].as_f64().map(|v| v.abs()).unwrap_or(0.0);
        bd.partial_cmp(&ad).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Json(serde_json::json!({
        "linked": true,
        "entries": entries,
    })))
}

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
