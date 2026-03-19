# AgentTrack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build AgentTrack — a Rust TUI + Web dashboard that monitors Claude Code agent teams in real-time by watching `~/.claude/` JSON files and receiving hook events.

**Architecture:** Three-layer design: Collectors (file watcher + hook HTTP server) feed events through tokio channels into an in-memory State Store, which broadcasts immutable snapshots to a Ratatui TUI and an axum-powered Web UI via `tokio::sync::watch`.

**Tech Stack:** Rust, Ratatui, crossterm, tokio, notify, axum, serde/serde_json, clap, rust-embed

**Spec:** `docs/superpowers/specs/2026-03-20-agenttrack-design.md`

---

## File Structure

All code lives under `agenttrack/` within the project root.

```
agenttrack/
├── Cargo.toml                     # Workspace-free, single crate
├── src/
│   ├── main.rs                    # CLI entry (clap), wires everything together
│   ├── lib.rs                     # Re-exports modules
│   ├── config.rs                  # TOML config loading (~/.agenttrack/config.toml)
│   ├── store/
│   │   ├── mod.rs                 # Re-exports
│   │   ├── models.rs              # All serde structs (TeamConfig, TaskFile, InboxMessage, etc.)
│   │   ├── event.rs               # Event enum + StoreSnapshot
│   │   └── state.rs               # process_events loop, snapshot broadcasting
│   ├── collector/
│   │   ├── mod.rs                 # Re-exports
│   │   ├── file_watcher.rs        # notify-based watcher for ~/.claude/ JSON files
│   │   ├── hook_server.rs         # axum POST /hook endpoint
│   │   └── hooks_installer.rs     # Read/merge/write ~/.claude/settings.json
│   ├── tui/
│   │   ├── mod.rs                 # Ratatui app loop + input handling
│   │   ├── app_state.rs           # TUI-local state (selected panel, selected agent, scroll)
│   │   ├── layout.rs              # Split terminal into panels
│   │   ├── agents_panel.rs        # Render agents table
│   │   ├── tasks_panel.rs         # Render tasks table
│   │   ├── activity_panel.rs      # Render live tool events
│   │   ├── messages_panel.rs      # Render message timeline
│   │   ├── top_bar.rs             # Render status bar
│   │   └── theme.rs               # Color palette, status symbols
│   └── web/
│       ├── mod.rs                 # axum router, SSE handler, static file serving
│       └── static/
│           ├── index.html         # Dashboard page
│           ├── app.js             # SSE client + DOM updates
│           └── style.css          # Dark theme styles
├── tests/
│   ├── fixtures/                  # Sample JSON files from real Claude Code
│   │   ├── team_config.json
│   │   ├── task_1.json
│   │   ├── task_2.json
│   │   ├── inbox_team_lead.json
│   │   └── inbox_brainstormer.json
│   ├── models_test.rs             # Deserialize fixture files
│   ├── store_test.rs              # Event processing, snapshot correctness
│   ├── file_watcher_test.rs       # Temp dir + file mutation tests
│   ├── hook_server_test.rs        # HTTP POST tests
│   └── hooks_installer_test.rs    # settings.json merge tests
├── Makefile
├── LICENSE
└── README.md
```

---

### Task 1: Project Scaffold + Cargo.toml

**Files:**
- Create: `agenttrack/Cargo.toml`
- Create: `agenttrack/src/main.rs`
- Create: `agenttrack/src/lib.rs`
- Create: `agenttrack/LICENSE`

- [ ] **Step 1: Create project directory**

```bash
mkdir -p /Users/jerry/Documents/Clipal/agenttrack/src
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agenttrack"
version = "0.1.0"
edition = "2021"
description = "Real-time observability dashboard for Claude Code agent teams"
license = "MIT"

[dependencies]
# TUI
ratatui = "0.29"
crossterm = "0.28"

# Async runtime
tokio = { version = "1", features = ["full"] }

# File watching
notify = "7"
notify-debouncer-mini = "0.5"

# HTTP server (hooks + web)
axum = { version = "0.8", features = ["tokio"] }
tower-http = { version = "0.6", features = ["cors"] }
tokio-stream = "0.1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# CLI
clap = { version = "4", features = ["derive"] }

# Utilities
chrono = { version = "0.4", features = ["serde"] }
dirs = "6"
tracing = "0.1"
tracing-subscriber = "0.3"
rust-embed = "8"
open = "5"
mime_guess = "2"

[profile.release]
strip = true
lto = true
codegen-units = 1
```

- [ ] **Step 3: Write minimal main.rs that compiles**

```rust
fn main() {
    println!("agenttrack v0.1.0");
}
```

- [ ] **Step 4: Write lib.rs with module declarations (commented out for now)**

```rust
// Modules will be uncommented as they are implemented
// pub mod config;
// pub mod store;
// pub mod collector;
// pub mod tui;
// pub mod web;
```

- [ ] **Step 5: Add MIT LICENSE file**

Create `agenttrack/LICENSE` with standard MIT license text, copyright 2026.

- [ ] **Step 6: Verify it compiles**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo build`
Expected: Compiles successfully (downloads deps, builds binary)

- [ ] **Step 7: Commit**

```bash
cd /Users/jerry/Documents/Clipal
git add agenttrack/
git commit -m "feat: scaffold agenttrack Rust project with dependencies"
```

---

### Task 2: Data Models + Fixture Tests

**Files:**
- Create: `agenttrack/src/store/mod.rs`
- Create: `agenttrack/src/store/models.rs`
- Create: `agenttrack/tests/fixtures/team_config.json`
- Create: `agenttrack/tests/fixtures/task_1.json`
- Create: `agenttrack/tests/fixtures/task_2.json`
- Create: `agenttrack/tests/fixtures/inbox_team_lead.json`
- Create: `agenttrack/tests/fixtures/inbox_brainstormer.json`
- Create: `agenttrack/tests/models_test.rs`

The JSON fixtures come from real Claude Code data observed at `~/.claude/teams/` and `~/.claude/tasks/`. These are the source of truth for what our serde structs must handle.

- [ ] **Step 1: Create fixture JSON files**

`tests/fixtures/team_config.json` — A realistic team config:
```json
{
  "name": "my-project",
  "description": "Working on feature X",
  "createdAt": 1773937440760,
  "leadAgentId": "team-lead@my-project",
  "leadSessionId": "fff799d7-662d-43b3-9939-f1054a54d0ab",
  "members": [
    {
      "agentId": "team-lead@my-project",
      "name": "team-lead",
      "agentType": "team-leader",
      "model": "claude-sonnet-4-6",
      "joinedAt": 1773937440760,
      "tmuxPaneId": "",
      "cwd": "/Users/dev/project",
      "subscriptions": []
    },
    {
      "agentId": "brainstormer@my-project",
      "name": "brainstormer",
      "agentType": "general-purpose",
      "model": "claude-opus-4-6",
      "color": "blue",
      "planModeRequired": false,
      "joinedAt": 1773937474005,
      "tmuxPaneId": "in-process",
      "cwd": "/Users/dev/project",
      "subscriptions": [],
      "backendType": "in-process"
    }
  ]
}
```

`tests/fixtures/task_1.json`:
```json
{
  "id": "1",
  "subject": "brainstormer",
  "description": "Brainstorm the feature design",
  "status": "completed",
  "blocks": [],
  "blockedBy": [],
  "metadata": { "_internal": true }
}
```

`tests/fixtures/task_2.json`:
```json
{
  "id": "2",
  "subject": "spec-reviewer",
  "description": "Review the spec document",
  "status": "in_progress",
  "blocks": ["3", "4"],
  "blockedBy": ["1"],
  "metadata": { "_internal": true }
}
```

`tests/fixtures/inbox_team_lead.json`:
```json
[
  {
    "from": "brainstormer",
    "text": "Status: DONE. The design spec is complete.",
    "summary": "Brainstorming complete, spec ready",
    "timestamp": "2026-03-19T16:28:49.266Z",
    "read": false,
    "color": "blue"
  },
  {
    "type": "idle_notification",
    "from": "brainstormer",
    "timestamp": "2026-03-19T16:29:01.000Z",
    "idleReason": "available"
  }
]
```

`tests/fixtures/inbox_brainstormer.json`:
```json
[
  {
    "from": "team-lead",
    "text": "Start brainstorming the new feature design.",
    "summary": "Begin brainstorming phase",
    "timestamp": "2026-03-19T16:24:00.000Z",
    "read": true
  }
]
```

- [ ] **Step 2: Write the serde models in `store/models.rs`**

Copy the exact structs from the spec (Section 5). Key points:
- `TeamConfig` and `MemberConfig` use `#[serde(rename_all = "camelCase")]` because Claude Code writes camelCase JSON
- `MemberConfig` needs `#[serde(default)]` on optional fields (`color`, `backend_type`, `tmux_pane_id`) because not all members have them
- `InboxMessage` has `#[serde(rename = "type")] pub msg_type: Option<String>` because `type` is a Rust keyword
- `TaskFile` uses `#[serde(default)]` on `blocks`, `blocked_by`, and `metadata`
- Also include the runtime types: `Agent`, `AgentStatus`, `Message`, `MessageType`, `ToolEvent`, `Metrics`
- Add `impl InboxMessage` with a `fn classify_type(&self) -> MessageType` method that inspects `msg_type` and `text` to determine message classification

- [ ] **Step 3: Write `store/mod.rs`**

```rust
pub mod models;
// pub mod event;  // Task 3
// pub mod state;  // Task 3
```

- [ ] **Step 4: Update `lib.rs` to export store module**

```rust
pub mod store;
```

- [ ] **Step 5: Write deserialization tests in `tests/models_test.rs`**

```rust
use std::fs;
use agenttrack::store::models::*;

#[test]
fn parse_team_config() {
    let json = fs::read_to_string("tests/fixtures/team_config.json").unwrap();
    let config: TeamConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.name, "my-project");
    assert_eq!(config.members.len(), 2);
    assert_eq!(config.members[1].name, "brainstormer");
    assert_eq!(config.members[1].model, "claude-opus-4-6");
    assert_eq!(config.members[1].color, "blue");
    assert_eq!(config.lead_session_id, "fff799d7-662d-43b3-9939-f1054a54d0ab");
}

#[test]
fn parse_task_with_dependencies() {
    let json = fs::read_to_string("tests/fixtures/task_2.json").unwrap();
    let task: TaskFile = serde_json::from_str(&json).unwrap();
    assert_eq!(task.id, "2");
    assert_eq!(task.subject, "spec-reviewer");
    assert_eq!(task.status, "in_progress");
    assert_eq!(task.blocks, vec!["3", "4"]);
    assert_eq!(task.blocked_by, vec!["1"]);
    assert!(task.metadata.as_ref().unwrap().internal);
}

#[test]
fn parse_inbox_with_mixed_message_types() {
    let json = fs::read_to_string("tests/fixtures/inbox_team_lead.json").unwrap();
    let messages: Vec<InboxMessage> = serde_json::from_str(&json).unwrap();
    assert_eq!(messages.len(), 2);

    // Normal message
    assert_eq!(messages[0].from, "brainstormer");
    assert!(messages[0].text.as_ref().unwrap().contains("DONE"));
    assert!(!messages[0].read);
    let msg_type = messages[0].classify_type();
    assert!(matches!(msg_type, MessageType::Normal));

    // Idle notification
    assert_eq!(messages[1].from, "brainstormer");
    assert_eq!(messages[1].msg_type.as_deref(), Some("idle_notification"));
    let msg_type = messages[1].classify_type();
    assert!(matches!(msg_type, MessageType::IdleNotification));
}

#[test]
fn parse_task_without_optional_fields() {
    // Task with no blocks/blockedBy/metadata
    let json = r#"{"id":"3","subject":"writer","description":"Write code","status":"pending"}"#;
    let task: TaskFile = serde_json::from_str(json).unwrap();
    assert_eq!(task.id, "3");
    assert!(task.blocks.is_empty());
    assert!(task.blocked_by.is_empty());
    assert!(task.metadata.is_none());
}

#[test]
fn parse_member_without_optional_fields() {
    // Member with only required fields
    let json = r#"{
        "agentId": "lead@team",
        "name": "lead",
        "agentType": "team-leader",
        "model": "claude-sonnet-4-6",
        "joinedAt": 1773937440760,
        "cwd": "/tmp"
    }"#;
    let member: MemberConfig = serde_json::from_str(json).unwrap();
    assert_eq!(member.name, "lead");
    assert_eq!(member.color, ""); // default empty string
    assert_eq!(member.backend_type, "");
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test --test models_test`
Expected: All 5 tests pass.

- [ ] **Step 7: Commit**

```bash
git add agenttrack/src/store/ agenttrack/tests/
git commit -m "feat: add data models with serde deserialization + fixture tests"
```

---

### Task 3: State Store + Event Processing

**Files:**
- Create: `agenttrack/src/store/event.rs`
- Create: `agenttrack/src/store/state.rs`
- Create: `agenttrack/tests/store_test.rs`

- [ ] **Step 1: Write `store/event.rs`**

```rust
use super::models::*;

#[derive(Debug)]
pub enum Event {
    TeamUpdate {
        team_name: String,
        config: TeamConfig,
    },
    TaskUpdate {
        team_name: String,
        task: TaskFile,
    },
    MessageUpdate {
        team_name: String,
        agent_name: String,  // inbox owner (derived from filename)
        messages: Vec<InboxMessage>,
    },
    ToolCall(ToolEvent),
}

/// Immutable snapshot of the entire store, sent to views via watch channel
#[derive(Debug, Clone, Default)]
pub struct StoreSnapshot {
    pub teams: Vec<TeamSnapshot>,
}

#[derive(Debug, Clone)]
pub struct TeamSnapshot {
    pub name: String,
    pub description: String,
    pub agents: Vec<Agent>,
    pub tasks: Vec<TaskFile>,
    pub messages: Vec<Message>,  // All messages across all inboxes, sorted by time
    pub tool_events: Vec<ToolEvent>,
    pub metrics: Metrics,
}
```

- [ ] **Step 2: Write `store/state.rs`**

Implement `process_events`: a function that receives events from an `mpsc::Receiver`, updates internal state, and broadcasts snapshots via `watch::Sender`.

Internal state is a `HashMap<String, TeamState>` keyed by team name. Each `TeamState` holds:
- `config: TeamConfig`
- `agents: Vec<Agent>` (enriched from config + inbox signals)
- `tasks: HashMap<String, TaskFile>` (keyed by task ID)
- `messages: Vec<Message>` (all messages, deduplicated by timestamp+from)
- `tool_events: Vec<ToolEvent>` (ring buffer, keep last 500)

Key logic:
- On `TeamUpdate`: rebuild agents from config members, preserve existing status
- On `TaskUpdate`: upsert task by ID
- On `MessageUpdate`: parse raw `InboxMessage` into enriched `Message` (derive `to` from agent_name, classify type, parse timestamp). Deduplicate by (from, timestamp) pair. Update agent status based on idle/shutdown notifications.
- On `ToolCall`: append to ring buffer, try to correlate agent by session_id/cwd
- After each event: recompute `Metrics`, build `StoreSnapshot`, send via `watch::Sender`

- [ ] **Step 3: Update `store/mod.rs`**

```rust
pub mod models;
pub mod event;
pub mod state;
```

- [ ] **Step 4: Write store tests in `tests/store_test.rs`**

```rust
use agenttrack::store::{event::*, models::*, state::Store};
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn team_update_creates_agents() {
    let (tx, rx) = mpsc::channel(16);
    let (snap_tx, mut snap_rx) = watch::channel(StoreSnapshot::default());

    let handle = tokio::spawn(Store::process_events(rx, snap_tx));

    let config = load_fixture_team_config(); // helper that reads fixtures/team_config.json
    tx.send(Event::TeamUpdate {
        team_name: "my-project".into(),
        config,
    }).await.unwrap();

    drop(tx); // close channel so process_events exits
    handle.await.unwrap();

    let snap = snap_rx.borrow();
    assert_eq!(snap.teams.len(), 1);
    assert_eq!(snap.teams[0].agents.len(), 2);
    assert_eq!(snap.teams[0].agents[0].config.name, "team-lead");
    assert_eq!(snap.teams[0].agents[0].status, AgentStatus::Unknown);
}

#[tokio::test]
async fn task_update_tracks_status() {
    // Send TeamUpdate, then TaskUpdate, verify task appears in snapshot
}

#[tokio::test]
async fn message_update_derives_to_field_and_classifies_type() {
    // Send MessageUpdate for team-lead inbox, verify Message.to == "team-lead"
    // Verify idle_notification classified correctly
}

#[tokio::test]
async fn idle_notification_updates_agent_status() {
    // Send TeamUpdate, then MessageUpdate with idle_notification
    // Verify agent status changed to Idle
}

#[tokio::test]
async fn metrics_computed_correctly() {
    // Send team + tasks (mix of statuses), verify metrics counts
}
```

Include a test helper `fn load_fixture_team_config() -> TeamConfig` that reads and parses the fixture file.

- [ ] **Step 5: Run tests**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test --test store_test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add agenttrack/src/store/ agenttrack/tests/store_test.rs
git commit -m "feat: add event-driven state store with snapshot broadcasting"
```

---

### Task 4: FileWatcher Collector

**Files:**
- Create: `agenttrack/src/collector/mod.rs`
- Create: `agenttrack/src/collector/file_watcher.rs`
- Create: `agenttrack/tests/file_watcher_test.rs`

- [ ] **Step 1: Write `collector/file_watcher.rs`**

Implement `pub async fn run(claude_home: PathBuf, event_tx: mpsc::Sender<Event>)`:
1. On startup, scan `claude_home/teams/` for existing team dirs
2. For each team dir found, read and parse `config.json`, send `TeamUpdate`
3. Read all task files in `claude_home/tasks/<team>/`, send `TaskUpdate` for each
4. Read all inbox files in `claude_home/teams/<team>/inboxes/`, send `MessageUpdate` for each
5. Set up `notify` watcher on `claude_home/teams/` and `claude_home/tasks/` (recursive)
6. On file change events, debounce 500ms, then:
   - If `config.json` changed → re-read and send `TeamUpdate`
   - If `inboxes/*.json` changed → re-read and send `MessageUpdate`
   - If `tasks/*/*.json` changed → re-read and send `TaskUpdate`
   - Ignore `.lock` files and any non-JSON files

Key details:
- Extract team name from path: `teams/{team_name}/config.json` → `team_name`
- Extract agent name from inbox path: `teams/{team}/inboxes/{agent}.json` → `agent`
- Use `notify_debouncer_mini` for debouncing
- If a file read fails (e.g., partial write), log warning and skip (don't crash)

- [ ] **Step 2: Write `collector/mod.rs`**

```rust
pub mod file_watcher;
// pub mod hook_server;      // Task 5
// pub mod hooks_installer;  // Task 6
```

- [ ] **Step 3: Update `lib.rs`**

```rust
pub mod store;
pub mod collector;
```

- [ ] **Step 4: Write integration tests in `tests/file_watcher_test.rs`**

```rust
use std::fs;
use tempfile::TempDir;
use tokio::sync::mpsc;
use agenttrack::collector::file_watcher;
use agenttrack::store::event::Event;

#[tokio::test]
async fn discovers_existing_team_on_startup() {
    let tmp = TempDir::new().unwrap();
    let teams_dir = tmp.path().join("teams/test-team/inboxes");
    let tasks_dir = tmp.path().join("tasks/test-team");
    fs::create_dir_all(&teams_dir).unwrap();
    fs::create_dir_all(&tasks_dir).unwrap();

    // Write fixture config
    let config_json = fs::read_to_string("tests/fixtures/team_config.json").unwrap();
    fs::write(tmp.path().join("teams/test-team/config.json"), &config_json).unwrap();

    // Write fixture task
    let task_json = fs::read_to_string("tests/fixtures/task_1.json").unwrap();
    fs::write(tasks_dir.join("1.json"), &task_json).unwrap();

    let (tx, mut rx) = mpsc::channel(32);

    // Run file_watcher in background, let it do initial scan
    let watcher_path = tmp.path().to_path_buf();
    let handle = tokio::spawn(async move {
        file_watcher::run(watcher_path, tx).await;
    });

    // Should receive TeamUpdate from initial scan
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(2), rx.recv()
    ).await.unwrap().unwrap();
    assert!(matches!(event, Event::TeamUpdate { .. }));

    handle.abort();
}

#[tokio::test]
async fn detects_new_task_file_creation() {
    // Setup tmp dir with a team, start watcher, then create a new task file
    // Verify TaskUpdate event is received within 2 seconds
}

#[tokio::test]
async fn detects_inbox_modification() {
    // Setup tmp dir with a team + inbox, start watcher, then append to inbox
    // Verify MessageUpdate event is received
}
```

- [ ] **Step 5: Run tests**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test --test file_watcher_test`
Expected: All tests pass. (May need `tokio::time::timeout` to prevent hangs.)

- [ ] **Step 6: Commit**

```bash
git add agenttrack/src/collector/ agenttrack/tests/file_watcher_test.rs
git commit -m "feat: add file watcher collector for ~/.claude/ JSON files"
```

---

### Task 5: HookServer Collector

**Files:**
- Create: `agenttrack/src/collector/hook_server.rs`
- Create: `agenttrack/tests/hook_server_test.rs`

- [ ] **Step 1: Write `collector/hook_server.rs`**

Implement `pub async fn run(port: u16, event_tx: mpsc::Sender<Event>) -> Result<(), ...>`:
1. Create an axum `Router` with a single route: `POST /hook`
2. The handler receives JSON body, deserializes into a `HookPayload` struct
3. `HookPayload` is lenient: uses `#[serde(default)]` on all fields, captures unknown fields with `#[serde(flatten)] pub extra: HashMap<String, serde_json::Value>`
4. Convert `HookPayload` into a `ToolEvent`:
   - `tool_name` from payload
   - `input_summary`: extract meaningful info from input (e.g., `file_path` for Read, `pattern` for Grep, `command` for Bash)
   - `session_id` from payload
   - `agent_name`: initially "unknown" (correlation happens in the store)
   - `timestamp`: `Utc::now()`
   - `duration`: from `duration_ms` field
5. Send `Event::ToolCall(tool_event)` to channel
6. Return 200 OK

Port fallback: try `port`, if bind fails try `port+1` through `port+9`. Return the actual bound port.

```rust
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
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}
```

- [ ] **Step 2: Add `fn summarize_input(tool_name: &str, input: &serde_json::Value) -> String`**

Extracts human-readable summary from tool input:
- `Read` → `input.file_path` (just the filename, not full path)
- `Edit` → `input.file_path` + line range if available
- `Bash` → first 80 chars of `input.command`
- `Grep` → `input.pattern` + `input.path`
- `Write` → `input.file_path`
- `Agent` → `input.description` or "subagent"
- Default → tool_name

- [ ] **Step 3: Update `collector/mod.rs`**

```rust
pub mod file_watcher;
pub mod hook_server;
```

- [ ] **Step 4: Write HTTP tests in `tests/hook_server_test.rs`**

```rust
use axum::http::StatusCode;
use tokio::sync::mpsc;
use agenttrack::collector::hook_server;
use agenttrack::store::event::Event;

#[tokio::test]
async fn accepts_valid_hook_payload() {
    let (tx, mut rx) = mpsc::channel(16);
    let port = 18901; // Use high port to avoid conflicts

    let server_handle = tokio::spawn(hook_server::run(port, tx));

    // Give server time to bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client.post(format!("http://localhost:{port}/hook"))
        .json(&serde_json::json!({
            "session_id": "abc-123",
            "tool_name": "Read",
            "input": {"file_path": "/tmp/test.rs"},
            "duration_ms": 42
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(1), rx.recv()
    ).await.unwrap().unwrap();

    match event {
        Event::ToolCall(te) => {
            assert_eq!(te.tool_name, "Read");
            assert_eq!(te.session_id, "abc-123");
            assert!(te.input_summary.contains("test.rs"));
        },
        _ => panic!("Expected ToolCall event"),
    }

    server_handle.abort();
}

#[tokio::test]
async fn handles_unknown_fields_gracefully() {
    // Send payload with extra fields not in our struct
    // Verify it still deserializes and produces a ToolCall event
}

#[tokio::test]
async fn handles_empty_payload() {
    // Send {} (empty JSON), verify server returns 200 and produces
    // a ToolEvent with default/empty values (doesn't crash)
}
```

Add `reqwest = { version = "0.12", features = ["json"] }` to `[dev-dependencies]` in Cargo.toml.

- [ ] **Step 5: Run tests**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test --test hook_server_test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add agenttrack/src/collector/hook_server.rs agenttrack/tests/hook_server_test.rs agenttrack/Cargo.toml
git commit -m "feat: add hook server collector (axum POST /hook endpoint)"
```

---

### Task 6: Hooks Installer

**Files:**
- Create: `agenttrack/src/collector/hooks_installer.rs`
- Create: `agenttrack/tests/hooks_installer_test.rs`

- [ ] **Step 1: Write `collector/hooks_installer.rs`**

Two public functions:

`pub fn install_hooks(claude_home: &Path, hook_port: u16) -> Result<(), String>`:
1. Read `claude_home/settings.json` (or create `{"hooks":{}}` if doesn't exist)
2. Backup to `claude_home/settings.json.agenttrack-backup`
3. Parse as `serde_json::Value`
4. Navigate to `hooks.PostToolUse` array (create path if missing)
5. Check if our hook entry already exists (match by command containing "agenttrack" or our port)
6. If not present, append our entry: `{"matcher":"*","hooks":[{"type":"command","command":"curl -s -X POST http://localhost:{port}/hook -d @-"}]}`
7. Serialize back with pretty-printing, write to `settings.json`

`pub fn uninstall_hooks(claude_home: &Path) -> Result<(), String>`:
1. Read `settings.json`
2. Remove entries from `hooks.PostToolUse` whose command contains `localhost:7890/hook` (or any port in 7890-7899)
3. If `PostToolUse` array becomes empty, remove the key
4. Write back

- [ ] **Step 2: Write tests in `tests/hooks_installer_test.rs`**

```rust
use tempfile::TempDir;
use std::fs;
use agenttrack::collector::hooks_installer;

#[test]
fn install_into_empty_settings() {
    let tmp = TempDir::new().unwrap();
    // No settings.json exists yet
    hooks_installer::install_hooks(tmp.path(), 7890).unwrap();

    let content = fs::read_to_string(tmp.path().join("settings.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    let hooks = &val["hooks"]["PostToolUse"];
    assert!(hooks.is_array());
    assert_eq!(hooks.as_array().unwrap().len(), 1);
    assert!(hooks[0]["hooks"][0]["command"].as_str().unwrap().contains("7890"));
}

#[test]
fn install_preserves_existing_hooks() {
    let tmp = TempDir::new().unwrap();
    // Write settings.json with an existing PreToolUse hook
    let existing = serde_json::json!({
        "hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "echo test"}]}]
        },
        "env": {"PATH": "/usr/bin"}
    });
    fs::write(tmp.path().join("settings.json"), serde_json::to_string_pretty(&existing).unwrap()).unwrap();

    hooks_installer::install_hooks(tmp.path(), 7890).unwrap();

    let content = fs::read_to_string(tmp.path().join("settings.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    // Existing hook preserved
    assert!(val["hooks"]["PreToolUse"].is_array());
    // New hook added
    assert!(val["hooks"]["PostToolUse"].is_array());
    // Env preserved
    assert_eq!(val["env"]["PATH"].as_str().unwrap(), "/usr/bin");
}

#[test]
fn install_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    hooks_installer::install_hooks(tmp.path(), 7890).unwrap();
    hooks_installer::install_hooks(tmp.path(), 7890).unwrap(); // second call

    let content = fs::read_to_string(tmp.path().join("settings.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    // Should still have exactly 1 entry, not 2
    assert_eq!(val["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
}

#[test]
fn install_creates_backup() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("settings.json"), "{}").unwrap();
    hooks_installer::install_hooks(tmp.path(), 7890).unwrap();
    assert!(tmp.path().join("settings.json.agenttrack-backup").exists());
}

#[test]
fn uninstall_removes_hook() {
    let tmp = TempDir::new().unwrap();
    hooks_installer::install_hooks(tmp.path(), 7890).unwrap();
    hooks_installer::uninstall_hooks(tmp.path()).unwrap();

    let content = fs::read_to_string(tmp.path().join("settings.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    // PostToolUse should be gone or empty
    assert!(val["hooks"]["PostToolUse"].is_null() || val["hooks"]["PostToolUse"].as_array().unwrap().is_empty());
}
```

- [ ] **Step 3: Update `collector/mod.rs`**

```rust
pub mod file_watcher;
pub mod hook_server;
pub mod hooks_installer;
```

- [ ] **Step 4: Run tests**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test --test hooks_installer_test`
Expected: All 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add agenttrack/src/collector/hooks_installer.rs agenttrack/tests/hooks_installer_test.rs
git commit -m "feat: add hooks installer for Claude Code settings.json"
```

---

### Task 7: TUI Theme + App State

**Files:**
- Create: `agenttrack/src/tui/mod.rs`
- Create: `agenttrack/src/tui/theme.rs`
- Create: `agenttrack/src/tui/app_state.rs`

- [ ] **Step 1: Write `tui/theme.rs`**

Define color constants and status symbols:

```rust
use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub active: Style,        // Green
    pub idle: Style,          // Blue
    pub shutdown: Style,      // Gray
    pub unknown: Style,       // DarkGray
    pub completed: Style,     // Green
    pub in_progress: Style,   // Yellow
    pub pending: Style,       // White
    pub blocked: Style,       // Red
    pub border: Style,        // Gray border
    pub selected: Style,      // Highlighted row
    pub header: Style,        // Bold header text
    pub title: Style,         // Panel title
}

impl Theme {
    pub fn dark() -> Self { /* dark theme colors */ }
}

pub fn status_symbol(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active => "●",
        AgentStatus::Idle => "○",
        AgentStatus::Shutdown => "✕",
        AgentStatus::Unknown => "?",
    }
}

pub fn task_status_symbol(status: &str) -> &'static str {
    match status {
        "completed" => "✓",
        "in_progress" => "●",
        "pending" => "○",
        _ => "?",
    }
}
```

- [ ] **Step 2: Write `tui/app_state.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Agents,
    Tasks,
    Activity,
    Messages,
}

pub struct AppState {
    pub active_panel: Panel,
    pub selected_agent_index: usize,
    pub scroll_offsets: HashMap<Panel, usize>,
    pub selected_rows: HashMap<Panel, usize>,
    pub show_detail: bool,
    pub search_query: Option<String>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self { /* defaults */ }
    pub fn next_panel(&mut self) { /* Tab cycling */ }
    pub fn select_panel(&mut self, panel: Panel) { /* Direct jump */ }
    pub fn next_agent(&mut self, max: usize) { /* →  key */ }
    pub fn prev_agent(&mut self, max: usize) { /* ← key */ }
    pub fn scroll_up(&mut self) { /* j key */ }
    pub fn scroll_down(&mut self, max: usize) { /* k key */ }
}
```

- [ ] **Step 3: Write `tui/mod.rs`** (skeleton)

```rust
pub mod theme;
pub mod app_state;
// pub mod layout;          // Task 8
// pub mod top_bar;         // Task 8
// pub mod agents_panel;    // Task 8
// pub mod tasks_panel;     // Task 8
// pub mod activity_panel;  // Task 8
// pub mod messages_panel;  // Task 8
```

- [ ] **Step 4: Update `lib.rs`**

```rust
pub mod store;
pub mod collector;
pub mod tui;
```

- [ ] **Step 5: Verify compilation**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo build`
Expected: Compiles. No tests needed for pure UI state — tested indirectly in Task 9.

- [ ] **Step 6: Commit**

```bash
git add agenttrack/src/tui/
git commit -m "feat: add TUI theme (dark mode) and app state management"
```

---

### Task 8: TUI Panel Widgets + Layout

**Files:**
- Create: `agenttrack/src/tui/layout.rs`
- Create: `agenttrack/src/tui/top_bar.rs`
- Create: `agenttrack/src/tui/agents_panel.rs`
- Create: `agenttrack/src/tui/tasks_panel.rs`
- Create: `agenttrack/src/tui/activity_panel.rs`
- Create: `agenttrack/src/tui/messages_panel.rs`

Each widget is a function `pub fn render(frame: &mut Frame, area: Rect, snapshot: &TeamSnapshot, app: &AppState)`. They do not own state — they render from the snapshot.

- [ ] **Step 1: Write `tui/layout.rs`**

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Returns (top_bar, agents, tasks, activity, messages, help_bar)
pub fn build_layout(area: Rect) -> LayoutAreas {
    // Vertical split:
    //   top_bar:    3 lines
    //   upper_half: 40% (split horizontally into agents | tasks)
    //   activity:   30%
    //   messages:   remaining
    //   help_bar:   1 line
}
```

- [ ] **Step 2: Write `tui/top_bar.rs`**

Renders: `AgentTrack ─ team: {name} ─ {n} agents ─ {completed}/{total} tasks ─ {events} events`

Uses `Paragraph` widget with styled spans.

- [ ] **Step 3: Write `tui/agents_panel.rs`**

Renders a `Table` widget with columns: NAME, MODEL, STATUS.

- Highlight the selected row (from `app.selected_agent_index`)
- Show status symbol + color: `●active` in green, `○idle` in blue, `✕shutdown` in gray
- Truncate model name: `claude-opus-4-6` → `opus`

- [ ] **Step 4: Write `tui/tasks_panel.rs`**

Renders a `Table` widget with columns: ID, STATUS, SUBJECT.

- Status symbol + color: `✓` green, `●` yellow, `○` white, `⊘blocked` red
- If task has `blocked_by` entries, show `(by #N)` suffix
- Highlight row matching `app.selected_rows[Panel::Tasks]`

- [ ] **Step 5: Write `tui/activity_panel.rs`**

Renders a `List` widget showing tool events for the selected agent.

- Filter `tool_events` by `agent_name == selected_agent.name` (or show all if "unknown")
- Format: `HH:MM:SS  {tool_name}  {input_summary}`
- Color-code tool names: Read=blue, Edit=yellow, Bash=green, Grep=magenta, Write=cyan
- Panel title: `Live Activity ({agent_name})`
- If no hook data: show `"Enable hooks for live activity: agenttrack hooks install"`

- [ ] **Step 6: Write `tui/messages_panel.rs`**

Renders a `List` widget showing messages.

- Format: `HH:MM:SS  {from} → {to}: "{summary}"`
- Skip idle_notification and internal protocol messages
- Most recent messages at bottom (auto-scroll)

- [ ] **Step 7: Update `tui/mod.rs`**

Uncomment all module declarations. Add the main render function:

```rust
pub fn render(frame: &mut Frame, snapshot: &StoreSnapshot, app: &AppState) {
    let areas = layout::build_layout(frame.area());
    let team = snapshot.teams.first(); // Show first team (or selected)
    if let Some(team) = team {
        top_bar::render(frame, areas.top_bar, team);
        agents_panel::render(frame, areas.agents, team, app);
        tasks_panel::render(frame, areas.tasks, team, app);
        activity_panel::render(frame, areas.activity, team, app);
        messages_panel::render(frame, areas.messages, team, app);
    } else {
        // Render "No teams found" centered message
    }
    // Render help bar at bottom
}
```

- [ ] **Step 8: Verify compilation**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo build`
Expected: Compiles. Widget rendering will be tested visually in Task 9.

- [ ] **Step 9: Commit**

```bash
git add agenttrack/src/tui/
git commit -m "feat: add TUI layout and all panel widgets (agents, tasks, activity, messages)"
```

---

### Task 9: TUI App Loop + Input Handling

**Files:**
- Modify: `agenttrack/src/tui/mod.rs` — add `run_tui` function
- Modify: `agenttrack/src/main.rs` — wire everything together

This is where the TUI comes alive: the main event loop reads key presses, updates app state, and re-renders with latest snapshot data.

- [ ] **Step 1: Write `run_tui` in `tui/mod.rs`**

```rust
pub async fn run_tui(
    mut snapshot_rx: watch::Receiver<StoreSnapshot>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Enable raw mode, enter alternate screen
    // 2. Create Terminal with CrosstermBackend
    // 3. Loop:
    //    a. Check for new snapshot (snapshot_rx.changed())
    //    b. Poll for keyboard input (crossterm::event::poll with 100ms timeout)
    //    c. Handle input → update AppState
    //    d. Render frame with latest snapshot + app state
    // 4. On quit: restore terminal
}
```

Key input handling:
- `q` / `Ctrl+C` → set `should_quit = true`
- `j` / `Down` → scroll down in active panel
- `k` / `Up` → scroll up in active panel
- `Left` → prev agent
- `Right` → next agent
- `Tab` → next panel
- `1`-`4` → jump to panel
- `Enter` → toggle detail view
- `w` → open web UI in browser (use `open::that` crate)
- `?` → toggle help overlay

- [ ] **Step 2: Wire everything in `main.rs`**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "agenttrack", version, about = "Real-time observability for Claude Code agent teams")]
struct Cli {
    #[arg(long, help = "Monitor a specific team")]
    team: Option<String>,

    #[arg(long, help = "Also start web UI")]
    web: bool,

    #[arg(long, help = "Web UI only, no TUI")]
    web_only: bool,

    #[arg(long, default_value = "7891", help = "Web UI port")]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
}

#[derive(clap::Subcommand)]
enum HooksAction {
    Install,
    Uninstall,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle subcommands
    match &cli.command {
        Some(Commands::Hooks { action }) => {
            let claude_home = dirs::home_dir().unwrap().join(".claude");
            match action {
                HooksAction::Install => {
                    collector::hooks_installer::install_hooks(&claude_home, 7890)?;
                    println!("Hooks installed successfully.");
                }
                HooksAction::Uninstall => {
                    collector::hooks_installer::uninstall_hooks(&claude_home)?;
                    println!("Hooks removed.");
                }
            }
            return Ok(());
        }
        None => {}
    }

    // Main monitoring mode
    let claude_home = dirs::home_dir().unwrap().join(".claude");
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(256);
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(StoreSnapshot::default());

    // Start store processor
    tokio::spawn(store::state::Store::process_events(event_rx, snapshot_tx));

    // Start collectors
    tokio::spawn(collector::file_watcher::run(claude_home.clone(), event_tx.clone()));
    tokio::spawn(collector::hook_server::run(7890, event_tx.clone()));

    // Start web if requested
    if cli.web || cli.web_only {
        let web_rx = snapshot_rx.clone();
        tokio::spawn(web::run(cli.port, web_rx));
    }

    // Start TUI (or wait if web-only)
    if cli.web_only {
        println!("Web UI running at http://localhost:{}", cli.port);
        tokio::signal::ctrl_c().await?;
    } else {
        tui::run_tui(snapshot_rx).await?;
    }

    Ok(())
}
```

- [ ] **Step 3: Manual test — run with no active teams**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo run`
Expected: TUI launches, shows "No teams found. Start an agent team in Claude Code." Press `q` to quit cleanly.

- [ ] **Step 4: Manual test — run with fixture data**

```bash
# Create fake team data
mkdir -p ~/.claude/teams/test-demo/inboxes ~/.claude/tasks/test-demo
cp tests/fixtures/team_config.json ~/.claude/teams/test-demo/config.json
cp tests/fixtures/task_1.json ~/.claude/tasks/test-demo/1.json
cp tests/fixtures/task_2.json ~/.claude/tasks/test-demo/2.json
cp tests/fixtures/inbox_team_lead.json ~/.claude/teams/test-demo/inboxes/team-lead.json
cp tests/fixtures/inbox_brainstormer.json ~/.claude/teams/test-demo/inboxes/brainstormer.json

cargo run
# Should show 2 agents, 2 tasks, messages in the TUI
# Test: j/k navigation, ←/→ agent switching, Tab panel switching
# Press q to quit

# Cleanup
rm -rf ~/.claude/teams/test-demo ~/.claude/tasks/test-demo
```

- [ ] **Step 5: Commit**

```bash
git add agenttrack/src/
git commit -m "feat: wire TUI app loop with input handling and CLI entry point"
```

---

### Task 10: Web UI (MVP — SSE Dashboard)

**Files:**
- Create: `agenttrack/src/web/mod.rs`
- Create: `agenttrack/src/web/static/index.html`
- Create: `agenttrack/src/web/static/app.js`
- Create: `agenttrack/src/web/static/style.css`

- [ ] **Step 1: Write `web/mod.rs`**

```rust
use axum::{Router, routing::get, response::{Html, sse::{Event, Sse}}, extract::State};
use rust_embed::Embed;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use std::convert::Infallible;

#[derive(Embed)]
#[folder = "src/web/static/"]
struct StaticAssets;

pub async fn run(port: u16, snapshot_rx: watch::Receiver<StoreSnapshot>) {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/sse", get(sse_handler))
        .route("/static/{*path}", get(static_handler))
        .with_state(snapshot_rx);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Web UI at http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}
```

SSE handler: converts `watch::Receiver` changes into SSE events. Each event is the full `StoreSnapshot` serialized to JSON. Throttle to max 2 events/second to avoid flooding the browser.

Static handler: serves files from `StaticAssets` (embedded at compile time).

- [ ] **Step 2: Write `web/static/index.html`**

A single-page dashboard with 4 sections:
- **Header**: "AgentTrack" + team name + agent count
- **Agents Table**: Name, Model, Status (with colored dot)
- **Tasks Table**: ID, Status, Subject, Blocked By
- **Messages Feed**: Scrolling list of messages
- **Activity Feed**: Scrolling list of tool events

Dark theme, monospace font, minimal CSS.

- [ ] **Step 3: Write `web/static/app.js`**

```javascript
const eventSource = new EventSource('/api/sse');

eventSource.onmessage = (event) => {
    const snapshot = JSON.parse(event.data);
    if (snapshot.teams.length > 0) {
        renderTeam(snapshot.teams[0]);
    }
};

function renderTeam(team) {
    renderAgents(team.agents);
    renderTasks(team.tasks);
    renderMessages(team.messages);
    renderActivity(team.tool_events);
    renderHeader(team);
}
// ... DOM update functions for each section
```

- [ ] **Step 4: Write `web/static/style.css`**

Dark theme: `#0d1117` background (GitHub dark), `#c9d1d9` text, `#238636` green for active, `#1f6feb` blue for idle, `#8b949e` gray for shutdown. Monospace font. Responsive layout.

- [ ] **Step 5: Update `lib.rs`**

```rust
pub mod store;
pub mod collector;
pub mod tui;
pub mod web;
```

- [ ] **Step 6: Manual test**

```bash
# Setup fake data as in Task 9
cargo run -- --web-only
# Open http://localhost:7891 in browser
# Should see agents, tasks, messages
# Modify a fixture file → verify SSE updates the page within 1 second
```

- [ ] **Step 7: Commit**

```bash
git add agenttrack/src/web/
git commit -m "feat: add MVP web UI with SSE-powered real-time dashboard"
```

---

### Task 11: Config File + README + Polish

**Files:**
- Create: `agenttrack/src/config.rs`
- Create: `agenttrack/README.md`
- Create: `agenttrack/Makefile`
- Modify: `agenttrack/src/main.rs` — load config

- [ ] **Step 1: Write `config.rs`**

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default)]
    pub enabled: bool,
}

fn default_web_port() -> u16 { 7891 }

// ... similar for HooksConfig, UiConfig

impl Config {
    pub fn load() -> Self {
        let path = dirs::home_dir()
            .map(|h| h.join(".agenttrack/config.toml"));
        match path {
            Some(p) if p.exists() => {
                let content = std::fs::read_to_string(&p).unwrap_or_default();
                toml::from_str(&content).unwrap_or_default()
            }
            _ => Config::default(),
        }
    }
}
```

- [ ] **Step 2: Update `main.rs` to load config**

Load config at startup, use config values as defaults for CLI args (CLI args override config).

- [ ] **Step 3: Write README.md**

Include: what it is, screenshot placeholder, installation (`cargo install agenttrack`), usage examples, hook setup, configuration, project status badge, contributing link.

- [ ] **Step 4: Write Makefile**

```makefile
.PHONY: build test run clean release

build:
	cargo build

test:
	cargo test

run:
	cargo run

release:
	cargo build --release
	@ls -lh target/release/agenttrack
	@echo "Binary size: $$(du -h target/release/agenttrack | cut -f1)"

clean:
	cargo clean
```

- [ ] **Step 5: Update `lib.rs`**

```rust
pub mod config;
pub mod store;
pub mod collector;
pub mod tui;
pub mod web;
```

- [ ] **Step 6: Full test suite**

Run: `cd /Users/jerry/Documents/Clipal/agenttrack && cargo test`
Expected: All tests pass.

- [ ] **Step 7: Release build size check**

Run: `make release`
Expected: Binary size <5MB (stripped release build).

- [ ] **Step 8: Commit**

```bash
git add agenttrack/
git commit -m "feat: add config loading, README, Makefile — AgentTrack v0.1 complete"
```

---

## Task Dependency Graph

```
Task 1 (scaffold)
   │
   ▼
Task 2 (models + fixtures)
   │
   ▼
Task 3 (state store)
   │
   ├──────────────────┐
   ▼                  ▼
Task 4 (file watcher) Task 5 (hook server)
   │                  │
   │                  ▼
   │            Task 6 (hooks installer)
   │                  │
   ├──────────────────┘
   ▼
Task 7 (TUI theme + state)
   │
   ▼
Task 8 (TUI widgets)
   │
   ▼
Task 9 (TUI app loop + main.rs)
   │
   ├──────────┐
   ▼          ▼
Task 10    Task 11
(web UI)   (config + README)
```

**Parallelizable**: Tasks 4, 5, 6 can run in parallel after Task 3. Tasks 10, 11 can run in parallel after Task 9.

**Total**: 11 tasks, estimated ~60-80 steps.
