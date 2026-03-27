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

## What you see

- **Live cost** — per-session, per-project, updates in real time
- **Every session** — status, model (opus/sonnet/haiku), running cost
- **Sub-agent trees** — spawned agents, their models, individual costs
- **30-day charts** — token usage (output, input, cache) and daily cost
- **Cache hit rate** — is prompt caching actually saving you money?
- **Quota bars** — 5h / 7d usage from Claude's API, no more surprise rate limits

<div align="center">

<img src="assets/tui.png" width="720" />

*TUI mode — `Tab` to switch sessions, runs alongside your editor*

</div>

## Commands

```bash
cctrack                     # TUI only
cctrack --web               # TUI + web dashboard
cctrack --web-only          # web only (headless)
cctrack stats               # quick cost summary in terminal
```

## Privacy

cctrack runs 100% on your machine. It reads Claude Code's local transcript files and receives hook events on localhost. No data leaves your machine, no telemetry, no cloud.

## Quota monitor

For real-time quota bars (5h / 7d), log in with `claude /login` then click "Connect to Claude for quota" in the web dashboard.

## Built with Claude Code

This entire project was vibe-coded with Claude Code — from the Rust daemon to the web dashboard. Single binary, ~3MB, <10MB RAM, instant startup.

## See also

- [ccusage](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — analyze past Claude Code usage and costs. cctrack was inspired by ccusage and focuses on real-time monitoring instead.

## License

MIT
