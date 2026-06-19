mod db;
mod docs;
mod librarian;
mod optimizer;
mod routes;
mod state;

use std::{path::PathBuf, sync::Arc};
use sqlx::sqlite::SqlitePoolOptions;
use state::AppState;
use docs::DocsTree;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("FARGA_DB").unwrap_or("farga.db".into());
    let docs_path = std::env::var("FARGA_DOCS").unwrap_or("docs".into());
    let port = std::env::var("FARGA_PORT").unwrap_or("7500".into());

    let pool = SqlitePoolOptions::new()
        .connect(&format!("sqlite://{}?mode=rwc", db_path)).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let librarian_pool = pool.clone();
    tokio::spawn(librarian::run_librarian(librarian_pool));

    let state = AppState {
        pool,
        docs: Arc::new(DocsTree::new(PathBuf::from(docs_path))),
    };

    let app = routes::router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("farga-server listening on :{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
