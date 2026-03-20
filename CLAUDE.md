# Clipal Project

## Session Logging Rule

**At the end of every session**, before the user leaves, write a session log to `/Users/jerry/Documents/Clipal/MEMORY/session-YYYY-MM-DD-HHMMSS.md` containing:

1. **Date**: session date/time
2. **Summary**: brief description of what was discussed and accomplished
3. **Key Decisions**: any important decisions made
4. **Files Modified**: list of files created or changed
5. **Next Steps**: open items or follow-up tasks

Always do this proactively when the conversation is wrapping up or the user says goodbye/done/结束/再见 etc.

## Design Principle

在讨论 AgentPay 功能时，始终用 **Agent CLI 视角** 和 **chat 交互视角** 来模拟用户体验：
- 展示 CLI 命令的输入输出
- 模拟 agent 和 human owner 的交互场景
- 不要用抽象的架构术语，用具体的 `agentpay xxx` 命令演示

## Project Overview

- **Product Name**: Clipay (clipay.com)
- **Codename**: AgentPay (internal/repo name)
- **What**: Agent CLI 支付基础设施 — 给 Agent 一个钱包，让它自主付费调服务，Human Owner 保持控制
- **Tech**: Rust (Axum + SQLite/rusqlite) — CLI + Server 全 Rust; JS MVP 保留为逻辑参考
- **Location**: `/Users/jerry/Documents/Clipal/Agentpay/`
- **Status**: MVP complete (30 E2E + 20 edge case tests passing)
- **Phase 1 Target**: Usage Pay (subscribe + 按量扣费) + Purchase Pay (buy + escrow 交付)
