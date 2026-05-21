use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Artifact {
    pub id: i64,
    pub project_id: i64,
    pub kind: ArtifactKind,
    pub uri: String,
    pub size_bytes: Option<i64>,
    pub created_by_run_id: Option<Uuid>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "artifact_kind", rename_all = "lowercase")]
pub enum ArtifactKind {
    Adapter,
    Dataset,
    Trace,
    Baseline,
}
