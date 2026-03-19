use serde::{Deserialize, Serialize};

// ─── Team Config (from ~/.claude/teams/{name}/config.json) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: Option<u64>,
    #[serde(default)]
    pub lead_agent_id: Option<String>,
    #[serde(default)]
    pub lead_session_id: Option<String>,
    #[serde(default)]
    pub members: Vec<MemberConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberConfig {
    pub agent_id: String,
    pub name: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub plan_mode_required: Option<bool>,
    #[serde(default)]
    pub joined_at: Option<u64>,
    #[serde(default)]
    pub tmux_pane_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub subscriptions: Vec<String>,
    #[serde(default)]
    pub backend_type: Option<String>,
}

// ─── Task File (from ~/.claude/tasks/{team}/task-{id}.json) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFile {
    pub id: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub metadata: Option<TaskMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    #[serde(default, rename = "_internal")]
    pub internal: Option<bool>,
}

// ─── Inbox Message (from ~/.claude/teams/{team}/inboxes/{agent}.json) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxMessage {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub read: Option<bool>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default, rename = "type")]
    pub msg_type: Option<String>,
    #[serde(default)]
    pub idle_reason: Option<String>,
}

impl InboxMessage {
    /// Classify the message into a MessageType based on the `type` field and content.
    pub fn classify_type(&self) -> MessageType {
        match self.msg_type.as_deref() {
            Some("idle_notification") => MessageType::IdleNotification,
            Some("shutdown_notification") | Some("shutdown_request") => {
                MessageType::ShutdownNotification
            }
            Some("plan_approval_request") => MessageType::PlanApproval,
            Some("task_completed") => MessageType::TaskCompleted,
            _ => {
                // Check text content for patterns
                if let Some(text) = &self.text {
                    let lower = text.to_lowercase();
                    if lower.starts_with("status: done") || lower.contains("task completed") {
                        MessageType::TaskCompleted
                    } else {
                        MessageType::DirectMessage
                    }
                } else {
                    MessageType::DirectMessage
                }
            }
        }
    }
}

// ─── Runtime Types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub agent_id: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Idle,
    Shutdown,
    Unknown,
}

impl Default for AgentStatus {
    fn default() -> Self {
        AgentStatus::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub summary: String,
    pub timestamp: String,
    pub msg_type: MessageType,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    DirectMessage,
    IdleNotification,
    ShutdownNotification,
    TaskCompleted,
    PlanApproval,
    Broadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvent {
    pub agent_name: String,
    pub tool_name: String,
    pub timestamp: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub success: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub total_agents: usize,
    pub active_agents: usize,
    pub idle_agents: usize,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub in_progress_tasks: usize,
    pub pending_tasks: usize,
    pub blocked_tasks: usize,
    pub total_messages: usize,
    pub total_tool_calls: usize,
}
