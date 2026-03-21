use std::collections::HashMap;

use axum::{body::Bytes, extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::store::event::Event;
use crate::store::models::{TokenUsage, ToolEvent};

/// Payload received from Claude Code PostToolUse hooks.
///
/// Claude Code sends: session_id, tool_name, tool_input, tool_response, etc.
/// We accept both `tool_input`/`input` and `tool_response`/`output` for robustness.
#[derive(Debug, Deserialize)]
pub struct HookPayload {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub tool_name: String,
    /// Claude Code uses "tool_input"; we also accept "input" for manual testing
    #[serde(default, alias = "input")]
    pub tool_input: serde_json::Value,
    #[serde(default, alias = "output")]
    pub tool_response: serde_json::Value,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Extract just the filename from a path.
fn shorten_path(full_path: &str) -> String {
    std::path::Path::new(full_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(full_path)
        .to_string()
}

/// Summarize the tool input into a short human-readable string.
fn summarize_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_else(|| "Read".to_string()),
        "Edit" => {
            let file = input.get("file_path").and_then(|v| v.as_str()).map(shorten_path);
            match file {
                Some(f) => f,
                None => "Edit".to_string(),
            }
        }
        "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_else(|| "Write".to_string()),
        "Bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let truncated: String = cmd.chars().take(80).collect();
            if truncated.is_empty() { "Bash".to_string() } else { truncated }
        }
        "Grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if pattern.is_empty() && path.is_empty() {
                "Grep".to_string()
            } else {
                format!("{} {}", pattern, path).trim().to_string()
            }
        }
        "Glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if pattern.is_empty() {
                "Glob".to_string()
            } else if path.is_empty() {
                pattern.to_string()
            } else {
                format!("{} in {}", pattern, shorten_path(path))
            }
        }
        "Agent" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("subagent")
            .to_string(),
        "TodoWrite" => {
            let count = input.get("todos")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("{} items", count)
        }
        "WebSearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("search")
            .chars().take(60).collect(),
        "WebFetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("fetch")
            .chars().take(60).collect(),
        "Skill" => input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("skill")
            .to_string(),
        "NotebookEdit" => input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_else(|| "notebook".to_string()),
        _ => tool_name.to_string(),
    }
}

/// Parse TodoWrite tool_input into TodoItem vec.
fn parse_todo_items(input: &serde_json::Value) -> Option<Vec<crate::store::models::TodoItem>> {
    let arr = input.get("todos")?.as_array()?;
    let items: Vec<crate::store::models::TodoItem> = arr.iter().filter_map(|v| {
        Some(crate::store::models::TodoItem {
            content: v.get("content")?.as_str()?.to_string(),
            status: v.get("status")?.as_str()?.to_string(),
            active_form: v.get("activeForm").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        })
    }).collect();
    if items.is_empty() { None } else { Some(items) }
}

#[derive(Clone)]
struct AppState {
    event_tx: mpsc::Sender<Event>,
}

async fn handle_hook(
    State(state): State<AppState>,
    body: Bytes,
) -> StatusCode {
    let payload: HookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let summary = summarize_input(&payload.tool_name, &payload.tool_input);
    let session_id = payload.session_id;

    let cwd = payload.extra.get("cwd").and_then(|v| v.as_str()).map(String::from);
    let transcript_path = payload.extra.get("transcript_path").and_then(|v| v.as_str()).map(String::from);

    let tool_event = ToolEvent {
        agent_name: session_id.clone(),
        tool_name: payload.tool_name,
        timestamp: chrono::Utc::now().to_rfc3339(),
        summary,
        duration_ms: if payload.duration_ms > 0 {
            Some(payload.duration_ms)
        } else {
            None
        },
        success: None,
        cwd,
        transcript_path: transcript_path.clone(),
    };

    // Extract TodoWrite items before moving tool_event
    let todo_items = if tool_event.tool_name == "TodoWrite" {
        parse_todo_items(&payload.tool_input)
    } else {
        None
    };

    let sid = session_id.clone();
    let _ = state.event_tx.send(Event::ToolCall(tool_event)).await;

    // Emit TodoUpdate if we parsed todo items
    if let Some(todos) = todo_items {
        let _ = state.event_tx.send(Event::TodoUpdate {
            session_id: sid.clone(),
            todos,
        }).await;
    }

    // Parse transcript for token usage (best-effort, async)
    if let Some(ref tp) = transcript_path {
        let transcript_path = tp.clone();
        let tx = state.event_tx.clone();
        let sid2 = sid;
        tokio::spawn(async move {
            if let Some(usage) = read_transcript_usage(&transcript_path) {
                let _ = tx.send(Event::TokenUpdate { session_id: sid2, usage }).await;
            }
        });
    }

    StatusCode::OK
}

/// Read session title from transcript first line (queue-operation content).
/// Truncates to 30 chars for display.
pub fn read_session_title(path: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;
    for line in reader.lines().take(5) {
        let line = line.ok()?;
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            if val.get("type").and_then(|v| v.as_str()) == Some("queue-operation") {
                if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                    let title: String = content.chars().take(30).collect();
                    if !title.is_empty() {
                        return Some(title);
                    }
                }
            }
        }
    }
    None
}

/// Read a transcript .jsonl file and sum all token usage.
fn read_transcript_usage(path: &str) -> Option<TokenUsage> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut usage = TokenUsage::default();
    for line in content.lines() {
        if !line.contains("\"usage\"") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(u) = val.get("message").and_then(|m| m.get("usage")) {
                usage.input_tokens += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.output_tokens += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.cache_read_tokens += u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.cache_create_tokens += u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            }
        }
    }
    if usage.total() > 0 { Some(usage) } else { None }
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
        assert_eq!(summarize_input("Edit", &input), "foo.rs");
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
