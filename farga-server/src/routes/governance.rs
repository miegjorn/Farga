use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use farga_core::types::GovernanceContribution;
use serde::Deserialize;
use crate::{
    db::{insert_governance_contribution, count_precedent_rejections, insert_governance_decision, get_assessment_by_node},
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

pub async fn get_governance_config(State(s): State<AppState>) -> String {
    s.docs.read_governance_config().unwrap_or_default()
}

#[derive(Deserialize)]
pub struct GovernanceDecisionRequest {
    pub node_id: String,
    pub outcome: String,
    pub rationale: String,
}

pub async fn post_governance_decision(
    State(s): State<AppState>,
    Json(req): Json<GovernanceDecisionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match insert_governance_decision(&s.pool, &req.node_id, &req.outcome, &req.rationale).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => {
            tracing::error!("insert governance decision failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}

pub async fn get_assessment(
    State(s): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match get_assessment_by_node(&s.pool, &node_id).await {
        Ok(Some(row)) => (StatusCode::OK, Json(row)),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "assessment not found" })),
        ),
        Err(e) => {
            tracing::error!("get_assessment failed for node {}: {}", node_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tower::ServiceExt;
    use crate::docs::DocsTree;
    use std::path::PathBuf;

    async fn test_router(pool: SqlitePool) -> Router {
        let state = AppState {
            pool,
            docs: Arc::new(DocsTree::new(PathBuf::from("."))),
        };
        Router::new()
            .route(
                "/governance/assessments/:node_id",
                axum::routing::get(get_assessment),
            )
            .with_state(state)
    }

    #[sqlx::test]
    async fn get_assessment_returns_404_for_missing_node(pool: SqlitePool) {
        // governance_assessments table must exist — create it inline for the in-memory pool
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS governance_assessments (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                reversibility TEXT,
                impact TEXT,
                routing TEXT,
                notes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let app = test_router(pool).await;
        let req = Request::builder()
            .uri("/governance/assessments/nonexistent-node")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
