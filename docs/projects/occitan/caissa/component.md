# Caissa — Container Toolbox

**No server port** | **Basque:** kutxa

Docker-based container toolbox and k8s operator for agent lifecycle. Builds agent
images from Fondament definitions, manages sandboxed Claude Code sessions, and
houses the Helm charts + bootstrap scripts for the Occitan kind cluster.

## Key commands

- `caissa build <generation>` — reads Fondament def, bakes ~/.claude/CLAUDE.md, tags `caissa-sandbox:<generation>`
- `caissa push <generation>` — pushes to registry
- `caissa spawn <generation> [--project <proj>] [--session <id>]` — interactive claude session
- `caissa listen` — webhook daemon (event listener for Guilhem Deployment)
- `caissa sandbox` — run arbitrary commands in the base sandbox
- `caissa report` — check Farga connectivity

## Generation model

Agent images are tagged by generation name, not semantic version.
`caissa-sandbox:guilhem` is the Phase II generation.
New generation = new Fondament definition + new image tag.

## Infrastructure (deploy/)

```
deploy/kind/          kind cluster config + MetalLB + CoreDNS patch
deploy/argocd/        ArgoCD project + app-of-apps root + per-app manifests
deploy/charts/        Helm charts: occitan (umbrella) + guilhem (agent)
deploy/workflows/     Argo Workflows: chronicle CronWorkflow + build templates
scripts/bootstrap.sh  one-shot cluster bootstrap
docs/install.md       human how-to
```

## Status (Phase II start)

build/push/spawn commands implemented and compiling.
listen command: pending (task #7).
Dockerfiles for Rust services: pending (task #8).
