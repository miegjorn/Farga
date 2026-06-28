use axum::{body::Body, http::{Request, StatusCode}};
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn get_components_returns_subdirs_with_component_md() {
    let dir = tempfile::tempdir().unwrap();
    let occitan = dir.path().join("projects/occitan");
    std::fs::create_dir_all(occitan.join("amassada")).unwrap();
    std::fs::write(occitan.join("amassada/component.md"), "# Amassada").unwrap();
    std::fs::create_dir_all(occitan.join("gardian")).unwrap();
    std::fs::write(occitan.join("gardian/component.md"), "# Gardian").unwrap();
    // a subdir WITHOUT component.md must be excluded
    std::fs::create_dir_all(occitan.join("empty-dir")).unwrap();

    let pool = test_pool().await;
    use farga_server::{docs::DocsTree, routes, state::AppState};
    let state = AppState { pool, docs: Arc::new(DocsTree::new(dir.path().to_path_buf())) };
    let app = routes::router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/context/components/occitan")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let components: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(components, vec!["amassada", "gardian"]); // sorted
}

#[tokio::test]
async fn get_components_returns_empty_for_unknown_project() {
    let dir = tempfile::tempdir().unwrap();
    let pool = test_pool().await;
    use farga_server::{docs::DocsTree, routes, state::AppState};
    let state = AppState { pool, docs: Arc::new(DocsTree::new(dir.path().to_path_buf())) };
    let app = routes::router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/context/components/nonexistent")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let components: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert!(components.is_empty());
}
