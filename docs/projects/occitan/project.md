# Occitan — Project Context

The Occitan project is the stack developing itself. Seven sub-projects, each a
git repo under `github.com/bedardpl/`, each with an Occitan name.

## Architecture

```
Gardian (7600)      credential gateway — two-hop token resolution
Fondament           agent definition tree — primitives, not fixed agents
Farga (7500)        fractal context substrate — SQLite-backed, project/component tree
Amassada (7700)     multi-agent session engine — canvas-driven, REST + WebSocket
Charradissa (9000)  Matrix chat runtime — agents as room members
Cor                 plugin marketplace — distributes agent defs, canvases, MCPs
Caissa              container toolbox — agent image builds, sandbox, k8s operator
```

## Deployment

**Kind cluster** (`occitan`) running locally.
**Domain:** `occitane.guilhem` — permanent Matrix server_name.
**ArgoCD** watches `github.com/bedardpl/Caissa` for Helm chart changes.
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
