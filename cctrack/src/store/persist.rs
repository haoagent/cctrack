//! Persist session state to disk for restart recovery.
//!
//! Saves to `~/.claude/cctrack-state.json` on each Tick (debounced).
//! Loads on startup before startup_scan runs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::store::models::TokenUsage;

/// Top-level persisted state.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub saved_at: String,
    pub sessions: Vec<PersistedSession>,
}

/// A single session's persisted data.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedSession {
    pub agent_id: String,
    pub name: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub tokens: TokenUsage,
    /// If this session is a sub-agent, the parent's session_id.
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// Path to the state file.
pub fn state_path() -> PathBuf {
    crate::config::Config::claude_home().join("cctrack-state.json")
}

/// Load persisted state from disk. Returns None if file missing or corrupt.
pub fn load() -> Option<PersistedState> {
    let path = state_path();
    let data = std::fs::read_to_string(&path).ok()?;
    let state: PersistedState = serde_json::from_str(&data).ok()?;
    if state.version != 1 {
        return None;
    }
    Some(state)
}

/// Save state to disk. Writes atomically via temp file + rename.
pub fn save(state: &PersistedState) {
    let path = state_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(data) = serde_json::to_string_pretty(state) {
        if std::fs::write(&tmp, &data).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}
