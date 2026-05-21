pub mod events;
pub mod me;
pub mod orgs;
pub mod projects;
pub mod runs;
pub mod sweeps;
pub mod training_runs;

use crate::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        // org endpoints
        .route("/orgs/:org_slug", get(orgs::get_org))
        .route("/projects/:org_slug", post(orgs::create_project))
        // project-scoped
        .route(
            "/projects/:org_slug/:project_slug",
            get(projects::get_project),
        )
        .route(
            "/projects/:org_slug/:project_slug/runs",
            get(projects::list_runs).post(projects::create_run),
        )
        .route(
            "/projects/:org_slug/:project_slug/sweeps",
            get(projects::list_sweeps).post(projects::create_sweep),
        )
        .route(
            "/projects/:org_slug/:project_slug/training_runs",
            get(projects::list_training_runs).post(projects::create_training_run),
        )
        // run endpoints
        .route("/runs/:run_id", get(runs::get_run).delete(runs::delete_run))
        .route(
            "/runs/:run_id/events",
            get(events::list_events).post(events::append_events),
        )
        .route("/runs/:run_id/status", post(runs::update_status))
        // sweep endpoints
        .route("/sweeps/:sweep_id", get(sweeps::get_sweep))
        .route(
            "/sweeps/:sweep_id/runs",
            get(orgs::get_sweep_runs).post(sweeps::register_run),
        )
        .route("/sweeps/:sweep_id/status", post(sweeps::update_status))
        .route("/sweeps/:sweep_id/cancel", post(sweeps::cancel_sweep))
        // training run endpoints
        .route("/training_runs/:id", get(training_runs::get_training_run))
        .route(
            "/training_runs/:id/metrics",
            get(orgs::get_training_metrics).post(training_runs::append_metrics),
        )
        .route(
            "/training_runs/:id/status",
            post(training_runs::update_status),
        )
        // me / api keys
        .route("/me", get(me::get_me))
        .route(
            "/me/api_keys",
            get(me::list_api_keys).post(me::create_api_key),
        )
        .route("/me/api_keys/:id", delete(me::revoke_api_key))
}
