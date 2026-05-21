use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    response::Html,
    routing::{delete, get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{middleware::hash_api_key, MaybeUser, RequireUser},
    AppError, AppState,
};

pub struct UserCtx {
    pub github_login: Option<String>,
}

impl UserCtx {
    fn from(m: &MaybeUser) -> Self {
        UserCtx {
            github_login: m.0.as_ref().map(|u| u.github_login.clone()),
        }
    }
}

pub struct RunRow {
    pub id: String,
    pub id_short: String,
    pub started_at: String,
    pub scenario: String,
    pub world: String,
    pub backend: String,
    pub status: String,
    pub outcome: String,
    pub cost: String,
    pub duration: String,
    pub detail_url: String,
}

pub struct ApiKeyRow {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub created_at: String,
    pub last_used_at: String,
    pub expires_at: String,
}

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectTemplate {
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    runs: Vec<RunRow>,
    next_cursor: Option<String>,
    filter: String,
    sort: String,
    partial_url: String,
    base_url: String,
}

#[derive(Template)]
#[template(path = "partials/runs_rows.html")]
struct RunsRowsPartial {
    runs: Vec<RunRow>,
    next_cursor: Option<String>,
    filter: String,
    sort: String,
    partial_url: String,
    base_url: String,
}

#[derive(Template)]
#[template(path = "run_detail.html")]
struct RunDetailTemplate {
    run_id: String,
    run_id_short: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    scenario: String,
    world: String,
    backend: String,
    status: String,
    started_at: String,
    duration: String,
    cost: String,
    outcome_display: String,
    outcome_json: String,
    metadata_display: String,
    metadata_json: String,
}

#[derive(Template)]
#[template(path = "sweep.html")]
struct SweepTemplate {
    sweep_id: String,
    sweep_id_short: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    status: String,
    created_at: String,
    started_at: String,
    ended_at: String,
    config_summary: String,
}

#[derive(Template)]
#[template(path = "training_run.html")]
struct TrainingRunTemplate {
    training_run_id: String,
    training_run_id_short: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    status: String,
    persona_name: String,
    base_model: String,
    created_at: String,
    started_at: String,
    ended_at: String,
    artifact_uri: String,
}

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    user: UserCtx,
    keys: Vec<ApiKeyRow>,
    new_key: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/api_keys_section.html")]
struct ApiKeysSectionPartial {
    keys: Vec<ApiKeyRow>,
    new_key: Option<String>,
}

fn render<T: Template>(t: T) -> Result<Html<String>, AppError> {
    t.render()
        .map(Html)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template: {e}")))
}

fn format_duration(ms: Option<i64>) -> String {
    let ms = match ms {
        Some(v) => v,
        None => return "—".into(),
    };
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let m = ms / 60_000;
        let s = (ms % 60_000) / 1000;
        format!("{m}m {s}s")
    }
}

fn format_cost(cost: Option<&Value>) -> String {
    let cost = match cost {
        Some(v) => v,
        None => return "—".into(),
    };
    if let Some(usd) = cost.get("usd").and_then(|v| v.as_f64()) {
        format!("${:.4}", usd)
    } else {
        "—".into()
    }
}

fn format_outcome(outcome: Option<&Value>) -> String {
    let outcome = match outcome {
        Some(v) => v,
        None => return "—".into(),
    };
    if let Some(scores) = outcome.get("scores").and_then(|v| v.as_object()) {
        let pairs: Vec<String> = scores
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = if let Some(f) = v.as_f64() {
                    format!("{:.2}", f)
                } else {
                    v.to_string()
                };
                format!("{k}={val}")
            })
            .collect();
        pairs.join(" ")
    } else {
        "—".into()
    }
}

fn summarize_sweep_config(config: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(scenarios) = config.get("scenarios").and_then(|v| v.as_array()) {
        parts.push(format!("{} scenario{}", scenarios.len(), if scenarios.len() == 1 { "" } else { "s" }));
    }
    if let Some(backends) = config.get("backends").and_then(|v| v.as_array()) {
        parts.push(format!("{} backend{}", backends.len(), if backends.len() == 1 { "" } else { "s" }));
    }
    if let Some(n) = config.get("n_trials").and_then(|v| v.as_i64()) {
        parts.push(format!("{n} trial{}", if n == 1 { "" } else { "s" }));
    }
    parts.join(", ")
}

fn format_ts(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| t.format("%b %d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

type RunDbRow = (
    Uuid,
    String,
    String,
    String,
    String,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<i64>,
    Option<Value>,
    Option<Value>,
    DateTime<Utc>,
);

async fn fetch_runs(
    state: &AppState,
    project_id: i64,
    org_slug: &str,
    project_slug: &str,
    filter: &str,
    sort: &str,
    cursor_str: &str,
    limit: i64,
) -> Result<(Vec<RunRow>, Option<String>), AppError> {
    #[derive(serde::Deserialize)]
    struct Cursor {
        created_at: DateTime<Utc>,
        id: Uuid,
    }

    let cursor: Option<Cursor> = if cursor_str.is_empty() {
        None
    } else {
        URL_SAFE_NO_PAD
            .decode(cursor_str)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    };

    let cursor_at: Option<DateTime<Utc>> = cursor.as_ref().map(|c| c.created_at);
    let cursor_id: Option<Uuid> = cursor.as_ref().map(|c| c.id);
    let filter_val = if filter.is_empty() { None } else { Some(filter) };

    let (order_sql, cursor_cmp) = match sort {
        "created_at:asc" => (
            "ORDER BY r.created_at ASC, r.id ASC",
            "AND ($3::timestamptz IS NULL OR r.created_at > $3 OR (r.created_at = $3 AND r.id > $4::uuid))",
        ),
        "wall_time_ms:asc" => (
            "ORDER BY r.wall_time_ms ASC NULLS LAST, r.created_at ASC, r.id ASC",
            "AND ($3::timestamptz IS NULL OR r.created_at > $3 OR (r.created_at = $3 AND r.id > $4::uuid))",
        ),
        "wall_time_ms:desc" => (
            "ORDER BY r.wall_time_ms DESC NULLS LAST, r.created_at DESC, r.id DESC",
            "AND ($3::timestamptz IS NULL OR r.created_at < $3 OR (r.created_at = $3 AND r.id < $4::uuid))",
        ),
        _ => (
            "ORDER BY r.created_at DESC, r.id DESC",
            "AND ($3::timestamptz IS NULL OR r.created_at < $3 OR (r.created_at = $3 AND r.id < $4::uuid))",
        ),
    };

    let sql = format!(
        r#"
        SELECT r.id, r.scenario, r.world, r.backend, r.status::text,
               r.started_at, r.ended_at, r.wall_time_ms, r.outcome, r.total_cost, r.created_at
        FROM runs r
        WHERE r.project_id = $1
          {cursor_cmp}
          AND ($2::text IS NULL
               OR r.scenario ILIKE '%' || $2 || '%'
               OR r.world    ILIKE '%' || $2 || '%'
               OR r.backend  ILIKE '%' || $2 || '%'
               OR r.status::text ILIKE '%' || $2 || '%')
        {order_sql}
        LIMIT $5
        "#
    );

    let fetch_limit = limit + 1;
    let rows = sqlx::query_as::<_, RunDbRow>(&sql)
        .bind(project_id)
        .bind(filter_val)
        .bind(cursor_at)
        .bind(cursor_id)
        .bind(fetch_limit)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    let has_more = rows.len() as i64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = if has_more {
        rows.last().map(|r| {
            let json = serde_json::json!({"created_at": r.10, "id": r.0});
            URL_SAFE_NO_PAD.encode(json.to_string())
        })
    } else {
        None
    };

    let run_rows = rows
        .into_iter()
        .map(|r| RunRow {
            id: r.0.to_string(),
            id_short: r.0.to_string()[..8].to_string(),
            started_at: format_ts(r.5),
            scenario: r.1,
            world: r.2,
            backend: r.3,
            status: r.4,
            outcome: format_outcome(r.8.as_ref()),
            cost: format_cost(r.9.as_ref()),
            duration: format_duration(r.7),
            detail_url: format!("/{org_slug}/{project_slug}/runs/{}", r.0),
        })
        .collect();

    Ok((run_rows, next_cursor))
}

async fn fetch_project_id(
    state: &AppState,
    org_slug: &str,
    project_slug: &str,
) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT p.id FROM projects p JOIN orgs o ON o.id = p.org_id WHERE o.slug = $1 AND p.slug = $2",
    )
    .bind(org_slug)
    .bind(project_slug)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)
}

async fn fetch_api_keys(state: &AppState, user_id: i64) -> Result<Vec<ApiKeyRow>, AppError> {
    type KeyRow = (i64, String, String, Option<DateTime<Utc>>, DateTime<Utc>, Option<DateTime<Utc>>);
    let rows = sqlx::query_as::<_, KeyRow>(
        "SELECT id, name, scope::text, last_used_at, created_at, expires_at FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    Ok(rows
        .into_iter()
        .map(|r| ApiKeyRow {
            id: r.0,
            name: r.1,
            scope: r.2,
            last_used_at: r.3.map(|t| t.format("%b %d %H:%M").to_string()).unwrap_or_else(|| "never".into()),
            created_at: r.4.format("%b %d %Y").to_string(),
            expires_at: r.5.map(|t| t.format("%b %d %Y").to_string()).unwrap_or_else(|| "never".into()),
        })
        .collect())
}

#[derive(Deserialize)]
struct RunsQuery {
    filter: Option<String>,
    sort: Option<String>,
    cursor: Option<String>,
}

async fn landing(maybe_user: MaybeUser) -> Result<Html<String>, AppError> {
    render(LandingTemplate {
        user: UserCtx::from(&maybe_user),
    })
}

async fn project(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<RunsQuery>,
) -> Result<Html<String>, AppError> {
    let project_id = fetch_project_id(&state, &org_slug, &project_slug).await?;
    let filter = q.filter.as_deref().unwrap_or("");
    let sort = q.sort.as_deref().unwrap_or("created_at:desc");
    let cursor = q.cursor.as_deref().unwrap_or("");

    let (runs, next_cursor) = fetch_runs(&state, project_id, &org_slug, &project_slug, filter, sort, cursor, 50).await?;
    let partial_url = format!("/{org_slug}/{project_slug}/runs-partial");

    render(ProjectTemplate {
        org_slug: org_slug.clone(),
        project_slug: project_slug.clone(),
        user: UserCtx::from(&maybe_user),
        runs,
        next_cursor,
        filter: filter.to_string(),
        sort: sort.to_string(),
        partial_url,
        base_url: state.config.base_url.clone(),
    })
}

async fn runs_partial(
    State(state): State<AppState>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<RunsQuery>,
) -> Result<Html<String>, AppError> {
    let project_id = fetch_project_id(&state, &org_slug, &project_slug).await?;
    let filter = q.filter.as_deref().unwrap_or("");
    let sort = q.sort.as_deref().unwrap_or("created_at:desc");
    let cursor = q.cursor.as_deref().unwrap_or("");

    let (runs, next_cursor) = fetch_runs(&state, project_id, &org_slug, &project_slug, filter, sort, cursor, 50).await?;
    let partial_url = format!("/{org_slug}/{project_slug}/runs-partial");

    render(RunsRowsPartial {
        runs,
        next_cursor,
        filter: filter.to_string(),
        sort: sort.to_string(),
        partial_url,
        base_url: state.config.base_url.clone(),
    })
}

async fn run_detail(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let run_uuid: Uuid = run_id
        .parse()
        .map_err(|_| AppError::NotFound)?;

    type RunRow = (
        String, String, String, String,
        Option<DateTime<Utc>>, Option<i64>, Option<Value>, Option<Value>, Option<Value>,
    );
    let row = sqlx::query_as::<_, RunRow>(
        r#"
        SELECT r.scenario, r.world, r.backend, r.status::text,
               r.started_at, r.wall_time_ms, r.total_cost, r.outcome, r.metadata
        FROM runs r
        WHERE r.id = $1
        "#,
    )
    .bind(run_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let outcome_json = serde_json::to_string(&row.7).unwrap_or_else(|_| "null".into());
    let outcome_display = row.7.as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| "—".into()))
        .unwrap_or_else(|| "—".into());
    let metadata_json = serde_json::to_string(&row.8).unwrap_or_else(|_| "null".into());
    let metadata_display = row.8.as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| "—".into()))
        .unwrap_or_else(|| "—".into());

    render(RunDetailTemplate {
        run_id_short: run_id[..8.min(run_id.len())].to_string(),
        run_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        scenario: row.0,
        world: row.1,
        backend: row.2,
        status: row.3,
        started_at: format_ts(row.4),
        duration: format_duration(row.5),
        cost: format_cost(row.6.as_ref()),
        outcome_display,
        outcome_json,
        metadata_display,
        metadata_json,
    })
}

async fn sweep(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, sweep_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let sweep_uuid: Uuid = sweep_id
        .parse()
        .map_err(|_| AppError::NotFound)?;

    type SweepRow = (String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, DateTime<Utc>, Value);
    let row = sqlx::query_as::<_, SweepRow>(
        "SELECT status::text, started_at, ended_at, created_at, config FROM sweeps WHERE id = $1",
    )
    .bind(sweep_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let config_summary = summarize_sweep_config(&row.4);

    render(SweepTemplate {
        sweep_id_short: sweep_id[..8.min(sweep_id.len())].to_string(),
        sweep_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        status: row.0,
        started_at: format_ts(row.1),
        ended_at: format_ts(row.2),
        created_at: row.3.format("%b %d %Y %H:%M").to_string(),
        config_summary,
    })
}

async fn training_run(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, training_run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let tr_uuid: Uuid = training_run_id
        .parse()
        .map_err(|_| AppError::NotFound)?;

    type TrRow = (String, String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, DateTime<Utc>, Option<String>);
    let row = sqlx::query_as::<_, TrRow>(
        "SELECT status::text, persona_name, base_model, started_at, ended_at, created_at, artifact_uri FROM training_runs WHERE id = $1",
    )
    .bind(tr_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    render(TrainingRunTemplate {
        training_run_id_short: training_run_id[..8.min(training_run_id.len())].to_string(),
        training_run_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        status: row.0,
        persona_name: row.1,
        base_model: row.2,
        started_at: format_ts(row.3),
        ended_at: format_ts(row.4),
        created_at: row.5.format("%b %d %Y %H:%M").to_string(),
        artifact_uri: row.6.unwrap_or_default(),
    })
}

async fn account(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
) -> Result<Html<String>, AppError> {
    let user = maybe_user.0.as_ref();
    let keys = if let Some(u) = user {
        fetch_api_keys(&state, u.user_id).await?
    } else {
        vec![]
    };

    render(AccountTemplate {
        user: UserCtx::from(&maybe_user),
        keys,
        new_key: None,
    })
}

#[derive(Deserialize)]
struct CreateKeyForm {
    name: String,
    scope: String,
}

async fn create_key(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Form(form): Form<CreateKeyForm>,
) -> Result<Html<String>, AppError> {
    let scope_str = match form.scope.as_str() {
        "push" | "admin" => form.scope.as_str(),
        _ => return Err(AppError::BadRequest("scope must be 'push' or 'admin'".into())),
    };

    let prefix = if scope_str == "push" { "stage_sk_" } else { "stage_ak_" };
    let raw_key = format!(
        "{}{}{}",
        prefix,
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple(),
    );
    let key_hash = hash_api_key(&raw_key);

    sqlx::query(
        "INSERT INTO api_keys (user_id, scope, name, key_hash) VALUES ($1, $2::api_key_scope, $3, $4)",
    )
    .bind(user.user_id)
    .bind(scope_str)
    .bind(&form.name)
    .bind(&key_hash)
    .execute(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let keys = fetch_api_keys(&state, user.user_id).await?;

    render(ApiKeysSectionPartial {
        keys,
        new_key: Some(raw_key),
    })
}

async fn revoke_key(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.user_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    let keys = fetch_api_keys(&state, user.user_id).await?;

    render(ApiKeysSectionPartial {
        keys,
        new_key: None,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(landing))
        .route("/:org_slug/:project_slug", get(project))
        .route("/:org_slug/:project_slug/runs-partial", get(runs_partial))
        .route("/:org_slug/:project_slug/runs/:run_id", get(run_detail))
        .route("/:org_slug/:project_slug/sweeps/:sweep_id", get(sweep))
        .route(
            "/:org_slug/:project_slug/training_runs/:id",
            get(training_run),
        )
        .route("/me", get(account))
        .route("/me/keys", post(create_key))
        .route("/me/keys/:id", delete(revoke_key))
}
