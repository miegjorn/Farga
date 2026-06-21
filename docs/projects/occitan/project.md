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

## Open questions

- Guilhem event listener architecture (task #7): webhook vs polling
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
