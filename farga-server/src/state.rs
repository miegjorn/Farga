use std::sync::Arc;
use sqlx::SqlitePool;
use crate::docs::DocsTree;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub docs: Arc<DocsTree>,
}
