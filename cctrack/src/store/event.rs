use serde::Serialize;

use super::models::*;

/// Events produced by the file-watcher / collector and consumed by the Store.
#[derive(Debug)]
pub enum Event {
    TeamUpdate {
        team_name: String,
        config: TeamConfig,
    },
    TaskUpdate {
        team_name: String,
        task: TaskFile,
    },
    MessageUpdate {
        team_name: String,
        agent_name: String,
        messages: Vec<InboxMessage>,
    },
    ToolCall(ToolEvent),
    TokenUpdate {
        session_id: String,
        usage: TokenUsage,
    },
    TodoUpdate {
        session_id: String,
        todos: Vec<TodoItem>,
    },
    /// Periodic tick to refresh timeout-based status.
    Tick,
}

/// Immutable snapshot of all state, sent to the TUI and Web layers via `watch`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StoreSnapshot {
    pub teams: Vec<TeamSnapshot>,
}

/// Per-team snapshot included in `StoreSnapshot`.
#[derive(Debug, Clone, Serialize)]
pub struct TeamSnapshot {
    pub name: String,
    pub description: String,
    pub agents: Vec<Agent>,
    pub tasks: Vec<TaskFile>,
    pub todos: Vec<TodoItem>,
    pub messages: Vec<Message>,
    pub tool_events: Vec<ToolEvent>,
    pub metrics: Metrics,
}
