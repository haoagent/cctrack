<div align="center">

# cctrack

**Know what your Claude Code agents are doing — and what it costs. In real time.**

[![GitHub stars](https://img.shields.io/github/stars/haoagent/cctrack?style=flat)](https://github.com/haoagent/cctrack)
[![npm](https://img.shields.io/npm/v/cctrack)](https://www.npmjs.com/package/cctrack)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)

</div>

Running Claude Code with Max plan? Agents spawn sub-agents, burn through tokens, and you have no idea what's happening until you check the bill. cctrack fixes that.

<div align="center">
<img src="assets/web-top.png" width="720" />

*Web dashboard — live cost, sessions, usage charts, cache hit rate, quota*

<img src="assets/tui.png" width="720" />

*Terminal UI — always-on monitoring alongside your editor*
</div>

## Install

```bash
npm install -g cctrack
```

Or download a [pre-built binary](https://github.com/haoagent/cctrack/releases) (macOS, Linux, Windows).

## Quick Start

```bash
cctrack hooks install    # one-time setup
cctrack --web            # start monitoring
```

Open **http://localhost:7891**. That's it. Use Claude Code normally — cctrack picks up everything automatically.

Press `Tab` to switch between session panels and see what each agent team is doing.

## Why cctrack

| | [ccusage](https://github.com/ryoppippi/ccusage) | cctrack |
|---|---|---|
| **When** | After the fact — analyze past usage | Right now — watch agents live |
| **How** | Run once, get a report | Always-on daemon |
| **See** | Token totals and costs | Sessions, sub-agents, tool calls, models |
| **UI** | CLI output | Web dashboard + TUI |

ccusage tells you what happened. cctrack shows you what's happening.

## Features

- **Live cost** — per-session, per-project, updates as agents work
- **Session tracking** — every session with status, model (opus/sonnet/haiku), and running cost
- **Sub-agent trees** — see spawned agents, their models, individual costs
- **Usage charts** — 30 days of token usage (stacked: output, input, cache) and daily cost
- **Cache hit rate** — verify prompt caching is saving you money
- **Quota bars** — real 5h and 7d usage from Claude's API, no more surprise rate limits
- **Web + TUI** — browser dashboard (SSE, real-time) or lightweight terminal UI
- **Local-only** — everything stays on your machine, no telemetry

## Usage

```bash
cctrack                     # TUI only
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (localhost:7891)
cctrack stats               # quick cost summary in terminal
```

## How It Works

cctrack installs a [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code) that sends tool call events to a local server. It also reads transcript files for session history and token counts.

```
Claude Code ──→ hook events (localhost:7890)
           └──→ transcripts (~/.claude/projects/)
                          │
                   ┌──────┴──────┐
                   │  TUI   Web  │
                   └─────────────┘
```

Everything stays on your machine.

## Quota Monitor

See real quota usage (5h / 7d bars) by logging in:

```bash
claude /login
```

cctrack reads your OAuth token locally and calls Anthropic's usage API. Click **"Connect to Claude for quota"** in the web dashboard.

## Tech

Single Rust binary. ~3MB, <10MB RAM. Built with Ratatui + Axum + Chart.js.

This entire project was vibe-coded with Claude Code.

## Acknowledgments

Inspired by [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — the excellent Claude Code cost analyzer. cctrack builds on the same idea, reimagined as an always-on real-time monitor.

## License

MIT
