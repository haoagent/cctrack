//! Scan recent Claude Code transcripts on startup to pre-populate sessions.
//!
//! Finds .jsonl files modified in the last 24 hours and sends synthetic events
//! so the Store shows them immediately without waiting for hooks.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;

use crate::collector::hook_server::read_transcript_usage;
use crate::store::event::Event;
use crate::store::models::ToolEvent;

const RECENT_THRESHOLD_SECS: u64 = 86400; // 24 hours

/// Scan for recently active transcripts and emit events to pre-populate sessions.
pub async fn scan_recent(claude_home: &Path, event_tx: mpsc::Sender<Event>) {
    let projects_dir = claude_home.join("projects");
    if !projects_dir.exists() {
        return;
    }

    let now = SystemTime::now();
    let threshold = Duration::from_secs(RECENT_THRESHOLD_SECS);

    let project_dirs = match std::fs::read_dir(&projects_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for project_entry in project_dirs.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Derive CWD from project dir: "-Users-jerry-Documents-Clipal" → "/Users/jerry/Documents/Clipal"
        let cwd = project_dir_to_cwd(&project_path);

        // Collect all .jsonl files: top-level + subagents directories
        let mut jsonl_files: Vec<std::path::PathBuf> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    jsonl_files.push(path);
                } else if path.is_dir() {
                    // Check for subagents/ directory inside session dirs
                    let subagents_dir = path.join("subagents");
                    if subagents_dir.is_dir() {
                        if let Ok(sub_entries) = std::fs::read_dir(&subagents_dir) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if sub_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                                    jsonl_files.push(sub_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        for file_path in jsonl_files {
            // Only process recently modified files
            let modified = match file_path.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if now.duration_since(modified).unwrap_or(Duration::MAX) > threshold {
                continue;
            }

            // Session ID = filename without extension
            let session_id = match file_path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let transcript_path = file_path.to_string_lossy().to_string();

            // Detect sub-agent from transcript path
            let subagent_info = crate::collector::hook_server::parse_subagent_path(&transcript_path)
                .map(|(pid, aid)| (pid, aid, None)); // No agent_type from filesystem scan

            let effective_id = match subagent_info {
                Some((_, ref agent_id, _)) => agent_id.clone(),
                None => session_id.clone(),
            };

            let tool_event = ToolEvent {
                agent_name: effective_id.clone(),
                tool_name: "startup_scan".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                summary: String::new(),
                duration_ms: None,
                success: None,
                cwd: cwd.clone(),
                transcript_path: Some(transcript_path.clone()),
                subagent_info,
            };

            let _ = event_tx.send(Event::ToolCall(tool_event)).await;

            // Send token usage if available
            if let Some(usage) = read_transcript_usage(&transcript_path) {
                let _ = event_tx.send(Event::TokenUpdate {
                    session_id: effective_id.to_string(),
                    usage,
                }).await;
            }
        }
    }
}

/// Convert project directory name to original CWD path.
/// "-Users-jerry-Documents-Clipal" → "/Users/jerry/Documents/Clipal"
fn project_dir_to_cwd(path: &Path) -> Option<String> {
    let dir_name = path.file_name()?.to_str()?;
    if dir_name.starts_with('-') {
        // Replace leading dashes with /
        Some(dir_name.replace('-', "/"))
    } else {
        None
    }
}
