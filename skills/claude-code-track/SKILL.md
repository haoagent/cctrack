---
name: claude-code-track
description: Query Claude Code usage costs, token stats, and session activity using cctrack. Use when the user asks about costs, spending, token usage, cache hit rates, which project or session is most expensive, or any billing/usage question about Claude Code.
---

# Claude Code Track

Query and analyze Claude Code usage data via `cctrack`.

## Prerequisites

`cctrack` must be installed and hooks configured. If `cctrack` is not found, tell the user:
```
cctrack is not installed. Install it from https://github.com/haoagent/cctrack
```

## Commands

### Cost summary
```bash
cctrack stats
```
Returns today/week/month/total costs, token counts, and per-project breakdown.

### Start live dashboard
```bash
cctrack --web-only &
```
Opens web dashboard at http://localhost:7891. Only start if the user explicitly asks for the dashboard.

## How to answer questions

- "How much did I spend today/this week/this month?" → run `cctrack stats`, read the output
- "Which project costs the most?" → run `cctrack stats`, compare the By Project section
- "How many tokens have I used?" → run `cctrack stats`, report token counts
- "Show me the dashboard" → start `cctrack --web-only &`, tell user to open localhost:7891
- "Is caching working?" → start dashboard for cache hit rate charts

## Output format

`cctrack stats` outputs a table like:
```
                 sess      tokens        cost
Today               4       14.2M       $8.34
This week         161      963.9M     $801.25
March             450     2300.4M    $1737.49
Total             506     2439.4M    $1832.26

By Project
ProjectA          448     1968.6M    $1475.39
ProjectB           46      438.8M     $322.21
```

Parse this output to answer the user's question directly. Give concise answers with the relevant numbers, don't dump raw output.
