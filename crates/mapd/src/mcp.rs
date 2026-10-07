//! MCP over streamable HTTP (JSON responses): JSON-RPC 2.0 requests posted to `/mcp`, single
//! or batched. Implements `initialize`, `ping`, `tools/list` and `tools/call`; notifications
//! are accepted with 202.

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::Shared;

const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub async fn handle(app: &Shared, _headers: &HeaderMap, body: &str) -> Response {
    let Ok(req) = serde_json::from_str::<Value>(body) else {
        return json_response(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } }), None);
    };
    let mut session = None;
    let out = if let Value::Array(list) = req {
        let mut replies = Vec::new();
        for r in list {
            if let Some(v) = one(app, r, &mut session).await {
                replies.push(v);
            }
        }
        (!replies.is_empty()).then_some(Value::Array(replies))
    } else {
        one(app, req, &mut session).await
    };
    match out {
        Some(v) => json_response(v, session),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

fn json_response(v: Value, session: Option<String>) -> Response {
    let mut r = (StatusCode::OK, [("content-type", "application/json")], v.to_string()).into_response();
    if let Some(s) = session
        && let Ok(h) = HeaderValue::from_str(&s)
    {
        r.headers_mut().insert("mcp-session-id", h);
    }
    r
}

async fn one(app: &Shared, req: Value, session: &mut Option<String>) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req["method"].as_str().unwrap_or("");
    // Notifications get no reply.
    let id = id?;
    let result: Result<Value, (i64, String)> = match method {
        "initialize" => {
            let asked = req["params"]["protocolVersion"].as_str().unwrap_or(VERSIONS[0]);
            let version = VERSIONS.iter().copied().find(|v| *v == asked).unwrap_or(VERSIONS[0]);
            *session = Some(format!("mapd-{}", app.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "mapd", "title": "Worldspring", "version": env!("CARGO_PKG_VERSION") },
                "instructions": crate::tools::INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": crate::tools::list() })),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("").to_string();
            let args = req["params"].get("arguments").cloned().unwrap_or(json!({}));
            println!("mapd: tool {name} {args}");
            // (In its own task: a client that hangs up can't stop an edit halfway.)
            let app = app.clone();
            let done = tokio::spawn(async move { crate::tools::call(&app, &name, args).await }).await;
            Ok(match done.unwrap_or_else(|e| Err(format!("the tool stopped: {e}"))) {
                Ok(content) => json!({ "content": content, "isError": false }),
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            })
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}
