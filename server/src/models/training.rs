use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrainingRun {
    pub id: Uuid,
    pub project_id: i64,
    pub persona_name: String,
    pub base_model: String,
    pub status: TrainingRunStatus,
    pub hyperparameters: Option<Value>,
    pub final_metrics: Option<Value>,
    pub artifact_uri: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_by_user_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "training_run_status", rename_all = "lowercase")]
pub enum TrainingRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrainingMetric {
    pub training_run_id: Uuid,
    pub step: i64,
    pub metric_name: String,
    pub value: f64,
    pub recorded_at: DateTime<Utc>,
}
