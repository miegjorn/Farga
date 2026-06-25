pub mod artifacts;
pub mod audit;
pub mod context;
pub mod governance;
pub mod kv;
pub mod mcp;
pub mod signals;

use axum::{routing::{delete, get, patch, post, put}, Router};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
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
        .route("/audit", post(audit::post_audit))
        .route("/audit", get(audit::get_audit))
        .route("/governance", post(governance::post_governance))
        .route("/governance/precedent", get(governance::get_precedent))
        .route("/governance/config", get(governance::get_governance_config))
        .route("/governance/decisions", post(governance::post_governance_decision))
        .route("/governance/assessments/:node_id", get(governance::get_assessment))
        // KV store — mutable, TTL-aware, used for inter-instance coordination
        .route("/kv/*path",
            get(kv::get_kv_or_list)
            .put(kv::put_kv)
            .delete(kv::delete_kv_handler)
            .patch(kv::patch_kv_handler))
        .with_state(state)
}
