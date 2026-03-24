# cctrack

Real-time observability dashboard for Claude Code sessions. Track active agents, token usage, costs, and tool activity across all your Claude Code sessions.

## Features

- **Real-time monitoring** — See active sessions, sub-agents, and tool calls as they happen
- **Token & cost tracking** — Per-session token counts with tiered pricing (Opus/Sonnet/Haiku)
- **Web dashboard** — Premium web UI with charts, dark/light theme, responsive layout
- **TUI dashboard** — Terminal UI with keyboard navigation, scrollable panels
- **Multi-session** — Track multiple concurrent Claude Code sessions and sub-agents
- **Team support** — Organize agents into teams with shared state
- **Auto-discovery** — Detects Claude Code sessions via hooks and transcript scanning
- **Stats & trends** — Daily token usage charts, cost trends, project breakdowns

## Install

```bash
# Clone and build
git clone https://github.com/haoagent/cctrack.git
cd cctrack/cctrack
cargo install --path .

# Install hooks into Claude Code (enables real-time tracking)
cctrack hooks install
```

## Usage

```bash
# Start TUI + Web dashboard
cctrack

# Web only (no TUI)
cctrack --web-only

# Custom web port
cctrack --port 8080

# View stats in terminal
cctrack stats
```

## Web Dashboard

Premium web dashboard at `http://localhost:7891` with:

- **Sidebar** — Live session list with status indicators (green pulse = active, yellow = idle)
- **Charts** — Token usage trend (30 days), daily cost bars, project cost donut
- **Activity feed** — Real-time tool call stream with auto-scroll
- **Todos & Messages** — Tabbed panels for todo tracking and inter-agent messages
- **Theme toggle** — Dark/light mode, persisted to localStorage

Data flows via Server-Sent Events (SSE) for real-time updates, stats refresh every 60s.

### Layout

```
+---------------------------------------------------+
|  cctrack    [Active: 2] [Sessions: 3] [Cost: $41] |
+---------------------------------------------------+
| ALL  | SESSION:CCTRACK | SESSION:REELA             |
+------+--------------------------------------------+
|      |  [Token Usage Chart]  [Cost]  [Projects]   |
| Side |                                             |
| bar  +---------------------------------------------+
|      |  Activity | Todos | Messages                |
| List |  10:30 Read src/main.rs                     |
|      |  10:31 Edit src/web/mod.rs                  |
+------+---------------------------------------------+
```

## TUI Dashboard

Terminal-based dashboard with vim-style navigation:

- **Arrow keys** — `↑↓` scroll, `←→` switch panels
- **Tab** — Cycle through team tabs
- **1-4** — Jump to panel (Agents, Todos, Activity, Messages)
- **q** — Quit

### Panels

| Panel | Description |
|-------|-------------|
| Sessions/Agents | Active sessions with status dot, token count, cost |
| Stats/Todos | Usage stats (ALL tab) or todo list (team tab) |
| Activity | Live tool call feed with timestamps |
| Messages | Inter-agent communication log |

## Architecture

```
Claude Code sessions
        |
   PostToolUse hooks (HTTP → port 7890)
        |
   cctrack collector
        |
   Store (event processing + state)
        |
   watch::Receiver<StoreSnapshot>
      /            \
   TUI            Web Server
  (ratatui)       (Axum + SSE)
   port N/A       port 7891
```

- **Collectors**: Hook server, file watcher, startup scanner
- **Store**: Event-driven state machine, builds immutable snapshots
- **Persistence**: Saves to `~/.claude/cctrack-state.json`, survives restarts
- **Pricing**: Tiered per-message pricing (200K threshold for Opus)

## Configuration

Config file at `~/.cctrack/config.toml`:

```toml
[web]
port = 7891
enabled = true

[hooks]
auto_install = true
port = 7890
```

## Requirements

- Rust 1.75+
- Claude Code with hooks support
- Modern terminal (for TUI) or browser (for web dashboard)

## License

MIT
