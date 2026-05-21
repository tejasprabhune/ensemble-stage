use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Org {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub github_org_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrgMember {
    pub org_id: i64,
    pub user_id: i64,
    pub role: OrgRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "org_role", rename_all = "lowercase")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
}
