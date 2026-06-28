# Occitan — Project Context

The Occitan project is the stack developing itself. Seven sub-projects plus
one meta-repo, each a git repo under `github.com/miegjorn/` (the org all
these repos were transferred into on 2026-06-20), each with an Occitan name.

## Architecture

```
Gardian (7600)      credential gateway — two-hop token resolution
Fondament           agent definition tree — primitives, not fixed agents
Farga (7500)        fractal context substrate — SQLite-backed, project/component tree
Amassada (7700)     multi-agent session engine — canvas-driven, REST + WebSocket
Charradissa (9000)  Matrix chat runtime — agents as room members
Cor                 plugin marketplace — distributes agent defs, canvases, MCPs
Caissa              container toolbox — agent image builds, sandbox, k8s operator
Occitan             meta-repo — cross-component Initiatives, GitHub scaffold scripts, Project board
```

## Deployment

**Kind cluster** (`occitan`) running locally.
**Domain:** `occitane.guilhem` — permanent Matrix server_name.
**ArgoCD** watches `github.com/miegjorn/Caissa` for Helm chart changes.
**Generation model:** agent images tagged by generation name (`caissa-sandbox:guilhem`).

## Build order

Gardian → Fondament → Farga → Amassada → Cor → Charradissa → Caissa

## Current generation

Phase II / Guilhem. The stack is being handed to its own agents to manage.
Guilhem is the org agent: always on, watches activity, chronicles into Farga,
available for direct conversation via `caissa spawn guilhem`.

## Matrix room topology (2026-06-27)

Charradissa now provisions rooms dynamically at startup via `provision_project_rooms`.
It reads the component list from Farga (`GET /context/components/occitan`), resolves
each component's system prompt from Fondament (`GET /resolve/fondament/{name}-agent`),
and creates-or-joins aliased Matrix rooms:

| Room alias | Purpose |
|---|---|
| `#occitan:occitane.guilhem` | Project room — Guilhem lives here as `@charradissa` (display name "Guilhem") |
| `#amassada:occitane.guilhem` | Amassada component agent room |
| `#farga:occitane.guilhem` | Farga component agent room |
| `#fondament:occitane.guilhem` | Fondament component agent room |
| `#charradissa:occitane.guilhem` | Charradissa component agent room |
| `#cor:occitane.guilhem` | Cor component agent room |
| `#caissa:occitane.guilhem` | Caissa component agent room |
| `#gardian:occitane.guilhem` | Gardian component agent room |

Guilhem responds in `#occitan` via the default HTTP agent URL. Each `#component` room
gets its own Responder with the Fondament-resolved system prompt. If Fondament returns
an empty prompt, that component room is skipped (fallback: static `[component_agents]`
TOML fires if all components are skipped). `fondament-server` is built but not yet
deployed — that's the next step before provisioning is fully live.

## Open questions

- Guilhem event listener architecture (task #7): webhook vs polling
- Drive provisioning off `fondament-server /component-agents` instead of Farga doc-listing (eliminates the `{name}-agent` template assumption — deferred from issue #22)
- `create_or_join_aliased_room` treats any 400 as "create" — should verify `M_NOT_FOUND` errcode (minor, deferred)
- Farga migration from SQLite to Postgres when write contention becomes an issue
- Federation with future generations (occitane.arnaut, etc.)

## GitHub Topology

How work is tracked across the 8 repos under `github.com/miegjorn/`. If
you're filing or looking for an issue, this is the convention.

### Hierarchy: Initiative → Epic → Story

Work is typed using native GitHub Issue Types (org-level, not labels) —
`miegjorn` has five: `Task`, `Bug`, `Feature` (GitHub defaults) plus two
custom types added for this stack: `Epic` and `Initiative`.

- **Story** — the leaf work item. Typed `Task`, `Bug`, or `Feature`. Lives
  in the component repo it belongs to (Gardian, Fondament, Farga, Amassada,
  Charradissa, Cor, or Caissa).
- **Epic** — a component-scoped grouping of Stories. Typed `Epic`. Lives in
  the same component repo as the Stories it groups.
- **Initiative** — a cross-component goal. Typed `Initiative`. Lives only
  in `Occitan`. Its children are Epics (or, for small cases, Stories
  directly) from across component repos.

Parent/child relationships use GitHub's native sub-issues (not a markdown
checklist) — confirmed working cross-repo: `Occitan#1` ("Ingest todo.md
into issues", type `Initiative`) has sub-issues `Gardian#3` and `Caissa#8`
(both type `Task`), each in their own repo. Query the tree with the
`subIssues` GraphQL field; there's no `gh issue create --parent` shortcut,
so creating these requires a GraphQL `createIssue` mutation with
`issueTypeId` and `parentIssueId` set — see `Occitan/scripts/github-scaffold/`
for the label/template side of the scaffold (issue-type and sub-issue
wiring isn't scriptable as a static template, it's done per-issue via the
GitHub API or web UI's Type picker at creation time).

### Component labels

Every component repo carries one `component:<slug>` label for itself
(lowercase repo name, e.g. `component:gardian`), applied automatically by
that repo's issue templates. `Occitan` carries no component label — nothing
lives "in" Occitan except Initiatives. There is no `type:*` label anywhere
— that's the native Issue Type field now, not a label.

### Cross-repo board

One GitHub Project (v2), titled "Occitan Stack" (`miegjorn` org project
#1, `https://github.com/orgs/miegjorn/projects/1`), linked to all 8 repos.
Default Status field (Backlog/Todo/In Progress/Done); Issue Type and
Component are visible via their native columns.

### Filing and finding issues

```bash
# File a Story in a component repo (component label auto-applied by the
# issue template; pick the Task/Bug/Feature type in GitHub's Type picker
# when creating, gh CLI has no --type flag):
gh issue create --repo miegjorn/<repo> --label component:<slug> --title "..." --body "..."

# List open issues for one component:
gh issue list --repo miegjorn/<repo> --label component:<slug>

# Find all Initiatives across the stack (Occitan only):
gh issue list --repo miegjorn/Occitan

# Inspect an Initiative's cross-repo sub-issue tree:
gh api graphql -f query='query{repository(owner:"miegjorn",name:"Occitan"){issue(number:N){title issueType{name} subIssues(first:20){nodes{number repository{name} title issueType{name}}}}}}'

# Create a Story as a child of an existing Epic/Initiative (GraphQL, no CLI shortcut):
gh api graphql -f query='mutation{createIssue(input:{repositoryId:"<REPO_NODE_ID>",title:"...",body:"...",issueTypeId:"<TASK_OR_BUG_OR_FEATURE_TYPE_ID>",parentIssueId:"<PARENT_ISSUE_NODE_ID>"}){issue{id number url}}}'
```

Do not file cross-component work as a single issue in one of the component
repos — open the Initiative in Occitan and let its Epic/Story children live
in their own repos.

## GitLab Mirror Policy

GitLab (`gitlab.com/cor912026/`) is a **mirror/backup only**. GitHub
(`github.com/miegjorn/`) is the single source of truth for all 8 repos.

- **No merge requests, ever, on GitLab.** All review and merge activity
  happens on GitHub. Any MR opened on GitLab should be closed, not merged.
- **No branch protection on GitLab.** `main` (and any other branch) is
  intentionally left unprotected on all 8 GitLab projects so that mirror
  pushes can always force-update, matching whatever GitHub's history looks
  like — including history rewrites.
- Each repo's local `origin` remote points to GitHub (canonical); a
  separate `gitlab` remote points to the mirror. Don't push directly to the
  `gitlab` remote from a feature branch — mirroring is a full history
  mirror (`git clone --mirror` + `git push --mirror`) run from GitHub's
  state, not an incremental push.
- GitHub's synthetic `refs/pull/N/head` refs are not part of the mirror —
  GitLab rejects that ref namespace and it carries no meaning there anyway
  (no MRs are ever tracked on GitLab).
- If GitHub history is rewritten (e.g. to strip accidentally-committed
  binaries — see `Fondament` and `Cor`, which had `target/` build
  artifacts purged from history on 2026-06-21), re-run the full mirror
  push; GitLab is expected to follow, not to be reconciled by hand.
