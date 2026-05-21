use askama::Template;
use axum::{
    Router,
    extract::Path,
    response::Html,
    routing::get,
};

use crate::{AppError, AppState, auth::MaybeUser};

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate;

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectTemplate {
    org_slug: String,
    project_slug: String,
}

#[derive(Template)]
#[template(path = "run_detail.html")]
struct RunDetailTemplate {
    run_id: String,
    org_slug: String,
    project_slug: String,
}

#[derive(Template)]
#[template(path = "sweep.html")]
struct SweepTemplate {
    sweep_id: String,
    org_slug: String,
    project_slug: String,
}

#[derive(Template)]
#[template(path = "training_run.html")]
struct TrainingRunTemplate {
    training_run_id: String,
    org_slug: String,
    project_slug: String,
}

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate;

fn render<T: Template>(t: T) -> Result<Html<String>, AppError> {
    t.render()
        .map(Html)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template: {}", e)))
}

async fn landing(maybe_user: MaybeUser) -> Result<Html<String>, AppError> {
    render(LandingTemplate)
}

async fn project(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    render(ProjectTemplate { org_slug, project_slug })
}

async fn run_detail(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(RunDetailTemplate { run_id, org_slug, project_slug })
}

async fn sweep(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, sweep_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(SweepTemplate { sweep_id, org_slug, project_slug })
}

async fn training_run(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, training_run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(TrainingRunTemplate { training_run_id, org_slug, project_slug })
}

async fn account(maybe_user: MaybeUser) -> Result<Html<String>, AppError> {
    render(AccountTemplate)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(landing))
        .route("/:org_slug/:project_slug", get(project))
        .route("/:org_slug/:project_slug/runs/:run_id", get(run_detail))
        .route("/:org_slug/:project_slug/sweeps/:sweep_id", get(sweep))
        .route("/:org_slug/:project_slug/training_runs/:id", get(training_run))
        .route("/me", get(account))
}
