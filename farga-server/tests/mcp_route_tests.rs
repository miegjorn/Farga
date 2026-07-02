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

// ── write_signal TTL / supersedes tests ──────────────────────────────────────

async fn mcp_call(app: axum::Router, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&call).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn extract_text(json: &serde_json::Value) -> &str {
    json["result"]["content"][0]["text"].as_str().unwrap_or("")
}

/// A signal written with ttl_hours should have expires_at set in the DB.
#[tokio::test]
async fn write_signal_with_ttl_sets_expires_at() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    let resp = mcp_call(app, "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "health check OK",
        "source": "sre-watchdog",
        "ttl_hours": 1
    })).await;

    assert!(resp["error"].is_null(), "unexpected error: {:?}", resp["error"]);
    let text = extract_text(&resp);
    assert!(text.contains("Signal written"), "unexpected: {}", text);

    // Extract the signal ID from the response text: "Signal written (id: <uuid>)"
    let id = text.split("id: ").nth(1).and_then(|s| s.strip_suffix(')')).unwrap();

    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT expires_at FROM nodes WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .unwrap();

    let (expires_at,) = row.expect("signal node not found");
    assert!(expires_at.is_some(), "expires_at should be set for a TTL signal");
}

/// search_signals must NOT return a signal whose expires_at is in the past.
/// We simulate this by writing a signal and then manually back-dating its expires_at.
#[tokio::test]
async fn search_signals_excludes_expired_signals() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // Write a signal with a future TTL (1 hour).
    let write_resp = mcp_call(app.clone(), "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "transient health ping",
        "source": "sre-watchdog",
        "ttl_hours": 1
    })).await;
    assert!(write_resp["error"].is_null());
    let text = extract_text(&write_resp);
    let id = text.split("id: ").nth(1).and_then(|s| s.strip_suffix(')')).unwrap();

    // Back-date expires_at to the past so the signal appears expired.
    sqlx::query("UPDATE nodes SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    // search_signals should return nothing.
    let search_resp = mcp_call(app, "search_signals", serde_json::json!({
        "project": "occitan"
    })).await;
    assert!(search_resp["error"].is_null());
    let search_text = extract_text(&search_resp);
    assert!(
        search_text.contains("No signals found"),
        "expected expired signal to be excluded, got: {}",
        search_text
    );
}

/// search_signals must return live (non-expired) signals and include their IDs.
#[tokio::test]
async fn search_signals_returns_id_in_output() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    let write_resp = mcp_call(app.clone(), "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "component farga is healthy",
        "source": "farga-agent"
    })).await;
    assert!(write_resp["error"].is_null());
    let text = extract_text(&write_resp);
    let id = text.split("id: ").nth(1).and_then(|s| s.strip_suffix(')')).unwrap();

    let search_resp = mcp_call(app, "search_signals", serde_json::json!({
        "project": "occitan"
    })).await;
    assert!(search_resp["error"].is_null());
    let search_text = extract_text(&search_resp);
    assert!(
        search_text.contains(id),
        "expected signal ID '{}' in search output, got: {}",
        id,
        search_text
    );
    assert!(
        search_text.contains("component farga is healthy"),
        "expected signal content in output, got: {}",
        search_text
    );
}

/// write_signal with supersedes should mark the old signal stale
/// and insert a supersedes edge.
#[tokio::test]
async fn write_signal_with_supersedes_marks_old_stale() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // Write the old signal.
    let old_resp = mcp_call(app.clone(), "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "guilhem /health unreachable",
        "source": "sre-watchdog"
    })).await;
    assert!(old_resp["error"].is_null());
    let old_text = extract_text(&old_resp);
    let old_id = old_text.split("id: ").nth(1).and_then(|s| s.strip_suffix(')')).unwrap();

    // Write the superseding signal.
    let new_resp = mcp_call(app.clone(), "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "guilhem /health recovered",
        "source": "sre-watchdog",
        "supersedes": old_id
    })).await;
    assert!(new_resp["error"].is_null(), "unexpected error: {:?}", new_resp["error"]);
    let new_text = extract_text(&new_resp);
    let new_id = new_text.split("id: ").nth(1).and_then(|s| s.strip_suffix(')')).unwrap();

    // Old signal must be stale.
    let row: Option<(i64,)> = sqlx::query_as("SELECT stale FROM nodes WHERE id = ?")
        .bind(old_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert_eq!(row.unwrap().0, 1, "old signal should be marked stale");

    // A supersedes edge must exist from new → old.
    let edge: Option<(String,)> = sqlx::query_as(
        "SELECT kind FROM edges WHERE from_id = ? AND to_id = ?"
    )
    .bind(new_id)
    .bind(old_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(edge.is_some(), "supersedes edge should exist");
    assert_eq!(edge.unwrap().0, "supersedes");

    // search_signals must not return the old stale signal.
    let search_resp = mcp_call(app, "search_signals", serde_json::json!({
        "project": "occitan"
    })).await;
    let search_text = extract_text(&search_resp);
    assert!(
        !search_text.contains("guilhem /health unreachable"),
        "stale signal must not appear in search, got: {}",
        search_text
    );
    assert!(
        search_text.contains("guilhem /health recovered"),
        "new signal must appear in search, got: {}",
        search_text
    );
}

/// write_signal with a negative ttl_hours must return an error.
#[tokio::test]
async fn write_signal_rejects_non_positive_ttl() {
    let pool = test_pool().await;
    let app = test_app(pool);

    let resp = mcp_call(app, "write_signal", serde_json::json!({
        "project": "occitan",
        "content": "bad ttl signal",
        "ttl_hours": -1
    })).await;

    // JSON-RPC errors surface in the error field.
    assert!(
        !resp["error"].is_null(),
        "expected error for negative ttl_hours, got: {:?}",
        resp
    );
}
