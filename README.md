<div align="center">

# cctrack

**Real-time Claude Code agent monitor. See what they're doing — and what it costs.**

[![GitHub stars](https://img.shields.io/github/stars/haoagent/cctrack?style=flat)](https://github.com/haoagent/cctrack)
[![npm](https://img.shields.io/npm/v/cctrack)](https://www.npmjs.com/package/cctrack)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

<table><tr>
<td><img src="assets/web-top.png" width="480" /><br><em>Web Dashboard</em></td>
<td><img src="assets/tui.png" width="480" /><br><em>Terminal UI</em></td>
</tr></table>

</div>

## Install

```bash
npm install -g cctrack
```

Or download a [pre-built binary](https://github.com/haoagent/cctrack/releases) for macOS, Linux, or Windows.

## Quick Start

```bash
cctrack hooks install    # one-time: adds a hook to Claude Code
cctrack --web            # start TUI + web dashboard
```

Open **http://localhost:7891** — cctrack picks up everything automatically.

## What You Get

- **Live cost tracking** — real-time per-session, per-project cost as agents work
- **Session monitor** — status, model (opus/sonnet/haiku), tokens, and cost for every session
- **Usage charts** — 30 days of token usage and daily cost with 7d/30d/All views
- **Cache hit rate** — see if prompt caching is actually saving you money
- **Quota monitor** — real 5h and 7d usage from Claude's API, no more surprise rate limits
- **Agent teams** — sub-agents with models, individual costs, and the full agent tree
- **Web + TUI** — browser dashboard or lightweight terminal UI
- **Local-only** — nothing leaves your machine

## Usage

```bash
cctrack                     # TUI
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (localhost:7891)
cctrack stats               # cost summary in terminal
```

## How It Works

cctrack installs a [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code) that sends tool call events to a local server. It also reads transcript files from `~/.claude/projects/` for session history and token counts.

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

To see real quota usage (5h / 7d bars), log in to Claude Code:

```bash
claude /login
```

cctrack reads your OAuth token from the macOS Keychain and calls Anthropic's usage API locally. Click **"Connect to Claude for quota"** in the web dashboard.

## Tech

Single Rust binary. ~3MB, <10MB RAM. Ratatui + Axum + Chart.js. Vibe-coded with Claude Code.

## Acknowledgments

Inspired by [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi).

## License

MIT
</div>
