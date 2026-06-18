use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use farga_core::types::GovernanceContribution;
use serde::Deserialize;
use crate::{
    db::{insert_governance_contribution, count_precedent_rejections},
    state::AppState,
};

pub async fn post_governance(
    State(s): State<AppState>,
    Json(contrib): Json<GovernanceContribution>,
) -> (StatusCode, Json<serde_json::Value>) {
    match insert_governance_contribution(&s.pool, &contrib).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))),
        Err(e) => {
            tracing::error!("insert governance contribution failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}

#[derive(Deserialize)]
pub struct PrecedentQuery {
    pub keywords: String,
}

pub async fn get_precedent(
    State(s): State<AppState>,
    Query(q): Query<PrecedentQuery>,
) -> Json<serde_json::Value> {
    let count = count_precedent_rejections(&s.pool, &q.keywords)
        .await
        .unwrap_or(0);
    Json(serde_json::json!({ "rejection_count": count }))
}
