use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use farga_core::types::{GovernanceContribution, FargaLayer};
use chrono::Utc;
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

fn test_app(pool: SqlitePool) -> axum::Router {
    use farga_server::{docs::DocsTree, routes, state::AppState};
    let state = AppState {
        pool,
        docs: Arc::new(DocsTree::new(PathBuf::from("/tmp/farga-test-docs"))),
    };
    routes::router(state)
}

fn make_contrib(title: &str) -> GovernanceContribution {
    GovernanceContribution {
        title: title.into(),
        narrative: "Two projects converged on RS256.".into(),
        lessons: vec!["Use RS256 org-wide".into()],
        open_questions: vec![],
        involved_projects: vec!["auth-service".into()],
        concurrence: vec![],
        target_layer: FargaLayer::ProjectLevel,
        first_observed_at: Utc::now(),
        last_observed_at: Utc::now(),
        event_count: 2,
        reversibility: None,
        impact: None,
    }
}

#[tokio::test]
async fn post_governance_returns_201_with_id() {
    let pool = test_pool().await;
    let app = test_app(pool);
    let body = serde_json::to_string(&make_contrib("JWT Signing Pattern")).unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/governance")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["id"].as_str().map_or(false, |s| !s.is_empty()), "response must contain non-empty id");
}

#[tokio::test]
async fn get_precedent_returns_rejection_count() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    let contrib = make_contrib("JWT Signing Pattern");
    let node_id = farga_server::db::insert_governance_contribution(&pool, &contrib).await.unwrap();
    sqlx::query("UPDATE governance_assessments SET status = 'rejected' WHERE node_id = ?")
        .bind(&node_id)
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/governance/precedent?keywords=jwt")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["rejection_count"].as_u64(), Some(1));
}

#[tokio::test]
async fn get_governance_config_returns_yaml_if_present() {
    let pool = test_pool().await;
    let docs_dir = tempfile::tempdir().unwrap();
    let config_yaml = "governance:\n  risk_weights:\n    primitive_proximity: 0.25\n";
    std::fs::write(docs_dir.path().join("governance.yaml"), config_yaml).unwrap();

    use farga_server::{docs::DocsTree, routes, state::AppState};
    let state = AppState {
        pool,
        docs: Arc::new(DocsTree::new(docs_dir.path().to_path_buf())),
    };
    let app = routes::router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/governance/config")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("primitive_proximity"), "should return governance.yaml content");
}

#[tokio::test]
async fn get_governance_config_returns_empty_if_missing() {
    let pool = test_pool().await;
    let docs_dir = tempfile::tempdir().unwrap(); // no governance.yaml written
    use farga_server::{docs::DocsTree, routes, state::AppState};
    let state = AppState {
        pool,
        docs: Arc::new(DocsTree::new(docs_dir.path().to_path_buf())),
    };
    let app = routes::router(state);
    let req = Request::builder()
        .method("GET")
        .uri("/governance/config")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes.len(), 0);
}
