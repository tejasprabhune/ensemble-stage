use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Run {
    pub id: Uuid,
    pub project_id: i64,
    pub sweep_id: Option<Uuid>,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: RunStatus,
    pub outcome: Option<Value>,
    pub wall_time_ms: Option<i64>,
    pub total_cost: Option<Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_by_user_id: i64,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "run_status", rename_all = "lowercase")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RunEvent {
    pub run_id: Uuid,
    pub sequence_number: i64,
    pub kind: String,
    pub payload: Value,
    pub event_id: Uuid,
    pub wall_time_ms: Option<i64>,
}
