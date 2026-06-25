use axum::{body::Body, http::{Request, StatusCode}};
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
        docs: Arc::new(DocsTree::new(PathBuf::from("/tmp/farga-kv-test-docs"))),
    };
    routes::router(state)
}

// ── PUT then GET round-trip ────────────────────────────────────────────────

#[tokio::test]
async fn put_and_get_kv_entry() {
    let pool = test_pool().await;
    let app = test_app(pool);

    // PUT
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/guilhem/instances/pod-abc")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":{"version":"1.0","started_at":"2026-06-25T00:00:00Z"}}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "PUT should return 204");

    // GET
    let req = Request::builder()
        .method("GET")
        .uri("/kv/guilhem/instances/pod-abc")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["key"].as_str(), Some("pod-abc"));
    assert_eq!(json["namespace"].as_str(), Some("guilhem/instances"));
    assert!(json["value"].is_object());
}

// ── PUT is idempotent (upsert) ─────────────────────────────────────────────

#[tokio::test]
async fn put_kv_upserts_existing_key() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    for i in 0..3u32 {
        let body = format!(r#"{{"value":{{"tick":{}}}}}"#, i);
        let req = Request::builder()
            .method("PUT")
            .uri("/kv/ns/key1")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();
    }

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nodes WHERE kind = 'KV' AND title = 'ns/key1' AND stale = 0"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1, "upsert must not create duplicate rows");

    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns/key1")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["value"]["tick"], 2, "GET must return the latest value");
}

// ── GET on missing key returns 404 ─────────────────────────────────────────

#[tokio::test]
async fn get_missing_kv_key_returns_404() {
    let pool = test_pool().await;
    let app = test_app(pool);
    let req = Request::builder()
        .method("GET")
        .uri("/kv/guilhem/instances/nonexistent")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── LIST namespace ─────────────────────────────────────────────────────────

#[tokio::test]
async fn list_kv_namespace_returns_all_live_keys() {
    let pool = test_pool().await;
    let app = test_app(pool);

    for key in &["pod-a", "pod-b", "pod-c"] {
        let req = Request::builder()
            .method("PUT")
            .uri(&format!("/kv/guilhem/instances/{}", key))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"value":{"alive":true}}"#))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();
    }

    let req = Request::builder()
        .method("GET")
        .uri("/kv/guilhem/instances")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 3);
}

// ── DELETE ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_kv_key_removes_from_list() {
    let pool = test_pool().await;
    let app = test_app(pool);

    // create
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns/to-delete")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":"bye"}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // delete
    let req = Request::builder()
        .method("DELETE")
        .uri("/kv/ns/to-delete")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET should 404
    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns/to-delete")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── TTL: expired key is invisible ─────────────────────────────────────────

#[tokio::test]
async fn expired_kv_key_returns_404() {
    let pool = test_pool().await;
    let app = test_app(pool.clone());

    // Write with ttl_seconds = 0 (expires immediately)
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns/expiring")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":"soon gone","ttl_seconds":0}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // Manually backdate expires_at so the entry is already expired
    sqlx::query(
        "UPDATE nodes SET expires_at = '2000-01-01T00:00:00Z' WHERE kind = 'KV' AND title = 'ns/expiring'"
    )
    .execute(&pool)
    .await
    .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns/expiring")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "expired key must be invisible");
}

// ── PATCH: merge JSON object value ────────────────────────────────────────

#[tokio::test]
async fn patch_kv_merges_json_fields() {
    let pool = test_pool().await;
    let app = test_app(pool);

    // Create with initial value
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/guilhem/proposals/uuid-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":{"state":"open","acks":[]}}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // PATCH to merge ack
    let req = Request::builder()
        .method("PATCH")
        .uri("/kv/guilhem/proposals/uuid-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"merge":{"acks":["pod-a"]}}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET to verify merge — acks should now be ["pod-a"]
    let req = Request::builder()
        .method("GET")
        .uri("/kv/guilhem/proposals/uuid-1")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let acks = &json["value"]["acks"];
    assert!(acks.as_array().unwrap().contains(&serde_json::json!("pod-a")));
}

// ── PATCH: update TTL in place ─────────────────────────────────────────────

#[tokio::test]
async fn patch_kv_can_update_ttl() {
    let pool = test_pool().await;
    let app = test_app(pool);

    // Create a permanent (no-TTL) entry.
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns/ttl-patch")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":{"n":1}}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // PATCH it to expire in the past — proves PATCH writes expires_at.
    let req = Request::builder()
        .method("PATCH")
        .uri("/kv/ns/ttl-patch")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"ttl_seconds":-100}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET must now treat it as expired → 404.
    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns/ttl-patch")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "PATCH ttl_seconds must update expiry");
}

#[tokio::test]
async fn patch_kv_ttl_only_preserves_value() {
    let pool = test_pool().await;
    let app = test_app(pool);

    // Create a permanent entry.
    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns/ttl-keep")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":{"n":42}}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // PATCH only the TTL (no merge) — value must survive, expiry set.
    let req = Request::builder()
        .method("PATCH")
        .uri("/kv/ns/ttl-keep")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"ttl_seconds":3600}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET: value intact, expires_at now populated.
    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns/ttl-keep")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["value"]["n"].as_i64(), Some(42), "ttl-only PATCH must not drop value");
    assert!(json["expires_at"].is_string(), "expires_at must be set after PATCH ttl_seconds");
}

#[tokio::test]
async fn patch_missing_kv_key_returns_404() {
    let pool = test_pool().await;
    let app = test_app(pool);

    let req = Request::builder()
        .method("PATCH")
        .uri("/kv/ns/nope")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"ttl_seconds":30}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Namespaces are isolated ────────────────────────────────────────────────

#[tokio::test]
async fn list_kv_namespace_scoped_correctly() {
    let pool = test_pool().await;
    let app = test_app(pool);

    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns-a/key1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":1}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("PUT")
        .uri("/kv/ns-b/key1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"value":2}"#))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/kv/ns-a")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 1, "ns-a must only see its own keys");
    assert_eq!(entries[0]["namespace"].as_str(), Some("ns-a"));
}
