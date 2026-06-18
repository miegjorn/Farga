use farga_server::db::{insert_node, get_node, insert_edge, insert_governance_contribution, count_precedent_rejections};
use farga_core::types::{Node, NodeKind, Edge, EdgeKind, GovernanceContribution, FargaLayer};
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

fn make_contrib(title: &str) -> GovernanceContribution {
    GovernanceContribution {
        title: title.into(),
        narrative: "Test narrative".into(),
        lessons: vec![],
        open_questions: vec![],
        involved_projects: vec!["proj-a".into()],
        concurrence: vec![],
        target_layer: FargaLayer::ProjectLevel,
        first_observed_at: Utc::now(),
        last_observed_at: Utc::now(),
        event_count: 1,
        reversibility: None,
        impact: None,
    }
}

#[tokio::test]
async fn insert_governance_contribution_creates_node_and_assessment() {
    let pool = test_pool().await;
    let contrib = make_contrib("JWT Signing Pattern");
    let node_id = insert_governance_contribution(&pool, &contrib).await.unwrap();
    assert!(!node_id.is_empty());

    let node = get_node(&pool, &node_id).await.unwrap();
    assert_eq!(node.kind, NodeKind::GovernanceContribution);
    assert_eq!(node.title.as_deref(), Some("JWT Signing Pattern"));

    let status: (String,) = sqlx::query_as(
        "SELECT status FROM governance_assessments WHERE node_id = ?"
    )
    .bind(&node_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status.0, "pending");
}

#[tokio::test]
async fn count_precedent_rejections_returns_zero_when_empty() {
    let pool = test_pool().await;
    let count = count_precedent_rejections(&pool, "jwt").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn count_precedent_rejections_counts_only_rejected_rows() {
    let pool = test_pool().await;
    let id1 = insert_governance_contribution(&pool, &make_contrib("JWT Signing Pattern")).await.unwrap();
    let _id2 = insert_governance_contribution(&pool, &make_contrib("JWT Key Rotation")).await.unwrap();

    sqlx::query("UPDATE governance_assessments SET status = 'rejected' WHERE node_id = ?")
        .bind(&id1)
        .execute(&pool)
        .await
        .unwrap();

    let count = count_precedent_rejections(&pool, "jwt").await.unwrap();
    assert_eq!(count, 1);

    let count2 = count_precedent_rejections(&pool, "auth").await.unwrap();
    assert_eq!(count2, 0);
}
