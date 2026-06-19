# Gardian — Credential Gateway

**Port:** 7600 | **Binary:** `gardian-server` | **Basque:** atarizain

Two-hop credential resolution: coordinator token → per-agent token → actual
credentials (AWS, Vault, etc.). Shields the rest of the stack from raw secrets.

## Key design

- `GARDIAN_MODE=server` runs the HTTP gateway
- Backends: AWS Secrets Manager, HashiCorp Vault, JSON file (local dev)
- `list_keys` CLI command for inspecting available credentials

## Status (Phase II start)

Implemented. Subprocess backend, JSON file backend, list_keys command all working.
No Dockerfile yet — image to be built by Argo Workflows pipeline.

## Interfaces

- `GET /health`
- `GET /credentials/:key` — resolves a credential by key
- `POST /credentials/:key` — register a new credential (server backend)
