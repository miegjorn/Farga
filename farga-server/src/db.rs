use farga_core::types::{Edge, Node, NodeKind, EdgeKind, GovernanceContribution};
use sqlx::SqlitePool;
use anyhow::Result;
use std::str::FromStr;
use uuid::Uuid;

pub async fn insert_node(pool: &SqlitePool, node: &Node) -> Result<()> {
    let stale = node.stale as i64;
    let created_at = node.created_at.to_rfc3339();
    let updated_at = node.updated_at.to_rfc3339();
    let kind = node.kind.as_str().to_string();
    sqlx::query(
        "INSERT INTO nodes (id, kind, address, project, component, title, content, created_at, updated_at, stale)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&node.id)
    .bind(&kind)
    .bind(&node.address)
    .bind(&node.project)
    .bind(&node.component)
    .bind(&node.title)
    .bind(&node.content)
    .bind(&created_at)
    .bind(&updated_at)
    .bind(stale)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_node(pool: &SqlitePool, id: &str) -> Result<Node> {
    let row: (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String, String, i64) =
        sqlx::query_as(
            "SELECT id, kind, address, project, component, title, content, created_at, updated_at, stale FROM nodes WHERE id = ?"
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

    Ok(Node {
        id: row.0,
        kind: NodeKind::from_str(&row.1).map_err(|e| anyhow::anyhow!(e))?,
        address: row.2,
        project: row.3,
        component: row.4,
        title: row.5,
        content: row.6,
        created_at: chrono::DateTime::parse_from_rfc3339(&row.7)?.into(),
        updated_at: chrono::DateTime::parse_from_rfc3339(&row.8)?.into(),
        stale: row.9 != 0,
    })
}

pub async fn mark_stale(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("UPDATE nodes SET stale = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_edge(pool: &SqlitePool, edge: &Edge) -> Result<()> {
    let kind = edge.kind.as_str().to_string();
    let created_at = edge.created_at.to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&edge.from_id)
    .bind(&edge.to_id)
    .bind(&kind)
    .bind(edge.weight)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_subgraph(pool: &SqlitePool, root_id: &str, depth: u32) -> Result<(Vec<Node>, Vec<Edge>)> {
    // BFS from root_id up to depth hops
    let mut visited_ids: Vec<String> = vec![root_id.to_string()];
    let mut frontier = vec![root_id.to_string()];

    for _ in 0..depth {
        if frontier.is_empty() { break; }
        let mut next_frontier = Vec::new();
        for id in &frontier {
            let rows: Vec<(Option<String>,)> = sqlx::query_as(
                "SELECT to_id FROM edges WHERE from_id = ? UNION SELECT from_id FROM edges WHERE to_id = ?"
            )
            .bind(id)
            .bind(id)
            .fetch_all(pool)
            .await?;
            for (neighbor_opt,) in rows {
                let neighbor = neighbor_opt.unwrap_or_default();
                if !visited_ids.contains(&neighbor) {
                    visited_ids.push(neighbor.clone());
                    next_frontier.push(neighbor);
                }
            }
        }
        frontier = next_frontier;
    }

    let mut nodes = Vec::new();
    for id in &visited_ids {
        if let Ok(n) = get_node(pool, id).await {
            nodes.push(n);
        }
    }

    let edge_rows: Vec<(String, String, String, f64, String)> = sqlx::query_as(
        "SELECT from_id, to_id, kind, weight, created_at FROM edges WHERE from_id IN (SELECT id FROM nodes WHERE stale = 0)"
    )
    .fetch_all(pool)
    .await?;

    let edges = edge_rows.into_iter().filter_map(|(from_id, to_id, kind, weight, created_at)| {
        Some(Edge {
            from_id,
            to_id,
            kind: EdgeKind::from_str(&kind).ok()?,
            weight,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at).ok()?.into(),
        })
    }).collect();

    Ok((nodes, edges))
}

pub async fn insert_governance_contribution(
    pool: &SqlitePool,
    contrib: &GovernanceContribution,
) -> Result<String> {
    let content = serde_json::to_string(contrib)?;
    let mut node = Node::new(
        NodeKind::GovernanceContribution,
        Some("system".into()),
        Some(content),
    );
    node.title = Some(contrib.title.clone());
    let node_id = node.id.clone();
    insert_node(pool, &node).await?;

    let assess_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO governance_assessments (id, node_id, status, created_at, updated_at)
         VALUES (?, ?, 'pending', ?, ?)",
    )
    .bind(&assess_id)
    .bind(&node_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(node_id)
}

pub async fn insert_governance_decision(
    pool: &SqlitePool,
    node_id: &str,
    outcome: &str,
    rationale: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE governance_assessments SET status = ?, notes = ?, updated_at = ? WHERE node_id = ?",
    )
    .bind(outcome)
    .bind(rationale)
    .bind(&now)
    .bind(node_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_precedent_rejections(pool: &SqlitePool, keywords: &str) -> Result<u32> {
    let pattern = format!("%{}%", keywords);
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM governance_assessments ga
         JOIN nodes n ON ga.node_id = n.id
         WHERE ga.status = 'rejected' AND n.title LIKE ?",
    )
    .bind(&pattern)
    .fetch_one(pool)
    .await?;
    Ok(row.0 as u32)
}

pub async fn get_assessment_by_node(
    pool: &SqlitePool,
    node_id: &str,
) -> Result<Option<serde_json::Value>> {
    let row: Option<(String, Option<String>, Option<String>, Option<String>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT status, reversibility, impact, routing, notes, updated_at
             FROM governance_assessments
             WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|(status, reversibility, impact, routing, notes, updated_at)| {
        serde_json::json!({
            "node_id": node_id,
            "status": status,
            "reversibility": reversibility,
            "impact": impact,
            "routing": routing,
            "notes": notes,
            "updated_at": updated_at,
        })
    }))
}
