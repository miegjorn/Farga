use axum::{extract::{Path, State}, Json};
use crate::state::AppState;

pub async fn get_org(State(s): State<AppState>, Path(_org): Path<String>) -> String {
    s.docs.read_org().unwrap_or_default()
}

pub async fn get_initiatives(State(s): State<AppState>, Path(_org): Path<String>) -> Json<Vec<String>> {
    Json(s.docs.read_initiatives().unwrap_or_default())
}

pub async fn get_project(State(s): State<AppState>, Path(project): Path<String>) -> String {
    s.docs.read_project(&project).unwrap_or_default()
}

pub async fn get_component(
    State(s): State<AppState>,
    Path((project, component)): Path<(String, String)>,
) -> String {
    s.docs.read_component(&project, &component).unwrap_or_default()
}
