use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comparison {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub run_ids: Vec<Uuid>,
    pub view_state: Option<Value>,
    pub created_by_user_id: i64,
    pub created_at: DateTime<Utc>,
}
