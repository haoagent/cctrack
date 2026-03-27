<div align="center">

# cctrack

**See your Claude Code agent teams work — and what they cost — in real time**

[![CI](https://github.com/haoagent/cctrack/actions/workflows/ci.yml/badge.svg)](https://github.com/haoagent/cctrack/actions)
[![GitHub stars](https://img.shields.io/github/stars/haoagent/cctrack?style=flat)](https://github.com/haoagent/cctrack)
[![npm](https://img.shields.io/npm/v/cctrack)](https://www.npmjs.com/package/cctrack)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

<img src="assets/web-top.png" width="720" />

</div>

You kick off a Claude Code session. It spawns 4 sub-agents. They're all running in parallel — calling tools, editing files, burning tokens. But you can't see any of it.

cctrack gives you a live dashboard showing every agent, what it's doing, which model it's using, and how much it's costing you — right now, not after the fact.

## Get started

```bash
npm install -g cctrack       # binary wrapper — no JS runtime needed
cctrack hooks install        # one-time: adds a hook to Claude Code (safe to uninstall)
cctrack --web                # → open localhost:7891
```

That's it. Use Claude Code normally — cctrack picks up everything automatically. It does not slow down Claude Code.

## Agent team visibility

This is why cctrack exists. When Claude Code spawns sub-agents, cctrack:

- Shows **every agent** in the tree — parent and all sub-agents
- Tracks each agent's **model** (opus/sonnet/haiku), **status**, and **individual cost**
- Creates a **dedicated tab** for each session with sub-agents — press `Tab` to switch
- Shows **live tool calls** — Bash, Edit, Read, Grep, Agent — with duration
- Updates in **real time** as agents work

You can finally see what your $50 agent team session is actually doing.

## Web Dashboard

Open **http://localhost:7891** after starting `cctrack --web`.

- **Sessions** — every session with status, model, tokens, and running cost
- **Cost chart** — daily spending over 30 days, with 7d/30d/All toggle
- **Token chart** — stacked usage (output, input, cache read/write) per day
- **Cache hit rate** — is prompt caching actually saving you money?
- **Quota bars** — real 5h / 7d usage from Claude's API. No more surprise rate limits

## TUI

<div align="center">
<img src="assets/tui.png" width="720" />
</div>

Runs alongside your editor. Three panels:

| Panel | What it shows |
|-------|--------------|
| **Sessions** | All sessions with status, model, tokens, cost |
| **Stats** | Today / week / month totals, per-project breakdown |
| **Live Activity** | Tool calls as they happen, with duration |

| Symbol | Meaning |
|--------|---------|
| ● green | Active — running right now |
| ○ yellow | Idle — waiting for input |
| · gray | Shutdown — session ended |

| Key | Action |
|-----|--------|
| `Tab` | Switch session tabs |
| `↑↓` / `jk` | Scroll within panel |
| `←→` | Switch panel |
| `q` | Quit |

## Commands

```bash
cctrack                     # TUI only
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (headless)
cctrack stats               # quick cost summary
cctrack hooks install       # add hook (one-time)
cctrack hooks uninstall     # remove hook
```

### `cctrack stats`

Quick cost summary without starting the dashboard:

```
               sessions     tokens        cost
Today               3        8.4M        $5.12
This week          27       84.2M       $62.30
March             142      312.5M      $198.40
Total             168      396.7M      $260.70

By Project
my-app             89      201.3M      $142.50
side-project       52      128.4M       $86.20
scripts            27       67.0M       $32.00
```

Projects are detected by working directory — each directory you run Claude Code in becomes a separate project.

## Quota monitor

See real-time quota bars (5h / 7d):

```bash
claude /login
```

Then click **"Connect to Claude for quota"** in the web dashboard. cctrack reads your OAuth token locally — nothing is sent to third parties.

## How it works

cctrack installs a [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code) that sends tool call events to a local server (`localhost:7890`). It also reads transcript files from `~/.claude/projects/` for session history and token counts.

Everything runs locally. No telemetry, no cloud, no data leaves your machine. cctrack is free and open source.

## Development

```bash
git clone https://github.com/haoagent/cctrack
cd cctrack/cctrack
cargo build                 # debug build
cargo test                  # run tests
cargo install --path .      # install locally
```

Architecture: single Rust binary containing a hook server (Axum), TUI (Ratatui), web dashboard (embedded static files + SSE), and a local state store. ~3MB binary, <10MB RAM.

The npm package is a thin wrapper that downloads the pre-built binary for your platform from GitHub Releases.

## Built with Claude Code

This entire project was vibe-coded with Claude Code.

## See also

- [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — analyze past Claude Code usage and costs. cctrack was inspired by ccusage and focuses on real-time monitoring instead.

## License

MIT
