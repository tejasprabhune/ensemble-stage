use axum::{Router, routing::get};
use crate::AppState;
use crate::auth::github;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/github/login", get(github::login))
        .route("/github/callback", get(github::callback))
        .route("/logout", get(github::logout))
}
