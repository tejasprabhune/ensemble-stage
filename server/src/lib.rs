use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::{services::ServeDir, trace::TraceLayer};

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;

pub use config::Config;
pub use error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
}

pub fn app(config: Config, pool: PgPool) -> Router {
    let state = AppState {
        pool,
        config: Arc::new(config),
    };

    Router::new()
        .nest("/v1", routes::v1::router())
        .nest("/auth", routes::auth::router())
        .merge(routes::web::router())
        .nest_service("/static", ServeDir::new("web/static"))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
