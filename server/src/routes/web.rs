use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    response::{Html, IntoResponse, Redirect},
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

pub struct OrgProject {
    pub slug: String,
    pub name: String,
    pub public: bool,
    pub description: String,
    pub created_at: String,
    pub url: String,
}

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "org.html")]
struct OrgTemplate {
    org_slug: String,
    org_name: String,
    user: UserCtx,
    projects: Vec<OrgProject>,
    form_error: String,
    form_slug: String,
    form_name: String,
}

#[derive(Template)]
#[template(path = "compare.html")]
struct CompareTemplate {
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    run_id_a: String,
    run_id_b: String,
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
    // For the empty state onboarding snippet.
    // Some(name) if the user has a push-scoped key; None if they need to create one.
    api_key_name: Option<String>,
    has_any_runs: bool,
}

#[derive(Template)]
#[template(path = "partials/runs_rows.html")]
struct RunsRowsPartial {
    runs: Vec<RunRow>,
    next_cursor: Option<String>,
    filter: String,
    sort: String,
    partial_url: String,
    has_any_runs: bool,
}

#[derive(Template)]
#[template(path = "run_detail.html")]
struct RunDetailTemplate {
    run_id: String,
    run_id_short: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    is_authed: bool,
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
#[template(path = "settings.html")]
struct SettingsTemplate {
    org_slug: String,
    project_slug: String,
    user: UserCtx,
    project_name: String,
    project_description: String,
    project_public: bool,
    project_id: i64,
    created_at: String,
    run_count: i64,
    form_error: String,
}

#[derive(Template)]
#[template(path = "sweeps_list.html")]
struct SweepsListTemplate {
    org_slug: String,
    project_slug: String,
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "training_list.html")]
struct TrainingListTemplate {
    org_slug: String,
    project_slug: String,
    user: UserCtx,
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
    config_json: String,
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
        format!("${usd:.4}")
    } else {
        "—".into()
    }
}

fn format_outcome(outcome: Option<&Value>) -> String {
    let outcome = match outcome {
        Some(v) => v,
        None => return "—".into(),
    };
    let map = match outcome.as_object() {
        Some(m) => m,
        None => return "—".into(),
    };
    let pairs: Vec<String> = map
        .iter()
        .take(2)
        .map(|(k, v)| {
            let val = if let Some(f) = v.as_f64() {
                format!("{f:.2}")
            } else {
                v.to_string()
            };
            format!("{k}={val}")
        })
        .collect();
    if pairs.is_empty() {
        "—".into()
    } else {
        pairs.join(" ")
    }
}

fn summarize_sweep_config(config: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(scenarios) = config.get("scenarios").and_then(|v| v.as_array()) {
        parts.push(format!(
            "{} scenario{}",
            scenarios.len(),
            if scenarios.len() == 1 { "" } else { "s" }
        ));
    }
    if let Some(backends) = config.get("backends").and_then(|v| v.as_array()) {
        parts.push(format!(
            "{} backend{}",
            backends.len(),
            if backends.len() == 1 { "" } else { "s" }
        ));
    }
    if let Some(n) = config.get("n_trials").and_then(|v| v.as_i64()) {
        parts.push(format!("{n} trial{}", if n == 1 { "" } else { "s" }));
    }
    parts.join(", ")
}

fn format_ts(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| t.format("%b %d %H:%M").to_string())
        .unwrap_or_default()
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

struct FetchRunsParams<'a> {
    project_id: i64,
    org_slug: &'a str,
    project_slug: &'a str,
    filter: &'a str,
    sort: &'a str,
    cursor_str: &'a str,
    limit: i64,
}

async fn fetch_runs(
    state: &AppState,
    p: FetchRunsParams<'_>,
) -> Result<(Vec<RunRow>, Option<String>), AppError> {
    let FetchRunsParams {
        project_id,
        org_slug,
        project_slug,
        filter,
        sort,
        cursor_str,
        limit,
    } = p;
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
    let filter_val = if filter.is_empty() {
        None
    } else {
        Some(filter)
    };

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
    type KeyRow = (
        i64,
        String,
        String,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    );
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
            last_used_at: r
                .3
                .map(|t| t.format("%b %d %H:%M").to_string())
                .unwrap_or_else(|| "never".into()),
            created_at: r.4.format("%b %d %Y").to_string(),
            expires_at: r
                .5
                .map(|t| t.format("%b %d %Y").to_string())
                .unwrap_or_else(|| "never".into()),
        })
        .collect())
}

#[derive(Deserialize)]
struct WebRunsQuery {
    filter: Option<String>,
    sort: Option<String>,
    cursor: Option<String>,
}

async fn fetch_push_key_name(state: &AppState, user_id: i64) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT name FROM api_keys
        WHERE user_id = $1 AND scope = 'push'
          AND (expires_at IS NULL OR expires_at > NOW())
        ORDER BY last_used_at DESC NULLS LAST, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
}

async fn fetch_project_run_count(state: &AppState, project_id: i64) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0)
        > 0
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
    Query(q): Query<WebRunsQuery>,
) -> Result<Html<String>, AppError> {
    let project_id = fetch_project_id(&state, &org_slug, &project_slug).await?;
    let filter = q.filter.as_deref().unwrap_or("");
    let sort = q.sort.as_deref().unwrap_or("created_at:desc");
    let cursor = q.cursor.as_deref().unwrap_or("");

    let has_any_runs = fetch_project_run_count(&state, project_id).await;
    let api_key_name = if let Some(ref u) = maybe_user.0 {
        fetch_push_key_name(&state, u.user_id).await
    } else {
        None
    };

    let (runs, next_cursor) = fetch_runs(
        &state,
        FetchRunsParams {
            project_id,
            org_slug: &org_slug,
            project_slug: &project_slug,
            filter,
            sort,
            cursor_str: cursor,
            limit: 50,
        },
    )
    .await?;
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
        api_key_name,
        has_any_runs,
    })
}

async fn runs_partial(
    State(state): State<AppState>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<WebRunsQuery>,
) -> Result<Html<String>, AppError> {
    let project_id = fetch_project_id(&state, &org_slug, &project_slug).await?;
    let filter = q.filter.as_deref().unwrap_or("");
    let sort = q.sort.as_deref().unwrap_or("created_at:desc");
    let cursor = q.cursor.as_deref().unwrap_or("");

    let (runs, next_cursor) = fetch_runs(
        &state,
        FetchRunsParams {
            project_id,
            org_slug: &org_slug,
            project_slug: &project_slug,
            filter,
            sort,
            cursor_str: cursor,
            limit: 50,
        },
    )
    .await?;
    let partial_url = format!("/{org_slug}/{project_slug}/runs-partial");

    let has_any_runs = fetch_project_run_count(&state, project_id).await;

    render(RunsRowsPartial {
        runs,
        next_cursor,
        filter: filter.to_string(),
        sort: sort.to_string(),
        partial_url,
        has_any_runs,
    })
}

async fn run_detail(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let run_uuid: Uuid = run_id.parse().map_err(|_| AppError::NotFound)?;

    type RunRow = (
        String,
        String,
        String,
        String,
        Option<DateTime<Utc>>,
        Option<i64>,
        Option<Value>,
        Option<Value>,
        Option<Value>,
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
    let outcome_display = row
        .7
        .as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| "—".into()))
        .unwrap_or_else(|| "—".into());
    let metadata_json = serde_json::to_string(&row.8).unwrap_or_else(|_| "null".into());
    let metadata_display = row
        .8
        .as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| "—".into()))
        .unwrap_or_else(|| "—".into());

    let is_authed = maybe_user.0.is_some();
    render(RunDetailTemplate {
        run_id_short: run_id[..8.min(run_id.len())].to_string(),
        run_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        is_authed,
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
    let sweep_uuid: Uuid = sweep_id.parse().map_err(|_| AppError::NotFound)?;

    type SweepRow = (
        String,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        Value,
    );
    let row = sqlx::query_as::<_, SweepRow>(
        "SELECT status::text, started_at, ended_at, created_at, config FROM sweeps WHERE id = $1",
    )
    .bind(sweep_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let config_summary = summarize_sweep_config(&row.4);
    let config_json = serde_json::to_string(&row.4).unwrap_or_else(|_| "{}".into());

    fn fmt_ts_empty(ts: Option<DateTime<Utc>>) -> String {
        ts.map(|t| t.format("%b %d %H:%M").to_string())
            .unwrap_or_default()
    }

    render(SweepTemplate {
        sweep_id_short: sweep_id[..8.min(sweep_id.len())].to_string(),
        sweep_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        status: row.0,
        started_at: fmt_ts_empty(row.1),
        ended_at: fmt_ts_empty(row.2),
        created_at: row.3.format("%b %d %Y %H:%M").to_string(),
        config_summary,
        config_json,
    })
}

async fn training_run(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, training_run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let tr_uuid: Uuid = training_run_id.parse().map_err(|_| AppError::NotFound)?;

    type TrRow = (
        String,
        String,
        String,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        Option<String>,
    );
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
    expires_at: Option<String>,
}

async fn create_key(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Form(form): Form<CreateKeyForm>,
) -> Result<Html<String>, AppError> {
    let scope_str = match form.scope.as_str() {
        "push" | "admin" => form.scope.as_str(),
        _ => {
            return Err(AppError::BadRequest(
                "scope must be 'push' or 'admin'".into(),
            ))
        }
    };

    let prefix = if scope_str == "push" {
        "stage_sk_"
    } else {
        "stage_ak_"
    };
    let raw_key = format!(
        "{}{}{}",
        prefix,
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple(),
    );
    let key_hash = hash_api_key(&raw_key);

    let expires_at: Option<chrono::NaiveDate> = form
        .expires_at
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let expires_at_utc = expires_at
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap())
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));

    sqlx::query(
        "INSERT INTO api_keys (user_id, scope, name, key_hash, expires_at) VALUES ($1, $2::api_key_scope, $3, $4, $5)",
    )
    .bind(user.user_id)
    .bind(scope_str)
    .bind(&form.name)
    .bind(&key_hash)
    .bind(expires_at_utc)
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

async fn sweeps_list(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    render(SweepsListTemplate {
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn training_list(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    render(TrainingListTemplate {
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn org_page(
    State(state): State<AppState>,
    maybe_user: MaybeUser,
    Path(org_slug): Path<String>,
) -> Result<axum::response::Response, AppError> {
    let user_id = match maybe_user.0.as_ref().map(|u| u.user_id) {
        Some(id) => id,
        None => {
            let dest = format!("/auth/github/login?next=/{org_slug}");
            return Ok(axum::response::Redirect::to(&dest).into_response());
        }
    };

    // Verify org exists and user is a member; 404 for both missing and non-member
    let row = sqlx::query_as::<_, (i64, String, bool)>(
        r#"
        SELECT o.id, o.name, EXISTS(
            SELECT 1 FROM org_members om WHERE om.org_id = o.id AND om.user_id = $2
        )
        FROM orgs o WHERE o.slug = $1
        "#,
    )
    .bind(&org_slug)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let (org_id, org_name, is_member) = row;
    if !is_member {
        return Err(AppError::NotFound);
    }

    let projects = fetch_org_projects(&state, org_id, &org_slug).await?;

    render(OrgTemplate {
        org_slug,
        org_name,
        user: UserCtx::from(&maybe_user),
        projects,
        form_error: String::new(),
        form_slug: String::new(),
        form_name: String::new(),
    })
    .map(|h| h.into_response())
}

async fn fetch_org_projects(
    state: &AppState,
    org_id: i64,
    org_slug: &str,
) -> Result<Vec<OrgProject>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, bool, Option<String>, DateTime<Utc>)>(
        "SELECT slug, name, public, description, created_at FROM projects WHERE org_id = $1 ORDER BY created_at DESC",
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    Ok(rows
        .into_iter()
        .map(|r| OrgProject {
            url: format!("{}/{}", org_slug, r.0),
            slug: r.0,
            name: r.1,
            public: r.2,
            description: r.3.unwrap_or_default(),
            created_at: r.4.format("%b %d %Y").to_string(),
        })
        .collect())
}

#[derive(Deserialize)]
struct CreateProjectForm {
    slug: String,
    name: String,
    public: Option<String>,
    description: Option<String>,
}

async fn create_project_web(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path(org_slug): Path<String>,
    Form(form): Form<CreateProjectForm>,
) -> Result<axum::response::Response, AppError> {
    let public = form.public.as_deref() == Some("on") || form.public.as_deref() == Some("true");

    // Check org membership
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT o.id, o.name FROM orgs o JOIN org_members om ON om.org_id = o.id WHERE o.slug = $1 AND om.user_id = $2",
    )
    .bind(&org_slug)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let (org_id, org_name) = match row {
        Some(r) => r,
        None => return Err(AppError::NotFound),
    };

    let slug = form.slug.trim().to_string();
    let name = form.name.trim().to_string();

    // Inline validation so we can return a rich error
    let form_error = if slug.is_empty() || slug.len() > 50 {
        Some("Slug must be between 1 and 50 characters.".to_string())
    } else if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Some("Slug may only contain lowercase letters, digits, and hyphens.".to_string())
    } else if name.is_empty() || name.len() > 100 {
        Some("Name must be between 1 and 100 characters.".to_string())
    } else if form.description.as_deref().unwrap_or("").len() > 500 {
        Some("Description must be at most 500 characters.".to_string())
    } else {
        None
    };

    if let Some(err) = form_error {
        let projects = fetch_org_projects(&state, org_id, &org_slug).await?;
        let maybe_user = MaybeUser(Some(crate::auth::middleware::AuthUser {
            user_id: user.user_id,
            github_login: user.github_login.clone(),
        }));
        return render(OrgTemplate {
            org_slug: org_slug.clone(),
            org_name,
            user: UserCtx::from(&maybe_user),
            projects,
            form_error: err,
            form_slug: slug,
            form_name: name,
        })
        .map(|h| h.into_response());
    }

    let result = sqlx::query(
        "INSERT INTO projects (org_id, slug, name, public, description) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(org_id)
    .bind(&slug)
    .bind(&name)
    .bind(public)
    .bind(form.description.as_deref().filter(|d| !d.is_empty()))
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => Ok(Redirect::to(&format!("/{org_slug}/{slug}")).into_response()),
        Err(sqlx::Error::Database(ref e)) if e.constraint() == Some("projects_org_id_slug_key") => {
            let projects = fetch_org_projects(&state, org_id, &org_slug).await?;
            let maybe_user = MaybeUser(Some(crate::auth::middleware::AuthUser {
                user_id: user.user_id,
                github_login: user.github_login.clone(),
            }));
            render(OrgTemplate {
                org_slug: org_slug.clone(),
                org_name,
                user: UserCtx::from(&maybe_user),
                projects,
                form_error: format!("A project named '{slug}' already exists in this org."),
                form_slug: slug,
                form_name: name,
            })
            .map(|h| h.into_response())
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

#[derive(Deserialize)]
struct CompareQuery {
    a: Option<String>,
    b: Option<String>,
}

async fn settings(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    type PRow = (i64, String, Option<String>, bool, DateTime<Utc>);
    let row = sqlx::query_as::<_, PRow>(
        r#"
        SELECT p.id, p.name, p.description, p.public, p.created_at
        FROM projects p
        JOIN orgs o ON o.id = p.org_id
        JOIN org_members om ON om.org_id = o.id
        WHERE o.slug = $1 AND p.slug = $2 AND om.user_id = $3
        "#,
    )
    .bind(&org_slug)
    .bind(&project_slug)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::NotFound)?;

    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs WHERE project_id = $1")
        .bind(row.0)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let maybe_user = MaybeUser(Some(crate::auth::middleware::AuthUser {
        user_id: user.user_id,
        github_login: user.github_login.clone(),
    }));
    render(SettingsTemplate {
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        project_name: row.1,
        project_description: row.2.unwrap_or_default(),
        project_public: row.3,
        project_id: row.0,
        created_at: row.4.format("%b %d %Y").to_string(),
        run_count,
        form_error: String::new(),
    })
}

#[derive(Deserialize)]
struct UpdateSettingsForm {
    name: String,
    description: Option<String>,
    public: Option<String>,
}

async fn update_settings(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Form(form): Form<UpdateSettingsForm>,
) -> Result<axum::response::Response, AppError> {
    let public = form.public.as_deref() == Some("on") || form.public.as_deref() == Some("true");
    let name = form.name.trim().to_string();
    let description = form
        .description
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(str::trim)
        .map(str::to_string);

    if name.is_empty() || name.len() > 100 {
        return Err(AppError::BadRequest(
            "Name must be between 1 and 100 characters.".into(),
        ));
    }

    let updated = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE projects p SET name = $3, description = $4, public = $5
        FROM orgs o
        JOIN org_members om ON om.org_id = o.id
        WHERE p.org_id = o.id AND o.slug = $1 AND p.slug = $2 AND om.user_id = $6
        RETURNING p.id
        "#,
    )
    .bind(&org_slug)
    .bind(&project_slug)
    .bind(&name)
    .bind(&description)
    .bind(public)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if updated.is_none() {
        return Err(AppError::NotFound);
    }

    Ok(
        axum::response::Redirect::to(&format!("/{org_slug}/{project_slug}/settings"))
            .into_response(),
    )
}

#[derive(Deserialize)]
struct DeleteProjectForm {
    confirm_slug: String,
}

async fn delete_project(
    State(state): State<AppState>,
    RequireUser(user): RequireUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Form(form): Form<DeleteProjectForm>,
) -> Result<axum::response::Response, AppError> {
    if form.confirm_slug.trim() != project_slug {
        return Err(AppError::BadRequest(
            "Slug confirmation did not match.".into(),
        ));
    }

    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        DELETE FROM projects p
        USING orgs o, org_members om
        WHERE p.org_id = o.id AND om.org_id = o.id
          AND o.slug = $1 AND p.slug = $2 AND om.user_id = $3
        RETURNING p.id
        "#,
    )
    .bind(&org_slug)
    .bind(&project_slug)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;

    if deleted.is_none() {
        return Err(AppError::NotFound);
    }

    Ok(axum::response::Redirect::to(&format!("/{org_slug}")).into_response())
}

async fn compare(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(q): Query<CompareQuery>,
) -> Result<Html<String>, AppError> {
    let run_id_a = q.a.unwrap_or_default();
    let run_id_b = q.b.unwrap_or_default();
    render(CompareTemplate {
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
        run_id_a,
        run_id_b,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(landing))
        .route("/:org_slug", get(org_page))
        .route("/:org_slug/projects", post(create_project_web))
        .route("/:org_slug/:project_slug", get(project))
        .route("/:org_slug/:project_slug/runs-partial", get(runs_partial))
        .route("/:org_slug/:project_slug/runs/:run_id", get(run_detail))
        .route("/:org_slug/:project_slug/sweeps", get(sweeps_list))
        .route("/:org_slug/:project_slug/sweeps/:sweep_id", get(sweep))
        .route("/:org_slug/:project_slug/training", get(training_list))
        .route(
            "/:org_slug/:project_slug/training_runs/:id",
            get(training_run),
        )
        .route("/:org_slug/:project_slug/compare", get(compare))
        .route(
            "/:org_slug/:project_slug/settings",
            get(settings).post(update_settings),
        )
        .route(
            "/:org_slug/:project_slug/settings/delete",
            post(delete_project),
        )
        .route("/me", get(account))
        .route("/me/keys", post(create_key))
        .route("/me/keys/:id", delete(revoke_key))
}
