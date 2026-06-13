# Farga — Fractal Project Context Substrate

**Farga** (Occitan: *forge*) is the fractal project context substrate and collective-memory layer of the Occitan stack. Projects have nested components as first-class peers, each with their own context-map, agent, and lifecycle.

**Fondament defines the shape of context. Farga holds the content.**

Farga is simultaneously a git repository and a running service. The `docs/` file tree holds human-readable foundation documents (org, initiatives, projects). A SQLite graph database holds everything the system produces dynamically — artifacts, signals, decisions, patterns. Farga's own optimizer agent proposes improvements as pull requests against itself.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Concepts](#core-concepts)
3. [Crate Structure](#crate-structure)
4. [farga-core](#farga-core)
5. [farga-server](#farga-server)
6. [REST API](#rest-api)
7. [Database Schema](#database-schema)
8. [Optimizer Agent](#optimizer-agent)
9. [farga-cli](#farga-cli)
10. [Configuration](#configuration)
11. [Docs File Tree](#docs-file-tree)
12. [Consumers: Charradissa and Amassada](#consumers-charradissa-and-amassada)
13. [Testing](#testing)
14. [Dependencies](#dependencies)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Occitan Stack                         │
│                                                             │
│   Charradissa (signals)        Amassada (artifacts)         │
│         │                            │                      │
│         └──────────────┬─────────────┘                      │
│                        │                                    │
│               farga-server (:7500)                          │
│               ┌─────────────────┐                           │
│               │  Axum HTTP API  │                           │
│               │  SQLite graph   │                           │
│               │  docs/ reader   │                           │
│               │  optimizer      │                           │
│               └────────┬────────┘                           │
│                        │                                    │
│              farga-core (library)                           │
│         FargaReader / FargaWriter traits                    │
│                                                             │
│   farga-cli ──── queries farga-server via HTTP              │
└─────────────────────────────────────────────────────────────┘
```

Farga uses two storage layers:

| Layer | Format | Contents | Git-friendly |
|---|---|---|---|
| Foundation | Markdown file tree (`docs/`) | org, initiative, project documents | Yes — fully |
| Dynamic | SQLite (`farga.db`) | nodes, edges, signals, artifacts | Yes — schema only |

`farga.db` is gitignored and regenerated from incoming writes. Schema migrations in `migrations/` are committed and reviewed as PRs. The optimizer agent's proposed changes to `docs/` and `migrations/` are also opened as PRs.

---

## Core Concepts

### Fractal Hierarchy

Farga organises context in four layers, each nestable:

| Layer | File path | NodeKind |
|---|---|---|
| Org | `docs/org.md` | `OrgLayer` |
| Initiative | `docs/initiatives/<id>.md` | `InitiativeLayer` |
| Project | `docs/projects/<id>/project.md` | `ProjectLayer` |
| Component | `docs/projects/<id>/<path>/component.md` | `ComponentLayer` |

Components are fractal — a project can contain arbitrary-depth sub-components, each a first-class peer with its own context-map, agent registration, and lifecycle.

### Context Maps

A context map is the assembled set of nodes contributed to an address. At dispatch time the server performs a graph traversal:

```
resolve(address):
  find all nodes WHERE address matches AND stale = FALSE
  follow contributes_to edges, ordered by weight DESC
  → assembled context chunks in priority order
```

The file tree (org/initiative/project foundation) is mirrored as graph nodes on startup and on hot-reload. Traversal unifies both storage layers into a single resolved context document.

### Signals

A `Signal` is a lightweight observation written by `ConciergeAgent` during room archival sweeps. Signals carry a `project`, `content`, and `source` (`"concierge"`, `"session"`, or `"manual"`). They are stored as `Signal` nodes in the SQLite graph and surfaced to the optimizer's convergence sweep.

### Artifacts

An `Artifact` is a structured output from an Amassada session — an ADR, implementation-notes document, test plan, or similar. Artifacts carry a `project`, `title`, `content`, `session_id`, and `kind` string. They are stored as `Artifact` nodes and participate in graph traversal.

### Proposals (FondamentProposals)

A `FondamentProposal` is a candidate for promotion to a Fondament primitive, identified by the optimizer agent during a scheduled sweep. Proposals are materialised as `FondamentProposal` nodes connected via `promotes_to` edges to the target Fondament primitive path, and opened as pull requests.

### Node and Edge Kinds

**Node kinds:**

| Kind | Source | Description |
|---|---|---|
| `OrgLayer` | file tree | Org culture and standing rules (mirrors `docs/org.md`) |
| `InitiativeLayer` | file tree | Strategic goal (mirrors `docs/initiatives/<id>.md`) |
| `ProjectLayer` | file tree | Project foundation (mirrors `docs/projects/<id>/project.md`) |
| `ComponentLayer` | file tree / sessions | Fractal component within a project, unlimited depth |
| `Artifact` | Amassada sessions | Session output (ADR, implementation notes, test plans…) |
| `Signal` | ConciergeAgent | Observation from room archival sweep |
| `Decision` | Sessions | Human-approved decision |
| `Pattern` | Optimizer agent | Cross-project pattern identified by sweep |
| `FondamentProposal` | Optimizer agent | Candidate for promotion to a Fondament primitive |
| `AuditEntry` | Gardian | Credential audit entry (long-term retention) |

**Edge kinds:**

| Kind | Meaning |
|---|---|
| `contributes_to` | This node feeds the assembled context for an address |
| `is_part_of` | Fractal nesting: component → project, project → initiative |
| `supersedes` | This node replaces an older one (older marked `stale = TRUE`) |
| `conflicts_with` | Optimizer-detected conflict, surfaced to a human |
| `derived_from` | This node was produced by combining other nodes |
| `referenced_by` | Cited in another node's content |
| `promotes_to` | FondamentProposal → target Fondament primitive path |

---

## Crate Structure

```
farga/
├── Cargo.toml                  # workspace root
├── farga-core/                 # traits, graph model, HTTP client — consumed as library
│   └── src/
│       ├── lib.rs
│       ├── error.rs            # FargaError, Result alias
│       ├── types.rs            # Node, Edge, Signal, Artifact, NodeKind, EdgeKind
│       ├── reader.rs           # FargaReader trait + HttpFargaReader
│       └── writer.rs           # FargaWriter trait + HttpFargaWriter
├── farga-server/               # HTTP service, SQLite graph, optimizer stub
│   └── src/
│       ├── main.rs             # startup, env config, axum serve
│       ├── lib.rs              # module declarations
│       ├── state.rs            # AppState (SqlitePool + DocsTree)
│       ├── db.rs               # insert_node, get_node, mark_stale, insert_edge, get_subgraph
│       ├── docs.rs             # DocsTree file tree reader
│       ├── optimizer.rs        # optimizer agent stubs (v0.2.0)
│       └── routes/
│           ├── mod.rs          # axum Router wiring
│           ├── context.rs      # GET /context/…
│           ├── signals.rs      # POST/GET /signals
│           └── artifacts.rs    # POST/GET /artifacts
├── farga-cli/                  # command-line interface
│   └── src/
│       ├── main.rs             # clap CLI, subcommand dispatch
│       └── commands/
│           ├── mod.rs
│           ├── context.rs      # farga context {org,project,component}
│           ├── signals.rs      # farga signals
│           ├── artifacts.rs    # farga artifacts
│           └── proposals.rs    # farga proposals (stub, v0.2.0)
├── migrations/
│   ├── 001_initial_schema.sql  # nodes + edges tables
│   └── 002_add_indexes.sql     # performance indexes
└── docs/                       # foundation file tree — human-readable, git-friendly
    ├── org.md
    ├── initiatives/
    └── projects/
```

---

## farga-core

`farga-core` is a pure library crate with no binary. It defines the canonical types and the async traits that all consumers of Farga implement against.

### Types (`types.rs`)

- **`Node`** — graph vertex with `id` (UUIDv4), `kind: NodeKind`, optional `address`, `project`, `component`, `title`, `content`, timestamps, and a `stale` flag.
- **`Edge`** — directed relationship between two nodes with `from_id`, `to_id`, `kind: EdgeKind`, `weight: f64`, and `created_at`.
- **`Signal`** — lightweight observation: `project`, `content`, `source`.
- **`Artifact`** — session output: `project`, `title`, `content`, optional `session_id`, `kind` string.

### FargaReader trait (`reader.rs`)

```rust
#[async_trait]
pub trait FargaReader: Send + Sync {
    async fn org_layer(&self, org: &str) -> Result<OrgContext>;
    async fn initiative_layer(&self, org: &str) -> Result<Vec<InitiativeContext>>;
    async fn project_layer(&self, project: &str) -> Result<ProjectContext>;
    async fn component_layer(&self, project: &str, path: &str) -> Result<ProjectContext>;
    async fn recent_signals(&self, project: &str, since_hours: u64) -> Result<Vec<Signal>>;
}
```

`HttpFargaReader` is the production implementation. It calls the farga-server REST endpoints using `reqwest`.

### FargaWriter trait (`writer.rs`)

```rust
#[async_trait]
pub trait FargaWriter: Send + Sync {
    async fn write_signals(&self, project: &str, signals: Vec<Signal>) -> Result<()>;
    async fn write_artifact(&self, artifact: Artifact) -> Result<()>;
    async fn write_audit(&self, entry: AuditEntry) -> Result<()>;
}
```

`HttpFargaWriter` is the production implementation. `AuditEntry` carries `timestamp`, `agent`, `capability`, `outcome`, and `token_id`.

### Error model (`error.rs`)

`FargaError` is a `thiserror`-derived enum with variants: `NotFound(String)`, `Database(String)`, `Http(String)`, `Io(std::io::Error)`.

---

## farga-server

`farga-server` is an `axum` HTTP server backed by SQLite via `sqlx`. It owns both storage layers: the `docs/` file tree (read-only at runtime, hot-reloaded on webhook) and the SQLite graph database.

### AppState

```rust
pub struct AppState {
    pub pool: SqlitePool,
    pub docs: Arc<DocsTree>,
}
```

Shared across all route handlers via axum's `State` extractor.

### DocsTree (`docs.rs`)

Reads the `docs/` file tree synchronously:

| Method | Path read |
|---|---|
| `read_org()` | `docs/org.md` |
| `read_initiatives()` | All `*.md` files under `docs/initiatives/` |
| `read_project(id)` | `docs/projects/<id>/project.md` |
| `read_component(id, path)` | `docs/projects/<id>/<path>/component.md` |

### Database layer (`db.rs`)

| Function | Description |
|---|---|
| `insert_node(pool, node)` | Insert a new node row |
| `get_node(pool, id)` | Fetch a single node by primary key |
| `mark_stale(pool, id)` | Set `stale = 1` on a node |
| `insert_edge(pool, edge)` | Insert an edge (INSERT OR IGNORE on duplicate PK) |
| `get_subgraph(pool, root_id, depth)` | BFS from root up to `depth` hops; returns `(Vec<Node>, Vec<Edge>)` |

---

## REST API

The server listens on `0.0.0.0:<FARGA_PORT>` (default `7500`).

### Context endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/context/org/:org` | Returns the org-layer context document (`docs/org.md`) |
| `GET` | `/context/initiatives/:org` | Returns a JSON array of initiative context strings |
| `GET` | `/context/project/:project` | Returns the project foundation document |
| `GET` | `/context/component/:project/*path` | Returns the component document at arbitrary depth |

Context endpoints serve the `docs/` file tree directly. The path segment `:project` matches `docs/projects/<project>/project.md`; `*path` is joined to locate `component.md` at any depth.

### Signal endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/signals` | Write one or more signals for a project |
| `GET` | `/signals/recent` | Fetch recent signals for a project |

**POST /signals** — request body:

```json
{
  "project": "auth-service",
  "signals": [
    { "project": "auth-service", "content": "...", "source": "concierge" }
  ]
}
```

Each signal is stored as a `Signal` node. Returns `201 Created` on success.

**GET /signals/recent** — query parameters:

| Parameter | Required | Description |
|---|---|---|
| `project` | Yes | Project identifier |
| `since` | No | Time window, e.g. `24h` |

Returns a JSON array of `Signal` objects. The server returns the 100 most recent non-stale `Signal` nodes for the project, ordered newest-first.

### Artifact endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/artifacts` | Store a session artifact |
| `GET` | `/artifacts/:project` | List all non-stale artifacts for a project |

**POST /artifacts** — request body:

```json
{
  "project": "auth-service",
  "title": "ADR-042: Use PASETO tokens",
  "content": "...",
  "session_id": "sess_abc123",
  "kind": "adr"
}
```

Returns `201 Created`. The artifact is stored as an `Artifact` node in the graph.

**GET /artifacts/:project** — returns a JSON array of artifact objects.

---

## Database Schema

Managed by `sqlx` migrations under `migrations/`. Migrations run automatically on server startup.

### migration 001 — initial schema

```sql
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

`stale` is an integer (`0` = live, `1` = superseded). The composite primary key on `edges` prevents duplicate relationships.

### migration 002 — indexes

```sql
CREATE INDEX IF NOT EXISTS idx_nodes_project ON nodes(project);
CREATE INDEX IF NOT EXISTS idx_nodes_kind    ON nodes(kind);
CREATE INDEX IF NOT EXISTS idx_nodes_address ON nodes(address);
CREATE INDEX IF NOT EXISTS idx_edges_from    ON edges(from_id);
CREATE INDEX IF NOT EXISTS idx_edges_to      ON edges(to_id);
```

These five indexes cover the common query patterns: project-scoped node fetches, kind-filtered lookups, address-based context assembly, and BFS graph traversal.

---

## Optimizer Agent

`farga-server/src/optimizer.rs` — currently stubbed for v0.2.0, with two entry points defined:

### Job 1 — Write-triggered scoped pass

Fires on every `POST /signals` or `POST /artifacts`. No LLM call. Fast, bounded cost per write.

```
on_write(node_id):
  subgraph = graph.subgraph(root: node_id, depth: 3)
  for each node in subgraph:
    if newer node supersedes it → mark stale = TRUE, add supersedes edge
    if content contradicts sibling → add conflicts_with edge, queue for sweep
```

Implemented as `run_write_triggered(node_id: &str)`.

### Job 2 — Scheduled full sweep

LLM-assisted. Runs on a configurable interval (default 24 h). Daily token budget is hard-capped before firing.

```
all_projects = farga.list_projects()
summaries = [ recent_signals(p) + recent_artifacts(p) for p in all_projects ]

findings = llm_call(optimizer_persona, summaries)

for each finding:
  stale_content       → propose_pr("optimizer/prune-<id>", ...)
  conflict            → propose_pr("optimizer/resolve-<id>", ...)
  pattern             → propose_pr("optimizer/pattern-<id>", ...)
  fondament_candidate → propose_pr("optimizer/promote-<id>", ...)

whisper OrgAgent with all PR links via Charradissa
```

Implemented as `run_scheduled_sweep()`.

The optimizer authenticates to the code host (GitHub or GitLab) via Gardian (`github_access` / `gitlab_access` capability). On merge, a webhook triggers hot-reload of `docs/` and applies any pending migrations.

---

## farga-cli

A `clap`-based binary that queries `farga-server` over HTTP. The default server URL is `http://localhost:7500`; override with `--server <url>`.

```
farga [--server <url>] <COMMAND>
```

### Commands

#### `farga context`

```
farga context org <org>                    # print org layer (docs/org.md)
farga context project <project>            # print project foundation document
farga context component <project> <path>   # print component document at <path>
```

Uses `HttpFargaReader` from `farga-core`.

#### `farga signals`

```
farga signals --project <id> [--since-hours <n>]
```

Lists recent signals for a project. Default window is 24 hours. Prints each signal as `[<source>] <content>` followed by a count.

#### `farga artifacts`

```
farga artifacts --project <id>
```

Lists artifact titles for a project.

#### `farga proposals`

```
farga proposals list      # list open optimizer PRs (v0.2.0)
farga proposals trigger   # manually trigger optimizer sweep (v0.2.0)
```

Currently prints a not-yet-implemented notice. Full implementation scheduled for v0.2.0.

---

## Configuration

`farga-server` is configured exclusively through environment variables:

| Variable | Default | Description |
|---|---|---|
| `FARGA_DB` | `farga.db` | Path to the SQLite database file (created if absent, via `mode=rwc`) |
| `FARGA_DOCS` | `docs` | Path to the foundation file tree root |
| `FARGA_PORT` | `7500` | TCP port the server listens on |

The full `farga.toml` configuration (optimizer budget, code host, Gardian URL, webhook secret) is defined in the design spec and targeted for v0.2.0 when the optimizer agent is fully wired.

### Example

```sh
FARGA_DB=/var/farga/farga.db \
FARGA_DOCS=/var/farga/docs \
FARGA_PORT=7500 \
farga-server
```

---

## Docs File Tree

The `docs/` directory is the human-readable, git-friendly foundation layer. All files are Markdown. Changes to `docs/` are proposed by the optimizer as pull requests and merged through normal code review.

```
docs/
├── org.md                              # org culture, standing rules
├── initiatives/
│   └── <id>.md                         # one file per strategic initiative
└── projects/
    └── <project-id>/
        ├── project.md                  # project foundation document
        └── <component-path>/
            └── component.md            # fractal component document
```

The server reads these files on startup and on hot-reload. There is no import step — the file tree is the source of truth for the foundation layer.

---

## Consumers: Charradissa and Amassada

### Charradissa

Charradissa (the Occitan messaging and session-routing layer) writes signals via `POST /signals` after each ConciergeAgent room archival sweep. It reads recent signals via `GET /signals/recent` as input to its convergence sweep. Charradissa uses `HttpFargaWriter` from `farga-core` to write and `HttpFargaReader` to read.

### Amassada

Amassada (the Occitan session execution layer) writes session artifacts via `POST /artifacts` at the end of each work session. It reads project and component context at session start via `GET /context/project/:id` and `GET /context/component/:project/*path`, assembling the context map for the working agent. Amassada uses both `HttpFargaReader` and `HttpFargaWriter` from `farga-core`.

Both consumers depend on `farga-core` as a library crate. Neither links against `farga-server` directly.

---

## Testing

### Unit tests

`farga-core` types and trait impls are tested with standard Rust unit tests (`cargo test -p farga-core`).

### Integration tests

`farga-server` integration tests use an in-memory SQLite pool (`sqlite::memory:`) and run migrations before each test suite. Run with:

```sh
cargo test -p farga-server
```

### End-to-end

Start the server against a scratch database, then exercise the CLI:

```sh
FARGA_DB=/tmp/farga-test.db FARGA_DOCS=./docs cargo run -p farga-server &
farga --server http://localhost:7500 context org acme
farga --server http://localhost:7500 signals --project auth-service
```

### Running all tests

```sh
cargo test --workspace
```

---

## Dependencies

Key crates, all declared as workspace dependencies in the root `Cargo.toml`:

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `axum` | HTTP server framework |
| `sqlx` | SQLite driver and migration runner |
| `serde` / `serde_json` | Serialization |
| `async-trait` | `FargaReader` / `FargaWriter` traits |
| `reqwest` | HTTP client (reader/writer impls) |
| `clap` | CLI argument parsing |
| `chrono` | Timestamps |
| `uuid` | UUIDv4 node IDs |
| `notify` | File watcher for `docs/` hot-reload (v0.2.0) |
| `thiserror` | `FargaError` derive |
| `tracing` / `tracing-subscriber` | Structured logging |
| `anyhow` | Error handling in server and CLI |
