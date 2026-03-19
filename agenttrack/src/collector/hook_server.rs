use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::store::event::Event;
use crate::store::models::ToolEvent;

/// Payload received from Claude Code PostToolUse hooks.
#[derive(Debug, Deserialize)]
pub struct HookPayload {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub output: serde_json::Value,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Summarize the tool input into a short human-readable string.
fn summarize_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => {
            if let Some(fp) = input.get("file_path").and_then(|v| v.as_str()) {
                // Extract just the filename
                std::path::Path::new(fp)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(fp)
                    .to_string()
            } else {
                "Read".to_string()
            }
        }
        "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or(tool_name)
            .to_string(),
        "Bash" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let truncated: String = cmd.chars().take(80).collect();
            if truncated.is_empty() {
                "Bash".to_string()
            } else {
                truncated
            }
        }
        "Grep" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if pattern.is_empty() && path.is_empty() {
                "Grep".to_string()
            } else {
                format!("{} {}", pattern, path).trim().to_string()
            }
        }
        "Agent" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("subagent")
            .to_string(),
        _ => tool_name.to_string(),
    }
}

#[derive(Clone)]
struct AppState {
    event_tx: mpsc::Sender<Event>,
}

async fn handle_hook(
    State(state): State<AppState>,
    Json(payload): Json<HookPayload>,
) -> StatusCode {
    let _summary = summarize_input(&payload.tool_name, &payload.input);

    let tool_event = ToolEvent {
        agent_name: payload.session_id,
        tool_name: payload.tool_name,
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_ms: if payload.duration_ms > 0 {
            Some(payload.duration_ms)
        } else {
            None
        },
        success: None,
    };

    let _ = state.event_tx.send(Event::ToolCall(tool_event)).await;

    StatusCode::OK
}

/// Start the hook HTTP server. Tries ports `port` through `port + 9`.
/// Returns the actual port that was successfully bound.
pub async fn run(port: u16, event_tx: mpsc::Sender<Event>) -> u16 {
    let app_state = AppState { event_tx };

    let app = Router::new()
        .route("/hook", post(handle_hook))
        .with_state(app_state);

    // Try up to 10 consecutive ports
    for offset in 0u16..10 {
        let try_port = port + offset;
        let addr = format!("127.0.0.1:{}", try_port);
        match TcpListener::bind(&addr).await {
            Ok(listener) => {
                let actual_port = listener.local_addr().unwrap().port();
                tokio::spawn(async move {
                    axum::serve(listener, app).await.ok();
                });
                return actual_port;
            }
            Err(_) => continue,
        }
    }

    panic!(
        "hook_server: could not bind to any port in range {}..{}",
        port,
        port + 9
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_read() {
        let input = serde_json::json!({"file_path": "/home/user/project/src/main.rs"});
        assert_eq!(summarize_input("Read", &input), "main.rs");
    }

    #[test]
    fn summarize_edit() {
        let input = serde_json::json!({"file_path": "/home/user/foo.rs"});
        assert_eq!(summarize_input("Edit", &input), "/home/user/foo.rs");
    }

    #[test]
    fn summarize_bash_long() {
        let long_cmd = "a".repeat(200);
        let input = serde_json::json!({"command": long_cmd});
        let result = summarize_input("Bash", &input);
        assert_eq!(result.len(), 80);
    }

    #[test]
    fn summarize_grep() {
        let input = serde_json::json!({"pattern": "TODO", "path": "src/"});
        assert_eq!(summarize_input("Grep", &input), "TODO src/");
    }

    #[test]
    fn summarize_agent() {
        let input = serde_json::json!({"description": "refactor auth module"});
        assert_eq!(summarize_input("Agent", &input), "refactor auth module");
    }

    #[test]
    fn summarize_agent_default() {
        let input = serde_json::json!({});
        assert_eq!(summarize_input("Agent", &input), "subagent");
    }

    #[test]
    fn summarize_unknown_tool() {
        let input = serde_json::json!({"some_field": "value"});
        assert_eq!(summarize_input("CustomTool", &input), "CustomTool");
    }
}
