<div align="center">

# cctrack

**Real-time cost and activity monitor for Claude Code**

Your agents are burning tokens. See exactly how much — live.

[![GitHub stars](https://img.shields.io/github/stars/haoagent/cctrack?style=flat)](https://github.com/haoagent/cctrack)
[![npm](https://img.shields.io/npm/v/cctrack)](https://www.npmjs.com/package/cctrack)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

<img src="assets/web-top.png" width="720" />

</div>

## 3 commands. That's it.

```bash
npm install -g cctrack       # install
cctrack hooks install        # one-time setup
cctrack --web                # start monitoring → localhost:7891
```

## What you see

- **Live cost** — per-session, per-project, updates as agents work
- **Every session** — status, model (opus/sonnet/haiku), running cost
- **Sub-agent trees** — spawned agents, their models, individual costs
- **30-day charts** — token usage (output, input, cache) and daily cost
- **Cache hit rate** — is prompt caching actually saving you money?
- **Quota bars** — real 5h / 7d usage from Claude's API. No more surprise rate limits
- **Web + TUI** — browser dashboard or terminal UI, your choice

<div align="center">
<img src="assets/tui.png" width="720" />

*TUI — runs alongside your editor. `Tab` to switch sessions.*
</div>

## cctrack vs ccusage

| | [ccusage](https://github.com/ryoppippi/ccusage) | cctrack |
|---|---|---|
| **When** | After — analyze past usage | Now — watch agents live |
| **How** | Run once, get a report | Always-on daemon |
| **UI** | CLI output | Web dashboard + TUI |

Both are great. ccusage tells you what happened. cctrack shows you what's happening.

## Usage

```bash
cctrack                     # TUI only
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (headless)
cctrack stats               # quick cost summary in terminal
```

## How it works

cctrack installs a [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code) that sends tool call events to a local server. It also reads transcript files for history and token counts.  Everything stays on your machine — no telemetry, no cloud.

For quota monitoring, log in with `claude /login` and click "Connect to Claude for quota" in the web dashboard.

## Tech

Single Rust binary. ~3MB, <10MB RAM. Instant startup.

This entire project was vibe-coded with Claude Code.

## Acknowledgments

Inspired by [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi).

## License

MIT
