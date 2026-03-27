<div align="center">

# cctrack

**Real-time usage & activity dashboard for Claude Code**

> See how hard your Claude Code subscription is working for you.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Claude Code](https://img.shields.io/badge/Claude_Code-cc5500?logo=anthropic&logoColor=white)](https://docs.anthropic.com/en/docs/claude-code)

<img src="assets/web-top.png" width="720" />

</div>

## Install

```bash
# From source
git clone https://github.com/haoagent/cctrack && cd cctrack
cargo install --path .
```

## Quick Start

```bash
cctrack hooks install    # one-time: adds a hook to ~/.claude/settings.json
cctrack --web            # starts TUI + web dashboard at localhost:7891
```

Use Claude Code normally. cctrack picks up everything automatically.

## Features

- **💰 Live Usage** — `$420.69 today` in API-equivalent cost — see how much value you're extracting from your subscription
- **📊 Sessions** — every active session with status, model (opus/sonnet/haiku), and running cost
- **📈 Charts** — 30 days of token usage (stacked: output, input, cache) and daily cost with 7d/30d/All selector
- **🎯 Cache Hit Rate** — see if caching is actually working (spoiler: 97%)
- **⚡ Quota Monitor** — real 5h and 7d usage from Claude's OAuth API. No more surprise rate limits
- **🔍 Live Activity** — watch tool calls happen: `Bash`, `Edit`, `Read`, `Grep`, `Agent` — with duration
- **🤖 Agent Teams** — see sub-agents, their models, individual costs. Track the full team tree
- **📋 Per-Project Stats** — today / this week / total, broken down by project
- **🖥️ Web + TUI** — browser dashboard (SSE) or lightweight terminal UI
- **🔒 Local-Only** — all computation on your machine. No telemetry, no cloud
- **🦀 Tiny Footprint** — single Rust binary, ~3MB, <10MB RAM

## Acknowledgments

Inspired by [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — the excellent Claude Code cost analyzer. cctrack builds on the same concept, reimagined in Rust as an always-on daemon: real-time monitoring, live sessions, quota bars, activity feed, and a web dashboard. Runs in the background at <10MB RAM.

## Usage

```bash
# Dashboard
cctrack                     # TUI dashboard
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (localhost:7891)

# Tools
cctrack stats               # quick cost summary in terminal
cctrack pricing-check       # validate pricing vs LiteLLM

# Hooks
cctrack hooks install       # add hook to Claude Code
cctrack hooks uninstall     # remove hook
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Scroll within panel |
| `←→` | Switch panel |
| `Tab` | Cycle session tabs |
| `q` | Quit |

## What `hooks install` Does

It adds one line to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "postToolExecution": "curl -s -X POST http://localhost:7890/hook -d @-"
  }
}
```

Tool call events go to cctrack's local server. **Nothing leaves your machine.** Run `cctrack hooks uninstall` to remove it.

## How It Works

```
You ──→ Claude Code ──→ transcripts (~/.claude/projects/)
                    └──→ hook events (localhost:7890)
                              │
                       ┌──────┴──────┐
                       │  TUI   Web  │
                       │      SSE    │
                       └─────────────┘
```

## Tech

Single Rust binary. Ratatui (TUI) + Axum (web + SSE) + Chart.js (CDN). tokio async.

## Contributing

PRs and issues welcome! This project is in active development. If you have ideas, bugs, or want to add features — just open an issue or submit a PR.

## License

MIT — use it however you want.
