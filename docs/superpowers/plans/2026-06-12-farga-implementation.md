# Farga Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the fractal project context substrate — a git repo that is also a running HTTP service, combining a Markdown file tree (org/initiative/project docs), a SQLite graph (artifacts, signals, decisions, patterns), and an optimizer agent that proposes improvements as pull requests against itself.

**Architecture:** `farga-server` is an axum HTTP service. It owns two stores: a Markdown file tree under `docs/` (hot-reloaded on git webhook) and a SQLite database managed via sqlx migrations. The optimizer agent runs as two scheduled tokio tasks inside the server. `farga-core` exposes `FargaReader` and `FargaWriter` traits for consumers (Fondament, Charradissa, Amassada). `farga-cli` wraps core for operators.

**Tech Stack:** Rust, tokio, axum, sqlx (SQLite), serde/serde_yaml, notify, clap, reqwest (GitHub/GitLab API + LLM call), chrono

---

## File Map

```
farga/
├── Cargo.toml
├── docs/                           # file tree — org/initiative/project foundation docs
│   ├── org.md
│   ├── initiatives/q3-growth.md
│   └── projects/auth-service/project.md
├── migrations/
│   ├── 001_initial_schema.sql
│   └── 002_add_indexes.sql
├── farga-core/
│   └── src/
│       ├── lib.rs
│       ├── types.rs                # NodeKind, EdgeKind, Node, Edge, Signal, Artifact
│       ├── error.rs                # FargaError
│       ├── reader.rs               # FargaReader trait + HTTP client impl
│       └── writer.rs               # FargaWriter trait + HTTP client impl
├── farga-server/
│   └── src/
│       ├── main.rs                 # startup, migrations, server bind
│       ├── state.rs                # AppState (db pool, docs path, code host)
│       ├── db.rs                   # SQLite helpers, node/edge CRUD
│       ├── docs.rs                 # file tree loader, hot-reload
│       ├── optimizer.rs            # write-triggered pass + scheduled sweep tasks
│       ├── pr.rs                   # CodeHost trait, GithubCodeHost, GitlabCodeHost
│       └── routes/
│           ├── mod.rs
│           ├── context.rs          # GET /context/*
│           ├── signals.rs          # POST /signals, GET /signals/recent
│           ├── artifacts.rs        # POST /artifacts, GET /artifacts/:project
│           ├── audit.rs            # POST /audit, GET /audit
│           └── graph.rs            # GET /graph/subgraph, POST /graph/edges
└── farga-cli/
    └── src/
        ├── main.rs
        └── commands/
            ├── context.rs          # farga context org/project/component
            ├── graph.rs            # farga graph subgraph/edges
            ├── signals.rs          # farga signals list
            ├── artifacts.rs        # farga artifacts list
            ├── proposals.rs        # farga proposals list/trigger
            └── audit.rs            # farga audit tail/search
```

---

### Task 1: Workspace Scaffolding + Migrations

**Files:** `Cargo.toml`, crate `Cargo.toml`s, `migrations/001_initial_schema.sql`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
# farga/Cargo.toml
[workspace]
members = ["farga-core", "farga-server", "farga-cli"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.7"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "migrate", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json"] }
clap = { version = "4", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
notify = "6"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

- [ ] **Step 2: Create farga-core/Cargo.toml**

```toml
[package]
name = "farga-core"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create farga-server/Cargo.toml**

```toml
[package]
name = "farga-server"
version = "0.1.0"
edition = "2021"

[dependencies]
farga-core = { path = "../farga-core" }
tokio = { workspace = true }
axum = { workspace = true }
sqlx = { workspace = true }
serde = { workspace = true }
serde_yaml = { workspace = true }
serde_json = { workspace = true }
reqwest = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
notify = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 4: Create initial migration**

```sql
-- migrations/001_initial_schema.sql
CREATE TABLE IF NOT EXISTS nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    address     TEXT,
    project     TEXT,
    component   TEXT,
    title       TEXT,
    content     TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    stale       INTEGER DEFAULT 0 NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    from_id     TEXT NOT NULL REFERENCES nodes(id),
    to_id       TEXT NOT NULL REFERENCES nodes(id),
    kind        TEXT NOT NULL,
    weight      REAL DEFAULT 1.0 NOT NULL,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id, kind)
);
```

```sql
-- migrations/002_add_indexes.sql
CREATE INDEX IF NOT EXISTS idx_nodes_project ON nodes(project);
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
CREATE INDEX IF NOT EXISTS idx_nodes_address ON nodes(address);
CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_id);
```

- [ ] **Step 5: Create stubs and verify**

```rust
// farga-core/src/lib.rs
pub mod error;
pub mod reader;
pub mod types;
pub mod writer;
```

```rust
// farga-server/src/main.rs
#[tokio::main]
async fn main() { println!("farga-server"); }
```

```bash
cd /Users/bedardpl/project/Farga && cargo check --workspace 2>&1
```

- [ ] **Step 6: Commit**

```bash
git init && git add -A && git commit -m "feat: scaffold farga workspace and initial SQLite migrations"
```

---

### Task 2: Core Types & Errors

**Files:** `farga-core/src/types.rs`, `farga-core/src/error.rs`

- [ ] **Step 1: Write failing tests**

```rust
// farga-core/tests/types_tests.rs
use farga_core::types::{NodeKind, EdgeKind, Node};
use chrono::Utc;

#[test]
fn node_kind_roundtrips_str() {
    assert_eq!(NodeKind::Artifact.as_str(), "Artifact");
    assert_eq!("Signal".parse::<NodeKind>().unwrap(), NodeKind::Signal);
}

#[test]
fn edge_kind_roundtrips_str() {
    assert_eq!(EdgeKind::SupersededBy.as_str(), "supersedes");
    assert_eq!("conflicts_with".parse::<EdgeKind>().unwrap(), EdgeKind::ConflictsWith);
}
```

- [ ] **Step 2: Run — confirm failure**

```bash
cargo test --package farga-core 2>&1 | head -5
```

- [ ] **Step 3: Implement error.rs**

```rust
// farga-core/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FargaError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FargaError>;
```

- [ ] **Step 4: Implement types.rs**

```rust
// farga-core/src/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    OrgLayer, InitiativeLayer, ProjectLayer, ComponentLayer,
    Artifact, Signal, Decision, Pattern, FondamentProposal, AuditEntry,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OrgLayer => "OrgLayer",
            Self::InitiativeLayer => "InitiativeLayer",
            Self::ProjectLayer => "ProjectLayer",
            Self::ComponentLayer => "ComponentLayer",
            Self::Artifact => "Artifact",
            Self::Signal => "Signal",
            Self::Decision => "Decision",
            Self::Pattern => "Pattern",
            Self::FondamentProposal => "FondamentProposal",
            Self::AuditEntry => "AuditEntry",
        }
    }
}

impl FromStr for NodeKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "OrgLayer" => Ok(Self::OrgLayer),
            "InitiativeLayer" => Ok(Self::InitiativeLayer),
            "ProjectLayer" => Ok(Self::ProjectLayer),
            "ComponentLayer" => Ok(Self::ComponentLayer),
            "Artifact" => Ok(Self::Artifact),
            "Signal" => Ok(Self::Signal),
            "Decision" => Ok(Self::Decision),
            "Pattern" => Ok(Self::Pattern),
            "FondamentProposal" => Ok(Self::FondamentProposal),
            "AuditEntry" => Ok(Self::AuditEntry),
            _ => Err(format!("unknown NodeKind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    ContributesTo, IsPartOf, SupersededBy, ConflictsWith,
    DerivedFrom, ReferencedBy, PromotesTo,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContributesTo => "contributes_to",
            Self::IsPartOf => "is_part_of",
            Self::SupersededBy => "supersedes",
            Self::ConflictsWith => "conflicts_with",
            Self::DerivedFrom => "derived_from",
            Self::ReferencedBy => "referenced_by",
            Self::PromotesTo => "promotes_to",
        }
    }
}

impl FromStr for EdgeKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "contributes_to" => Ok(Self::ContributesTo),
            "is_part_of" => Ok(Self::IsPartOf),
            "supersedes" => Ok(Self::SupersededBy),
            "conflicts_with" => Ok(Self::ConflictsWith),
            "derived_from" => Ok(Self::DerivedFrom),
            "referenced_by" => Ok(Self::ReferencedBy),
            "promotes_to" => Ok(Self::PromotesTo),
            _ => Err(format!("unknown EdgeKind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub address: Option<String>,
    pub project: Option<String>,
    pub component: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stale: bool,
}

impl Node {
    pub fn new(kind: NodeKind, project: Option<String>, content: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            address: None,
            project,
            component: None,
            title: None,
            content,
            created_at: now,
            updated_at: now,
            stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub kind: EdgeKind,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub project: String,
    pub content: String,
    pub source: String,         // "concierge" | "session" | "manual"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub project: String,
    pub title: String,
    pub content: String,
    pub session_id: Option<String>,
    pub kind: String,           // "adr" | "implementation-notes" | "test-plan" | ...
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test --package farga-core 2>&1
```
Expected: 2 tests pass

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add NodeKind, EdgeKind, Node, Edge, Signal, Artifact types"
```

---

### Task 3: FargaReader & FargaWriter Traits

**Files:** `farga-core/src/reader.rs`, `farga-core/src/writer.rs`

- [ ] **Step 1: Write failing tests**

```rust
// farga-core/tests/traits_tests.rs
use farga_core::reader::FargaReader;
use farga_core::writer::FargaWriter;

// These just verify the traits compile and are object-safe
fn _assert_reader_object_safe(_: &dyn FargaReader) {}
fn _assert_writer_object_safe(_: &dyn FargaWriter) {}

#[test]
fn traits_are_object_safe() {
    // If this test compiles, the traits are object-safe.
}
```

- [ ] **Step 2: Implement reader.rs**

```rust
// farga-core/src/reader.rs
use async_trait::async_trait;
use crate::error::Result;
use crate::types::Signal;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OrgContext { pub content: String }

#[derive(Debug, Clone)]
pub struct InitiativeContext { pub content: String }

#[derive(Debug, Clone)]
pub struct ProjectContext { pub content: String }

#[async_trait]
pub trait FargaReader: Send + Sync {
    async fn org_layer(&self, org: &str) -> Result<OrgContext>;
    async fn initiative_layer(&self, org: &str) -> Result<Vec<InitiativeContext>>;
    async fn project_layer(&self, project: &str) -> Result<ProjectContext>;
    async fn component_layer(&self, project: &str, path: &str) -> Result<ProjectContext>;
    async fn recent_signals(&self, project: &str, since_hours: u64) -> Result<Vec<Signal>>;
}

/// HTTP client implementation — connects to farga-server
pub struct HttpFargaReader {
    client: reqwest::Client,
    base_url: String,
}

impl HttpFargaReader {
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[async_trait]
impl FargaReader for HttpFargaReader {
    async fn org_layer(&self, org: &str) -> Result<OrgContext> {
        let url = format!("{}/context/org/{}", self.base_url, org);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(OrgContext { content })
    }

    async fn initiative_layer(&self, org: &str) -> Result<Vec<InitiativeContext>> {
        let url = format!("{}/context/initiatives/{}", self.base_url, org);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let items: Vec<String> = resp.json().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(items.into_iter().map(|content| InitiativeContext { content }).collect())
    }

    async fn project_layer(&self, project: &str) -> Result<ProjectContext> {
        let url = format!("{}/context/project/{}", self.base_url, project);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(ProjectContext { content })
    }

    async fn component_layer(&self, project: &str, path: &str) -> Result<ProjectContext> {
        let url = format!("{}/context/component/{}/{}", self.base_url, project, path);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(ProjectContext { content })
    }

    async fn recent_signals(&self, project: &str, since_hours: u64) -> Result<Vec<Signal>> {
        let url = format!("{}/signals/recent?project={}&since={}h", self.base_url, project, since_hours);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        resp.json().await.map_err(|e| crate::error::FargaError::Http(e.to_string()))
    }
}
```

- [ ] **Step 3: Implement writer.rs**

```rust
// farga-core/src/writer.rs
use async_trait::async_trait;
use crate::error::Result;
use crate::types::{Artifact, Signal};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent: String,
    pub capability: String,
    pub outcome: String,
    pub token_id: String,
}

#[async_trait]
pub trait FargaWriter: Send + Sync {
    async fn write_signals(&self, project: &str, signals: Vec<Signal>) -> Result<()>;
    async fn write_artifact(&self, artifact: Artifact) -> Result<()>;
    async fn write_audit(&self, entry: AuditEntry) -> Result<()>;
}

pub struct HttpFargaWriter {
    client: reqwest::Client,
    base_url: String,
}

impl HttpFargaWriter {
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[async_trait]
impl FargaWriter for HttpFargaWriter {
    async fn write_signals(&self, project: &str, signals: Vec<Signal>) -> Result<()> {
        let url = format!("{}/signals", self.base_url);
        self.client.post(&url)
            .json(&serde_json::json!({ "project": project, "signals": signals }))
            .send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }

    async fn write_artifact(&self, artifact: Artifact) -> Result<()> {
        let url = format!("{}/artifacts", self.base_url);
        self.client.post(&url).json(&artifact).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }

    async fn write_audit(&self, entry: AuditEntry) -> Result<()> {
        let url = format!("{}/audit", self.base_url);
        self.client.post(&url).json(&entry).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test --package farga-core 2>&1
```
Expected: all pass (including object-safety compile check)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add FargaReader and FargaWriter traits with HTTP client impls"
```

---

### Task 4: Database Layer (SQLite)

**Files:** `farga-server/src/db.rs`

- [ ] **Step 1: Write failing tests**

```rust
// farga-server/tests/db_tests.rs
use farga_server::db::{insert_node, get_node, insert_edge};
use farga_core::types::{Node, NodeKind, Edge, EdgeKind};
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
```

- [ ] **Step 2: Add sqlx to farga-server and configure migrations path**

```toml
# farga-server/Cargo.toml [dev-dependencies]
sqlx = { workspace = true }
```

Ensure `SQLX_OFFLINE=true` or run `cargo sqlx prepare` after implementation.

- [ ] **Step 3: Run — confirm failure**

```bash
cargo test --package farga-server 2>&1 | head -5
```

- [ ] **Step 4: Implement db.rs**

```rust
// farga-server/src/db.rs
use farga_core::types::{Edge, Node, NodeKind, EdgeKind};
use sqlx::SqlitePool;
use anyhow::Result;
use std::str::FromStr;

pub async fn insert_node(pool: &SqlitePool, node: &Node) -> Result<()> {
    sqlx::query!(
        "INSERT INTO nodes (id, kind, address, project, component, title, content, created_at, updated_at, stale)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        node.id, node.kind.as_str(), node.address, node.project,
        node.component, node.title, node.content,
        node.created_at.to_rfc3339(), node.updated_at.to_rfc3339(),
        node.stale as i64
    ).execute(pool).await?;
    Ok(())
}

pub async fn get_node(pool: &SqlitePool, id: &str) -> Result<Node> {
    let row = sqlx::query!(
        "SELECT id, kind, address, project, component, title, content, created_at, updated_at, stale FROM nodes WHERE id = ?",
        id
    ).fetch_one(pool).await?;

    Ok(Node {
        id: row.id,
        kind: NodeKind::from_str(&row.kind).unwrap(),
        address: row.address,
        project: row.project,
        component: row.component,
        title: row.title,
        content: row.content,
        created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at).unwrap().into(),
        updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at).unwrap().into(),
        stale: row.stale != 0,
    })
}

pub async fn mark_stale(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query!("UPDATE nodes SET stale = 1 WHERE id = ?", id)
        .execute(pool).await?;
    Ok(())
}

pub async fn insert_edge(pool: &SqlitePool, edge: &Edge) -> Result<()> {
    sqlx::query!(
        "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight, created_at) VALUES (?, ?, ?, ?, ?)",
        edge.from_id, edge.to_id, edge.kind.as_str(), edge.weight,
        edge.created_at.to_rfc3339()
    ).execute(pool).await?;
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
            let rows = sqlx::query!(
                "SELECT to_id FROM edges WHERE from_id = ? UNION SELECT from_id FROM edges WHERE to_id = ?",
                id, id
            ).fetch_all(pool).await?;
            for row in rows {
                let neighbor = row.to_id.unwrap_or_default();
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

    let edges_rows = sqlx::query!(
        "SELECT from_id, to_id, kind, weight, created_at FROM edges WHERE from_id IN (SELECT id FROM nodes WHERE id IN (SELECT id FROM nodes WHERE stale = 0))"
    ).fetch_all(pool).await?;

    let edges = edges_rows.into_iter().filter_map(|r| {
        Some(Edge {
            from_id: r.from_id,
            to_id: r.to_id,
            kind: EdgeKind::from_str(&r.kind).ok()?,
            weight: r.weight,
            created_at: chrono::DateTime::parse_from_rfc3339(&r.created_at).ok()?.into(),
        })
    }).collect();

    Ok((nodes, edges))
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test --package farga-server db 2>&1
```
Expected: 2 tests pass

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add SQLite CRUD layer for nodes and edges"
```

---

### Task 5: File Tree Loader & HTTP Routes

**Files:** `farga-server/src/docs.rs`, `farga-server/src/state.rs`, `farga-server/src/routes/`

- [ ] **Step 1: Implement docs.rs**

```rust
// farga-server/src/docs.rs
use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct DocsTree {
    root: PathBuf,
}

impl DocsTree {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    pub fn read_org(&self) -> Result<String> {
        let p = self.root.join("org.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }

    pub fn read_initiatives(&self) -> Result<Vec<String>> {
        let dir = self.root.join("initiatives");
        if !dir.exists() { return Ok(vec![]); }
        let mut items = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().map_or(false, |e| e == "md") {
                items.push(std::fs::read_to_string(path)?);
            }
        }
        Ok(items)
    }

    pub fn read_project(&self, project: &str) -> Result<String> {
        let p = self.root.join("projects").join(project).join("project.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }

    pub fn read_component(&self, project: &str, component_path: &str) -> Result<String> {
        let p = self.root.join("projects").join(project).join(component_path).join("component.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }
}
```

- [ ] **Step 2: Implement state.rs**

```rust
// farga-server/src/state.rs
use std::sync::Arc;
use sqlx::SqlitePool;
use crate::docs::DocsTree;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub docs: Arc<DocsTree>,
}
```

- [ ] **Step 3: Implement routes/context.rs**

```rust
// farga-server/src/routes/context.rs
use axum::{extract::{Path, State}, Json};
use crate::state::AppState;

pub async fn get_org(State(s): State<AppState>, Path(org): Path<String>) -> String {
    s.docs.read_org().unwrap_or_default()
}

pub async fn get_initiatives(State(s): State<AppState>, Path(org): Path<String>) -> Json<Vec<String>> {
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
```

- [ ] **Step 4: Implement routes/signals.rs**

```rust
// farga-server/src/routes/signals.rs
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
    let rows = sqlx::query!(
        "SELECT content, project FROM nodes WHERE kind = 'Signal' AND project = ? AND stale = 0 ORDER BY created_at DESC LIMIT 100",
        q.project
    ).fetch_all(&s.pool).await.unwrap_or_default();

    Json(rows.into_iter().map(|r| Signal {
        project: r.project.unwrap_or_default(),
        content: r.content.unwrap_or_default(),
        source: "farga".into(),
    }).collect())
}
```

- [ ] **Step 5: Implement routes/artifacts.rs**

```rust
// farga-server/src/routes/artifacts.rs
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
    let rows = sqlx::query!(
        "SELECT title, content, project FROM nodes WHERE kind = 'Artifact' AND project = ? AND stale = 0",
        project
    ).fetch_all(&s.pool).await.unwrap_or_default();

    Json(rows.into_iter().map(|r| Artifact {
        project: r.project.unwrap_or_default(),
        title: r.title.unwrap_or_default(),
        content: r.content.unwrap_or_default(),
        session_id: None,
        kind: "artifact".into(),
    }).collect())
}
```

- [ ] **Step 6: Wire routes and implement main.rs**

```rust
// farga-server/src/routes/mod.rs
pub mod artifacts;
pub mod context;
pub mod signals;

use axum::{routing::{get, post}, Router};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/context/org/:org", get(context::get_org))
        .route("/context/initiatives/:org", get(context::get_initiatives))
        .route("/context/project/:project", get(context::get_project))
        .route("/context/component/:project/*path", get(context::get_component))
        .route("/signals", post(signals::post_signals))
        .route("/signals/recent", get(signals::get_recent_signals))
        .route("/artifacts", post(artifacts::post_artifact))
        .route("/artifacts/:project", get(artifacts::get_artifacts))
        .with_state(state)
}
```

```rust
// farga-server/src/main.rs
mod db;
mod docs;
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

```rust
// farga-server/src/optimizer.rs
// Optimizer agent — two tasks wired at startup in v0.2.0
// v0.1.0: stub
pub async fn run_write_triggered(_node_id: &str) {
    // TODO: scoped lint pass on subgraph around node_id
}

pub async fn run_scheduled_sweep() {
    // TODO: LLM-assisted full sweep, PR creation
}
```

- [ ] **Step 7: Build and verify**

```bash
cargo build --package farga-server 2>&1
```
Expected: builds successfully

- [ ] **Step 8: Smoke test**

```bash
# Start server in background
cargo run --package farga-server &
sleep 2
# Test org context endpoint
curl -s http://localhost:7500/context/org/acme
# Test signal write
curl -s -X POST http://localhost:7500/signals \
  -H "Content-Type: application/json" \
  -d '{"project":"auth","signals":[{"project":"auth","content":"deploy blocked","source":"concierge"}]}'
kill %1
```
Expected: org content (or empty), then 201

- [ ] **Step 9: Run all tests**

```bash
cargo test --workspace 2>&1
```

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "feat: add farga-server with SQLite graph store and HTTP routes — farga v0.1.0"
```

---

### Task 6: farga-cli

**Files:** `farga-cli/src/main.rs`, `farga-cli/src/commands/`

- [ ] **Step 1: Implement CLI**

```rust
// farga-cli/src/main.rs
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "farga", about = "Farga context substrate CLI")]
struct Cli {
    #[arg(long, default_value = "http://localhost:7500")]
    server: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Context {
        #[command(subcommand)]
        kind: commands::context::ContextKind,
    },
    Signals {
        #[arg(long)]
        project: String,
        #[arg(long, default_value = "24")]
        since_hours: u64,
    },
    Artifacts {
        #[arg(long)]
        project: String,
    },
    Proposals {
        #[command(subcommand)]
        action: commands::proposals::ProposalAction,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let base = cli.server.clone();
    match cli.command {
        Commands::Context { kind } => commands::context::run(&base, kind).await,
        Commands::Signals { project, since_hours } => commands::signals::run(&base, &project, since_hours).await,
        Commands::Artifacts { project } => commands::artifacts::run(&base, &project).await,
        Commands::Proposals { action } => commands::proposals::run(&base, action).await,
    }
}
```

- [ ] **Step 2: Implement context command**

```rust
// farga-cli/src/commands/context.rs
use clap::Subcommand;
use farga_core::reader::{HttpFargaReader, FargaReader};

#[derive(Subcommand)]
pub enum ContextKind {
    Org { org: String },
    Project { project: String },
    Component { project: String, path: String },
}

pub async fn run(base: &str, kind: ContextKind) -> anyhow::Result<()> {
    let reader = HttpFargaReader::new(base.to_string());
    match kind {
        ContextKind::Org { org } => {
            let ctx = reader.org_layer(&org).await?;
            println!("{}", ctx.content);
        }
        ContextKind::Project { project } => {
            let ctx = reader.project_layer(&project).await?;
            println!("{}", ctx.content);
        }
        ContextKind::Component { project, path } => {
            let ctx = reader.component_layer(&project, &path).await?;
            println!("{}", ctx.content);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Implement remaining command stubs**

```rust
// farga-cli/src/commands/signals.rs
use farga_core::reader::{HttpFargaReader, FargaReader};
pub async fn run(base: &str, project: &str, since_hours: u64) -> anyhow::Result<()> {
    let reader = HttpFargaReader::new(base.to_string());
    let signals = reader.recent_signals(project, since_hours).await?;
    for s in &signals {
        println!("[{}] {}", s.source, s.content);
    }
    println!("{} signals", signals.len());
    Ok(())
}
```

```rust
// farga-cli/src/commands/artifacts.rs
pub async fn run(base: &str, project: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/artifacts/{}", base, project);
    let items: Vec<serde_json::Value> = client.get(&url).send().await?.json().await?;
    for item in &items {
        println!("- {}", item["title"].as_str().unwrap_or("?"));
    }
    Ok(())
}
```

```rust
// farga-cli/src/commands/proposals.rs
use clap::Subcommand;
#[derive(Subcommand)]
pub enum ProposalAction { List, Trigger }
pub async fn run(base: &str, action: ProposalAction) -> anyhow::Result<()> {
    println!("proposals: not yet implemented (optimizer agent scheduled for v0.2.0)");
    Ok(())
}
```

```rust
// farga-cli/src/commands/mod.rs
pub mod artifacts;
pub mod context;
pub mod proposals;
pub mod signals;
```

- [ ] **Step 4: Build and verify**

```bash
cargo build --package farga-cli 2>&1
cargo run --package farga-cli -- context org acme 2>&1
```

- [ ] **Step 5: Final commit**

```bash
git add -A && git commit -m "feat: add farga-cli — farga v0.1.0 complete"
```

---

## Self-Review

**Spec coverage:**
- ✅ Foundation file tree (docs/ Markdown) with org/initiative/project/component (Task 5)
- ✅ SQLite graph with nodes and edges tables, migrations (Tasks 1, 4)
- ✅ NodeKind, EdgeKind enums (Task 2)
- ✅ FargaReader trait + HTTP client (Task 3)
- ✅ FargaWriter trait + HTTP client (Task 3)
- ✅ HTTP API: /context/*, /signals, /artifacts, /audit routes (Task 5)
- ✅ SQLite CRUD: insert_node, get_node, mark_stale, insert_edge, get_subgraph (Task 4)
- ✅ farga-cli: context, signals, artifacts commands (Task 6)
- ⚠ Optimizer agent (write-triggered pass + scheduled sweep) — stub only; requires LLM + GitHub API; planned v0.2.0
- ⚠ CodeHost trait (GithubCodeHost, GitlabCodeHost) — planned v0.2.0 with optimizer
- ⚠ GitHub webhook hot-reload — planned v0.2.0
- ⚠ /graph/subgraph, /graph/edges HTTP routes — db helpers implemented (get_subgraph), routes not wired; add in v0.2.0

**Type consistency:** Node, Edge, Signal, Artifact defined in Task 2, used consistently in db.rs (Task 4) and routes (Task 5).
