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
    /// Classify the message into a MessageType.
    ///
    /// Claude Code stores idle/shutdown notifications in TWO possible ways:
    /// 1. Top-level `"type": "idle_notification"` field (our msg_type)
    /// 2. Embedded JSON inside the `"text"` field: `"text": "{\"type\":\"idle_notification\",...}"`
    ///
    /// We check both.
    pub fn classify_type(&self) -> MessageType {
        // First: check top-level type field
        if let Some(t) = self.msg_type.as_deref() {
            return Self::classify_type_str(t);
        }

        // Second: try to parse text as JSON and check for embedded type
        if let Some(text) = &self.text {
            if text.starts_with('{') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(t) = val.get("type").and_then(|v| v.as_str()) {
                        return Self::classify_type_str(t);
                    }
                }
            }
        }

        MessageType::DirectMessage
    }

    fn classify_type_str(type_str: &str) -> MessageType {
        match type_str {
            "idle_notification" => MessageType::IdleNotification,
            "shutdown_notification" | "shutdown_request" | "shutdown_response" => {
                MessageType::ShutdownNotification
            }
            "plan_approval_request" | "plan_approval_response" => MessageType::PlanApproval,
            "task_completed" => MessageType::TaskCompleted,
            _ => MessageType::DirectMessage,
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
    #[serde(default)]
    pub tokens: TokenUsage,
    /// Seconds since last activity (set when building snapshot)
    #[serde(default)]
    pub last_seen_secs: Option<u64>,
    /// Number of sub-agents (set when building ALL tab snapshot)
    #[serde(default)]
    pub sub_agent_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_5m_tokens: u64,
    pub cache_create_1h_tokens: u64,
    /// Pre-computed cost accumulated from per-message tiered pricing.
    #[serde(default)]
    pub cost_usd: f64,
}

/// Threshold for tiered pricing (per-message, in tokens).
const TIERED_THRESHOLD: u64 = 200_000;

/// Calculate cost for a token count with tiered pricing.
fn tiered_cost(tokens: u64, base_rate: f64, tiered_rate: f64) -> f64 {
    if tokens <= TIERED_THRESHOLD {
        tokens as f64 / 1_000_000.0 * base_rate
    } else {
        let below = TIERED_THRESHOLD as f64 / 1_000_000.0 * base_rate;
        let above = (tokens - TIERED_THRESHOLD) as f64 / 1_000_000.0 * tiered_rate;
        below + above
    }
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens
            + self.cache_create_5m_tokens + self.cache_create_1h_tokens
    }

    /// Estimate cost in USD. Uses pre-computed cost if available,
    /// otherwise falls back to flat-rate calculation.
    pub fn estimated_cost_usd(&self) -> f64 {
        if self.cost_usd > 0.0 {
            self.cost_usd
        } else {
            self.estimated_cost_for_model(None)
        }
    }

    /// Estimate cost with explicit model name.
    /// Uses tiered pricing + unified cache rate to match ccusage/stats.rs.
    pub fn estimated_cost_for_model(&self, model: Option<&str>) -> f64 {
        if self.cost_usd > 0.0 {
            return self.cost_usd;
        }
        let (inp, inp_t, out, out_t, cw, cw_t, cr, cr_t) = match model {
            Some(m) if m.to_lowercase().contains("opus") =>
                (5.0, 10.0, 25.0, 37.50, 6.25, 12.50, 0.50, 1.00),
            Some(m) if m.to_lowercase().contains("haiku") =>
                (1.0, 1.0, 5.0, 5.0, 1.25, 1.25, 0.10, 0.10),
            _ => // Sonnet
                (3.0, 6.0, 15.0, 22.50, 3.75, 7.50, 0.30, 0.60),
        };
        // Unified cache_creation (5m + 1h) at single cache_write rate
        let cache_write_total = self.cache_create_5m_tokens + self.cache_create_1h_tokens;
        tiered_cost(self.input_tokens, inp, inp_t)
            + tiered_cost(self.output_tokens, out, out_t)
            + tiered_cost(cache_write_total, cw, cw_t)
            + tiered_cost(self.cache_read_tokens, cr, cr_t)
    }

    /// Add a single message's tokens and compute its cost with tiered pricing.
    /// Uses unified cache_creation rate (no separate 1h rate) to match ccusage.
    pub fn add_message(&mut self, model: Option<&str>,
        input: u64, output: u64, cache_read: u64, cache_write_5m: u64, cache_write_1h: u64,
    ) {
        self.input_tokens += input;
        self.output_tokens += output;
        self.cache_read_tokens += cache_read;
        self.cache_create_5m_tokens += cache_write_5m;
        self.cache_create_1h_tokens += cache_write_1h;

        // Unified cache_creation = 5m + 1h, priced at single cache_write rate
        let cache_write_total = cache_write_5m + cache_write_1h;

        let (inp, inp_t, out, out_t, cw, cw_t, cr, cr_t) = match model {
            Some(m) if m.to_lowercase().contains("opus") =>
                (5.0, 10.0, 25.0, 37.50, 6.25, 12.50, 0.50, 1.00),
            Some(m) if m.to_lowercase().contains("haiku") =>
                (1.0, 1.0, 5.0, 5.0, 1.25, 1.25, 0.10, 0.10),
            _ => // Sonnet
                (3.0, 6.0, 15.0, 22.50, 3.75, 7.50, 0.30, 0.60),
        };

        self.cost_usd += tiered_cost(input, inp, inp_t)
            + tiered_cost(output, out, out_t)
            + tiered_cost(cache_write_total, cw, cw_t)
            + tiered_cost(cache_read, cr, cr_t);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Idle,
    Shutdown,
    Unknown,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Active => "Active",
            AgentStatus::Idle => "Idle",
            AgentStatus::Shutdown => "Shutdown",
            AgentStatus::Unknown => "Unknown",
        }
    }
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
    pub summary: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    /// If this event is from a sub-agent: (parent_session_id, agent_id, agent_type)
    #[serde(default)]
    pub subagent_info: Option<(String, String, Option<String>)>,
}

// ─── Todo Items (from TodoWrite tool calls) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: String, // "pending", "in_progress", "completed"
    #[serde(default)]
    pub active_form: String,
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
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
}
