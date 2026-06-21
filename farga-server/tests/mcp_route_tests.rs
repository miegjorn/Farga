use axum::{
    body::Body,
    http::{Request, StatusCode},
};
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
        docs: Arc::new(DocsTree::new(PathBuf::from("/tmp/farga-mcp-test-docs"))),
    };
    routes::router(state)
}

#[tokio::test]
async fn update_component_todo_tool_creates_and_updates_node() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "update_component_todo",
            "arguments": {
                "project": "occitan",
                "component": "gardian",
                "content": "fix flaky readiness probe"
            }
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&call).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["error"].is_null(), "expected no error, got: {:?}", json["error"]);
    let text = json["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Component TODO updated"), "unexpected response text: {}", text);

    // Confirm exactly one ComponentLayer node exists for this project+component.
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM nodes WHERE kind = 'ComponentLayer' AND project = 'occitan' AND component = 'gardian'"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows[0].0, 1);
}

#[tokio::test]
async fn update_component_todo_tool_rejects_missing_fields() {
    let pool = test_pool().await;
    let app = test_app(pool);

    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "update_component_todo",
            "arguments": { "project": "occitan" }
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&call).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "JSON-RPC errors are still HTTP 200");
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!json["error"].is_null(), "expected a JSON-RPC error for missing component/content");
}
