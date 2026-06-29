//! MCP (Model Context Protocol) server — JSON-RPC 2.0 over HTTP.
//!
//! Mounted at POST /mcp alongside the existing REST routes.
//! Exposes Farga's core operations as MCP tools consumable by Claude agents.

use axum::{extract::State, http::StatusCode, Json};
use farga_core::types::{Artifact, Node, NodeKind, Signal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{db::{insert_node, upsert_component_todo, upsert_context_node, get_context_node, list_context_nodes}, state::AppState};

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
                "description": "Read the current context for a project as markdown. Returns accumulated signals, artifacts, project documentation, and context graph nodes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier" },
                        "role": { "type": "string", "description": "Your role for context graph access: component | architect | org | human (optional, defaults to component)" }
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
            ,
            {
                "name": "update_component_todo",
                "description": "Create or update the TODO/follow-up record for a specific component within a project. Use this to log deferred work or known drift instead of leaving it unrecorded — each (project, component) pair has exactly one live record, overwritten on each call.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project identifier" },
                        "component": { "type": "string", "description": "Component name within the project (e.g. 'gardian', 'caissa-listen')" },
                        "content": { "type": "string", "description": "The TODO content — what's deferred and why" }
                    },
                    "required": ["project", "component", "content"]
                }
            },
            {
                "name": "write_context_node",
                "description": "Write a typed context node to the Farga context graph. Context nodes are role-scoped knowledge artifacts: codebase references, architecture summaries, design rationale, etc. Identified by hierarchical path like [gardian][codebase] or [occitan][system-rationale].",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Hierarchical path, e.g. [gardian][codebase] or [occitan][system-rationale]" },
                        "node_type": { "type": "string", "description": "Node type: codebase-ref | architecture | rationale | persona | decision | design" },
                        "content": { "type": "string", "description": "Node content (markdown, reference URL, or structured text)" },
                        "read_role": { "type": "string", "description": "Minimum role to read: component | architect | org | human" },
                        "project": { "type": "string", "description": "Project identifier" },
                        "component": { "type": "string", "description": "Component name (optional)" }
                    },
                    "required": ["path", "node_type", "content", "read_role", "project"]
                }
            },
            {
                "name": "read_context_node",
                "description": "Read a specific context node by path. Returns the node content if your role permits access.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "role": { "type": "string", "description": "Your role: component | architect | org | human" }
                    },
                    "required": ["path", "role"]
                }
            },
            {
                "name": "list_context_nodes",
                "description": "List context nodes accessible to your role for a project. Returns paths and node types.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "role": { "type": "string", "description": "Your role: component | architect | org | human" }
                    },
                    "required": ["project", "role"]
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

            let role_level = args["role"].as_str()
                .map(role_to_level)
                .unwrap_or(0);

            let ctx = fetch_project_context(&state.pool, &project).await?;
            let context_nodes = list_context_nodes(&state.pool, &project, role_level).await
                .unwrap_or_default();

            let mut parts: Vec<String> = Vec::new();
            if !ctx.is_empty() {
                parts.push(ctx);
            }
            if !context_nodes.is_empty() {
                let mut section = String::from("## Context Graph
");
                for node in &context_nodes {
                    let path = node.address.as_deref().unwrap_or("");
                    let node_type = node.title.as_deref().unwrap_or("");
                    let body = node.content.as_deref().unwrap_or("");
                    section.push_str(&format!("
### {} ({})
{}
", path, node_type, body));
                }
                parts.push(section);
            }

            if parts.is_empty() {
                Ok(text_result(format!("No context found for project '{}'", project)))
            } else {
                Ok(text_result(parts.join("

")))
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

            // Parse and validate the optional `since` parameter.
            let since_ts: Option<String> = args["since"].as_str().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.to_rfc3339())
            });

            let rows: Vec<(Option<String>,)> = if let Some(ref since) = since_ts {
                sqlx::query_as(
                    "SELECT content FROM nodes WHERE kind = 'Signal' AND project = ? AND stale = 0 AND created_at > ? ORDER BY created_at DESC LIMIT 50"
                )
                .bind(&project)
                .bind(since)
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default()
            } else {
                sqlx::query_as(
                    "SELECT content FROM nodes WHERE kind = 'Signal' AND project = ? AND stale = 0 ORDER BY created_at DESC LIMIT 50"
                )
                .bind(&project)
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default()
            };

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

        "update_component_todo" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            let component = args["component"].as_str().unwrap_or("").to_string();
            let content = args["content"].as_str().unwrap_or("").to_string();

            anyhow::ensure!(!project.is_empty(), "project is required");
            anyhow::ensure!(!component.is_empty(), "component is required");
            anyhow::ensure!(!content.is_empty(), "content is required");

            let id = upsert_component_todo(&state.pool, &project, &component, &content)
                .await
                .map_err(|e| anyhow::anyhow!("upsert failed: {}", e))?;

            Ok(text_result(format!("Component TODO updated (id: {})", id)))
        }

        "write_context_node" => {
            let path_arg = args["path"].as_str().unwrap_or("").to_string();
            let node_type = args["node_type"].as_str().unwrap_or("").to_string();
            let content_arg = args["content"].as_str().unwrap_or("").to_string();
            let read_role = args["read_role"].as_str().unwrap_or("component").to_string();
            let project = args["project"].as_str().unwrap_or("").to_string();
            let component = args["component"].as_str().map(|s| s.to_string());

            anyhow::ensure!(!path_arg.is_empty(), "path is required");
            anyhow::ensure!(!node_type.is_empty(), "node_type is required");
            anyhow::ensure!(!content_arg.is_empty(), "content is required");
            anyhow::ensure!(!project.is_empty(), "project is required");

            let level = role_to_level(&read_role);
            let id = upsert_context_node(
                &state.pool,
                &path_arg,
                &node_type,
                &content_arg,
                level,
                &project,
                component.as_deref(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("upsert failed: {}", e))?;

            Ok(text_result(format!("Context node written (id: {}, path: {})", id, path_arg)))
        }

        "read_context_node" => {
            let path_arg = args["path"].as_str().unwrap_or("").to_string();
            let role = args["role"].as_str().unwrap_or("component").to_string();

            anyhow::ensure!(!path_arg.is_empty(), "path is required");

            let level = role_to_level(&role);
            match get_context_node(&state.pool, &path_arg, level).await? {
                None => Ok(text_result(format!("Context node not found or not accessible at path '{}'", path_arg))),
                Some(node) => {
                    let body = node.content.unwrap_or_default();
                    let node_type = node.title.unwrap_or_default();
                    Ok(text_result(format!("### {} ({})
{}", path_arg, node_type, body)))
                }
            }
        }

        "list_context_nodes" => {
            let project = args["project"].as_str().unwrap_or("").to_string();
            let role = args["role"].as_str().unwrap_or("component").to_string();

            anyhow::ensure!(!project.is_empty(), "project is required");

            let level = role_to_level(&role);
            let nodes = list_context_nodes(&state.pool, &project, level).await
                .map_err(|e| anyhow::anyhow!("list failed: {}", e))?;

            if nodes.is_empty() {
                Ok(text_result(format!("No context nodes found for project '{}' at role '{}'", project, role)))
            } else {
                let lines: Vec<String> = nodes.iter().map(|n| {
                    let path = n.address.as_deref().unwrap_or("");
                    let node_type = n.title.as_deref().unwrap_or("");
                    format!("{} ({})", path, node_type)
                }).collect();
                Ok(text_result(lines.join("
")))
            }
        }

        _ => anyhow::bail!("unknown tool: {}", name),
    }
}

fn role_to_level(role: &str) -> i64 {
    match role {
        "architect" => 1,
        "org" => 2,
        "human" => 3,
        _ => 0, // default: component
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
