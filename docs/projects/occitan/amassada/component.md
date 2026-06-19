# Amassada — Session Engine

**Port:** 7700 | **Binary:** `amassada-server` | **Basque:** bilgune

Multi-agent session engine. Sessions are structured agent conversations driven by
a canvas YAML. A Moderator drives state. REST + WebSocket transport.

## Key concepts

- **Canvas:** YAML defining participants, budget pools, round bounds, expected output
- **Session state machine:** `Initializing → Round(n) → Complete`
- **Transport trait:** same session logic runs locally (CLI) or via Charradissa (Matrix)
- **Channels:** main_session, consult (private 2-agent), whisper (/btw out-of-turn)
- **Dispatch model:** `messages.create()` single-turn; system prompt assembled at
  dispatch from domain context + persona + block syntax

## Canvas stdlib

debate, design-session, code-review-council, architectural-design, planning

## Status (Phase II start)

Canvas stdlib, LLM selector, hot-switch, fork consultation, mission engine all
wired. Canvases baked into image at build time.
