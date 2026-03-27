---
name: claude-code-track
description: Start real-time cost and activity monitoring for Claude Code sessions. Use when the user wants to track costs, monitor agent sessions, view token usage, or launch the cctrack dashboard.
---

# Claude Code Track — Real-time Session Monitor

Start the cctrack dashboard to monitor Claude Code sessions, costs, and agent activity in real-time.

## Steps

1. Check if cctrack is installed:
```bash
which cctrack || echo "not installed"
```

2. If not installed, install from GitHub releases:
```bash
# macOS Apple Silicon
curl -fsSL https://github.com/haoagent/cctrack/releases/latest/download/cctrack-aarch64-apple-darwin.tar.gz | tar xz
sudo mv cctrack /usr/local/bin/

# Or build from source
git clone https://github.com/haoagent/cctrack && cd cctrack && cargo install --path .
```

3. Install hooks (one-time setup):
```bash
cctrack hooks install
```

4. Start the web dashboard in background:
```bash
cctrack --web-only &
```

5. Report to user:
```
cctrack is running. Open http://localhost:7891 in your browser.
Sessions with sub-agents get their own tab.
```
