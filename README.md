<div align="center">

# cctrack

**Real-time cost and activity monitor for Claude Code**

Your agents are burning tokens. See exactly how much — live.

[![GitHub stars](https://img.shields.io/github/stars/haoagent/cctrack?style=flat)](https://github.com/haoagent/cctrack)
[![npm](https://img.shields.io/npm/v/cctrack)](https://www.npmjs.com/package/cctrack)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

<img src="assets/web-top.png" width="720" />

</div>

## Get started

```bash
npm install -g cctrack       # or download binary from Releases
cctrack hooks install        # one-time setup
cctrack --web                # → open localhost:7891
```

Done. Use Claude Code normally — cctrack picks up everything automatically.

## Web Dashboard

Open **http://localhost:7891** after starting `cctrack --web`.

- **Sessions panel** — every active session with status indicator, model, tokens, and running cost
- **Cost chart** — daily spending over the last 30 days, with 7d/30d/All toggle
- **Token chart** — stacked token usage (output, input, cache read/write) per day
- **Cache hit rate** — percentage of tokens served from cache, so you know if caching is saving money
- **Quota bars** — real 5h and 7d usage pulled from Claude's API. No more guessing when you'll hit the rate limit

Sessions with sub-agents automatically get their own tab. Click a tab to drill into that session's agent tree — see each sub-agent's model, status, and individual cost.

## TUI

<div align="center">
<img src="assets/tui.png" width="720" />
</div>

The terminal UI runs alongside your editor. Panels:

| Panel | What it shows |
|-------|--------------|
| **Sessions** | All sessions with status, model, tokens, cost |
| **Stats** | Today / this week / this month totals, per-project breakdown |
| **Live Activity** | Tool calls as they happen — Bash, Edit, Read, Grep, Agent — with duration |

### Status indicators

| Symbol | Meaning |
|--------|---------|
| ● green | Active — running right now |
| ○ yellow | Idle — waiting for input |
| · gray | Shutdown — session ended |

### Keyboard

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
cctrack stats               # quick cost summary in terminal
cctrack hooks install       # add hook to Claude Code (one-time)
cctrack hooks uninstall     # remove hook
```

### `cctrack stats`

Quick cost summary without starting the dashboard:

```
               sessions     tokens        cost
Today               5       62.7M       $34.91
This week         162     1012.4M      $827.82
March             451     2348.9M     $1764.07
Total             507     2487.9M     $1858.83

By Project
ReAgent3          448     1988.3M     $1486.68
Clipal             47      468.5M      $337.50
```

## Quota monitor

See real-time quota usage (5h / 7d bars) by logging in:

```bash
claude /login
```

Then click **"Connect to Claude for quota"** in the web dashboard. cctrack reads your OAuth token locally — nothing is sent to third parties.

## How it works

cctrack installs a [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code) that sends tool call events to a local server (`localhost:7890`). It also reads transcript files from `~/.claude/projects/` for session history and token counts.

Everything runs locally. No telemetry, no cloud, no data leaves your machine.

## Built with Claude Code

This entire project was vibe-coded with Claude Code — from the Rust daemon to the web dashboard. Single binary, ~3MB, <10MB RAM, instant startup.

## See also

- [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — analyze past Claude Code usage and costs. cctrack was inspired by ccusage and focuses on real-time monitoring instead.

## License

MIT
