use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Sweep {
    pub id: Uuid,
    pub project_id: i64,
    pub config: Value,
    pub status: SweepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_by_user_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sweep_status", rename_all = "lowercase")]
pub enum SweepStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}
