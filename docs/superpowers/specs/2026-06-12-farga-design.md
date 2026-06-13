# Farga Design Spec
_2026-06-12_

## 1. Purpose

Farga (Occitan: *forge*) is the fractal project context substrate and collective memory layer of the Occitan stack. It holds the living context that agents are grown from — org layer, initiative layer, project layer, component layers, session artifacts, convergence signals, and decisions.

**Fondament defines the shape of context. Farga holds the content.**

Farga is a git repository and a running service simultaneously. The file tree holds human-readable foundation documents (org, initiatives, projects). A SQLite graph holds everything the system produces dynamically (artifacts, signals, decisions, patterns). The optimizer agent — Farga's own AI — proposes improvements as pull requests against itself.

---

## 2. Crate Structure

```
farga/
├── Cargo.toml
├── farga-core/             # traits, graph model, file tree reader — consumed as library
├── farga-server/           # HTTP service, SQLite writer, optimizer agent, PR creator
├── farga-cli/              # query, inspect, propose, audit
├── docs/                   # foundation file tree — human-readable, git-friendly
│   ├── org.md              ← org layer: culture, standing rules
│   ├── initiatives/
│   │   └── q3-growth.md   ← initiative layer: strategic goals
│   └── projects/
│       └── auth-service/
│           └── project.md ← project foundation layer
└── migrations/             # sqlx SQL migration files — committed, PR-reviewable
    ├── 001_initial_schema.sql
    └── 042_consolidate_auth_pattern.sql
```

**Two storage layers:**

| Layer | Format | Contents | Git-friendly |
|---|---|---|---|
| Foundation | Markdown file tree (`docs/`) | org, initiative, project documents | ✓ fully |
| Dynamic | SQLite (`farga.db`) | nodes, edges, signals, artifacts | ✓ schema only |

`farga.db` is gitignored — it is generated from incoming writes. Schema migrations in `migrations/` are committed and reviewed as PRs. The optimizer agent's proposed changes to `docs/` and `migrations/` are also opened as PRs.

---

## 3. Graph Model

**Core schema** (managed via sqlx migrations):

```sql
CREATE TABLE nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    address     TEXT,             -- composition address this contributes to
    project     TEXT,             -- project scope (null = org-wide)
    component   TEXT,             -- component scope (null = project-wide)
    title       TEXT,
    content     TEXT,             -- context document content
    created_at  DATETIME NOT NULL,
    updated_at  DATETIME NOT NULL,
    stale       BOOLEAN DEFAULT FALSE
);

CREATE TABLE edges (
    from_id     TEXT REFERENCES nodes(id),
    to_id       TEXT REFERENCES nodes(id),
    kind        TEXT NOT NULL,
    weight      REAL DEFAULT 1.0,
    created_at  DATETIME NOT NULL,
    PRIMARY KEY (from_id, to_id, kind)
);
```

**Node kinds:**

| Kind | Source | Description |
|---|---|---|
| `OrgLayer` | file tree | org culture, standing rules (mirrors `docs/org.md`) |
| `InitiativeLayer` | file tree | strategic goal (mirrors `docs/initiatives/<id>.md`) |
| `ProjectLayer` | file tree | project foundation (mirrors `docs/projects/<id>/project.md`) |
| `ComponentLayer` | file tree / sessions | fractal: component within a project, unlimited depth |
| `Artifact` | Amassada sessions | session output (ADR, implementation notes, test plans…) |
| `Signal` | ConciergeAgent | observation from room archival sweep |
| `Decision` | sessions | human-approved decision |
| `Pattern` | optimizer agent | cross-project pattern identified by sweep |
| `FondamentProposal` | optimizer agent | candidate for promotion to Fondament primitive |
| `AuditEntry` | Gardian | credential audit entry (long-term retention) |

**Edge kinds:**

| Kind | Meaning |
|---|---|
| `contributes_to` | this node feeds the assembled context for an address |
| `is_part_of` | fractal nesting: component → project, project → initiative |
| `supersedes` | this node replaces an older one (older marked `stale = TRUE`) |
| `conflicts_with` | optimizer-detected conflict, surfaces to human |
| `derived_from` | this node was produced from combining other nodes |
| `referenced_by` | cited in another node's content |
| `promotes_to` | FondamentProposal → target Fondament primitive path |

**Context assembly at dispatch** — graph traversal, not linear stack:

```
resolve(address):
  find all nodes WHERE address matches AND stale = FALSE
  follow contributes_to edges, ordered by weight DESC
  → assembled context chunks in priority order
```

The file tree (org/initiative/project foundation) is mirrored as nodes on startup and on hot-reload. Graph traversal unifies both storage layers into a single resolved context.

---

## 4. HTTP API & Consumer Interfaces

`farga-server` exposes a REST API. All consumers interact through it.

**Fondament reads:**
```
GET /context/org/:org_id
GET /context/initiatives/:org_id
GET /context/project/:project_id
GET /context/component/:project_id/*path      # fractal, arbitrary depth
```

**Charradissa writes:**
```
POST /signals                                 # ConciergeAgent room archival
GET  /signals/recent?project=:id&since=:ts    # convergence sweep input
```

**Amassada writes:**
```
POST /artifacts                               # session artifact output
GET  /artifacts/:project_id
```

**Gardian writes:**
```
POST /audit                                   # long-term audit retention
GET  /audit?agent=:address&since=:ts
```

**`farga-core` traits** (library side, implemented against the HTTP API):

```rust
#[async_trait]
pub trait FargaReader: Send + Sync {
    async fn org_layer(&self, org: &OrgId) -> Result<OrgContext>;
    async fn initiative_layer(&self, org: &OrgId) -> Result<Vec<InitiativeContext>>;
    async fn project_layer(&self, project: &ProjectId) -> Result<ProjectContext>;
    async fn component_layer(&self, project: &ProjectId, path: &str) -> Result<ComponentContext>;
}

#[async_trait]
pub trait FargaWriter: Send + Sync {
    async fn write_signals(&self, project: &ProjectId, signals: Vec<Signal>) -> Result<()>;
    async fn recent_signals(&self, project: &ProjectId, since: Duration) -> Result<Vec<Signal>>;
    async fn write_artifact(&self, project: &ProjectId, artifact: Artifact) -> Result<()>;
    async fn write_audit(&self, entry: AuditEntry) -> Result<()>;
}
```

---

## 5. Optimizer Agent

Farga's own AI agent. Runs two jobs — same dual-mode pattern as Fondament's conflict analyzer.

### Job 1 — Write-triggered scoped pass

Fires on every `POST /signals` or `POST /artifacts`. No LLM. Fast, bounded cost per write.

```
on_write(node_id):
  subgraph = graph.subgraph(root: node_id, depth: 3)
  for each node in subgraph:
    if newer node supersedes it → mark stale = TRUE, add supersedes edge
    if content contradicts sibling → add conflicts_with edge, queue for sweep
```

### Job 2 — Scheduled full sweep

LLM-assisted. Runs on configurable interval (default 24h). Daily token budget hard-capped before firing.

```
all_projects = farga.list_projects()
summaries = [ recent_signals(p) + recent_artifacts(p) for p in all_projects ]

findings = llm_call(optimizer_persona, summaries)

for each finding:
  stale_content      → propose_pr("optimizer/prune-<id>", mark stale + remove from docs/)
  conflict           → propose_pr("optimizer/resolve-<id>", conflict resolution document)
  pattern            → propose_pr("optimizer/pattern-<id>", new Pattern node + docs/ entry)
  fondament_candidate→ propose_pr("optimizer/promote-<id>",
                          farga: remove duplicate project context,
                          fondament_pr: new discipline/practice YAML)

whisper OrgAgent with all PR links via Charradissa
```

### PR creation

Optimizer authenticates to the code host via Gardian — `github_access` or `gitlab_access` capability depending on where Farga is hosted.

```rust
pub trait CodeHost: Send + Sync {
    async fn create_pr(&self, proposal: PrProposal) -> Result<PrUrl>;
}

pub struct PrProposal {
    pub branch: String,
    pub title: String,
    pub body: String,
    pub file_changes: Vec<FileChange>,        // docs/ tree changes
    pub migration: Option<SqlMigration>,      // new migrations/ file if graph changes
    pub linked_repo_pr: Option<FondamentPr>,  // for Fondament promotion candidates
}
```

Implementations: `GithubCodeHost`, `GitlabCodeHost`. Selected by `farga.toml`.

On merge → webhook (`X-GitHub-Event: push` or GitLab push hook) → `farga-server` hot-reloads `docs/` file tree + applies pending migrations.

---

## 6. CLI (`farga-cli`)

```
farga context org <org_id>                     # show assembled org layer
farga context project <project_id>             # show assembled project layer
farga context component <project_id> <path>    # show component layer (fractal)
farga graph subgraph <node_id> --depth 3       # visualise node neighbourhood
farga graph edges <node_id>                    # list all edges for a node
farga signals list --project <id> --since 24h  # recent ConciergeAgent signals
farga artifacts list --project <id>            # session artifacts for a project
farga proposals list                           # open optimizer PRs
farga proposals trigger                        # manually trigger sweep + PR creation
farga audit tail                               # stream audit entries
farga audit search --agent <address>           # filter by agent address
farga migrate run                              # apply pending sqlx migrations manually
```

---

## 7. Configuration (`farga.toml`)

```toml
[server]
port = 7500
docs_path = "./docs"              # file tree root — hot-reloaded on GitHub merge webhook

[database]
path = "./farga.db"

[optimizer]
sweep_interval_hours = 24
daily_token_budget = 80_000
repo = "acme/farga"               # PR target (GitHub or GitLab)
code_host = "github"              # "github" | "gitlab"

[gardian]
base_url = "http://gardian.internal:7400"
api_key = "${GARDIAN_API_KEY}"

[webhook]
secret = "${GITHUB_WEBHOOK_SECRET}"   # validates GitHub merge events
```

---

## 8. Key Crates

- `tokio` — async runtime
- `axum` — HTTP server
- `sqlx` — SQLite driver + migrations
- `serde` + `serde_yaml` — config and node serialization
- `notify` — file watcher for `docs/` hot-reload
- `async-trait` — FargaReader, FargaWriter traits
- `anthropic` (or `reqwest`) — optimizer agent LLM call
- `clap` — CLI argument parsing

---

## 9. Out of Scope (v1)

- Vector embeddings / semantic similarity search across nodes
- Multi-org Farga federation
- Non-GitHub PR backends (GitLab MR, Gitea, etc.)
- Real-time graph subscriptions (WebSocket)
- Component lifecycle management UI
- Farga-to-Farga sync across organizations
