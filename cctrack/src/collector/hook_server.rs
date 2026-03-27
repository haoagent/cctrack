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

/// Parse sub-agent info from transcript_path.
/// Sub-agent paths: .../{parent-session-id}/subagents/agent-{agentId}.jsonl
/// Returns (parent_session_id, agent_id) if this is a sub-agent.
pub fn parse_subagent_path(transcript_path: &str) -> Option<(String, String)> {
    let path = std::path::Path::new(transcript_path);
    // Check if parent dir is "subagents"
    let parent_dir = path.parent()?.file_name()?.to_str()?;
    if parent_dir != "subagents" {
        return None;
    }
    // Extract agent_id from filename: "agent-{id}.jsonl" → "{id}"
    let filename = path.file_stem()?.to_str()?;
    let agent_id = filename.strip_prefix("agent-")?.to_string();
    // Extract parent session_id: two levels up from the file
    let parent_session_id = path.parent()?.parent()?.file_name()?.to_str()?.to_string();
    Some((parent_session_id, agent_id))
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

    // Detect sub-agent from hook payload fields (preferred) or transcript path (fallback)
    // Claude Code sends: agent_id, agent_type, agent_transcript_path for sub-agents
    let agent_id = payload.extra.get("agent_id").and_then(|v| v.as_str()).map(String::from);
    let agent_type = payload.extra.get("agent_type").and_then(|v| v.as_str()).map(String::from);
    let agent_transcript_path = payload.extra.get("agent_transcript_path").and_then(|v| v.as_str()).map(String::from);

    let subagent_info = if let Some(ref aid) = agent_id {
        // Hook payload has explicit agent_id → this is a sub-agent
        Some((session_id.clone(), aid.clone(), agent_type.clone()))
    } else {
        // Fallback: try parsing from agent_transcript_path or transcript_path
        agent_transcript_path.as_deref().and_then(parse_subagent_path)
            .or_else(|| transcript_path.as_deref().and_then(parse_subagent_path))
            .map(|(pid, aid)| (pid, aid, None))
    };

    // For sub-agents, use agent_id as the effective identifier
    let effective_id = match subagent_info {
        Some((_, ref aid, _)) => aid.clone(),
        None => session_id.clone(),
    };

    // Use agent_transcript_path for sub-agents, or derive from transcript_path + agent_id
    let effective_transcript = agent_transcript_path
        .or_else(|| {
            // Claude Code doesn't send agent_transcript_path, so derive it:
            // parent transcript: /path/to/{session_id}.jsonl
            // sub-agent transcript: /path/to/{session_id}/subagents/agent-{agent_id}.jsonl
            if let (Some(ref tp), Some(ref aid)) = (&transcript_path, &agent_id) {
                let parent = std::path::Path::new(tp);
                if let Some(stem) = parent.file_stem().and_then(|s| s.to_str()) {
                    let dir = parent.parent()?;
                    let subagent_path = dir.join(stem).join("subagents").join(format!("agent-{}.jsonl", aid));
                    if subagent_path.exists() {
                        return Some(subagent_path.to_string_lossy().to_string());
                    }
                }
                None
            } else {
                None
            }
        })
        .or(transcript_path.clone());

    // Extract sub-agent name mapping before moving payload fields
    // When parent calls Agent tool: tool_input.description + tool_response.agentId
    let subagent_name_mapping = if payload.tool_name == "Agent" && agent_id.is_none() {
        let desc = payload.tool_input.get("description").and_then(|v| v.as_str()).map(String::from);
        let child_id = payload.tool_response.get("agentId").and_then(|v| v.as_str()).map(String::from);
        match (desc, child_id) {
            (Some(d), Some(c)) => Some((c, d)),
            _ => None,
        }
    } else {
        None
    };

    let tool_event = ToolEvent {
        agent_name: effective_id.clone(),
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
        transcript_path: effective_transcript.clone(),
        subagent_info: subagent_info.clone(),
    };

    // Extract TodoWrite items before moving tool_event
    let todo_items = if tool_event.tool_name == "TodoWrite" {
        parse_todo_items(&payload.tool_input)
    } else {
        None
    };

    let sid = effective_id.clone();
    let _ = state.event_tx.send(Event::ToolCall(tool_event)).await;

    // Emit SubAgentName when parent calls Agent tool
    if let Some((child_id, desc)) = subagent_name_mapping {
        let _ = state.event_tx.send(Event::SubAgentName {
            agent_id: child_id,
            name: desc,
        }).await;
    }

    // Emit TodoUpdate if we parsed todo items
    if let Some(todos) = todo_items {
        let _ = state.event_tx.send(Event::TodoUpdate {
            session_id: sid.clone(),
            todos,
        }).await;
    }

    // Parse transcript for token usage (best-effort, async)
    // For sub-agents: read their own transcript; for parent: read parent transcript
    if let Some(tp) = effective_transcript {
        let tx = state.event_tx.clone();
        let sid2 = sid;
        tokio::spawn(async move {
            if let Some(usage) = read_transcript_usage(&tp) {
                let _ = tx.send(Event::TokenUpdate { session_id: sid2, usage }).await;
            }
        });
    }

    StatusCode::OK
}

/// Read user's first meaningful message from transcript as a session title.
/// Tries all queue-operation entries in the first 30 lines.
/// Returns a clean, meaningful title or None.
pub fn read_session_title(path: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;
    for line in reader.lines().take(200) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
            // Try queue-operation first (older format)
            if msg_type == "queue-operation" {
                if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                    if let Some(title) = clean_session_title(content) {
                        return Some(title);
                    }
                }
            }
            // Also try user messages (newer format)
            if msg_type == "user" {
                if let Some(msg) = val.get("message") {
                    // message.content can be string or array
                    let text = msg.get("content").and_then(|c| {
                        if let Some(s) = c.as_str() { return Some(s.to_string()); }
                        if let Some(arr) = c.as_array() {
                            for item in arr {
                                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                                        return Some(t.to_string());
                                    }
                                }
                            }
                        }
                        None
                    });
                    if let Some(text) = text {
                        if let Some(title) = clean_session_title(&text) {
                            return Some(title);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Clean user's first message into a usable session title.
/// Returns None if the content is a file path, command, or too short.
fn clean_session_title(content: &str) -> Option<String> {
    let first_line = content.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return None;
    }
    let title: String = first_line.chars().take(32).collect();
    Some(title)
}

/// Read a transcript .jsonl file and return (token usage, model name).
pub fn read_transcript_usage(path: &str) -> Option<TokenUsage> {
    let (usage, _model) = read_transcript_usage_and_model(path)?;
    if usage.total() > 0 { Some(usage) } else { None }
}

/// Read transcript for both usage and model name.
/// Cost is computed per-message with tiered pricing.
pub fn read_transcript_usage_and_model(path: &str) -> Option<(TokenUsage, Option<String>)> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut usage = TokenUsage::default();
    let mut model: Option<String> = None;
    for line in content.lines() {
        if !line.contains("\"message\"") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            // Extract model — update per message (can change mid-session)
            if let Some(m) = val.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                model = Some(m.to_string());
            }
            if let Some(u) = val.get("message").and_then(|m| m.get("usage")) {
                let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let cache_read = u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let (cache_write_5m, cache_write_1h) = if let Some(cc) = u.get("cache_creation") {
                    (
                        cc.get("ephemeral_5m_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        cc.get("ephemeral_1h_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                    )
                } else {
                    (u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0), 0)
                };
                usage.add_message(model.as_deref(), input, output, cache_read, cache_write_5m, cache_write_1h);
            }
        }
    }
    if usage.total() > 0 || model.is_some() {
        Some((usage, model))
    } else {
        None
    }
}

/// Read just the model name from a transcript (first few lines).
pub fn read_session_model(path: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;
    for line in reader.lines().take(20) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if !line.contains("\"model\"") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(m) = val.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                return Some(m.to_string());
            }
        }
    }
    None
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

    #[test]
    fn parse_subagent_path_valid() {
        let path = "/Users/jerry/.claude/projects/-Users-jerry-Documents-Clipal/758a572d-55ef-4aa4-9118-7f304146254c/subagents/agent-a9e71542fc6453b4f.jsonl";
        let result = parse_subagent_path(path);
        assert_eq!(result, Some((
            "758a572d-55ef-4aa4-9118-7f304146254c".to_string(),
            "a9e71542fc6453b4f".to_string(),
        )));
    }

    #[test]
    fn parse_subagent_path_parent_transcript() {
        let path = "/Users/jerry/.claude/projects/-Users-jerry-Documents-Clipal/758a572d-55ef-4aa4-9118-7f304146254c.jsonl";
        assert_eq!(parse_subagent_path(path), None);
    }

    #[test]
    fn parse_subagent_path_not_agent_prefix() {
        let path = "/some/path/subagents/notanagent.jsonl";
        assert_eq!(parse_subagent_path(path), None);
    }
}
