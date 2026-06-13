use farga_server::db::{insert_node, get_node, insert_edge};
use farga_core::types::{Node, NodeKind, Edge, EdgeKind};
use chrono::Utc;
use sqlx::SqlitePool;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn inserts_and_retrieves_node() {
    let pool = test_pool().await;
    let node = Node::new(NodeKind::Signal, Some("auth-service".into()), Some("deployment issue".into()));
    let id = node.id.clone();
    insert_node(&pool, &node).await.unwrap();
    let retrieved = get_node(&pool, &id).await.unwrap();
    assert_eq!(retrieved.id, id);
    assert_eq!(retrieved.project, Some("auth-service".into()));
}

#[tokio::test]
async fn inserts_edge_between_nodes() {
    let pool = test_pool().await;
    let a = Node::new(NodeKind::Decision, Some("proj".into()), None);
    let b = Node::new(NodeKind::Decision, Some("proj".into()), None);
    insert_node(&pool, &a).await.unwrap();
    insert_node(&pool, &b).await.unwrap();
    let edge = Edge {
        from_id: b.id.clone(),
        to_id: a.id.clone(),
        kind: EdgeKind::SupersededBy,
        weight: 1.0,
        created_at: Utc::now(),
    };
    insert_edge(&pool, &edge).await.unwrap();
}
