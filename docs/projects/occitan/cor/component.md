# Cor — Plugin Marketplace

**No server port** | **Basque:** bihotza

Claude plugin marketplace CLI. Distributes agent definitions, canvases, MCPs.
Both a Rust CLI and a TypeScript registry/SDK.

## Key commands

- `cor install <package>` — installs a Fondament package (e.g. `cor install deconstructive`)
- `cor publish` — publishes a plugin package to the registry

## Structure

- Rust CLI (`cor`) — install, publish, list
- TypeScript registry + SDK — package hosting and resolution

## Status (Phase II start)

`install_atomic` wired in registry install path. Registry functional.
