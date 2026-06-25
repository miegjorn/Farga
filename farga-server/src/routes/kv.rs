use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use crate::{
    db::{delete_kv, get_kv, list_kv_namespace, patch_kv, upsert_kv},
    state::AppState,
};

#[derive(Deserialize)]
pub struct PutKvReq {
    pub value: Value,
    pub ttl_seconds: Option<i64>,
}

#[derive(Deserialize)]
pub struct PatchKvReq {
    /// Optional JSON object to shallow-merge into the current value.
    /// Omit to leave the value unchanged (e.g. a TTL-only refresh).
    pub merge: Option<Value>,
    /// Optional new TTL in seconds, resetting expiry to `now + ttl_seconds`.
    /// Omit to leave the current expiry untouched.
    pub ttl_seconds: Option<i64>,
}

/// PUT /kv/*path  — upsert with optional TTL
pub async fn put_kv(
    State(s): State<AppState>,
    Path(kv_path): Path<String>,
    Json(req): Json<PutKvReq>,
) -> StatusCode {
    let value_json = match serde_json::to_string(&req.value) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("kv put: serialise value failed: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };
    if let Err(e) = upsert_kv(&s.pool, &kv_path, &value_json, req.ttl_seconds).await {
        tracing::error!("kv put: upsert failed: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::NO_CONTENT
}

/// GET /kv/*path
/// If the full path resolves to a single KV entry → return it.
/// If not found → try treating the path as a namespace and list all live keys.
/// If neither → 404.
pub async fn get_kv_or_list(
    State(s): State<AppState>,
    Path(kv_path): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Try exact key lookup first.
    match get_kv(&s.pool, &kv_path).await {
        Ok(Some(row)) => {
            return Ok(Json(serde_json::json!({
                "namespace": row.namespace,
                "key": row.key,
                "value": row.value,
                "expires_at": row.expires_at,
            })));
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("kv get: lookup failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Fall back to namespace listing.
    match list_kv_namespace(&s.pool, &kv_path).await {
        Ok(rows) if !rows.is_empty() => {
            let entries: Vec<Value> = rows
                .into_iter()
                .map(|row| serde_json::json!({
                    "namespace": row.namespace,
                    "key": row.key,
                    "value": row.value,
                    "expires_at": row.expires_at,
                }))
                .collect();
            Ok(Json(Value::Array(entries)))
        }
        Ok(_) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("kv list: query failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// DELETE /kv/*path
pub async fn delete_kv_handler(
    State(s): State<AppState>,
    Path(kv_path): Path<String>,
) -> StatusCode {
    match delete_kv(&s.pool, &kv_path).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("kv delete: failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// PATCH /kv/*path  — update value (shallow JSON object merge) and/or TTL in place
pub async fn patch_kv_handler(
    State(s): State<AppState>,
    Path(kv_path): Path<String>,
    Json(req): Json<PatchKvReq>,
) -> StatusCode {
    let merge_json = match req.merge.as_ref() {
        Some(v) => match serde_json::to_string(v) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("kv patch: serialise merge failed: {}", e);
                return StatusCode::BAD_REQUEST;
            }
        },
        None => None,
    };
    match patch_kv(&s.pool, &kv_path, merge_json.as_deref(), req.ttl_seconds).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("kv patch: merge failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
