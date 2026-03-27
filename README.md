# cctrack

**Know exactly where your Claude Code dollars go.**

I spent $200 on Claude last week. $140 of it was one runaway agent I didn't notice until I got rate-limited. So I built this.

![cctrack dashboard](assets/web-top.png)

## Install

```bash
# Homebrew (macOS)
brew install cctrack          # coming soon

# Cargo
cargo install cctrack         # from crates.io (coming soon)

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

## What You Get

**`$420.69 today`** — one number, front and center. Updated in real-time.

**Sessions (3/5)** — every active session with status, model (opus/sonnet/haiku), and running cost. See which session is burning money right now.

**Stats** — today / this week / total, broken down by project. Finally know that `api-gateway` costs 3x more than `frontend`.

**Quota bars** — your real 5h and 7d usage from Claude's API. No more surprise rate limits.

**Charts** — 30 days of token usage (stacked: output, input, cache) and daily cost. Cache hit rate tells you if caching is working. Switch between 7d / 30d / All.

**Live activity** — watch tool calls happen: `Bash`, `Edit`, `Read`, `Grep`, `Agent` — with duration. Know what the agent is doing right now.

**Agent teams** — click a session to see all sub-agents, their costs, their models. Track the full tree.

## cctrack vs ccusage

[ccusage](https://github.com/ryoppippi/ccusage) is great for after-the-fact analysis. cctrack is for live monitoring. They complement each other:

| | cctrack | ccusage |
|---|---|---|
| **When** | While you're working | After you're done |
| **Updates** | Real-time (SSE) | On-demand scan |
| **Multi-session** | All sessions at once | One report |
| **Quota/cap** | Live 5h/7d bars | Not available |
| **Activity feed** | Live tool calls | Not available |
| **Historical cost** | Same accuracy | Same accuracy |
| **Install** | Rust binary + hooks | `npx ccusage` |

Both read the same `~/.claude/projects/` transcripts. Same pricing model. < 0.3% cost difference.

## What `hooks install` Does

It adds one line to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "postToolExecution": "curl -s -X POST http://localhost:7890/hook -d @-"
  }
}
```

This sends tool call events to cctrack's local server. **Nothing leaves your machine.** No telemetry, no cloud. Everything stays on localhost. (The web dashboard loads Chart.js from CDN for charts; use `--no-cdn` to bundle it locally.)

Run `cctrack hooks uninstall` to remove it.

## Usage

```bash
cctrack                     # TUI dashboard
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (localhost:7891)
cctrack stats               # quick cost summary in terminal
cctrack pricing-check       # validate pricing vs LiteLLM

cctrack hooks install       # add hook to Claude Code
cctrack hooks uninstall     # remove hook
```

## TUI

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Scroll within panel |
| `←→` | Switch panel |
| `Tab` | Cycle session tabs |
| `q` | Quit |

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

All computation is local. Reads transcripts + hook events. Computes costs with tiered pricing. Renders to TUI and/or web.

## Tech

Single Rust binary. ~3MB. <10MB RAM.

Ratatui (TUI) + Axum (web server + SSE) + Chart.js (CDN, no build step). tokio async runtime.

## License

MIT
