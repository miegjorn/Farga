//! MCP (Model Context Protocol) server — JSON-RPC 2.0 over HTTP.
//!
//! Mounted at POST /mcp alongside the existing REST routes.
//! Exposes Farga's core operations as MCP tools consumable by Claude agents.

use axum::{extract::State, http::StatusCode, Json};
use farga_core::types::{Artifact, Node, NodeKind, Signal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{db::insert_node, state::AppState};

// ── JSON-RPC 2.0 envelope ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }

    fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0", id, result: None, error: Some(JsonRpcError { code, message: message.into() }) }
    }
}

fn text_result(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}

// ── Tool definitions ──────────────────────────────────────────────────────────

fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "write_signal",
                "description": "Write an observation, event, or decision to Farga's project memory. Use this to record what happened, what was decided, and why.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier (e.g. 'occitan', 'farga')" },
                        "content": { "type": "string", "description": "The signal content — what happened or was decided" },
                        "source": { "type": "string", "description": "Source of this signal (e.g. 'guilhem', 'farga/architect')" }
                    },
                    "required": ["project", "content"]
                }
            },
            {
                "name": "read_context",
                "description": "Read the current context for a project as markdown. Returns accumulated signals, artifacts, and project documentation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier" }
                    },
                    "required": ["project"]
                }
            },
            {
                "name": "write_artifact",
                "description": "Store a structured artifact in Farga: design document, ADR, test plan, implementation notes, or any durable output from a session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier" },
                        "title": { "type": "string", "description": "Artifact title" },
                        "content": { "type": "string", "description": "Artifact content (markdown)" },
                        "kind": { "type": "string", "description": "Artifact kind: 'adr', 'design', 'test-plan', 'implementation-notes', 'decision'" }
                    },
                    "required": ["project", "title", "content", "kind"]
                }
            },
            {
                "name": "search_signals",
                "description": "Retrieve recent signals for a project. Useful for catching up on what has happened before acting.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier" },
                        "since": { "type": "string", "description": "ISO 8601 timestamp — only return signals after this time (optional)" }
                    },
                    "required": ["project"]
                }
            },
            {
                "name": "list_projects",
                "description": "List all known projects in Farga.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

// ── Handler ───────────────────────────────────────────────────────────────────

pub async fn handle(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let id = req.id.clone();
    let result = dispatch(&state, &req.method, req.params).await;
    match result {
        Ok(v) => (StatusCode::OK, Json(JsonRpcResponse::ok(id, v))),
        Err(e) => (StatusCode::OK, Json(JsonRpcResponse::err(id, -32603, e.to_string()))),
    }
}

async fn dispatch(
    state: &AppState,
    method: &str,
    params: Option<Value>,
) -> anyhow::Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "farga", "version": "0.1.0" }
        })),

        "tools/list" => Ok(tool_list()),

        "tools/call" => {
            let params = params.unwrap_or(Value::Null);
            let name = params["name"].as_str().unwrap_or("");
            let args = &params["arguments"];
            call_tool(state, name, args).await
        }

        // Notifications — no response needed but we must not error
        "notifications/initialized" => Ok(json!({})),

        _ => anyhow::bail!("unknown method: {}", method),
    }
}

async fn call_tool(state: &AppState, name: &str, args: &Value) -> anyhow::Result<Value> {
    match name {
        "write_signal" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            let content = args["content"].as_str().unwrap_or("").to_string();
            let source = args["source"].as_str().unwrap_or("agent").to_string();

            anyhow::ensure!(!project.is_empty(), "project is required");
            anyhow::ensure!(!content.is_empty(), "content is required");

            let sig = Signal { project: project.clone(), content, source };
            let node = Node::new(NodeKind::Signal, Some(project), Some(sig.content.clone()));
            insert_node(&state.pool, &node).await
                .map_err(|e| anyhow::anyhow!("insert failed: {}", e))?;

            Ok(text_result(format!("Signal written (id: {})", node.id)))
        }

        "read_context" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            anyhow::ensure!(!project.is_empty(), "project is required");

            let ctx = fetch_project_context(&state.pool, &project).await?;
            if ctx.is_empty() {
                Ok(text_result(format!("No context found for project '{}'", project)))
            } else {
                Ok(text_result(ctx))
            }
        }

        "write_artifact" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            let title = args["title"].as_str().unwrap_or("").to_string();
            let content = args["content"].as_str().unwrap_or("").to_string();
            let kind = args["kind"].as_str().unwrap_or("artifact").to_string();

            anyhow::ensure!(!project.is_empty(), "project is required");
            anyhow::ensure!(!title.is_empty(), "title is required");
            anyhow::ensure!(!content.is_empty(), "content is required");

            let artifact = Artifact { project: project.clone(), title: title.clone(), content, session_id: None, kind };
            let mut node = Node::new(NodeKind::Artifact, Some(project), Some(artifact.content.clone()));
            node.title = Some(title);
            insert_node(&state.pool, &node).await
                .map_err(|e| anyhow::anyhow!("insert failed: {}", e))?;

            Ok(text_result(format!("Artifact written (id: {})", node.id)))
        }

        "search_signals" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            anyhow::ensure!(!project.is_empty(), "project is required");

            let rows: Vec<(Option<String>,)> = sqlx::query_as(
                "SELECT content FROM nodes WHERE kind = 'Signal' AND project = ? AND stale = 0 ORDER BY created_at DESC LIMIT 50"
            )
            .bind(&project)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();

            let signals: Vec<String> = rows.into_iter()
                .filter_map(|(c,)| c)
                .collect();

            let out = if signals.is_empty() {
                format!("No signals found for project '{}'", project)
            } else {
                signals.iter().enumerate()
                    .map(|(i, s)| format!("{}. {}", i + 1, s))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            Ok(text_result(out))
        }

        "list_projects" => {
            let rows: Vec<(Option<String>,)> = sqlx::query_as(
                "SELECT DISTINCT project FROM nodes WHERE project IS NOT NULL ORDER BY project"
            )
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();

            let projects: Vec<String> = rows.into_iter().filter_map(|(p,)| p).collect();
            Ok(text_result(projects.join("\n")))
        }

        _ => anyhow::bail!("unknown tool: {}", name),
    }
}

async fn fetch_project_context(pool: &sqlx::SqlitePool, project: &str) -> anyhow::Result<String> {
    let rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT content FROM nodes WHERE project = ? AND stale = 0 AND kind IN ('Signal','Artifact','ProjectLayer') ORDER BY created_at DESC LIMIT 100"
    )
    .bind(project)
    .fetch_all(pool)
    .await?;

    let parts: Vec<String> = rows.into_iter().filter_map(|(c,)| c).collect();
    Ok(parts.join("\n\n"))
}
