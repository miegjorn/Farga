# Charradissa — Matrix Chat Runtime

**Port:** 9000 (external) / 8448 (internal) | **Binary:** `charradissa-daemon` | **Basque:** solasaldia

Matrix appservice runtime. Projects as rooms, agents as room members. Consumes
`amassada-core` for session orchestration.

## Structure

- `charradissa-core` — agents (org, project, specialist), concierge, approval, transport, tool_loop, farga integration
- `charradissa-daemon` — main binary + registry
- `charradissa-jira` — Jira backend
- `charradissa-matrix` — Matrix appservice client + backend

## Matrix identity

Guilhem's Matrix user: `@guilhem:occitane.guilhem`
Homeserver: `http://synapse.occitan-system.svc.cluster.local:8008`

## Status (Phase II start)

Farcaster Phase II complete — fractal cross-domain hierarchy implemented.
Governance pipeline pending.

## Config

`charradissa.toml` with org, backend, concierge, approval, tasks, projects sections.
Mounted via ConfigMap in k8s.
