# Fondament — Agent Definition Tree

**Library only (no server)** | **Basque:** elkarizar

Single source of truth for all agent and persona definitions. Disciplines, roles,
stances — the primitives from which agents are grown, not fixed entities.

## Key concepts

- **Modifier disciplines:** `modifier: true` disciplines (e.g. `deconstructive`)
  inject behaviour into prompt assembly without contributing corpus content.
- **CompositionAddress:** `fondament/role+deconstructive` — multi-`+` parser;
  modifiers go to `modifiers: Vec<String>`, others to `stance`.
- **ResolvedAgent.thinking_budget:** set when deconstructive modifier is active.
  Callers must pass `thinking: { type: "enabled", budget_tokens: n }`.

## Definitions (Phase II start)

```
disciplines/  rust-async, data/db/mysql, deconstructive (modifier)
roles/        security-sre
stances/      adversarial, builder, dreamer, moderator, realist
fondament/    app-architect, aws-architect, business-analyst, code-reviewer,
              data-architect, developer, guilhem, infra-engineer,
              project-moderator, qa-engineer, review-moderator,
              security-analyst, tech-moderator
```

## Guilhem definition

`fondament/guilhem` — the org agent for the Occitan stack. Chronicler stance.
Passive capture: watches, records, transmits. Default model: claude-sonnet-4-6.
