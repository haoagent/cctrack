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

        let jsonl_files = match std::fs::read_dir(&project_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for file_entry in jsonl_files.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

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

            // Send a synthetic ToolEvent to register the session
            let tool_event = ToolEvent {
                agent_name: session_id.clone(),
                tool_name: "startup_scan".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                summary: String::new(),
                duration_ms: None,
                success: None,
                cwd: cwd.clone(),
                transcript_path: Some(transcript_path.clone()),
            };

            let _ = event_tx.send(Event::ToolCall(tool_event)).await;

            // Send token usage if available
            if let Some(usage) = read_transcript_usage(&transcript_path) {
                let _ = event_tx.send(Event::TokenUpdate {
                    session_id,
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
