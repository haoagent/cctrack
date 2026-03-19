# AgentTrack Design Spec

**Date**: 2026-03-20
**Status**: Reviewed & Revised (v2 — Rust rewrite)
**Author**: Jerry + Claude

---

## 1. Problem Statement

When using Claude Code's agent teams (TeamCreate, SendMessage, TaskCreate), developers have no real-time visibility into:
- What each agent is currently doing (reading files, searching, editing)
- Task progress and dependency bottlenecks
- Inter-agent message flow and communication patterns
- Activity metrics per agent and per team

Current workarounds (`/tasks` command, reading team config JSON, manual message passing) are fragmented and provide only point-in-time snapshots, not continuous observability.

## 2. Product Overview

**AgentTrack** is an open-source, real-time observability dashboard for Claude Code agent teams.

- **Target user**: Individual developers running Claude Code agent teams
- **Core value**: See all agent activity, task progress, message flow, and metrics in one view
- **Form factor**: TUI (primary) + Web UI (optional, visually rich)
- **Tech stack**: Rust + Ratatui (TUI) + axum (Web)
- **Distribution**: Single static binary (~3-5MB), zero runtime dependencies
- **Usage**: Runs as an independent process alongside Claude Code (separate terminal or browser)

### Why Rust?

In the AI coding era, Rust's traditional drawback (slow development speed) is neutralized — AI writes Rust nearly as fast as Go/TypeScript. What remains are pure advantages for a monitoring tool:
- ~3-5MB binary vs ~15MB (Go) — faster download, smaller footprint
- ~5ms startup vs ~30ms — feels instant
- ~5MB memory vs ~20MB — minimal overhead alongside Claude Code
- Strong open-source CLI credibility (ripgrep, fd, bat, delta, gitui, zellij)

## 3. Architecture

```
┌──────────────────────────────────────────────────────┐
│                     AgentTrack                        │
│                                                       │
│  ┌──────────────┐    ┌────────────────────────────┐  │
│  │  Collector    │    │   State Store (in-memory)   │  │
│  │               │    │                             │  │
│  │ • FileWatcher │───>│ • Teams[]                   │  │
│  │   (notify-rs) │    │ • Agents[]                  │  │
│  │               │    │ • Tasks[]                   │  │
│  │ • HookServer  │───>│ • Messages[]                │  │
│  │   (axum)      │    │ • ToolEvents[]              │  │
│  │               │    │ • Metrics{}                  │  │
│  │ • LogParser   │───>│                             │  │
│  │   (optional)  │    └──────────┬──────────┬──────┘  │
│  └──────────────┘               │          │         │
│                        ┌────────┘          └───────┐ │
│                        ▼                           ▼ │
│               ┌──────────────┐          ┌──────────┐ │
│               │   TUI View    │          │ Web View  │ │
│               │  (Ratatui)    │          │ (axum +   │ │
│               │               │          │  SSE)     │ │
│               └──────────────┘          └──────────┘ │
└──────────────────────────────────────────────────────┘
```

Three layers:
1. **Collector** — Parallel data ingestion from three sources (tokio tasks)
2. **State Store** — Unified in-memory model, event-driven updates via channels
3. **View** — TUI and Web both subscribe to the same State Store

## 4. Data Collection Layer

### 4.1 FileWatcher (core, zero config)

Uses Rust's `notify` crate to watch Claude Code's local JSON files:

| File Path | Event | Extracted Data |
|-----------|-------|---------------|
| `~/.claude/teams/*/config.json` | CREATE/MODIFY | Team name, members, models, roles, timestamps |
| `~/.claude/teams/*/inboxes/*.json` | MODIFY | New messages, sender, timestamp, read status, idle notifications |
| `~/.claude/tasks/*/*.json` | CREATE/MODIFY | Task ID, status changes, subject, dependencies (blocks/blockedBy) |
| `~/.claude/tasks/*/.lock` | CREATE/DELETE | Task list modification in progress |

**Polling strategy**: notify event-driven + 500ms debounce (via `tokio::time::sleep`) to avoid excessive JSON parsing during rapid file updates.

**Auto-discovery**: On startup, scan `~/.claude/teams/` for existing team directories. Watch for new team creation dynamically.

### 4.2 HookServer (optional, enhances activity tracking)

A localhost HTTP server (axum) that receives Claude Code hook events via the PreToolUse/PostToolUse hook system.

**Hook configuration** (merged into `~/.claude/settings.json`):
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "curl -s -X POST http://localhost:7890/hook -d @-"
          }
        ]
      }
    ]
  }
}
```

**Hook event payload** (piped to stdin as JSON):
```json
{
  "session_id": "fff799d7-662d-43b3-9939-f1054a54d0ab",
  "tool_name": "Read",
  "input": { "file_path": "/Users/jerry/src/api/routes.ts" },
  "output": "...",
  "duration_ms": 42
}
```

**Agent correlation strategy**: Hook events include `session_id`. The team config.json stores `leadSessionId` for the team lead. For teammates with `backendType: "in-process"`, events come through the lead's session. For worktree-isolated agents, each has a distinct `cwd` in config.json. AgentTrack correlates events by:
1. Matching `session_id` to known sessions from config
2. Using `cwd` (working directory) from tool input paths to disambiguate in-process agents
3. Falling back to "unattributed" if correlation fails (still shows in a combined activity stream)

**Note**: Exact hook payload format needs validation against a live Claude Code session. The schema above is based on the PreToolUse hook pattern observed in settings.json. AgentTrack should handle unknown fields gracefully via `serde(flatten)` / `serde(deny_unknown_fields = false)`.

**Auto-install merge strategy**:
1. Read existing `~/.claude/settings.json`
2. Back up to `~/.claude/settings.json.agenttrack-backup`
3. If `hooks.PostToolUse` array exists, append AgentTrack's entry (don't replace)
4. If `hooks.PostToolUse` doesn't exist, create it
5. Preserve all other top-level keys (env, permissions, etc.)
6. Validate resulting JSON before writing

### 4.3 LogParser (passive supplement)

Parses `~/.claude/debug/*.txt` debug logs for:
- Session start/stop timestamps
- Plugin loading events
- Error conditions

Not a core dependency — used only for supplemental context.

## 5. Data Model

Models aligned with actual Claude Code JSON file formats.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Matches ~/.claude/teams/<name>/config.json
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamConfig {
    pub name: String,
    pub description: String,
    pub created_at: i64,           // Unix timestamp (milliseconds)
    pub lead_agent_id: String,     // e.g., "team-lead@team-name"
    pub lead_session_id: String,   // UUID of lead's session
    pub members: Vec<MemberConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberConfig {
    pub agent_id: String,          // e.g., "brainstormer@team-name"
    pub name: String,              // e.g., "brainstormer" (used for messaging)
    pub agent_type: String,        // e.g., "general-purpose", "team-leader"
    pub model: String,             // e.g., "claude-opus-4-6"
    #[serde(default)]
    pub color: String,             // Visual identifier (e.g., "blue")
    pub cwd: String,               // Working directory
    #[serde(default)]
    pub backend_type: String,      // "in-process" or other
    #[serde(default)]
    pub tmux_pane_id: String,      // Process/pane reference
}

/// Runtime agent state (enriched from config + inbox activity)
#[derive(Debug, Clone)]
pub struct Agent {
    pub config: MemberConfig,
    pub status: AgentStatus,       // Derived, NOT from config.json directly
    pub last_seen: Option<DateTime<Utc>>,
}

/// Agent status is INFERRED, not stored in any file
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Active,    // Recent tool events or messages
    Idle,      // idle_notification in inbox
    Shutdown,  // shutdown_response approved
    Unknown,   // No recent activity
}
// Inference: parse inbox for idle_notification messages, track time since
// last activity. If idle_notification received and no subsequent activity
// within 5s, mark as Idle. If shutdown_response with approve:true, mark
// as Shutdown.

/// Matches ~/.claude/tasks/<team>/<id>.json (one file per task)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFile {
    pub id: String,                // Numeric string: "1", "2", etc.
    pub subject: String,           // Agent name / role
    pub description: String,       // Task instructions (may be truncated)
    pub status: String,            // "pending" | "in_progress" | "completed"
    #[serde(default)]
    pub blocks: Vec<String>,       // Task IDs this task blocks
    #[serde(default)]
    pub blocked_by: Vec<String>,   // Task IDs blocking this task
    #[serde(default)]
    pub metadata: Option<TaskMetadata>,
}
// NOTE: "Owner" is not stored in task JSON. The "subject" field typically
// contains the agent name.

#[derive(Debug, Clone, Deserialize)]
pub struct TaskMetadata {
    #[serde(rename = "_internal", default)]
    pub internal: bool,
}

/// Derived from ~/.claude/teams/<name>/inboxes/<agent>.json
/// The inbox file is a JSON array of these messages
#[derive(Debug, Clone, Deserialize)]
pub struct InboxMessage {
    pub from: String,              // Sender agent name
    // "to" is DERIVED from inbox filename, not in JSON
    pub text: Option<String>,      // Message content (field is "text", not "content")
    #[serde(default)]
    pub summary: Option<String>,   // 5-10 word preview
    pub timestamp: String,         // ISO 8601
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub color: Option<String>,     // Optional visual indicator
    // Protocol messages (idle, shutdown) detected by "type" field
    #[serde(rename = "type")]
    pub msg_type: Option<String>,  // "idle_notification", etc.
}

/// Enriched message with derived "to" field
#[derive(Debug, Clone)]
pub struct Message {
    pub from: String,
    pub to: String,                // Derived from inbox filename
    pub text: String,
    pub summary: String,
    pub timestamp: DateTime<Utc>,
    pub read: bool,
    pub color: Option<String>,
    pub msg_type: MessageType,
}

#[derive(Debug, Clone)]
pub enum MessageType {
    Normal,
    IdleNotification,
    ShutdownRequest,
    ShutdownResponse { approved: bool },
    Other(String),
}

#[derive(Debug, Clone)]
pub struct ToolEvent {
    pub session_id: String,        // From hook event, for agent correlation
    pub agent_name: String,        // Resolved via correlation (may be "unknown")
    pub tool_name: String,         // "Read", "Edit", "Bash", "Grep", etc.
    pub input_summary: String,     // Summarized input (file path, search query)
    pub timestamp: DateTime<Utc>,
    pub duration: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub active_agents: usize,
    pub completed_tasks: usize,
    pub total_tasks: usize,
    pub messages_count: usize,
    pub tool_events_count: usize,
    pub start_time: Option<DateTime<Utc>>,
    // NOTE: Token/cost estimation is NOT available from file watching or hooks.
    // Future: parse debug logs for token usage if the format is stable.
    // For MVP, show activity-based metrics only (tool calls, messages, tasks).
}
```

### 5.1 Concurrency Model

Collectors run as tokio tasks and send events through `tokio::sync::mpsc` channels:

```rust
#[derive(Debug)]
enum Event {
    TeamUpdate(TeamConfig),
    TaskUpdate(TaskFile),
    MessageUpdate { to: String, messages: Vec<InboxMessage> },
    ToolCall(ToolEvent),
}

// Single task processes all events and updates the store
// Views subscribe to store snapshots via tokio::sync::watch
let (event_tx, mut event_rx) = mpsc::channel::<Event>(256);
let (snapshot_tx, snapshot_rx) = watch::channel(StoreSnapshot::default());

tokio::spawn(store::process_events(event_rx, snapshot_tx));  // single writer
tokio::spawn(file_watcher::run(event_tx.clone()));            // producer
tokio::spawn(hook_server::run(event_tx.clone()));             // producer
// TUI reads from snapshot_rx.clone()
// Web SSE reads from snapshot_rx.clone()
```

This avoids locks — the store has a single writer task, and views receive immutable snapshots via `watch::Receiver`.

## 6. TUI Design

### 6.1 Layout

k9s-style multi-panel layout with vim keybindings:

```
┌─ AgentTrack ─ team: my-team ─ 4 agents ─ 12 events ──────────┐
│                                                                 │
│  ┌─ Agents ──────────────────────┬─ Tasks ─────────────────────┐│
│  │ NAME       MODEL    STATUS    │ ID  STATUS       SUBJECT    ││
│  │►brainstormer opus   ●active   │  1  ✓completed   brainstormer│
│  │ spec-review  opus   ○idle     │  2  ●in_progress spec-review││
│  │ plan-writer  opus   ●active   │  3  ○pending     —          ││
│  │ team-lead    sonnet ●active   │  4  ○pending     —          ││
│  │                               │  5  ⊘blocked     (by #2)    ││
│  └───────────────────────────────┴─────────────────────────────┘│
│                                                                 │
│  ┌─ Live Activity (brainstormer) ──────────────────────────────┐│
│  │ 16:28:12  Read   src/api/routes.ts                          ││
│  │ 16:28:14  Grep   "handlePayment" in src/**/*.ts             ││
│  │ 16:28:18  Edit   src/api/routes.ts:42-58                    ││
│  │ 16:28:22  Bash   npm test -- --grep payment                 ││
│  │ 16:28:25  Write  docs/specs/payment-redesign.md             ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  ┌─ Messages ──────────────────────────────────────────────────┐│
│  │ 16:24:00  team-lead → brainstormer: "Start brainstorming.." ││
│  │ 16:28:49  brainstormer → team-lead: "Status: DONE"          ││
│  │ 16:29:01  team-lead → spec-review: "Review this spec..."    ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  j/k:nav  ←/→:agent  Tab:panel  Enter:detail  q:quit  w:web   │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Panels

1. **Agents Panel** (top-left) — Member list with real-time status indicators (●active, ○idle, ✕shutdown). Shows model type.
2. **Tasks Panel** (top-right) — Task list with color-coded status. Shows subject, dependency arrows, and blocked indicators.
3. **Live Activity Panel** (middle) — Real-time tool call stream for the selected agent. Requires Hook data source. Shows timestamp, tool name, and summarized input.
4. **Messages Panel** (bottom) — Chronological message timeline showing direction (→), sender, recipient, and summary.

### 6.3 Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate up/down within current panel |
| `←/→` | Switch selected agent (Live Activity follows) |
| `Tab` | Cycle between panels |
| `1-4` | Jump directly to panel |
| `Enter` | Expand detail view (full message text, task description, tool call args) |
| `/` | Search/filter within current panel |
| `w` | Open Web UI in browser |
| `q` | Quit |
| `?` | Help overlay |

### 6.4 Top Bar

Real-time stats: team name, active agent count, completed/total tasks ratio, tool event count.

## 7. Web UI Design

Accessible via `w` key in TUI or `agenttrack --web`. Default: `http://localhost:7891`.

### 7.1 MVP Web UI (v0.1) — Functional First

The MVP Web UI prioritizes function over flash. Ship a working dashboard first, then add visual polish in v0.2.

- **Agent Status Table** — Real-time table of all agents with status, model, last activity
- **Task List** — Filterable task list with status badges and dependency indicators
- **Activity Stream** — Combined or per-agent tool call log (scrolling list)
- **Message Timeline** — Chronological message feed with sender/recipient/summary
- **Auto-refresh** via SSE (Server-Sent Events)

Tech: axum HTTP server + SSE + plain HTML/CSS/JS (no framework, no build step). Static files embedded via `include_str!` / `rust-embed`.

### 7.2 Enhanced Web UI (v0.2) — Visual Impact for Sharing

Once MVP is solid, layer on the visually striking features:

- **Network Topology View**: Agents as glowing nodes, messages as animated particle flow (D3.js). Node colors pulse with status: green=active, blue=idle, gray=shutdown. Click to focus.
- **Activity Waterfall**: Matrix-style code rain, one column per agent, color-coded by tool type (Read=blue, Edit=yellow, Bash=green, Grep=purple). Canvas API.
- **Share Mode**: One-click screenshot generation — dark background + stats overlay + team topology. Export PNG/SVG with AgentTrack watermark. 16:9 aspect ratio for Twitter/Discord.

### 7.3 Tech Stack

- axum HTTP server serves static files via `rust-embed`
- SSE (Server-Sent Events) for real-time push updates to browser
- v0.1: Plain HTML + CSS + vanilla JS
- v0.2: D3.js (topology), Canvas API (code rain), html2canvas (screenshots)
- No npm, no build step — all frontend assets embedded in binary at compile time

## 8. CLI Interface

```bash
# Auto-discover active teams and start monitoring
agenttrack

# Monitor specific team
agenttrack --team <team-name>

# Start with Web UI
agenttrack --web

# Web UI only (no TUI)
agenttrack --web-only --port 7891

# Install hooks into Claude Code settings
agenttrack hooks install

# Remove hooks
agenttrack hooks uninstall

# Show version
agenttrack --version
```

### Configuration File

`~/.agenttrack/config.toml` (optional, TOML is idiomatic for Rust CLI tools):
```toml
version = 1

[web]
port = 7891
enabled = false

[hooks]
auto_install = true
port = 7890

[ui]
theme = "dark"  # dark | light | matrix
```

## 9. Project Structure

```
agenttrack/
├── Cargo.toml
├── src/
│   ├── main.rs                # CLI entry point (clap)
│   ├── lib.rs                 # Library root
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── file_watcher.rs    # notify-based JSON file watcher
│   │   ├── hook_server.rs     # axum HTTP server for hook events
│   │   └── log_parser.rs      # Debug log parser (optional)
│   ├── store/
│   │   ├── mod.rs
│   │   ├── models.rs          # Data model types (serde structs)
│   │   └── state.rs           # In-memory state store + event processing
│   ├── tui/
│   │   ├── mod.rs             # Ratatui app loop
│   │   ├── agents.rs          # Agents panel widget
│   │   ├── tasks.rs           # Tasks panel widget
│   │   ├── activity.rs        # Live activity panel widget
│   │   ├── messages.rs        # Messages panel widget
│   │   └── theme.rs           # Color themes and styles
│   └── web/
│       ├── mod.rs             # axum router + SSE handler
│       └── static/            # Frontend assets (embedded at compile time)
│           ├── index.html
│           ├── app.js         # v0.1: vanilla JS dashboard
│           ├── topology.js    # v0.2: D3.js network topology
│           ├── rain.js        # v0.2: Canvas code rain effect
│           └── style.css
├── Makefile
├── LICENSE                    # MIT
└── README.md
```

## 10. Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | TUI framework (immediate-mode rendering) |
| `crossterm` | Terminal backend for ratatui |
| `tokio` | Async runtime (channels, tasks, timers) |
| `notify` | Cross-platform file system notifications |
| `axum` | HTTP server for hooks + web UI |
| `serde` + `serde_json` | JSON parsing for Claude Code files |
| `clap` | CLI argument parsing |
| `toml` | Config file parsing |
| `chrono` | Timestamp handling |
| `rust-embed` | Embed static web assets in binary |
| `dirs` | Resolve ~/.claude and ~/.agenttrack paths |
| `tracing` | Structured logging |

Frontend (vendored, embedded at compile time):
| Library | Purpose |
|---------|---------|
| D3.js (v0.2) | Network topology visualization + animations |
| html2canvas (v0.2) | Share screenshot generation |

## 11. MVP Scope

### In Scope (v0.1)
- FileWatcher collector (auto-discover teams, watch JSON changes)
- HookServer collector (receive PostToolUse events)
- In-memory State Store with channel-based event processing
- TUI: 4-panel layout (Agents, Tasks, Activity, Messages)
- TUI: vim keybindings + ←/→ agent switching
- Web UI: functional SSE-powered dashboard (tables + lists)
- CLI: auto-discover, --team, --web flags
- Hook auto-install with user confirmation
- Single binary distribution (cargo-dist / cross)

### v0.2 (after MVP)
- Web UI: Network topology with animated message flow (D3.js)
- Web UI: Activity waterfall / code rain (Canvas)
- Web UI: Share mode (screenshot export)
- Token/cost estimation (if debug log format proves stable)

### Out of Scope (future)
- Multi-user / team features
- Persistent storage / history
- Cloud sync
- Custom alerting rules
- Plugin system
- Integration with Langfuse/OpenTelemetry
- Windows support (macOS/Linux first)

## 12. Success Criteria

1. `agenttrack` starts in <50ms and shows active team status
2. File changes reflected in TUI within 1 second
3. Hook events displayed in Live Activity within 200ms
4. Total memory usage <10MB during monitoring
5. Binary size <5MB (release build, stripped)
6. Zero impact on Claude Code performance (read-only observation)

## 13. Error Handling & Graceful Degradation

| Scenario | Behavior |
|----------|----------|
| `~/.claude/teams/` doesn't exist | Show "No teams found. Start an agent team in Claude Code." and poll for directory creation |
| fsnotify fails on a directory | Fall back to 2-second polling interval; show warning in TUI status bar |
| Hook server port 7890 in use | Try ports 7890-7899; show actual port in TUI; update config |
| settings.json is malformed | Abort hook auto-install with clear error; don't modify the file |
| Team directory deleted while watching | Remove team from state; show "Team removed" notification |
| Hook event has unknown fields | Deserialize with `#[serde(flatten)]` to capture extras; log warning |
| Inbox JSON parse error | Skip malformed messages; show parse error count in status bar |
| Web UI port in use | Try next available port; print URL to terminal |

## 14. Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Claude Code changes internal file format | Version-detect + serde `#[serde(default)]` for missing fields |
| High-frequency file changes overwhelm watcher | 500ms debounce + batch processing |
| Hook configuration breaks Claude Code | Backup settings.json; validate JSON; provide `hooks uninstall` command |
| Debug log format is unstable | LogParser is optional, not core dependency |
| Large inbox files slow parsing | Incremental parsing (track file size, only parse new bytes) |
| Hook events can't be correlated to agents | Fallback "unattributed" stream; still useful as combined view |

## 15. Open Source Strategy

- **License**: MIT
- **Repo**: `github.com/<owner>/agenttrack`
- **Distribution**: GitHub Releases + Homebrew tap + `cargo install` + `cargo-binstall`
- **CI**: GitHub Actions with cross-compilation (x86_64 + aarch64, macOS + Linux)
- **Community**: GitHub Discussions for feature requests, Issues for bugs
- **Contribution guide**: CONTRIBUTING.md with dev setup instructions

## 16. Testing Strategy

| Layer | Approach |
|-------|----------|
| Store / Models | Unit tests with fixture JSON files (real Claude Code data) |
| FileWatcher | Integration tests using temp directories with sample JSON |
| HookServer | HTTP tests with mock payloads via `axum::test` |
| TUI Rendering | Snapshot tests using ratatui's `TestBackend` |
| Web SSE | Integration tests with reqwest SSE client |
| End-to-end | Manual testing against live Claude Code agent team session |
