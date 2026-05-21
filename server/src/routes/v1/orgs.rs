use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{MaybeApiKey, MaybeUser},
    models::user::ApiKeyScope,
    AppError, AppState,
};

#[derive(Deserialize)]
pub struct CreateProjectBody {
    pub slug: String,
    pub name: String,
    pub public: bool,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectMetadata {
    pub org_slug: String,
    pub project_slug: String,
    pub name: String,
    pub public: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub url: String,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub slug: String,
    pub name: String,
    pub projects: Vec<ProjectSummary>,
}

#[derive(Serialize)]
pub struct ProjectSummary {
    pub slug: String,
    pub name: String,
    pub public: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub url: String,
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty() || slug.len() > 50 {
        return Err(AppError::BadRequest(
            "slug must be 1 to 50 characters".into(),
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::BadRequest(
            "slug may only contain lowercase letters, digits, and hyphens".into(),
        ));
    }
    Ok(())
}

async fn require_org_membership(
    state: &AppState,
    org_slug: &str,
    user_id: i64,
) -> Result<i64, AppError> {
    let row = sqlx::query_as::<_, (i64, bool)>(
        r#"
        SELECT o.id, EXISTS(
            SELECT 1 FROM org_members om WHERE om.org_id = o.id AND om.user_id = $2
        ) AS is_member
        FROM orgs o WHERE o.slug = $1
        "#,
    )
    .bind(org_slug)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let (org_id, is_member) = row;
    if !is_member {
        return Err(AppError::NotFound);
    }
    Ok(org_id)
}

pub async fn create_project(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Json(body): Json<CreateProjectBody>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = if let Some(ref u) = maybe_user.0 {
        u.user_id
    } else if let Some(ref k) = maybe_key.0 {
        if k.scope != ApiKeyScope::Admin {
            return Err(AppError::Forbidden);
        }
        k.user_id
    } else {
        return Err(AppError::Unauthorized);
    };

    validate_slug(&body.slug)?;

    if body.name.is_empty() || body.name.len() > 100 {
        return Err(AppError::BadRequest(
            "name must be 1 to 100 characters".into(),
        ));
    }
    if let Some(ref d) = body.description {
        if d.len() > 500 {
            return Err(AppError::BadRequest(
                "description must be at most 500 characters".into(),
            ));
        }
    }

    let org_id = require_org_membership(&state, &org_slug, user_id).await?;

    let row = sqlx::query_as::<_, (i64, DateTime<Utc>)>(
        r#"
        INSERT INTO projects (org_id, slug, name, public, description)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, created_at
        "#,
    )
    .bind(org_id)
    .bind(&body.slug)
    .bind(&body.name)
    .bind(body.public)
    .bind(&body.description)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.constraint() == Some("projects_org_id_slug_key") => {
            AppError::Conflict(format!(
                "a project with slug '{}' already exists in this org",
                body.slug
            ))
        }
        _ => AppError::Database(e),
    })?;

    let url = format!("{}/{}/{}", state.config.base_url, org_slug, body.slug);

    Ok((
        StatusCode::CREATED,
        Json(ProjectMetadata {
            org_slug,
            project_slug: body.slug,
            name: body.name,
            public: body.public,
            description: body.description,
            created_at: row.1,
            url,
        }),
    ))
}

pub async fn get_org(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
) -> Result<Json<OrgResponse>, AppError> {
    let user_id = if let Some(ref u) = maybe_user.0 {
        u.user_id
    } else if let Some(ref k) = maybe_key.0 {
        if k.scope != ApiKeyScope::Admin {
            return Err(AppError::Forbidden);
        }
        k.user_id
    } else {
        return Err(AppError::Unauthorized);
    };

    let org_id = require_org_membership(&state, &org_slug, user_id).await?;

    let org_name: String = sqlx::query_scalar("SELECT name FROM orgs WHERE id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;

    let rows = sqlx::query_as::<_, (String, String, bool, Option<String>, DateTime<Utc>)>(
        r#"
        SELECT slug, name, public, description, created_at
        FROM projects
        WHERE org_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let projects = rows
        .into_iter()
        .map(|r| ProjectSummary {
            url: format!("{}/{}/{}", state.config.base_url, org_slug, r.0),
            slug: r.0,
            name: r.1,
            public: r.2,
            description: r.3,
            created_at: r.4,
        })
        .collect();

    Ok(Json(OrgResponse {
        slug: org_slug,
        name: org_name,
        projects,
    }))
}

pub async fn get_sweep_runs(
    State(state): State<AppState>,
    Path(sweep_id): Path<Uuid>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
) -> Result<Json<serde_json::Value>, AppError> {
    let caller_id = maybe_user
        .0
        .as_ref()
        .map(|u| u.user_id)
        .or_else(|| maybe_key.0.as_ref().map(|k| k.user_id));

    let (project_id, public) = sqlx::query_as::<_, (i64, bool)>(
        "SELECT s.project_id, p.public FROM sweeps s JOIN projects p ON p.id = s.project_id WHERE s.id = $1",
    )
    .bind(sweep_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

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
        if !is_member {
            return Err(AppError::Forbidden);
        }
    }

    type RunRow = (
        Uuid,
        String,
        String,
        String,
        String,
        Option<serde_json::Value>,
        Option<i64>,
        Option<serde_json::Value>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
    );

    let rows = sqlx::query_as::<_, RunRow>(
        r#"
        SELECT id, scenario, world, backend, status::text, outcome, wall_time_ms, total_cost,
               started_at, ended_at, created_at
        FROM runs WHERE sweep_id = $1 ORDER BY created_at ASC
        "#,
    )
    .bind(sweep_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let runs: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.0,
                "scenario": r.1,
                "world": r.2,
                "backend": r.3,
                "status": r.4,
                "outcome": r.5,
                "wall_time_ms": r.6,
                "total_cost": r.7,
                "started_at": r.8,
                "ended_at": r.9,
                "created_at": r.10,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "runs": runs })))
}

pub async fn get_training_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
) -> Result<Json<serde_json::Value>, AppError> {
    let caller_id = maybe_user
        .0
        .as_ref()
        .map(|u| u.user_id)
        .or_else(|| maybe_key.0.as_ref().map(|k| k.user_id));

    let (project_id, public) = sqlx::query_as::<_, (i64, bool)>(
        "SELECT tr.project_id, p.public FROM training_runs tr JOIN projects p ON p.id = tr.project_id WHERE tr.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

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
        if !is_member {
            return Err(AppError::Forbidden);
        }
    }

    type MetricRow = (i64, String, f64, DateTime<Utc>);
    let rows = sqlx::query_as::<_, MetricRow>(
        "SELECT step, metric_name, value, recorded_at FROM training_metrics WHERE training_run_id = $1 ORDER BY step ASC, metric_name ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let metrics: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "step": r.0,
                "metric_name": r.1,
                "value": r.2,
                "recorded_at": r.3,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "metrics": metrics })))
}
