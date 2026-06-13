use axum::{extract::{Path, State}, http::StatusCode, Json};
use farga_core::types::{Artifact, Node, NodeKind};
use crate::{db::insert_node, state::AppState};

pub async fn post_artifact(
    State(s): State<AppState>,
    Json(artifact): Json<Artifact>,
) -> StatusCode {
    let mut node = Node::new(
        NodeKind::Artifact,
        Some(artifact.project.clone()),
        Some(artifact.content.clone()),
    );
    node.title = Some(artifact.title.clone());
    if insert_node(&s.pool, &node).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::CREATED
}

pub async fn get_artifacts(
    State(s): State<AppState>,
    Path(project): Path<String>,
) -> Json<Vec<Artifact>> {
    let rows: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT title, content, project FROM nodes WHERE kind = 'Artifact' AND project = ? AND stale = 0"
    )
    .bind(&project)
    .fetch_all(&s.pool)
    .await
    .unwrap_or_default();

    Json(rows.into_iter().map(|(title, content, project)| Artifact {
        project: project.unwrap_or_default(),
        title: title.unwrap_or_default(),
        content: content.unwrap_or_default(),
        session_id: None,
        kind: "artifact".into(),
    }).collect())
}
