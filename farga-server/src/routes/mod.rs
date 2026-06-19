pub mod artifacts;
pub mod context;
pub mod governance;
pub mod mcp;
pub mod signals;

use axum::{routing::{get, post}, Router};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        // MCP server — agents call this
        .route("/mcp", post(mcp::handle))
        // REST — existing routes unchanged
        .route("/context/org/:org", get(context::get_org))
        .route("/context/initiatives/:org", get(context::get_initiatives))
        .route("/context/project/:project", get(context::get_project))
        .route("/context/component/:project/*path", get(context::get_component))
        .route("/signals", post(signals::post_signals))
        .route("/signals/recent", get(signals::get_recent_signals))
        .route("/artifacts", post(artifacts::post_artifact))
        .route("/artifacts/:project", get(artifacts::get_artifacts))
        .route("/governance", post(governance::post_governance))
        .route("/governance/precedent", get(governance::get_precedent))
        .route("/governance/config", get(governance::get_governance_config))
        .route("/governance/decisions", post(governance::post_governance_decision))
        .route("/governance/assessments/:node_id", get(governance::get_assessment))
        .with_state(state)
}
