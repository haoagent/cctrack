# cctrack

Real-time cost, token, and activity dashboard for [Claude Code](https://docs.anthropic.com/en/docs/claude-code).

Track every dollar, every token, every tool call — across all your Claude Code sessions.

![Web Dashboard](../assets/web-top.png)

## Why

Claude Code is powerful but opaque. You run agents, they spawn sub-agents, they burn tokens — and you find out the cost later. cctrack gives you **live visibility**:

- **How much am I spending today?** One glance: `$847.32 today`
- **Which project is the most expensive?** By-project cost breakdown
- **Is my cache working?** Cache hit rate right on the chart
- **Am I about to hit my cap?** Real-time 5h/7d quota from Claude API
- **What is the agent doing right now?** Live tool call feed

## Screenshots

### Web Dashboard

![Dashboard Overview](../assets/web-top.png)
*Sessions, stats, quota, charts, live activity — all in one view*

### TUI

*Lightweight terminal dashboard — runs alongside Claude Code*

## Features

| Feature | Description |
|---------|-------------|
| **Cost Tracking** | Per-session, per-project, daily/weekly/monthly cost with ccusage-aligned pricing |
| **Token Analytics** | Stacked bar charts: Output, Input, Cache Read with cache hit rate |
| **Live Activity** | Real-time tool call feed (Read, Edit, Bash, Grep, Agent...) |
| **Multi-Session** | Track all active Claude Code sessions simultaneously |
| **Agent Teams** | See sub-agents, their models (opus/sonnet/haiku), and individual costs |
| **Quota Monitor** | Connect to Claude API for real 5-hour and 7-day usage limits |
| **Web + TUI** | Browser dashboard with SSE updates, or lightweight terminal UI |
| **Stats CLI** | `cctrack stats` for quick terminal summary |
| **Pricing Check** | `cctrack pricing-check` validates against LiteLLM rates |
| **Zero Config** | Auto-discovers sessions from `~/.claude/`, hooks install in one command |

## Install

```bash
# From source
cargo install --path .

# Or build locally
cargo build --release
./target/release/cctrack
```

## Quick Start

```bash
# 1. Install hooks (enables real-time tracking)
cctrack hooks install

# 2. Start the dashboard
cctrack              # TUI mode
cctrack --web        # TUI + Web dashboard
cctrack --web-only   # Web only (http://localhost:7891)

# 3. Use Claude Code normally — cctrack picks up everything automatically
```

## Usage

```bash
# Dashboard modes
cctrack                     # TUI dashboard
cctrack --web               # TUI + Web (default port 7891)
cctrack --web-only          # Web dashboard only
cctrack --web-only -p 8080  # Custom port

# Stats & tools
cctrack stats               # Quick cost summary in terminal
cctrack pricing-check       # Validate pricing against LiteLLM

# Hook management
cctrack hooks install       # Enable real-time tracking
cctrack hooks uninstall     # Remove hooks
```

## How It Works

```
Claude Code                          cctrack
┌────────────────────┐     ┌─────────────────────────┐
│ You: "refactor auth"│     │                         │
│                     │     │  TUI  ←─── Store ───→ Web│
│ Agent spawns...     │     │  ↑                   ↑  │
│ ├─ Read files       │────→│  Hook Server (7890)     │
│ ├─ Edit code        │     │  Watches transcripts    │
│ ├─ Run tests        │     │  Computes costs         │
│ └─ Sub-agent...     │     │  Tracks agents          │
└────────────────────┘     └─────────────────────────┘
```

cctrack reads Claude Code's transcript files (`~/.claude/projects/`) and receives real-time tool events via hooks. All cost computation happens locally — **no data leaves your machine**.

## Web Dashboard

The web dashboard runs on `localhost:7891` with:

- **Hero**: Today's cost at a glance
- **Sessions (N/N)**: Active/total sessions with model and cost
- **Stats**: Today / This week / Total with by-project breakdown
- **Quota**: 5-hour and 7-day usage (requires Claude OAuth)
- **Charts**: Token usage (stacked) and daily cost with 7d/30d/All range selector
- **Activity**: Live tool call feed with timestamps and duration
- **Tabs**: Switch between ALL view and individual session details

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j/k` or `↑/↓` | Scroll within panel |
| `←/→` | Switch panel |
| `Tab` | Cycle tabs (ALL → session → session...) |
| `1-4` | Jump to panel |
| `q` | Quit |

## Cost Accuracy

cctrack uses the same tiered pricing model as [ccusage](https://github.com/ryoppippi/ccusage):

- Per-message tiered pricing (200K token threshold)
- Model-specific rates (Opus, Sonnet, Haiku)
- Unified cache write pricing (5-minute + 1-hour ephemeral)
- Deduplication by messageId + requestId

Typical accuracy: **< 0.3%** difference vs ccusage.

## Tech

- **Rust** — single binary, ~3MB, <10MB memory
- **Ratatui** — terminal UI
- **Axum** — web server with SSE
- **Chart.js** — browser charts (CDN, no build step)
- **tokio** — async runtime

## Configuration

Optional: `~/.cctrack/config.toml`

```toml
version = 1

[web]
port = 7891

[hooks]
port = 7890
```

## License

MIT
