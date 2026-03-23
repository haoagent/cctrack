# cctrack Session Log — 2026-03-23

## Bugs Fixed

### 1. Sub-agent transcript path derivation
- **Problem**: Claude Code PostToolUse hook doesn't send `agent_transcript_path` field. cctrack fell back to parent's transcript path, so sub-agents couldn't be properly tracked.
- **Fix**: Derive sub-agent transcript path from `transcript_path` + `agent_id`: `{parent_dir}/{session_id}/subagents/agent-{agent_id}.jsonl`
- **File**: `src/collector/hook_server.rs` (line ~194)

### 2. Agent status stuck at Shutdown
- **Problem**: `update_timeout_status()` returned early if status was already `Shutdown`, preventing recovery to `Active` when new tool calls arrived. Also, parent sessions were hardcoded to `AgentStatus::Shutdown` on creation.
- **Fix**: Removed early return for Shutdown status; changed parent initial status to `Active` (corrected by timeout logic). Changed `last_event_at` for parent from expired to `Instant::now()`.
- **File**: `src/store/state.rs`

### 3. Sub-agent naming
- **Problem**: Sub-agents displayed as "Explore", "general-purpose" instead of their description (e.g., "Agent A: count R3 files").
- **Fix**: Added `SubAgentName` event. When parent's `Agent` tool call hook arrives, extract `tool_input.description` + `tool_response.agentId` mapping and rename the sub-agent.
- **Files**: `src/store/event.rs`, `src/collector/hook_server.rs`, `src/store/state.rs`

### 4. Session tab expiry not surviving restarts
- **Problem**: `is_expired()` used in-memory `last_activity_at` which reset on restart, so expired tabs reappeared.
- **Fix**: For session tabs, check parent transcript file mtime instead of in-memory timer. Sessions without transcript are treated as expired immediately.
- **File**: `src/store/state.rs`

### 5. Fake/orphan sessions persisting
- **Problem**: Manual curl test sessions (e.g., SESSION-TEST-MAN) persisted and reappeared after restart.
- **Fix**: On restore, skip sessions without transcript (`None`), and skip sub-agents whose transcript is not under `subagents/` dir or whose parent transcript doesn't exist.
- **File**: `src/store/state.rs`

## UI Changes

### 6. Agent count format: N/N
- **Change**: Agents panel header shows `Agents (active/total)` and Sessions panel shows `Sessions (active/total)`.
- **File**: `src/tui/agents_panel.rs`

### 7. Tab title simplified
- **Change**: Removed N/N count from tab titles (redundant with panel header).
- **File**: `src/tui/top_bar.rs`

### 8. Window resize removed
- **Change**: Removed automatic terminal resize (`\x1b[8;25;90t`), uses default terminal size.
- **File**: `src/tui/mod.rs`

### 9. Session tab expire time
- **Change**: `SESSION_TAB_EXPIRE_SECS` reduced from 1800 (30min) to 300 (5min).
- **File**: `src/store/state.rs`
