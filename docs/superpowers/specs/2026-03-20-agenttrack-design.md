# AgentTrack Design Spec

**Date**: 2026-03-20
**Status**: Draft
**Author**: Jerry + Claude

---

## 1. Problem Statement

When using Claude Code's agent teams (TeamCreate, SendMessage, TaskCreate), developers have no real-time visibility into:
- What each agent is currently doing (reading files, searching, editing)
- Task progress and dependency bottlenecks
- Inter-agent message flow and communication patterns
- Cost/token consumption per agent and per team

Current workarounds (`/tasks` command, reading team config JSON, manual message passing) are fragmented and provide only point-in-time snapshots, not continuous observability.

## 2. Product Overview

**AgentTrack** is an open-source, real-time observability dashboard for Claude Code agent teams.

- **Target user**: Individual developers running Claude Code agent teams
- **Core value**: See all agent activity, task progress, message flow, and cost in one view
- **Form factor**: TUI (primary) + Web UI (optional, visually rich)
- **Distribution**: Single Go binary, zero dependencies

## 3. Architecture

```
┌──────────────────────────────────────────────────────┐
│                     AgentTrack                        │
│                                                       │
│  ┌──────────────┐    ┌────────────────────────────┐  │
│  │  Collector    │    │   State Store (in-memory)   │  │
│  │               │    │                             │  │
│  │ • FileWatcher │───>│ • Teams[]                   │  │
│  │   (fsnotify)  │    │ • Agents[]                  │  │
│  │               │    │ • Tasks[]                   │  │
│  │ • HookServer  │───>│ • Messages[]                │  │
│  │   (localhost)  │    │ • ToolEvents[]              │  │
│  │               │    │ • Metrics{}                  │  │
│  │ • LogParser   │───>│                             │  │
│  │   (optional)   │    └──────────┬──────────┬──────┘  │
│  └──────────────┘               │          │         │
│                        ┌────────┘          └───────┐ │
│                        ▼                           ▼ │
│               ┌──────────────┐          ┌──────────┐ │
│               │   TUI View    │          │ Web View  │ │
│               │  (BubbleTea)  │          │ (embedded │ │
│               │               │          │  server)  │ │
│               └──────────────┘          └──────────┘ │
└──────────────────────────────────────────────────────┘
```

Three layers:
1. **Collector** — Parallel data ingestion from three sources
2. **State Store** — Unified in-memory model, event-driven updates
3. **View** — TUI and Web both read from the same State Store

## 4. Data Collection Layer

### 4.1 FileWatcher (core, zero config)

Uses Go's `fsnotify` to watch Claude Code's local JSON files:

| File Path | Event | Extracted Data |
|-----------|-------|---------------|
| `~/.claude/teams/*/config.json` | CREATE/MODIFY | Team name, members, models, roles, timestamps |
| `~/.claude/teams/*/inboxes/*.json` | MODIFY | New messages, sender, timestamp, read status, idle notifications |
| `~/.claude/tasks/*/*.json` | CREATE/MODIFY | Task ID, status changes, owner, dependencies (blocks/blockedBy) |
| `~/.claude/tasks/*/.lock` | CREATE/DELETE | Task list modification in progress |

**Polling strategy**: fsnotify event-driven + 500ms debounce to avoid excessive JSON parsing during rapid file updates.

**Auto-discovery**: On startup, scan `~/.claude/teams/` for existing team directories. Watch for new team creation dynamically.

### 4.2 HookServer (optional, one-line config)

A localhost HTTP server that receives Claude Code PostToolUse hook events.

**Hook configuration** (added to `~/.claude/settings.json`):
```json
{
  "hooks": {
    "PostToolUse": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "curl -s -X POST http://localhost:7890/hook -d @-"
      }]
    }]
  }
}
```

**Event payload contains**: Tool name (Bash/Read/Edit/Grep/Write/Agent...), input parameters, output result, duration.

This enables real-time visibility into what each agent is doing: which files they're reading, what keywords they're searching, what edits they're making.

**Auto-install**: On first run, AgentTrack prompts the user to auto-inject the hook config. Backs up existing settings.json before modification.

### 4.3 LogParser (passive supplement)

Parses `~/.claude/debug/*.txt` debug logs for:
- Session start/stop timestamps
- Plugin loading events
- Error conditions

Not a core dependency — used only for supplemental context.

## 5. Data Model

```go
type Team struct {
    Name        string
    Description string
    CreatedAt   time.Time
    Agents      []Agent
}

type Agent struct {
    ID        string    // e.g., "brainstormer@team-name"
    Name      string    // e.g., "brainstormer"
    Model     string    // e.g., "claude-opus-4-6"
    Role      string    // e.g., "general-purpose"
    Status    string    // "active" | "idle" | "shutdown"
    LastSeen  time.Time
}

type Task struct {
    ID          string
    Description string
    Status      string   // "pending" | "in_progress" | "completed"
    Owner       string   // Agent name
    Blocks      []string // Task IDs this task blocks
    BlockedBy   []string // Task IDs blocking this task
}

type Message struct {
    From      string
    To        string
    Content   string
    Summary   string
    Timestamp time.Time
    Type      string // "message" | "idle_notification" | "shutdown_request" | etc.
    Read      bool
}

type ToolEvent struct {
    AgentName  string
    ToolName   string    // "Read", "Edit", "Bash", "Grep", etc.
    Input      string    // Summarized input (e.g., file path, search query)
    Timestamp  time.Time
    Duration   time.Duration
}

type Metrics struct {
    TotalTokensEstimate int64
    CostEstimateUSD     float64
    ActiveAgents        int
    CompletedTasks      int
    TotalTasks          int
    MessagesCount       int
    StartTime           time.Time
}
```

## 6. TUI Design

### 6.1 Layout

k9s-style multi-panel layout with vim keybindings:

```
┌─ AgentTrack ─ team: my-team ─ 4 agents ─ $2.34 ──────────────┐
│                                                                 │
│  ┌─ Agents ──────────────────────┬─ Tasks ─────────────────────┐│
│  │ NAME       MODEL    STATUS    │ ID  STATUS       OWNER      ││
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
2. **Tasks Panel** (top-right) — Task list with color-coded status. Shows owner, dependency arrows, and blocked indicators.
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

Real-time stats: team name, active agent count, completed/total tasks ratio, estimated cost.

## 7. Web UI Design

Accessible via `w` key in TUI or `agenttrack --web`. Default: `http://localhost:7891`.

### 7.1 Network Topology View (Main)

- Agents as glowing nodes, sized by token consumption
- Messages as animated particles flowing between nodes (similar to GitHub Globe)
- Node colors pulse with status: green=active, blue=idle, gray=shutdown
- Hover reveals agent detail tooltip
- Click node to focus and filter activity stream

### 7.2 Activity Waterfall (Matrix-style)

- One column per agent, tool calls stream downward like code rain
- File names and search keywords highlighted
- Color-coded by tool type (Read=blue, Edit=yellow, Bash=green, Grep=purple)

### 7.3 Cost Dashboard

- Real-time ticking cost counter (stock ticker style)
- Per-agent token consumption pie chart
- Cumulative and per-minute trend lines

### 7.4 Share Mode

- One-click screenshot generation: dark background + stats overlay + team topology
- Export as PNG/SVG with AgentTrack watermark
- 16:9 aspect ratio optimized for Twitter/Discord sharing

### 7.5 Tech Stack

- Go embedded HTTP server (`net/http`) serves static files
- SSE (Server-Sent Events) for real-time updates to browser
- D3.js for network topology and particle animations
- Canvas API for code rain effect
- No build step — plain HTML + JS + CSS served directly

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

`~/.agenttrack/config.yaml` (optional):
```yaml
web:
  port: 7891
  enabled: false
hooks:
  auto_install: true
  port: 7890
theme: dark  # dark | light | matrix
```

## 9. Project Structure

```
agenttrack/
├── cmd/agenttrack/          # CLI entry point
│   └── main.go
├── internal/
│   ├── collector/           # Data collection layer
│   │   ├── filewatcher.go   # fsnotify-based JSON file watcher
│   │   ├── hookserver.go    # localhost HTTP server for hook events
│   │   └── logparser.go     # Debug log parser (optional)
│   ├── store/               # State management
│   │   ├── store.go         # Unified in-memory state store
│   │   └── models.go        # Data model types
│   ├── tui/                 # BubbleTea TUI
│   │   ├── app.go           # Main TUI application (Elm architecture)
│   │   ├── agents.go        # Agents panel component
│   │   ├── tasks.go         # Tasks panel component
│   │   ├── activity.go      # Live activity panel component
│   │   ├── messages.go      # Messages panel component
│   │   └── styles.go        # Lip Gloss styles and themes
│   └── web/                 # Embedded Web Server
│       ├── server.go        # HTTP server + SSE handler
│       ├── sse.go           # Server-Sent Events implementation
│       └── static/          # Frontend assets (no build step)
│           ├── index.html
│           ├── topology.js  # D3.js network topology
│           ├── rain.js      # Canvas code rain effect
│           ├── share.js     # Screenshot/share functionality
│           └── style.css
├── go.mod
├── go.sum
├── Makefile
├── LICENSE                  # MIT
└── README.md
```

## 10. Key Dependencies

| Package | Purpose |
|---------|---------|
| `github.com/charmbracelet/bubbletea` | TUI framework (Elm architecture) |
| `github.com/charmbracelet/lipgloss` | TUI styling |
| `github.com/charmbracelet/bubbles` | TUI components (table, viewport, textinput) |
| `github.com/fsnotify/fsnotify` | File system event notifications |
| `gopkg.in/yaml.v3` | Config file parsing |

Frontend (vendored, no npm):
| Library | Purpose |
|---------|---------|
| D3.js | Network topology visualization + animations |
| html2canvas (or dom-to-image) | Share screenshot generation |

## 11. MVP Scope

### In Scope (v0.1)
- [x] FileWatcher collector (auto-discover teams, watch JSON changes)
- [x] HookServer collector (receive PostToolUse events)
- [x] In-memory State Store with event-driven updates
- [x] TUI: 4-panel layout (Agents, Tasks, Activity, Messages)
- [x] TUI: vim keybindings + ←/→ agent switching
- [x] Web UI: Network topology with animated message flow
- [x] Web UI: Activity waterfall (code rain style)
- [x] Web UI: Basic cost estimation
- [x] CLI: auto-discover, --team, --web flags
- [x] Hook auto-install with user confirmation
- [x] Single binary distribution (Makefile + goreleaser)

### Out of Scope (future)
- Multi-user / team features
- Persistent storage / history
- Cloud sync
- Custom alerting rules
- Plugin system
- Integration with Langfuse/OpenTelemetry
- Windows support (macOS/Linux first)

## 12. Success Criteria

1. `agenttrack` starts in <100ms and shows active team status
2. File changes reflected in TUI within 1 second
3. Hook events displayed in Live Activity within 200ms
4. Web UI renders smooth 60fps animations
5. Total memory usage <50MB during monitoring
6. Zero impact on Claude Code performance (read-only observation)

## 13. Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Claude Code changes internal file format | Version-detect + graceful degradation |
| High-frequency file changes overwhelm watcher | 500ms debounce + batch processing |
| Hook configuration breaks Claude Code | Backup settings.json before modification; validate JSON |
| Debug log format is unstable | LogParser is optional, not core dependency |
| Large inbox files slow parsing | Incremental parsing (track last-read position) |

## 14. Open Source Strategy

- **License**: MIT
- **Repo**: `github.com/<owner>/agenttrack`
- **Distribution**: GitHub Releases + Homebrew tap + `go install`
- **Community**: GitHub Discussions for feature requests, Issues for bugs
- **Contribution guide**: CONTRIBUTING.md with dev setup instructions
