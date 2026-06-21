use axum::{extract::{Query, State}, http::StatusCode, Json};
use farga_core::types::{Node, NodeKind};
use farga_core::writer::AuditEntry;
use serde::Deserialize;
use crate::{db::insert_node, state::AppState};

pub async fn post_audit(
    State(s): State<AppState>,
    Json(entry): Json<AuditEntry>,
) -> StatusCode {
    let content = match serde_json::to_string(&entry) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("audit serialise failed: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };
    let mut node = Node::new(NodeKind::AuditEntry, Some("system".into()), Some(content));
    node.title = Some(format!("{} — {}", entry.agent, entry.capability));
    if let Err(e) = insert_node(&s.pool, &node).await {
        tracing::error!("insert audit entry failed: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::CREATED
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub agent: Option<String>,
    pub limit: Option<i64>,
}

pub async fn get_audit(
    State(s): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Json<Vec<AuditEntry>> {
    let limit = q.limit.unwrap_or(100).min(500);
    let rows: Vec<(Option<String>,)> = if let Some(agent) = q.agent {
        sqlx::query_as(
            "SELECT content FROM nodes WHERE kind = 'AuditEntry' AND content LIKE ? AND stale = 0 ORDER BY created_at DESC LIMIT ?"
        )
        .bind(format!("%\"agent\":\"{}\"", agent))
        .bind(limit)
        .fetch_all(&s.pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT content FROM nodes WHERE kind = 'AuditEntry' AND stale = 0 ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&s.pool)
        .await
        .unwrap_or_default()
    };

    Json(rows.into_iter().filter_map(|(content,)| {
        content.and_then(|c| serde_json::from_str::<AuditEntry>(&c).ok())
    }).collect())
}
