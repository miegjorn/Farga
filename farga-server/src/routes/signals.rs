use axum::{extract::{Query, State}, http::StatusCode, Json};
use farga_core::types::{Node, NodeKind, Signal};
use serde::Deserialize;
use crate::{db::insert_node, state::AppState};

#[derive(Deserialize)]
pub struct WriteSignalsReq {
    pub project: String,
    pub signals: Vec<Signal>,
}

pub async fn post_signals(
    State(s): State<AppState>,
    Json(req): Json<WriteSignalsReq>,
) -> StatusCode {
    for sig in &req.signals {
        let node = Node::new(NodeKind::Signal, Some(req.project.clone()), Some(sig.content.clone()));
        if let Err(e) = insert_node(&s.pool, &node).await {
            tracing::error!("insert signal failed: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    StatusCode::CREATED
}

#[derive(Deserialize)]
pub struct RecentQuery { pub project: String, pub since: Option<String> }

pub async fn get_recent_signals(
    State(s): State<AppState>,
    Query(q): Query<RecentQuery>,
) -> Json<Vec<Signal>> {
    let rows: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT content, project FROM nodes WHERE kind = 'Signal' AND project = ? AND stale = 0 ORDER BY created_at DESC LIMIT 100"
    )
    .bind(&q.project)
    .fetch_all(&s.pool)
    .await
    .unwrap_or_default();

    Json(rows.into_iter().map(|(content, project)| Signal {
        project: project.unwrap_or_default(),
        content: content.unwrap_or_default(),
        source: "farga".into(),
    }).collect())
}
