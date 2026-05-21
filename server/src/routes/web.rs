use askama::Template;
use axum::{extract::Path, response::Html, routing::get, Router};

use crate::{auth::MaybeUser, AppError, AppState};

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
}

#[derive(Template)]
#[template(path = "run_detail.html")]
struct RunDetailTemplate {
    run_id: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "sweep.html")]
struct SweepTemplate {
    sweep_id: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "training_run.html")]
struct TrainingRunTemplate {
    training_run_id: String,
    org_slug: String,
    project_slug: String,
    user: UserCtx,
}

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    user: UserCtx,
}

fn render<T: Template>(t: T) -> Result<Html<String>, AppError> {
    t.render()
        .map(Html)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template: {e}")))
}

async fn landing(maybe_user: MaybeUser) -> Result<Html<String>, AppError> {
    render(LandingTemplate {
        user: UserCtx::from(&maybe_user),
    })
}

async fn project(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    render(ProjectTemplate {
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn run_detail(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(RunDetailTemplate {
        run_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn sweep(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, sweep_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(SweepTemplate {
        sweep_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn training_run(
    maybe_user: MaybeUser,
    Path((org_slug, project_slug, training_run_id)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    render(TrainingRunTemplate {
        training_run_id,
        org_slug,
        project_slug,
        user: UserCtx::from(&maybe_user),
    })
}

async fn account(maybe_user: MaybeUser) -> Result<Html<String>, AppError> {
    render(AccountTemplate {
        user: UserCtx::from(&maybe_user),
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(landing))
        .route("/:org_slug/:project_slug", get(project))
        .route("/:org_slug/:project_slug/runs/:run_id", get(run_detail))
        .route("/:org_slug/:project_slug/sweeps/:sweep_id", get(sweep))
        .route(
            "/:org_slug/:project_slug/training_runs/:id",
            get(training_run),
        )
        .route("/me", get(account))
}
