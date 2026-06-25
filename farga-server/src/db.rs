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

pub async fn upsert_component_todo(
    pool: &SqlitePool,
    project: &str,
    component: &str,
    content: &str,
) -> Result<String> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM nodes WHERE kind = 'ComponentLayer' AND project = ? AND component = ? LIMIT 1"
    )
    .bind(project)
    .bind(component)
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = existing {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE nodes SET content = ?, updated_at = ?, stale = 0 WHERE id = ?")
            .bind(content)
            .bind(&updated_at)
            .bind(&id)
            .execute(pool)
            .await?;
        Ok(id)
    } else {
        let mut node = Node::new(NodeKind::ComponentLayer, Some(project.to_string()), Some(content.to_string()));
        node.component = Some(component.to_string());
        let id = node.id.clone();
        insert_node(pool, &node).await?;
        Ok(id)
    }
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

// ── KV store ─────────────────────────────────────────────────────────────────

/// A live KV entry returned from the DB.
#[derive(Debug, Clone)]
pub struct KvRow {
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub expires_at: Option<String>,
}

/// Split a kv_path like "guilhem/instances/pod-abc" into
/// ("guilhem/instances", "pod-abc").  Single-segment paths map to ("", path).
fn split_kv_path(kv_path: &str) -> (String, String) {
    match kv_path.rfind('/') {
        Some(pos) => (kv_path[..pos].to_string(), kv_path[pos + 1..].to_string()),
        None => (String::new(), kv_path.to_string()),
    }
}

fn parse_kv_row(title: Option<String>, content: Option<String>, expires_at: Option<String>) -> Option<KvRow> {
    let t = title?;
    let raw = content.unwrap_or_default();
    let value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw));
    let (namespace, key) = split_kv_path(&t);
    Some(KvRow { namespace, key, value, expires_at })
}

/// Upsert a KV entry.  kv_path is the full "{namespace}/{key}" string stored as title.
/// ttl_seconds = None means no expiry.
pub async fn upsert_kv(
    pool: &SqlitePool,
    kv_path: &str,
    value_json: &str,
    ttl_seconds: Option<i64>,
) -> Result<()> {
    let now = chrono::Utc::now();
    let expires_at = ttl_seconds.map(|ttl| {
        (now + chrono::Duration::seconds(ttl)).to_rfc3339()
    });
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM nodes WHERE kind = 'KV' AND title = ? AND stale = 0 LIMIT 1"
    )
    .bind(kv_path)
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = existing {
        let updated_at = now.to_rfc3339();
        sqlx::query(
            "UPDATE nodes SET content = ?, expires_at = ?, updated_at = ? WHERE id = ?"
        )
        .bind(value_json)
        .bind(&expires_at)
        .bind(&updated_at)
        .bind(&id)
        .execute(pool)
        .await?;
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let ts = now.to_rfc3339();
        sqlx::query(
            "INSERT INTO nodes (id, kind, title, content, expires_at, created_at, updated_at, stale)
             VALUES (?, 'KV', ?, ?, ?, ?, ?, 0)"
        )
        .bind(&id)
        .bind(kv_path)
        .bind(value_json)
        .bind(&expires_at)
        .bind(&ts)
        .bind(&ts)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Read a single KV entry.  Returns None if not found, stale, or expired.
pub async fn get_kv(pool: &SqlitePool, kv_path: &str) -> Result<Option<KvRow>> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: Option<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT title, content, expires_at FROM nodes
         WHERE kind = 'KV' AND title = ? AND stale = 0
           AND (expires_at IS NULL OR expires_at > ?)
         LIMIT 1"
    )
    .bind(kv_path)
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(title, content, exp)| parse_kv_row(title, content, exp)))
}

/// List all live, non-expired entries in a namespace.
/// namespace is the prefix before the last "/", e.g. "guilhem/instances".
pub async fn list_kv_namespace(pool: &SqlitePool, namespace: &str) -> Result<Vec<KvRow>> {
    let now = chrono::Utc::now().to_rfc3339();
    let prefix = format!("{}/", namespace);
    let rows: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT title, content, expires_at FROM nodes
         WHERE kind = 'KV' AND title LIKE ? AND stale = 0
           AND (expires_at IS NULL OR expires_at > ?)
         ORDER BY created_at ASC"
    )
    .bind(format!("{}%", prefix))
    .bind(&now)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(|(t, c, e)| parse_kv_row(t, c, e)).collect())
}

/// Mark a KV entry as stale (logical delete).  Returns true if a row was found and deleted.
pub async fn delete_kv(pool: &SqlitePool, kv_path: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE nodes SET stale = 1, updated_at = ? WHERE kind = 'KV' AND title = ? AND stale = 0"
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(kv_path)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// PATCH: merge `merge_json` (a JSON object) into the current value.
/// Non-object values are replaced wholesale.  Returns false if key not found.
pub async fn patch_kv_merge(pool: &SqlitePool, kv_path: &str, merge_json: &str) -> Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let row = get_kv(pool, kv_path).await?;
    let Some(entry) = row else { return Ok(false); };

    let patch: serde_json::Value = serde_json::from_str(merge_json)
        .unwrap_or(serde_json::Value::Null);

    let merged_value = match (entry.value, patch) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(overlay)) => {
            for (k, v) in overlay {
                base.insert(k, v);
            }
            serde_json::Value::Object(base)
        }
        (base, serde_json::Value::Null) => base,
        (_, overlay) => overlay,
    };

    let merged = serde_json::to_string(&merged_value)?;
    sqlx::query(
        "UPDATE nodes SET content = ?, updated_at = ? WHERE kind = 'KV' AND title = ? AND stale = 0"
    )
    .bind(&merged)
    .bind(&now)
    .bind(kv_path)
    .execute(pool)
    .await?;
    Ok(true)
}
