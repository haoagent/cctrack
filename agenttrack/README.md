# AgentTrack

Real-time observability dashboard for [Claude Code](https://claude.ai/code) agent teams.

**See what your agents are doing.** When you run Claude Code agent teams (TeamCreate, SendMessage, TaskCreate), AgentTrack gives you a live view of all agent activity, task progress, message flow, and metrics — in your terminal or browser.

## Features

- **Agent Status** — See which agents are active, idle, or shut down
- **Task Board** — Track task progress, dependencies, and blockers
- **Live Activity** — Watch tool calls in real-time (Read, Edit, Bash, Grep...)
- **Message Flow** — Monitor inter-agent communication
- **Web Dashboard** — Optional browser-based view with SSE real-time updates
- **Zero Config** — Auto-discovers active teams by watching `~/.claude/`

## Install

```bash
# From source
cargo install --path .

# Or build locally
cargo build --release
./target/release/agenttrack
```

## Usage

```bash
# Auto-discover active teams and start TUI
agenttrack

# Monitor a specific team
agenttrack --team my-project

# Also start web dashboard
agenttrack --web

# Web dashboard only (no TUI)
agenttrack --web-only --port 7891

# Enable live tool call tracking (recommended)
agenttrack hooks install

# Remove hooks
agenttrack hooks uninstall
```

## How It Works

AgentTrack runs as an **independent process** alongside Claude Code. It watches the JSON files that Claude Code writes to `~/.claude/teams/` and `~/.claude/tasks/`, and optionally receives tool call events via hooks.

```
┌──────────────────────┐  ┌───────────────────────┐
│  Claude Code          │  │  AgentTrack            │
│  (agent team running) │  │  (separate terminal)   │
│                       │  │                        │
│  Writes JSON to ──────┼──┼──> Watches JSON files  │
│  ~/.claude/teams/     │  │    + Hook events       │
│  ~/.claude/tasks/     │  │                        │
└──────────────────────┘  └───────────────────────┘
```

## Configuration

Optional config at `~/.agenttrack/config.toml`:

```toml
version = 1

[web]
port = 7891
enabled = false

[hooks]
auto_install = true
port = 7890

[ui]
theme = "dark"
```

## Keybindings (TUI)

| Key | Action |
|-----|--------|
| `j/k` | Navigate up/down |
| `←/→` | Switch agent (activity follows) |
| `Tab` | Cycle panels |
| `1-4` | Jump to panel |
| `Enter` | Show details |
| `/` | Search |
| `w` | Open web UI |
| `q` | Quit |

## Tech Stack

- **Rust** + **Ratatui** (TUI) + **axum** (Web)
- **notify** for file system watching
- **tokio** async runtime
- Single binary, ~3-5MB, <10MB memory

## License

MIT
