# Farga — Context Substrate

**Port:** 7500 | **Binary:** `farga-server` | **Basque:** sorlekua

Fractal project context substrate and collective memory. Projects have nested
components with their own context-map, agent, and lifecycle. SQLite-backed.

## Key concepts

- **DocsTree:** file-based context (markdown at `docs/projects/:project/...`)
- **Nodes:** SQLite records for signals, artifacts, decisions, patterns
- **Librarian:** background task that grooms Farga's own memory

## API routes

```
GET  /context/org/:org
GET  /context/project/:project        ← returns project.md
GET  /context/component/:project/*path
POST /signals                         ← { project, signals: [{content, source}] }
GET  /signals/recent?project=&since=
POST /artifacts
GET  /artifacts/:project
POST /governance
GET  /governance/precedent
```

## Role in Phase II

Farga IS Guilhem's memory. Every chronicle run reads from Farga at session start,
writes signals/artifacts back at session end. The PVC stores `farga.db`; the docs
tree stores the seeded markdown context (this file is part of that seed).
