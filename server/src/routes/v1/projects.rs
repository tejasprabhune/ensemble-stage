use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{ApiKeyAuth, MaybeApiKey, MaybeUser},
    AppError, AppState,
};

#[derive(Deserialize)]
pub struct ListRunsQuery {
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub org_slug: String,
    pub project_slug: String,
    pub name: String,
    pub public: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct RunSummary {
    pub id: Uuid,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub wall_time_ms: Option<i64>,
    pub sweep_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct ListRunsResponse {
    pub runs: Vec<RunSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRunBody {
    pub id: Option<Uuid>,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub sweep_id: Option<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Serialize)]
pub struct CreateRunResponse {
    pub id: Uuid,
    pub url: String,
}

#[derive(Deserialize)]
pub struct CreateSweepBody {
    pub config: Value,
}

#[derive(Serialize)]
pub struct CreateSweepResponse {
    pub id: Uuid,
    pub url: String,
}

#[derive(Deserialize)]
pub struct CreateTrainingRunBody {
    pub persona_name: String,
    pub base_model: String,
    pub hyperparameters: Option<Value>,
}

#[derive(Serialize)]
pub struct CreateTrainingRunResponse {
    pub id: Uuid,
    pub url: String,
}

#[derive(Deserialize)]
struct Cursor {
    created_at: DateTime<Utc>,
    id: Uuid,
}

fn decode_cursor(s: &str) -> Option<Cursor> {
    let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn encode_cursor(created_at: DateTime<Utc>, id: Uuid) -> String {
    let json = serde_json::json!({"created_at": created_at, "id": id});
    URL_SAFE_NO_PAD.encode(json.to_string())
}

async fn require_project_write_access(
    state: &AppState,
    auth: &ApiKeyAuth,
    org_slug: &str,
    project_slug: &str,
) -> Result<(i64, i64), AppError> {
    let row = sqlx::query_as::<_, (i64, i64, bool)>(
        r#"
        SELECT p.id, p.org_id, om.user_id IS NOT NULL AS is_member
        FROM projects p
        JOIN orgs o ON o.id = p.org_id
        LEFT JOIN org_members om ON om.org_id = o.id AND om.user_id = $3
        WHERE o.slug = $1 AND p.slug = $2
        "#,
    )
    .bind(org_slug)
    .bind(project_slug)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let (project_id, org_id, is_member) = row;
    if !is_member {
        return Err(AppError::Forbidden);
    }

    Ok((project_id, org_id))
}

pub async fn get_project(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<ProjectResponse>, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, String, String, String, bool, Option<String>, DateTime<Utc>)>(
        r#"
        SELECT p.id, p.org_id, o.slug, p.slug, p.name, p.public, p.description, p.created_at
        FROM projects p
        JOIN orgs o ON o.id = p.org_id
        WHERE o.slug = $1 AND p.slug = $2
        "#,
    )
    .bind(&org_slug)
    .bind(&project_slug)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let (_project_id, org_id, org, proj, name, public, description, created_at) = row;

    if !public {
        let authed = maybe_user.0.is_some() || maybe_key.0.is_some();
        if !authed {
            return Err(AppError::Forbidden);
        }
        let caller_id = maybe_user
            .0
            .map(|u| u.user_id)
            .or_else(|| maybe_key.0.map(|k| k.user_id));
        if let Some(uid) = caller_id {
            let is_member: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM org_members WHERE org_id = $1 AND user_id = $2)",
            )
            .bind(org_id)
            .bind(uid)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::Database)?;
            if !is_member {
                return Err(AppError::Forbidden);
            }
        }
    }

    Ok(Json(ProjectResponse {
        org_slug: org,
        project_slug: proj,
        name,
        public,
        description,
        created_at,
    }))
}

pub async fn list_runs(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    maybe_key: MaybeApiKey,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<ListRunsQuery>,
) -> Result<Json<ListRunsResponse>, AppError> {
    let (project_id, public) = sqlx::query_as::<_, (i64, bool)>(
        r#"
        SELECT p.id, p.public
        FROM projects p
        JOIN orgs o ON o.id = p.org_id
        WHERE o.slug = $1 AND p.slug = $2
        "#,
    )
    .bind(&org_slug)
    .bind(&project_slug)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    if !public && maybe_user.0.is_none() && maybe_key.0.is_none() {
        return Err(AppError::Forbidden);
    }

    let limit = q.limit.unwrap_or(50).min(200).max(1);
    let sort = q.sort.as_deref().unwrap_or("created_at:desc");
    let filter_pattern = q.filter.as_ref().map(|f| format!("%{}%", f));

    let cursor = q.cursor.as_deref().and_then(decode_cursor);

    // Sort direction controls both ORDER BY and the cursor comparison operator.
    // The strings are derived from our own match, not raw user input.
    let (order_sql, cursor_cmp) = match sort {
        "created_at:asc" => (
            "ORDER BY r.created_at ASC, r.id ASC",
            "AND ($2::timestamptz IS NULL OR r.created_at > $2 OR (r.created_at = $2 AND r.id > $3::uuid))",
        ),
        "wall_time_ms:asc" => (
            "ORDER BY r.wall_time_ms ASC NULLS LAST, r.created_at ASC, r.id ASC",
            "AND ($2::timestamptz IS NULL OR r.created_at > $2 OR (r.created_at = $2 AND r.id > $3::uuid))",
        ),
        "wall_time_ms:desc" => (
            "ORDER BY r.wall_time_ms DESC NULLS LAST, r.created_at DESC, r.id DESC",
            "AND ($2::timestamptz IS NULL OR r.created_at < $2 OR (r.created_at = $2 AND r.id < $3::uuid))",
        ),
        _ => (
            "ORDER BY r.created_at DESC, r.id DESC",
            "AND ($2::timestamptz IS NULL OR r.created_at < $2 OR (r.created_at = $2 AND r.id < $3::uuid))",
        ),
    };

    let cursor_at: Option<DateTime<Utc>> = cursor.as_ref().map(|c| c.created_at);
    let cursor_id: Option<Uuid> = cursor.as_ref().map(|c| c.id);

    // $1=project_id, $2=cursor_at, $3=cursor_id, $4=filter_pattern, $5=limit
    let sql = format!(
        r#"
        SELECT r.id, r.scenario, r.world, r.backend, r.status::text,
               r.started_at, r.ended_at, r.wall_time_ms, r.sweep_id, r.created_at
        FROM runs r
        WHERE r.project_id = $1
          {cursor_cmp}
          AND ($4::text IS NULL
               OR r.scenario ILIKE '%' || $4 || '%'
               OR r.world    ILIKE '%' || $4 || '%'
               OR r.backend  ILIKE '%' || $4 || '%'
               OR r.status::text ILIKE '%' || $4 || '%')
        {order_sql}
        LIMIT $5
        "#
    );

    type RunRow = (
        Uuid,
        String,
        String,
        String,
        String,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<i64>,
        Option<Uuid>,
        DateTime<Utc>,
    );

    let fetch_limit = limit + 1;

    let rows = sqlx::query_as::<_, RunRow>(&sql)
        .bind(project_id)
        .bind(cursor_at)
        .bind(cursor_id)
        .bind(filter_pattern.as_deref())
        .bind(fetch_limit)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    let has_more = rows.len() as i64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = if has_more {
        rows.last().map(|r| encode_cursor(r.9, r.0))
    } else {
        None
    };

    let runs = rows
        .into_iter()
        .map(|r| RunSummary {
            id: r.0,
            scenario: r.1,
            world: r.2,
            backend: r.3,
            status: r.4,
            started_at: r.5,
            ended_at: r.6,
            wall_time_ms: r.7,
            sweep_id: r.8,
        })
        .collect();

    Ok(Json(ListRunsResponse { runs, next_cursor }))
}

pub async fn create_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateRunBody>,
) -> Result<impl IntoResponse, AppError> {
    if body.scenario.is_empty() || body.world.is_empty() || body.backend.is_empty() {
        return Err(AppError::BadRequest(
            "scenario, world, and backend are required".into(),
        ));
    }

    let (project_id, _) =
        require_project_write_access(&state, &auth, &org_slug, &project_slug).await?;

    let run_id = body.id.unwrap_or_else(Uuid::now_v7);

    sqlx::query(
        r#"
        INSERT INTO runs (id, project_id, sweep_id, scenario, world, backend, metadata, created_by_user_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(run_id)
    .bind(project_id)
    .bind(body.sweep_id)
    .bind(&body.scenario)
    .bind(&body.world)
    .bind(&body.backend)
    .bind(&body.metadata)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let url = format!(
        "{}/{}/{}/runs/{}",
        state.config.base_url, org_slug, project_slug, run_id
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateRunResponse { id: run_id, url }),
    ))
}

pub async fn create_sweep(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateSweepBody>,
) -> Result<impl IntoResponse, AppError> {
    let (project_id, _) =
        require_project_write_access(&state, &auth, &org_slug, &project_slug).await?;

    let sweep_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO sweeps (id, project_id, config, created_by_user_id)
        VALUES (gen_random_uuid(), $1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(&body.config)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let url = format!(
        "{}/{}/{}/sweeps/{}",
        state.config.base_url, org_slug, project_slug, sweep_id
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateSweepResponse { id: sweep_id, url }),
    ))
}

pub async fn create_training_run(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(body): Json<CreateTrainingRunBody>,
) -> Result<impl IntoResponse, AppError> {
    if body.persona_name.is_empty() || body.base_model.is_empty() {
        return Err(AppError::BadRequest(
            "persona_name and base_model are required".into(),
        ));
    }

    let (project_id, _) =
        require_project_write_access(&state, &auth, &org_slug, &project_slug).await?;

    let training_run_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO training_runs (id, project_id, persona_name, base_model, hyperparameters, created_by_user_id)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(&body.persona_name)
    .bind(&body.base_model)
    .bind(&body.hyperparameters)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let url = format!(
        "{}/{}/{}/training_runs/{}",
        state.config.base_url, org_slug, project_slug, training_run_id
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateTrainingRunResponse {
            id: training_run_id,
            url,
        }),
    ))
}
