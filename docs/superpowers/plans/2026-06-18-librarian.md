# Farga Librarian Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a background librarian agent that fills in reversibility/impact/routing for pending governance contributions via LLM assessment.

**Architecture:** `tokio::spawn` background loop in farga-server; polls governance_assessments for pending rows; calls Claude; updates DB. New GET endpoint exposes assessment state.

**Tech Stack:** Rust, axum, sqlx (SQLite), reqwest, tokio, serde_json. Farga workspace at `/Users/bedardpl/project/Farga`.

---

## Task 1: Add librarian background task

**File to create:** `farga-server/src/librarian.rs`

**Commit message:** `feat: Farga librarian — background LLM assessment of pending governance contributions`

### Steps

- [ ] Create `farga-server/src/librarian.rs` with the full content below.

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Local verdict type — carries the raw strings stored in the DB.
/// Distinct from farga_core::types::LibrarianAssessment which uses enums.
#[derive(Debug, Deserialize, Serialize)]
pub struct LibrarianVerdict {
    pub reversibility: String, // "FullyReversible"|"EffectsLinger"|"CostlyReversible"|"Irreversible"
    pub impact: String,        // "Contained"|"CrossProject"|"DomainWide"|"OrgWide"
    pub routing: String,       // "ProjectLevel"|"InitiativeLevel"|"OrgLevel"
    pub notes: String,
}

/// Minimal shape of GovernanceContribution stored in nodes.content.
/// Only the fields the librarian needs for its prompt.
#[derive(Debug, Deserialize)]
struct ContribSnapshot {
    pub title: String,
    pub narrative: String,
    pub lessons: Vec<String>,
    pub involved_projects: Vec<String>,
}

/// Entry point — spawn this with `tokio::spawn(run_librarian(pool))`.
pub async fn run_librarian(pool: SqlitePool) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        if let Err(e) = assess_pending(&pool).await {
            tracing::error!("librarian: assessment cycle failed: {}", e);
        }
    }
}

async fn assess_pending(pool: &SqlitePool) -> Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT ga.node_id, n.content
         FROM governance_assessments ga
         JOIN nodes n ON ga.node_id = n.id
         WHERE ga.status = 'pending'
         LIMIT 10",
    )
    .fetch_all(pool)
    .await?;

    for (node_id, content) in rows {
        match assess_contribution(&content).await {
            Ok(verdict) => {
                if let Err(e) = update_assessment(pool, &node_id, &verdict).await {
                    tracing::error!("librarian: failed to update assessment for {}: {}", node_id, e);
                } else {
                    tracing::info!("librarian: assessed node {} -> routing={}", node_id, verdict.routing);
                }
            }
            Err(e) => {
                tracing::error!("librarian: LLM call failed for node {}: {}", node_id, e);
            }
        }
    }

    Ok(())
}

async fn assess_contribution(content: &str) -> Result<LibrarianVerdict> {
    let snap: ContribSnapshot = serde_json::from_str(content)?;

    let lessons = snap.lessons.join("; ");
    let projects = snap.involved_projects.join(", ");

    let prompt = format!(
        r#"You are the Farga librarian for the Occitan multi-agent system. You assess governance contributions to determine their reversibility, impact scope, and routing level.

Contribution title: {title}
Narrative: {narrative}
Lessons learned: {lessons}
Involved projects: {projects}

Respond ONLY with a JSON object, no markdown:
{{
  "reversibility": "FullyReversible"|"EffectsLinger"|"CostlyReversible"|"Irreversible",
  "impact": "Contained"|"CrossProject"|"DomainWide"|"OrgWide",
  "routing": "ProjectLevel"|"InitiativeLevel"|"OrgLevel",
  "notes": "one sentence explaining your assessment"
}}

Definitions:
- FullyReversible: decision can be undone with no lasting effects
- EffectsLinger: reversible but some effects persist (e.g. learned behaviors, written docs)
- CostlyReversible: reversible but requires significant rework
- Irreversible: cannot be undone (e.g. data deletion, published decisions)
- Contained: affects only one project
- CrossProject: affects 2-3 specific projects
- DomainWide: affects an entire domain/team
- OrgWide: affects the entire organization"#,
        title = snap.title,
        narrative = snap.narrative,
        lessons = lessons,
        projects = projects,
    );

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 256,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let resp_json: serde_json::Value = resp.json().await?;

    let text = resp_json["content"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("unexpected Anthropic response shape: {:?}", resp_json))?;

    let verdict: LibrarianVerdict = serde_json::from_str(text.trim())?;
    Ok(verdict)
}

async fn update_assessment(
    pool: &SqlitePool,
    node_id: &str,
    verdict: &LibrarianVerdict,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE governance_assessments
         SET status = 'assessed',
             reversibility = ?,
             impact = ?,
             routing = ?,
             notes = ?,
             updated_at = ?
         WHERE node_id = ?",
    )
    .bind(&verdict.reversibility)
    .bind(&verdict.impact)
    .bind(&verdict.routing)
    .bind(&verdict.notes)
    .bind(&now)
    .bind(node_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn librarian_verdict_deserializes_correctly() {
        let json = r#"{
            "reversibility": "EffectsLinger",
            "impact": "CrossProject",
            "routing": "InitiativeLevel",
            "notes": "This decision affects multiple projects and its documentation effects will persist."
        }"#;
        let verdict: LibrarianVerdict = serde_json::from_str(json).unwrap();
        assert_eq!(verdict.reversibility, "EffectsLinger");
        assert_eq!(verdict.impact, "CrossProject");
        assert_eq!(verdict.routing, "InitiativeLevel");
        assert!(!verdict.notes.is_empty());
    }

    #[sqlx::test]
    async fn update_assessment_sql_is_correct(pool: SqlitePool) {
        // Create schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                address TEXT,
                project TEXT,
                component TEXT,
                title TEXT,
                content TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                stale INTEGER DEFAULT 0 NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS governance_assessments (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL REFERENCES nodes(id),
                status TEXT NOT NULL DEFAULT 'pending',
                reversibility TEXT,
                impact TEXT,
                routing TEXT,
                notes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert seed data
        let node_id = "test-node-001";
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO nodes (id, kind, created_at, updated_at, stale) VALUES (?, 'GovernanceContribution', ?, ?, 0)",
        )
        .bind(node_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO governance_assessments (id, node_id, status, created_at, updated_at)
             VALUES ('assess-001', ?, 'pending', ?, ?)",
        )
        .bind(node_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Run the function under test
        let verdict = LibrarianVerdict {
            reversibility: "CostlyReversible".into(),
            impact: "DomainWide".into(),
            routing: "OrgLevel".into(),
            notes: "Test note.".into(),
        };
        update_assessment(&pool, node_id, &verdict).await.unwrap();

        // Verify the row was updated
        let row: (String, String, String, String, String) = sqlx::query_as(
            "SELECT status, reversibility, impact, routing, notes FROM governance_assessments WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, "assessed");
        assert_eq!(row.1, "CostlyReversible");
        assert_eq!(row.2, "DomainWide");
        assert_eq!(row.3, "OrgLevel");
        assert_eq!(row.4, "Test note.");
    }
}
```

---

## Task 2: New GET route for assessment state

**Files to edit:**
- `farga-server/src/db.rs` — add `get_assessment_by_node`
- `farga-server/src/routes/governance.rs` — add `get_assessment` handler
- `farga-server/src/routes/mod.rs` — register the route

**Commit message:** `feat: GET /governance/assessments/:node_id — query assessment state`

### Steps

- [ ] Add `get_assessment_by_node` to `farga-server/src/db.rs`.

  Append this function at the bottom of the file, after `count_precedent_rejections`:

  ```rust
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
  ```

- [ ] Add `get_assessment` handler to `farga-server/src/routes/governance.rs`.

  Add to the `use crate::db` import line: `, get_assessment_by_node`.
  
  Then append this handler:

  ```rust
  pub async fn get_assessment(
      State(s): State<AppState>,
      axum::extract::Path(node_id): axum::extract::Path<String>,
  ) -> (StatusCode, Json<serde_json::Value>) {
      match get_assessment_by_node(&s.pool, &node_id).await {
          Ok(Some(row)) => (StatusCode::OK, Json(row)),
          Ok(None) => (
              StatusCode::NOT_FOUND,
              Json(serde_json::json!({ "error": "assessment not found" })),
          ),
          Err(e) => {
              tracing::error!("get_assessment failed for node {}: {}", node_id, e);
              (
                  StatusCode::INTERNAL_SERVER_ERROR,
                  Json(serde_json::json!({ "error": e.to_string() })),
              )
          }
      }
  }
  ```

- [ ] Register the route in `farga-server/src/routes/mod.rs`.

  Add the following line after `.route("/governance/decisions", post(governance::post_governance_decision))`:

  ```rust
  .route("/governance/assessments/:node_id", get(governance::get_assessment))
  ```

- [ ] Add a test for 404 behavior to `farga-server/src/routes/governance.rs` (or a separate `tests/governance_routes.rs`).

  The test lives in `governance.rs` under `#[cfg(test)]`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use axum::{
          body::Body,
          http::{Request, StatusCode},
          Router,
      };
      use sqlx::SqlitePool;
      use std::sync::Arc;
      use tower::ServiceExt;
      use crate::docs::DocsTree;
      use std::path::PathBuf;

      async fn test_router(pool: SqlitePool) -> Router {
          let state = AppState {
              pool,
              docs: Arc::new(DocsTree::new(PathBuf::from("."))),
          };
          Router::new()
              .route(
                  "/governance/assessments/:node_id",
                  axum::routing::get(get_assessment),
              )
              .with_state(state)
      }

      #[sqlx::test]
      async fn get_assessment_returns_404_for_missing_node(pool: SqlitePool) {
          // governance_assessments table must exist — create it inline for the in-memory pool
          sqlx::query(
              "CREATE TABLE IF NOT EXISTS governance_assessments (
                  id TEXT PRIMARY KEY,
                  node_id TEXT NOT NULL,
                  status TEXT NOT NULL DEFAULT 'pending',
                  reversibility TEXT,
                  impact TEXT,
                  routing TEXT,
                  notes TEXT,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
              )",
          )
          .execute(&pool)
          .await
          .unwrap();

          let app = test_router(pool).await;
          let req = Request::builder()
              .uri("/governance/assessments/nonexistent-node")
              .body(Body::empty())
              .unwrap();

          let resp = app.oneshot(req).await.unwrap();
          assert_eq!(resp.status(), StatusCode::NOT_FOUND);
      }
  }
  ```

---

## Task 3: Wire librarian into `main.rs` and verify dependencies

**Files to edit:**
- `farga-server/src/main.rs` — add `mod librarian` and `tokio::spawn`
- `farga-server/Cargo.toml` — confirm `reqwest` and `anyhow` present (they are; no change needed)

**Commit message:** `feat: wire librarian background task into Farga server startup`

### Steps

- [ ] Verify `Cargo.toml` has `reqwest` and `anyhow`. Both are already present via workspace.
  No file change required for this step.

- [ ] Edit `farga-server/src/main.rs` to declare the `librarian` module and spawn the background task.

  Add `mod librarian;` after the existing `mod` declarations (after line 5, `mod state;`):

  ```rust
  mod librarian;
  ```

  Add the spawn call after `sqlx::migrate!("./migrations").run(&pool).await?;` and before creating `state`:

  ```rust
  let librarian_pool = pool.clone();
  tokio::spawn(librarian::run_librarian(librarian_pool));
  ```

  The final `main.rs` should look like:

  ```rust
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
  ```

- [ ] Run `cargo build -p farga-server` from the workspace root (`/Users/bedardpl/project/Farga`) to confirm everything compiles.

- [ ] Run `cargo test -p farga-server` to confirm all three tests pass:
  - `librarian::tests::librarian_verdict_deserializes_correctly`
  - `librarian::tests::update_assessment_sql_is_correct`
  - `routes::governance::tests::get_assessment_returns_404_for_missing_node`

---

## Dependency notes

- `reqwest` — already in workspace (`Cargo.toml` at root), already in `farga-server/Cargo.toml`. No change.
- `anyhow` — already in workspace and `farga-server/Cargo.toml`. No change.
- `chrono` — already in workspace and `farga-server/Cargo.toml`. Used for `Utc::now()` in `update_assessment`.
- `ANTHROPIC_API_KEY` — must be set in the environment at runtime. The librarian logs an error and skips the assessment if absent; it does not crash the server.

## Runtime behaviour

- The librarian wakes every 60 seconds. It processes up to 10 pending rows per cycle (LIMIT 10) to bound API call bursts.
- If the Anthropic API is unavailable or returns a non-JSON response, the row stays `status='pending'` and will be retried on the next cycle.
- Assessed rows have `status='assessed'`. Rows that go through the human governance flow (`insert_governance_decision`) will have their status overwritten by that function (`'rejected'`, `'approved'`, etc.) — the librarian skips non-pending rows, so no conflict.
