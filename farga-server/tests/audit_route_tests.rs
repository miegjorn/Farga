use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use farga_core::writer::AuditEntry;
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
        docs: Arc::new(DocsTree::new(PathBuf::from("/tmp/farga-audit-test-docs"))),
    };
    routes::router(state)
}

fn make_entry(agent: &str, capability: &str) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now(),
        agent: agent.to_string(),
        capability: capability.to_string(),
        outcome: "success".to_string(),
        token_id: "tok_test123".to_string(),
    }
}

// ── POST /audit ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn post_audit_returns_201_for_valid_entry() {
    let app = test_app(test_pool().await);
    let entry = make_entry("gardian", "github_access");

    let req = Request::builder()
        .method("POST")
        .uri("/audit")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&entry).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn post_audit_rejects_empty_body_with_422() {
    let app = test_app(test_pool().await);

    let req = Request::builder()
        .method("POST")
        .uri("/audit")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // Missing required fields (timestamp, agent, capability, outcome, token_id)
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ── GET /audit ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_audit_returns_empty_array_when_no_entries() {
    let app = test_app(test_pool().await);

    let req = Request::builder()
        .method("GET")
        .uri("/audit")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn get_audit_returns_entry_after_post() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // POST an entry
    let entry = make_entry("guilhem", "github_access");
    let post_req = Request::builder()
        .method("POST")
        .uri("/audit")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&entry).unwrap()))
        .unwrap();
    let post_resp = app.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_resp.status(), StatusCode::CREATED);

    // GET and verify
    let get_req = Request::builder()
        .method("GET")
        .uri("/audit")
        .body(Body::empty())
        .unwrap();
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let entries: Vec<AuditEntry> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].agent, "guilhem");
    assert_eq!(entries[0].capability, "github_access");
    assert_eq!(entries[0].outcome, "success");
    assert_eq!(entries[0].token_id, "tok_test123");
}

#[tokio::test]
async fn get_audit_filters_by_agent() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // POST two entries for different agents
    for (agent, cap) in [("gardian", "github_access"), ("guilhem", "openai_access")] {
        let req = Request::builder()
            .method("POST")
            .uri("/audit")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&make_entry(agent, cap)).unwrap()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // GET with agent filter -- should only return gardian's entry
    let req = Request::builder()
        .method("GET")
        .uri("/audit?agent=gardian")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let entries: Vec<AuditEntry> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(entries.len(), 1, "expected exactly one entry for agent=gardian");
    assert_eq!(entries[0].agent, "gardian");
    assert_eq!(entries[0].capability, "github_access");
}

#[tokio::test]
async fn get_audit_respects_limit_parameter() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // POST three entries
    for i in 0..3u32 {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            agent: "guilhem".to_string(),
            capability: format!("capability_{}", i),
            outcome: "success".to_string(),
            token_id: format!("tok_{}", i),
        };
        let req = Request::builder()
            .method("POST")
            .uri("/audit")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // GET with limit=2
    let req = Request::builder()
        .method("GET")
        .uri("/audit?limit=2")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let entries: Vec<AuditEntry> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(entries.len(), 2, "limit=2 must return at most 2 entries");
}

#[tokio::test]
async fn get_audit_returns_newest_first() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // POST two entries sequentially; ORDER BY created_at DESC means the last written appears first.
    for cap in ["first_capability", "second_capability"] {
        let req = Request::builder()
            .method("POST")
            .uri("/audit")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&make_entry("guilhem", cap)).unwrap()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let req = Request::builder()
        .method("GET")
        .uri("/audit")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let entries: Vec<AuditEntry> = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(entries.len(), 2);
    // The most recently inserted entry should appear first.
    assert_eq!(entries[0].capability, "second_capability");
    assert_eq!(entries[1].capability, "first_capability");
}
