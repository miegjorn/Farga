pub mod artifacts;
pub mod context;
pub mod signals;

use axum::{routing::{get, post}, Router};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/context/org/:org", get(context::get_org))
        .route("/context/initiatives/:org", get(context::get_initiatives))
        .route("/context/project/:project", get(context::get_project))
        .route("/context/component/:project/*path", get(context::get_component))
        .route("/signals", post(signals::post_signals))
        .route("/signals/recent", get(signals::get_recent_signals))
        .route("/artifacts", post(artifacts::post_artifact))
        .route("/artifacts/:project", get(artifacts::get_artifacts))
        .with_state(state)
}
