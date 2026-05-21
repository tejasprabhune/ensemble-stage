use crate::auth::github;
use crate::AppState;
use axum::{routing::{get, post}, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/github/login", get(github::login))
        .route("/github/callback", get(github::callback))
        .route("/logout", post(github::logout))
}
