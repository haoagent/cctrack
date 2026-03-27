use std::collections::HashMap;

use tokio::sync::{mpsc, watch};

use super::event::{Event, StoreSnapshot, TeamSnapshot};
use super::models::*;

const TOOL_EVENT_RING_SIZE: usize = 500;

/// Internal per-team mutable state.
#[derive(Debug)]
struct TeamState {
    config: TeamConfig,
    agents: Vec<Agent>,
    tasks: HashMap<String, TaskFile>,
    /// Latest todos per agent (agent_id → todo items)
    todos: HashMap<String, Vec<TodoItem>>,
    messages: Vec<Message>,
    /// Dedup key: (from, timestamp)
    message_keys: Vec<(String, String)>,
    tool_events: Vec<ToolEvent>,
    last_activity_at: std::time::Instant,
    /// Whether the lead session has a proper title (not just CWD fallback)
    has_lead_title: bool,
}

const TEAM_EXPIRE_SECS: u64 = 86400;         // 24 hours → remove team tab
const SESSION_TAB_EXPIRE_SECS: u64 = 300;    // 5 minutes → remove session tab

impl TeamState {
    fn new(config: TeamConfig) -> Self {
        let agents = config
            .members
            .iter()
            .map(|m| Agent {
                name: m.name.clone(),
                agent_id: m.agent_id.clone(),
                agent_type: m.agent_type.clone(),
                model: m.model.clone(),
                color: m.color.clone(),
                status: AgentStatus::Active,
                tokens: Default::default(),
                last_seen_secs: None,
                sub_agent_count: None,
            })
            .collect();

        Self {
            config,
            agents,
            tasks: HashMap::new(),
            todos: HashMap::new(),
            messages: Vec::new(),
            message_keys: Vec::new(),
            tool_events: Vec::new(),
            // Start as "old" — only real activity will make it fresh.
            // This ensures dead teams from previous runs don't appear active.
            last_activity_at: std::time::Instant::now() - std::time::Duration::from_secs(ENDED_TIMEOUT_SECS + 1),
            has_lead_title: false,
        }
    }

    /// Rebuild agents from a new config, preserving existing status where possible.
    fn update_config(&mut self, config: TeamConfig) {
        let mut new_agents: Vec<Agent> = config
            .members
            .iter()
            .map(|m| {
                // Preserve status if this agent was already known
                let existing_status = self
                    .agents
                    .iter()
                    .find(|a| a.name == m.name)
                    .map(|a| a.status.clone())
                    .unwrap_or(AgentStatus::Active);

                let existing_tokens = self
                    .agents
                    .iter()
                    .find(|a| a.name == m.name)
                    .map(|a| a.tokens.clone())
                    .unwrap_or_default();

                Agent {
                    name: m.name.clone(),
                    agent_id: m.agent_id.clone(),
                    agent_type: m.agent_type.clone(),
                    model: m.model.clone(),
                    color: m.color.clone(),
                    status: existing_status,
                    tokens: existing_tokens,
                    last_seen_secs: None,
                sub_agent_count: None,
                }
            })
            .collect();

        // Sort by name for deterministic ordering
        new_agents.sort_by(|a, b| a.name.cmp(&b.name));

        self.config = config;
        self.agents = new_agents;
    }

    /// Upsert a task by its ID.
    fn upsert_task(&mut self, task: TaskFile) {
        self.tasks.insert(task.id.clone(), task);
    }

    /// Process inbox messages for a given agent. Deduplicates and derives enriched Message.
    fn process_messages(&mut self, agent_name: &str, inbox_messages: Vec<InboxMessage>) {
        for msg in inbox_messages {
            let from = msg.from.clone().unwrap_or_default();
            let timestamp = msg.timestamp.clone().unwrap_or_default();
            let key = (from.clone(), timestamp.clone());

            // Deduplicate by (from, timestamp)
            if self.message_keys.contains(&key) {
                // Still update agent status from idle/shutdown notifications
                self.update_agent_status_from_message(&msg);
                continue;
            }

            // Update agent status based on notification type
            self.update_agent_status_from_message(&msg);

            let msg_type = msg.classify_type();

            // Derive the "to" field: the inbox owner is the recipient
            let to = agent_name.to_string();

            let enriched = Message {
                from: from.clone(),
                to,
                text: msg.text.clone().unwrap_or_default(),
                summary: msg.summary.clone().unwrap_or_default(),
                timestamp: timestamp.clone(),
                msg_type,
                read: msg.read.unwrap_or(false),
                color: msg.color.clone(),
            };

            self.message_keys.push(key);
            self.messages.push(enriched);
        }
    }

    /// Update agent status based on notification messages.
    fn update_agent_status_from_message(&mut self, msg: &InboxMessage) {
        let msg_type = msg.classify_type();
        let from = match &msg.from {
            Some(f) => f.clone(),
            None => return,
        };

        let new_status = match msg_type {
            MessageType::IdleNotification => Some(AgentStatus::Idle),
            MessageType::ShutdownNotification => Some(AgentStatus::Shutdown),
            _ => None,
        };

        if let Some(status) = new_status {
            if let Some(agent) = self.agents.iter_mut().find(|a| a.name == from) {
                agent.status = status;
            }
        }
    }

    /// Ensure a session is registered as an agent.
    /// Priority: transcript title > cwd dir name > truncated session_id
    fn ensure_agent(&mut self, session_id: &str, cwd: Option<&str>, transcript_path: Option<&str>) -> bool {
        if self.agents.iter().any(|a| a.agent_id == session_id || a.name == session_id) {
            return false;
        }
        // Priority: transcript title > cwd dir name > truncated session_id
        let base_name = transcript_path
            .and_then(|p| crate::collector::hook_server::read_session_title(p))
            .or_else(|| {
                cwd.and_then(|p| std::path::Path::new(p).file_name())
                    .and_then(|f| f.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| {
                if session_id.len() > 8 {
                    format!("session-{}", &session_id[..8])
                } else {
                    session_id.to_string()
                }
            });
        // Deduplicate: if name exists, append -2, -3, etc.
        let display_name = if self.agents.iter().any(|a| a.name == base_name) {
            let mut n = 2;
            loop {
                let candidate = format!("{}-{}", base_name, n);
                if !self.agents.iter().any(|a| a.name == candidate) {
                    break candidate;
                }
                n += 1;
            }
        } else {
            base_name
        };
        self.agents.push(Agent {
            name: display_name,
            agent_id: session_id.to_string(),
            agent_type: Some("session".to_string()),
            model: None,
            color: None,
            status: AgentStatus::Active,
            tokens: Default::default(),
            last_seen_secs: None,
            sub_agent_count: None,
        });
        true
    }

    /// Append a tool event to the ring buffer.
    fn push_tool_event(&mut self, event: ToolEvent) {
        if self.tool_events.len() >= TOOL_EVENT_RING_SIZE {
            self.tool_events.remove(0);
        }
        self.tool_events.push(event);
        self.last_activity_at = std::time::Instant::now();
    }

    /// Try to read the lead session's transcript title and update agent + team name.
    /// Returns the new team name if updated, so the caller can update session_teams.
    fn retry_lead_title(&mut self, transcript_path: Option<&str>, cwd: Option<&str>) -> Option<String> {
        if self.has_lead_title || !self.config.name.starts_with("session:") {
            return None;
        }
        let lead_id = self.config.lead_session_id.clone()?;

        // Helper: derive parent transcript from any transcript path
        let derive_parent_transcript = |tp: &str| -> Option<String> {
            let p = std::path::Path::new(tp);
            // Check if this IS the lead session's transcript (filename matches lead_id)
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if stem == lead_id {
                    if p.exists() { return Some(tp.to_string()); }
                }
            }
            // Otherwise derive from subagent path: .../parent-id/subagents/agent-xxx.jsonl
            let subagents_dir = p.parent()?;
            let session_dir = subagents_dir.parent()?;
            let project_dir = session_dir.parent()?;
            let parent_file = project_dir.join(format!("{}.jsonl", lead_id));
            if parent_file.exists() { Some(parent_file.to_string_lossy().to_string()) } else { None }
        };

        // Try the provided transcript path first, then fall back to recent tool events,
        // then search Claude's projects directory as last resort
        let tp = transcript_path.and_then(|p| derive_parent_transcript(p))
            .or_else(|| {
                self.tool_events.iter().rev().take(10)
                    .filter_map(|e| e.transcript_path.as_deref())
                    .find_map(|tp| derive_parent_transcript(tp))
            })
            .or_else(|| {
                // Last resort: search all project dirs for the lead transcript
                let project_dir = crate::config::Config::claude_home().join("projects");
                if let Ok(entries) = std::fs::read_dir(&project_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            let transcript = p.join(format!("{}.jsonl", lead_id));
                            if transcript.exists() {
                                return Some(transcript.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                None
            })?;

        let title = crate::collector::hook_server::read_session_title(&tp)?;
        let cwd_name = cwd
            .or_else(|| self.tool_events.last().and_then(|e| e.cwd.as_deref()))
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|f| f.to_str())
            .unwrap_or("");

        // If cwd_name is empty, derive project name from transcript path
        let effective_cwd = if cwd_name.is_empty() {
            std::path::Path::new(tp.as_str())
                .parent()
                .and_then(|d| d.file_name())
                .and_then(|f| f.to_str())
                .and_then(|s| s.rsplit('-').next())
                .unwrap_or("")
        } else {
            cwd_name
        };

        let new_name = if effective_cwd.is_empty() || title.starts_with(effective_cwd) {
            title
        } else {
            format!("{}: {}", effective_cwd, title)
        };

        // Update lead agent name
        if let Some(agent) = self.agents.iter_mut().find(|a| a.agent_id == lead_id) {
            agent.name = new_name.clone();
        }

        let new_team_name = format!("session:{}", new_name);
        self.config.name = new_team_name.clone();
        self.has_lead_title = true;
        Some(new_team_name)
    }

    /// Check if team is expired: all agents shutdown + no recent activity.
    /// Uses both in-memory timer AND config file mtime for restart resilience.
    fn is_expired(&self) -> bool {
        let all_inactive = !self.agents.is_empty()
            && self.agents.iter().all(|a| a.status != AgentStatus::Active);
        let all_shutdown = !self.agents.is_empty()
            && self.agents.iter().all(|a| a.status == AgentStatus::Shutdown);

        let expire_secs = if self.config.name.starts_with("session:") {
            SESSION_TAB_EXPIRE_SECS
        } else {
            TEAM_EXPIRE_SECS
        };

        // For session tabs: check transcript file mtime (survives restarts)
        // Session tabs expire when all agents are inactive (not just shutdown)
        if self.config.name.starts_with("session:") {
            if !all_inactive {
                return false;
            }
            // Use lead session's transcript mtime as the authoritative timer
            if let Some(ref sid) = self.config.lead_session_id {
                // Check parent transcript: find it from agents or derive from lead_session_id
                let project_dir = crate::config::Config::claude_home().join("projects");
                if let Ok(entries) = std::fs::read_dir(&project_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            let transcript = p.join(format!("{}.jsonl", sid));
                            if transcript.exists() {
                                if let Ok(meta) = std::fs::metadata(&transcript) {
                                    if let Ok(mtime) = meta.modified() {
                                        if let Ok(elapsed) = mtime.elapsed() {
                                            return elapsed.as_secs() >= expire_secs;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // No transcript found → treat as expired (e.g. fake/manual sessions)
            return true;
        }

        // For real teams: require all agents to be shutdown first
        if !all_shutdown {
            return false;
        }
        // Then check in-memory timer
        if self.last_activity_at.elapsed().as_secs() >= expire_secs {
            return true;
        }
        // Check config file mtime (survives restarts)
        let config_path = crate::config::Config::claude_home()
            .join("teams")
            .join(&self.config.name)
            .join("config.json");
        if let Ok(meta) = std::fs::metadata(&config_path) {
            if let Ok(mtime) = meta.modified() {
                if let Ok(elapsed) = mtime.elapsed() {
                    return elapsed.as_secs() >= expire_secs;
                }
            }
        }
        false
    }

    /// Compute metrics from current state.
    fn compute_metrics(&self) -> Metrics {
        let total_agents = self.agents.len();
        let active_agents = self
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Active)
            .count();
        let idle_agents = self
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Idle)
            .count();

        let tasks: Vec<&TaskFile> = self.tasks.values().collect();
        let total_tasks = tasks.len();
        let completed_tasks = tasks
            .iter()
            .filter(|t| t.status.as_deref() == Some("completed"))
            .count();
        let in_progress_tasks = tasks
            .iter()
            .filter(|t| t.status.as_deref() == Some("in_progress"))
            .count();
        let pending_tasks = tasks
            .iter()
            .filter(|t| t.status.as_deref() == Some("pending"))
            .count();
        let blocked_tasks = tasks
            .iter()
            .filter(|t| !t.blocked_by.is_empty() && t.status.as_deref() != Some("completed"))
            .count();

        let total_tokens: u64 = self.agents.iter().map(|a| a.tokens.total()).sum();
        let estimated_cost_usd: f64 = self.agents.iter()
            .map(|a| a.tokens.estimated_cost_for_model(a.model.as_deref()))
            .sum();

        Metrics {
            total_agents,
            active_agents,
            idle_agents,
            total_tasks,
            completed_tasks,
            in_progress_tasks,
            pending_tasks,
            blocked_tasks,
            total_messages: self.messages.len(),
            total_tool_calls: self.tool_events.len(),
            total_tokens,
            estimated_cost_usd,
        }
    }

    /// Build a TeamSnapshot from the current state.
    fn snapshot(&self) -> TeamSnapshot {
        // Flatten all todos from all agents into one list
        let all_todos: Vec<TodoItem> = self.todos.values().flat_map(|t| t.clone()).collect();
        TeamSnapshot {
            name: self.config.name.clone(),
            description: self.config.description.clone(),
            agents: self.agents.clone(),
            tasks: self.tasks.values().cloned().collect(),
            todos: all_todos,
            messages: self.messages.clone(),
            tool_events: self.tool_events.clone(),
            metrics: self.compute_metrics(),
        }
    }
}

/// An unregistered session (not part of any team).
#[derive(Debug)]
struct UnregisteredSession {
    agent: Agent,
    tool_events: Vec<ToolEvent>,
    todos: Vec<TodoItem>,
    last_event_at: std::time::Instant,
    has_custom_name: bool,
    cwd_name: String,
    transcript_path: Option<String>,
}

const IDLE_TIMEOUT_SECS: u64 = 120;    // 2 minutes → Idle
const ENDED_TIMEOUT_SECS: u64 = 600;   // 10 minutes → Shutdown
const SUBAGENT_IDLE_TIMEOUT_SECS: u64 = 30;    // 30 seconds → Idle for sub-agents
const SUBAGENT_ENDED_TIMEOUT_SECS: u64 = 120;  // 2 minutes → Shutdown for sub-agents

impl UnregisteredSession {
    fn push_tool_event(&mut self, event: ToolEvent) {
        // Update transcript_path if we get one
        if self.transcript_path.is_none() {
            if let Some(ref tp) = event.transcript_path {
                self.transcript_path = Some(tp.clone());
            }
        }

        if self.tool_events.len() >= TOOL_EVENT_RING_SIZE {
            self.tool_events.remove(0);
        }
        self.tool_events.push(event);
        self.last_event_at = std::time::Instant::now();
        self.agent.status = AgentStatus::Active;

        // Retry reading transcript title + model if we don't have them yet
        // Skip title for sub-agents — they should keep their "agent-XXXX" name
        let is_subagent = self.agent.agent_type.as_deref() == Some("subagent");
        if let Some(ref tp) = self.transcript_path {
            if !self.has_custom_name && !is_subagent {
                if let Some(title) = crate::collector::hook_server::read_session_title(tp) {
                    if self.cwd_name.is_empty() || title.starts_with(&self.cwd_name) {
                        self.agent.name = title;
                    } else {
                        self.agent.name = format!("{}: {}", self.cwd_name, title);
                    }
                    self.has_custom_name = true;
                }
            }
            if self.agent.model.is_none() {
                self.agent.model = crate::collector::hook_server::read_session_model(tp);
            }
        }
    }

    /// Update status based on transcript file modification time.
    /// More accurate than hook-based timing because Claude writes to
    /// the transcript even when not calling tools (thinking, generating).
    fn update_timeout_status(&mut self) {
        // Don't skip Shutdown — allow recovery when new tool calls arrive
        // (transcript mtime will be recent if agent is still active)

        // Prefer transcript file mtime over last hook event time
        let elapsed_secs = self.transcript_path.as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs())
            .unwrap_or_else(|| self.last_event_at.elapsed().as_secs());

        let is_subagent = self.agent.agent_type.as_deref() == Some("subagent");
        let ended_threshold = if is_subagent { SUBAGENT_ENDED_TIMEOUT_SECS } else { ENDED_TIMEOUT_SECS };
        let idle_threshold = if is_subagent { SUBAGENT_IDLE_TIMEOUT_SECS } else { IDLE_TIMEOUT_SECS };

        if elapsed_secs >= ended_threshold {
            self.agent.status = AgentStatus::Shutdown;
        } else if elapsed_secs >= idle_threshold {
            self.agent.status = AgentStatus::Idle;
        } else {
            self.agent.status = AgentStatus::Active;
        }
    }
}

/// Push to a global ring buffer.
fn push_global_event(buf: &mut Vec<ToolEvent>, event: ToolEvent) {
    if buf.len() >= TOOL_EVENT_RING_SIZE {
        buf.remove(0);
    }
    buf.push(event);
}

/// The central state store. Processes events from the collector and emits
/// immutable snapshots for the UI layers.
pub struct Store;

impl Store {
    /// Run the event processing loop. Consumes events from `rx` and sends
    /// updated snapshots via `snapshot_tx` after each event.
    pub async fn process_events(
        mut rx: mpsc::Receiver<Event>,
        snapshot_tx: watch::Sender<StoreSnapshot>,
    ) {
        let mut teams: HashMap<String, TeamState> = HashMap::new();
        let mut unregistered: Vec<UnregisteredSession> = Vec::new();
        let mut global_events: Vec<ToolEvent> = Vec::new();
        let mut all_sessions: HashMap<String, Agent> = HashMap::new();
        let mut global_todos: HashMap<String, Vec<TodoItem>> = HashMap::new();
        let mut child_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut session_teams: HashMap<String, String> = HashMap::new();

        // Persistence: load saved state, track dirty flag for save debouncing
        let mut dirty = false;
        let mut last_save = std::time::Instant::now();
        if let Some(persisted) = crate::store::persist::load() {
            for ps in persisted.sessions {
                // Skip sessions without transcript or whose transcript doesn't exist / is older than 24h
                match ps.transcript_path {
                    None => continue,
                    Some(ref tp) => {
                        let path = std::path::Path::new(tp);
                        if !path.exists() {
                            continue;
                        }
                        if let Ok(meta) = path.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                if mtime.elapsed().map(|d| d.as_secs() > 86400).unwrap_or(true) {
                                    continue;
                                }
                            }
                        }
                    }
                }
                // Skip sub-agents whose parent transcript doesn't exist
                if let Some(ref pid) = ps.parent_id {
                    if let Some(ref tp) = ps.transcript_path {
                        // Sub-agent transcript should be under .../subagents/
                        // Parent transcript: derive from sub-agent path or check directly
                        let parent_transcript_exists = std::path::Path::new(tp)
                            .parent() // subagents/
                            .and_then(|p| p.parent()) // {session_id}/
                            .and_then(|p| p.parent()) // project dir
                            .map(|dir| dir.join(format!("{}.jsonl", pid)).exists())
                            .unwrap_or(false);
                        // Also skip if transcript doesn't point to subagents/ dir
                        let is_subagent_path = tp.contains("/subagents/");
                        if !is_subagent_path || !parent_transcript_exists {
                            continue;
                        }
                    }
                }
                let is_subagent = ps.parent_id.is_some();
                let mut agent = Agent {
                    name: ps.name,
                    agent_id: ps.agent_id.clone(),
                    agent_type: ps.agent_type,
                    model: ps.model,
                    color: None,
                    status: AgentStatus::Shutdown, // recomputed from transcript mtime
                    tokens: ps.tokens,
                    last_seen_secs: None,
                sub_agent_count: None,
                };
                if is_subagent {
                    child_sessions.insert(ps.agent_id.clone());
                }

                let cwd_name = ps.cwd.as_deref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .and_then(|f| f.to_str())
                    .unwrap_or("").to_string();

                // Re-read title on restore if the persisted name looks like just a CWD fallback
                let looks_like_fallback = agent.name == cwd_name
                    || agent.name.starts_with("session-")
                    || (!agent.name.contains(": ") && cwd_name.is_empty());
                let has_title = if !is_subagent && !agent.name.contains(": ") && looks_like_fallback {
                    if let Some(ref tp) = ps.transcript_path {
                        if let Some(title) = crate::collector::hook_server::read_session_title(tp) {
                            // If cwd_name is empty, try deriving project name from transcript path
                            // e.g. .../-Users-jerry-Documents-ReAgent3/session.jsonl → ReAgent3
                            let effective_cwd = if cwd_name.is_empty() {
                                std::path::Path::new(tp.as_str())
                                    .parent()
                                    .and_then(|d| d.file_name())
                                    .and_then(|f| f.to_str())
                                    .and_then(|s| s.rsplit('-').next())
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                cwd_name.clone()
                            };
                            let new_name = if effective_cwd.is_empty() || title.starts_with(&effective_cwd) {
                                title
                            } else {
                                format!("{}: {}", effective_cwd, title)
                            };
                            agent.name = new_name;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    true
                };

                all_sessions.insert(ps.agent_id.clone(), agent.clone());

                unregistered.push(UnregisteredSession {
                    agent,
                    tool_events: Vec::new(),
                    todos: Vec::new(),
                    last_event_at: std::time::Instant::now() - std::time::Duration::from_secs(ENDED_TIMEOUT_SECS + 1),
                    has_custom_name: has_title,
                    cwd_name,
                    transcript_path: ps.transcript_path,
                });

                // Rebuild session_teams for sub-agents
                if let Some(ref pid) = ps.parent_id {
                    if !session_teams.contains_key(pid) {
                        let parent_session = unregistered.iter().find(|s| s.agent.agent_id == *pid);
                        let parent_name = parent_session
                            .map(|s| s.agent.name.clone())
                            .unwrap_or_else(|| format!("session-{}", &pid[..8.min(pid.len())]));
                        let parent_has_title = parent_session.map(|s| s.has_custom_name).unwrap_or(false);
                        let team_name = format!("session:{}", parent_name);
                        session_teams.insert(pid.clone(), team_name.clone());

                        let mut ts = TeamState::new(TeamConfig {
                            name: team_name.clone(),
                            description: format!("Session: {}", parent_name),
                            created_at: None,
                            lead_agent_id: None,
                            lead_session_id: Some(pid.clone()),
                            members: Vec::new(),
                        });
                        ts.has_lead_title = parent_has_title;
                        if let Some(parent) = unregistered.iter().find(|s| s.agent.agent_id == *pid) {
                            ts.agents.push(parent.agent.clone());
                        }
                        teams.insert(team_name, ts);
                    }
                    if let Some(team_name) = session_teams.get(pid) {
                        if let Some(ts) = teams.get_mut(team_name) {
                            if let Some(child) = unregistered.iter().find(|s| s.agent.agent_id == ps.agent_id) {
                                if !ts.agents.iter().any(|a| a.agent_id == ps.agent_id) {
                                    ts.agents.push(child.agent.clone());
                                }
                            }
                        }
                    }
                }
            }
            tracing::info!("restored {} sessions from state file", unregistered.len());
        }

        while let Some(event) = rx.recv().await {
            match event {
                Event::TeamUpdate { team_name, config } => {
                    match teams.get_mut(&team_name) {
                        Some(state) => state.update_config(config),
                        None => {
                            teams.insert(team_name.clone(), TeamState::new(config));
                        }
                    }
                }
                Event::TaskUpdate { team_name, task } => {
                    if let Some(state) = teams.get_mut(&team_name) {
                        state.upsert_task(task);
                    } else {
                        let mut ts = TeamState::new(TeamConfig {
                            name: team_name.clone(),
                            description: String::new(),
                            created_at: None,
                            lead_agent_id: None,
                            lead_session_id: None,
                            members: Vec::new(),
                        });
                        ts.upsert_task(task);
                        teams.insert(team_name.clone(), ts);
                    }
                }
                Event::MessageUpdate {
                    team_name,
                    agent_name,
                    messages,
                } => {
                    if let Some(state) = teams.get_mut(&team_name) {
                        state.process_messages(&agent_name, messages);
                        // Sync agent status changes to all_sessions
                        for agent in &state.agents {
                            if let Some(s) = all_sessions.get_mut(&agent.agent_id) {
                                s.status = agent.status.clone();
                            }
                        }
                    }
                }
                Event::ToolCall(tool_event) => {
                    let session_id = &tool_event.agent_name;
                    let mut found = false;

                    // Extract subagent info (set by hook_server from hook payload or transcript path)
                    let parent_id = tool_event.subagent_info.as_ref().map(|(pid, _, _)| pid.clone());
                    let subagent_type = tool_event.subagent_info.as_ref().and_then(|(_, _, t)| t.clone());
                    let is_subagent = parent_id.is_some();

                    // 1. Match by agent name/id in existing teams
                    let mut team_rename: Option<(String, String)> = None;
                    for (tname, state) in teams.iter_mut() {
                        if state.agents.iter().any(|a| a.name == *session_id || a.agent_id == *session_id) {
                            state.push_tool_event(tool_event.clone());
                            if let Some(new_name) = state.retry_lead_title(tool_event.transcript_path.as_deref(), tool_event.cwd.as_deref()) {
                                team_rename = Some((tname.clone(), new_name));
                            }
                            found = true;
                            break;
                        }
                    }

                    // 2. Match by lead_session_id
                    if !found {
                        for (tname, state) in teams.iter_mut() {
                            if state.config.lead_session_id.as_deref() == Some(session_id.as_str()) {
                                state.ensure_agent(session_id, tool_event.cwd.as_deref(), tool_event.transcript_path.as_deref());
                                state.push_tool_event(tool_event.clone());
                                if let Some(new_name) = state.retry_lead_title(tool_event.transcript_path.as_deref(), tool_event.cwd.as_deref()) {
                                    team_rename = Some((tname.clone(), new_name));
                                }
                                found = true;
                                break;
                            }
                        }
                    }

                    // Apply team rename if title was resolved
                    if let Some((old_name, new_name)) = team_rename {
                        if let Some(state) = teams.remove(&old_name) {
                            // Update all_sessions with the new lead agent name
                            if let Some(ref lid) = state.config.lead_session_id {
                                if let Some(agent) = state.agents.iter().find(|a| &a.agent_id == lid) {
                                    if let Some(a) = all_sessions.get_mut(lid) {
                                        a.name = agent.name.clone();
                                    }
                                }
                            }
                            teams.insert(new_name.clone(), state);
                            // Update session_teams mapping
                            for (_, tn) in session_teams.iter_mut() {
                                if *tn == old_name {
                                    *tn = new_name.clone();
                                }
                            }
                        }
                        dirty = true;
                    }

                    // 3. Route to unregistered sessions
                    if !found {
                        let existing = unregistered.iter_mut().find(|s| s.agent.agent_id == *session_id);
                        if let Some(session) = existing {
                            session.push_tool_event(tool_event.clone());
                        } else {
                            if is_subagent {
                                child_sessions.insert(session_id.to_string());
                            }

                            // Initial name: CWD dir > truncated session_id
                            let cwd_name = tool_event.cwd.as_deref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .and_then(|f| f.to_str())
                                .map(String::from)
                                .unwrap_or_default();
                            let base_name = if is_subagent {
                                // Sub-agents: use agent_type if available (e.g. "Explore", "Plan")
                                subagent_type.clone().unwrap_or_else(|| {
                                    let short_id = if session_id.len() > 8 { &session_id[..8] } else { session_id };
                                    format!("agent-{}", short_id)
                                })
                            } else if cwd_name.is_empty() {
                                if session_id.len() > 8 {
                                    format!("session-{}", &session_id[..8])
                                } else {
                                    session_id.to_string()
                                }
                            } else {
                                cwd_name.clone()
                            };

                            // Deduplicate name
                            let name_exists = |name: &str| {
                                unregistered.iter().any(|s| s.agent.name == name)
                                    || teams.values().any(|t| t.agents.iter().any(|a| a.name == name))
                            };
                            let display_name = if name_exists(&base_name) {
                                let mut n = 2;
                                loop {
                                    let candidate = format!("{}-{}", base_name, n);
                                    if !name_exists(&candidate) {
                                        break candidate;
                                    }
                                    n += 1;
                                }
                            } else {
                                base_name
                            };

                            let mut session = UnregisteredSession {
                                agent: Agent {
                                    name: display_name,
                                    agent_id: session_id.to_string(),
                                    agent_type: Some(if is_subagent { "subagent" } else { "session" }.to_string()),
                                    model: None,
                                    color: None,
                                    status: AgentStatus::Active,
                                    tokens: Default::default(),
                                    last_seen_secs: None,
                sub_agent_count: None,
                                },
                                tool_events: Vec::new(),
                                todos: Vec::new(),
                                last_event_at: std::time::Instant::now(),
                                has_custom_name: false,
                                cwd_name,
                                transcript_path: tool_event.transcript_path.clone(),
                            };
                            session.push_tool_event(tool_event.clone());
                            unregistered.push(session);
                        }

                        // Track in all_sessions
                        if !all_sessions.contains_key(session_id) {
                            if let Some(agent) = unregistered.iter().find(|s| s.agent.agent_id == *session_id).map(|s| s.agent.clone()) {
                                all_sessions.insert(session_id.to_string(), agent);
                                dirty = true;
                            }
                        }
                    }

                    // Create dynamic tab for sessions with sub-agents
                    if let Some(ref pid) = parent_id {
                        // Ensure parent is registered (may be missing if transcript is old)
                        if !unregistered.iter().any(|s| s.agent.agent_id == *pid) {
                            // Derive parent transcript path from sub-agent's transcript path
                            let parent_transcript = tool_event.transcript_path.as_deref().and_then(|tp| {
                                let p = std::path::Path::new(tp);
                                // .../parent-id/subagents/agent-xxx.jsonl → .../parent-id.jsonl
                                let subagents_dir = p.parent()?; // subagents/
                                let session_dir = subagents_dir.parent()?; // parent-id/
                                let project_dir = session_dir.parent()?;
                                let parent_file = project_dir.join(format!("{}.jsonl", pid));
                                if parent_file.exists() { Some(parent_file.to_string_lossy().to_string()) } else { None }
                            });
                            let parent_cwd = tool_event.cwd.clone();

                            // Try to read title from parent transcript
                            let parent_title = parent_transcript.as_deref()
                                .and_then(crate::collector::hook_server::read_session_title);
                            let cwd_name = parent_cwd.as_deref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .and_then(|f| f.to_str())
                                .unwrap_or("").to_string();
                            let display_name = match (&cwd_name, &parent_title) {
                                (c, Some(t)) if !c.is_empty() => format!("{}: {}", c, t),
                                (c, None) if !c.is_empty() => c.clone(),
                                (_, Some(t)) => t.clone(),
                                _ => format!("session-{}", &pid[..8.min(pid.len())]),
                            };

                            let parent_agent = Agent {
                                name: display_name,
                                agent_id: pid.clone(),
                                agent_type: Some("session".to_string()),
                                model: None,
                                color: None,
                                status: AgentStatus::Active, // will be corrected by update_timeout_status
                                tokens: Default::default(),
                                last_seen_secs: None,
                sub_agent_count: None,
                            };
                            all_sessions.insert(pid.clone(), parent_agent.clone());
                            unregistered.push(UnregisteredSession {
                                agent: parent_agent,
                                tool_events: Vec::new(),
                                todos: Vec::new(),
                                last_event_at: std::time::Instant::now(),
                                has_custom_name: parent_title.is_some(),
                                cwd_name,
                                transcript_path: parent_transcript,
                            });
                        }

                        if !session_teams.contains_key(pid) {
                            let parent_session = unregistered.iter().find(|s| s.agent.agent_id == *pid);
                            let parent_name = parent_session
                                .map(|s| s.agent.name.clone())
                                .unwrap_or_else(|| format!("session-{}", &pid[..8.min(pid.len())]));
                            let parent_has_title = parent_session.map(|s| s.has_custom_name).unwrap_or(false);
                            let team_name = format!("session:{}", parent_name);
                            session_teams.insert(pid.clone(), team_name.clone());

                            let mut ts = TeamState::new(TeamConfig {
                                name: team_name.clone(),
                                description: format!("Session: {}", parent_name),
                                created_at: None,
                                lead_agent_id: None,
                                lead_session_id: Some(pid.clone()),
                                members: Vec::new(),
                            });
                            ts.has_lead_title = parent_has_title;
                            if let Some(parent) = unregistered.iter().find(|s| s.agent.agent_id == *pid) {
                                ts.agents.push(parent.agent.clone());
                            }
                            teams.insert(team_name, ts);
                        }
                        // Add child to the dynamic team
                        if let Some(team_name) = session_teams.get(pid).cloned() {
                            let mut renamed: Option<String> = None;
                            if let Some(ts) = teams.get_mut(&team_name) {
                                if let Some(child) = unregistered.iter().find(|s| s.agent.agent_id == *session_id) {
                                    if !ts.agents.iter().any(|a| a.agent_id == *session_id) {
                                        ts.agents.push(child.agent.clone());
                                    }
                                }
                                ts.push_tool_event(tool_event.clone());
                                // Retry lead title on subagent events too
                                if let Some(new_name) = ts.retry_lead_title(None, tool_event.cwd.as_deref()) {
                                    renamed = Some(new_name);
                                }
                                // Keep session tab alive based on most recent sub-agent transcript mtime
                                if let Some(ref tp) = tool_event.transcript_path {
                                    if let Ok(meta) = std::fs::metadata(tp) {
                                        if let Ok(mtime) = meta.modified() {
                                            if let Ok(age) = mtime.elapsed() {
                                                if age.as_secs() < SESSION_TAB_EXPIRE_SECS {
                                                    ts.last_activity_at = std::time::Instant::now() - age;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Apply rename outside borrow
                            if let Some(new_name) = renamed {
                                if let Some(state) = teams.remove(&team_name) {
                                    teams.insert(new_name.clone(), state);
                                }
                                if let Some(tn) = session_teams.get_mut(pid) {
                                    *tn = new_name;
                                }
                            }
                        }
                    }

                    // Always push to global events
                    push_global_event(&mut global_events, tool_event);
                }
                Event::TokenUpdate { session_id, usage } => {
                    // Update unregistered sessions first (single source of truth)
                    if let Some(session) = unregistered.iter_mut().find(|s| s.agent.agent_id == session_id) {
                        session.agent.tokens = usage.clone();
                    }
                    // Update teams (by agent_id/name or lead_session_id)
                    for state in teams.values_mut() {
                        if let Some(agent) = state.agents.iter_mut().find(|a| {
                            a.agent_id == session_id || a.name == session_id
                        }) {
                            agent.tokens = usage.clone();
                            break;
                        }
                        if state.config.lead_session_id.as_deref() == Some(&session_id) {
                            if let Some(agent) = state.agents.iter_mut().find(|a| a.agent_id == session_id) {
                                agent.tokens = usage.clone();
                                break;
                            }
                        }
                    }
                    // Sync to all_sessions
                    if let Some(s) = all_sessions.get_mut(&session_id) {
                        s.tokens = usage;
                        dirty = true;
                    }
                }
                Event::TodoUpdate { session_id, todos } => {
                    let mut found = false;
                    // Check teams
                    for state in teams.values_mut() {
                        if state.agents.iter().any(|a| a.agent_id == session_id || a.name == session_id) {
                            state.todos.insert(session_id.clone(), todos.clone());
                            found = true;
                            break;
                        }
                    }
                    // Check unregistered
                    if !found {
                        if let Some(session) = unregistered.iter_mut().find(|s| s.agent.agent_id == session_id) {
                            session.todos = todos.clone();
                        }
                    }
                    // Store globally for ALL tab
                    global_todos.insert(session_id, todos);
                }
                Event::SubAgentName { agent_id, name } => {
                    // Rename sub-agent using description from parent's Agent tool call
                    let should_rename = unregistered.iter().any(|s| s.agent.agent_id == agent_id && !s.has_custom_name);
                    if should_rename {
                        // Deduplicate name (check before mutating)
                        let name_exists = |n: &str| {
                            unregistered.iter().any(|s| s.agent.name == n && s.agent.agent_id != agent_id)
                                || teams.values().any(|t| t.agents.iter().any(|a| a.name == n))
                        };
                        let display_name = if name_exists(&name) {
                            let mut n = 2;
                            loop {
                                let candidate = format!("{}-{}", name, n);
                                if !name_exists(&candidate) { break candidate; }
                                n += 1;
                            }
                        } else {
                            name
                        };
                        // Now mutate
                        if let Some(session) = unregistered.iter_mut().find(|s| s.agent.agent_id == agent_id) {
                            session.agent.name = display_name.clone();
                            session.has_custom_name = true;
                        }
                        if let Some(a) = all_sessions.get_mut(&agent_id) {
                            a.name = display_name.clone();
                        }
                        for ts in teams.values_mut() {
                            if let Some(a) = ts.agents.iter_mut().find(|a| a.agent_id == agent_id) {
                                a.name = display_name.clone();
                            }
                        }
                        dirty = true;
                    }
                }
                Event::Tick => {
                    // Persist state (debounced: at most every 30s)
                    if dirty && last_save.elapsed().as_secs() >= 30 {
                        let sessions: Vec<crate::store::persist::PersistedSession> = unregistered.iter().map(|s| {
                            let parent_id = if child_sessions.contains(&s.agent.agent_id) {
                                // Find parent from session_teams (reverse lookup)
                                session_teams.iter()
                                    .find(|(_, tn)| {
                                        teams.get(*tn).map(|ts| ts.agents.iter().any(|a| a.agent_id == s.agent.agent_id)).unwrap_or(false)
                                    })
                                    .map(|(pid, _)| pid.clone())
                            } else {
                                None
                            };
                            crate::store::persist::PersistedSession {
                                agent_id: s.agent.agent_id.clone(),
                                name: s.agent.name.clone(),
                                agent_type: s.agent.agent_type.clone(),
                                model: s.agent.model.clone(),
                                transcript_path: s.transcript_path.clone(),
                                cwd: s.tool_events.iter().find_map(|e| e.cwd.clone()),
                                tokens: s.agent.tokens.clone(),
                                parent_id,
                            }
                        }).collect();
                        crate::store::persist::save(&crate::store::persist::PersistedState {
                            version: 1,
                            saved_at: chrono::Utc::now().to_rfc3339(),
                            sessions,
                        });
                        dirty = false;
                        last_save = std::time::Instant::now();
                    }

                    // Discover new sub-agent transcripts from active parent sessions
                    let mut new_subagents: Vec<(String, String, String)> = Vec::new(); // (agent_id, parent_session_id, transcript_path)
                    for session in &unregistered {
                        if session.agent.status != AgentStatus::Active && session.agent.status != AgentStatus::Idle {
                            continue;
                        }
                        if let Some(ref tp) = session.transcript_path {
                            // Check for subagents/ dir next to the transcript
                            let parent_dir = std::path::Path::new(tp.as_str()).parent();
                            let session_dir = parent_dir.and_then(|p| {
                                // tp is like .../{session-id}.jsonl — the session dir is sibling
                                let stem = std::path::Path::new(tp.as_str()).file_stem()?.to_str()?;
                                Some(p.join(stem))
                            });
                            if let Some(sd) = session_dir {
                                let subagents_dir = sd.join("subagents");
                                if subagents_dir.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&subagents_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                                                if let Some((pid, aid)) = crate::collector::hook_server::parse_subagent_path(
                                                    &path.to_string_lossy()
                                                ) {
                                                    // Only add if not already known
                                                    if !child_sessions.contains(&aid)
                                                        && !unregistered.iter().any(|s| s.agent.agent_id == aid)
                                                        && !teams.values().any(|t| t.agents.iter().any(|a| a.agent_id == aid))
                                                    {
                                                        new_subagents.push((aid, pid, path.to_string_lossy().to_string()));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Register discovered sub-agents
                    for (agent_id, parent_session_id, transcript_path) in new_subagents {
                        let _cwd = unregistered.iter()
                            .find(|s| s.agent.agent_id == parent_session_id)
                            .and_then(|s| s.agent.agent_type.as_ref().and(
                                // Reuse parent's cwd from tool events
                                s.tool_events.first().and_then(|e| e.cwd.clone())
                            ));
                        child_sessions.insert(agent_id.clone());
                        let short_id = if agent_id.len() > 8 { &agent_id[..8] } else { &agent_id };
                        let name = format!("agent-{}", short_id);
                        let mut agent = Agent {
                            name,
                            agent_id: agent_id.clone(),
                            agent_type: Some("subagent".to_string()),
                            model: None,
                            color: None,
                            status: AgentStatus::Active,
                            tokens: Default::default(),
                            last_seen_secs: None,
                sub_agent_count: None,
                        };
                        // Read token usage + model from sub-agent transcript
                        if let Some((usage, model)) = crate::collector::hook_server::read_transcript_usage_and_model(&transcript_path) {
                            agent.tokens = usage;
                            agent.model = model;
                        }
                        all_sessions.insert(agent_id.clone(), agent.clone());
                        let session = UnregisteredSession {
                            agent,
                            tool_events: Vec::new(),
                            todos: Vec::new(),
                            last_event_at: std::time::Instant::now(),
                            has_custom_name: false,
                            cwd_name: String::new(),
                            transcript_path: Some(transcript_path),
                        };
                        unregistered.push(session);
                        dirty = true;

                        // Create/update dynamic tab
                        let pid = &parent_session_id;
                        if !session_teams.contains_key(pid) {
                            let parent_session = unregistered.iter().find(|s| s.agent.agent_id == *pid);
                            let parent_name = parent_session
                                .map(|s| s.agent.name.clone())
                                .unwrap_or_else(|| format!("session-{}", &pid[..8.min(pid.len())]));
                            let parent_has_title = parent_session.map(|s| s.has_custom_name).unwrap_or(false);
                            let team_name = format!("session:{}", parent_name);
                            session_teams.insert(pid.clone(), team_name.clone());
                            let mut ts = TeamState::new(TeamConfig {
                                name: team_name.clone(),
                                description: format!("Session: {}", parent_name),
                                created_at: None,
                                lead_agent_id: None,
                                lead_session_id: Some(pid.clone()),
                                members: Vec::new(),
                            });
                            ts.has_lead_title = parent_has_title;
                            if let Some(parent) = unregistered.iter().find(|s| s.agent.agent_id == *pid) {
                                ts.agents.push(parent.agent.clone());
                            }
                            teams.insert(team_name, ts);
                        }
                        if let Some(team_name) = session_teams.get(pid) {
                            if let Some(ts) = teams.get_mut(team_name) {
                                if !ts.agents.iter().any(|a| a.agent_id == agent_id) {
                                    if let Some(child) = unregistered.iter().find(|s| s.agent.agent_id == agent_id) {
                                        ts.agents.push(child.agent.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Re-read token usage for sub-agents with zero tokens
            for session in unregistered.iter_mut() {
                if session.agent.agent_type.as_deref() == Some("subagent")
                    && session.agent.tokens.total() == 0
                {
                    if let Some(ref tp) = session.transcript_path {
                        if let Some((usage, model)) = crate::collector::hook_server::read_transcript_usage_and_model(tp) {
                            session.agent.tokens = usage;
                            if session.agent.model.is_none() {
                                session.agent.model = model;
                            }
                            if let Some(s) = all_sessions.get_mut(&session.agent.agent_id) {
                                s.tokens = session.agent.tokens.clone();
                                s.model = session.agent.model.clone();
                                dirty = true;
                            }
                        }
                    }
                }
            }

            // Update timeout-based status for unregistered sessions
            for session in unregistered.iter_mut() {
                session.update_timeout_status();
            }
            // Update timeout-based status for team agents
            for (team_name, state) in teams.iter_mut() {
                if team_name.starts_with("session:") {
                    // Session tabs: sync agent status from unregistered sessions (single source of truth)
                    for agent in &mut state.agents {
                        if let Some(session) = unregistered.iter().find(|s| s.agent.agent_id == agent.agent_id) {
                            agent.status = session.agent.status.clone();
                            agent.name = session.agent.name.clone();
                            agent.tokens = session.agent.tokens.clone();
                            agent.model = session.agent.model.clone();
                            agent.last_seen_secs = Some(session.last_event_at.elapsed().as_secs());
                        }
                    }
                } else {
                    // Real team tabs: use team-level timeout
                    let elapsed = state.last_activity_at.elapsed().as_secs();
                    if elapsed >= ENDED_TIMEOUT_SECS {
                        for agent in &mut state.agents {
                            if agent.status != AgentStatus::Shutdown {
                                agent.status = AgentStatus::Shutdown;
                            }
                        }
                    } else if elapsed >= IDLE_TIMEOUT_SECS {
                        for agent in &mut state.agents {
                            if agent.status == AgentStatus::Active {
                                agent.status = AgentStatus::Idle;
                            }
                        }
                    }
                }
            }
            // Sync status + name + last_seen to all_sessions
            for session in &unregistered {
                if let Some(s) = all_sessions.get_mut(&session.agent.agent_id) {
                    s.status = session.agent.status.clone();
                    s.name = session.agent.name.clone();
                    s.last_seen_secs = Some(session.last_event_at.elapsed().as_secs());
                }
            }

            // Build ALL snapshot — top-level sessions only (exclude sub-agents)
            // Set sub_agent_count for sessions that have sub-agents
            let mut all_agents: Vec<Agent> = all_sessions.values()
                .filter(|a| !child_sessions.contains(&a.agent_id))
                .cloned()
                .map(|mut a| {
                    if let Some(team_name) = session_teams.get(&a.agent_id) {
                        if let Some(ts) = teams.get(team_name) {
                            let count = ts.agents.iter()
                                .filter(|ag| ag.agent_type.as_deref() == Some("subagent"))
                                .count();
                            if count > 0 {
                                a.sub_agent_count = Some(count as u32);
                            }
                        }
                    }
                    a
                })
                .collect();
            // Sort: Active first, then by cost descending
            all_agents.sort_by(|a, b| {
                let a_active = a.status == AgentStatus::Active;
                let b_active = b.status == AgentStatus::Active;
                b_active.cmp(&a_active).then(
                    b.tokens.estimated_cost_for_model(b.model.as_deref())
                        .partial_cmp(&a.tokens.estimated_cost_for_model(a.model.as_deref()))
                        .unwrap_or(std::cmp::Ordering::Equal)
                )
            });

            let all_tasks: Vec<TaskFile> = teams.values()
                .flat_map(|t| t.tasks.values().cloned())
                .collect();

            let all_messages: Vec<Message> = teams.values()
                .flat_map(|t| t.messages.clone())
                .collect();

            let all_metrics = compute_all_metrics(&all_agents, &all_tasks, &all_messages, &global_events);

            let all_todos: Vec<TodoItem> = global_todos.values().flat_map(|t| t.clone()).collect();

            let all_snapshot = TeamSnapshot {
                name: "all".to_string(),
                description: "All sessions".to_string(),
                agents: all_agents,
                tasks: all_tasks,
                todos: all_todos,
                messages: all_messages,
                tool_events: global_events.clone(),
                metrics: all_metrics,
            };

            // Build per-team snapshots: hide expired teams
            let mut team_snapshots: Vec<TeamSnapshot> = teams.values()
                .filter(|ts| {
                    if ts.is_expired() { return false; }
                    true
                })
                .map(|ts| ts.snapshot())
                .collect();
            // Sort: active tabs first, then by name
            team_snapshots.sort_by(|a, b| {
                let a_active = a.agents.iter().any(|ag| ag.status == AgentStatus::Active);
                let b_active = b.agents.iter().any(|ag| ag.status == AgentStatus::Active);
                b_active.cmp(&a_active).then(a.name.cmp(&b.name))
            });

            // Final: [ALL, team1, team2, ...]
            let mut all_teams = vec![all_snapshot];
            all_teams.extend(team_snapshots);

            let snapshot = StoreSnapshot { teams: all_teams };
            let _ = snapshot_tx.send(snapshot);
        }
    }
}

/// Compute aggregate metrics across all data.
fn compute_all_metrics(agents: &[Agent], tasks: &[TaskFile], messages: &[Message], events: &[ToolEvent]) -> Metrics {
    let total_agents = agents.len();
    let active_agents = agents.iter().filter(|a| a.status == AgentStatus::Active).count();
    let idle_agents = agents.iter().filter(|a| a.status == AgentStatus::Idle).count();

    let total_tasks = tasks.len();
    let completed_tasks = tasks.iter().filter(|t| t.status.as_deref() == Some("completed")).count();
    let in_progress_tasks = tasks.iter().filter(|t| t.status.as_deref() == Some("in_progress")).count();
    let pending_tasks = tasks.iter().filter(|t| t.status.as_deref() == Some("pending")).count();
    let blocked_tasks = tasks.iter().filter(|t| !t.blocked_by.is_empty() && t.status.as_deref() != Some("completed")).count();

    let total_tokens: u64 = agents.iter().map(|a| a.tokens.total()).sum();
    let estimated_cost_usd: f64 = agents.iter()
        .map(|a| a.tokens.estimated_cost_for_model(a.model.as_deref()))
        .sum();

    Metrics {
        total_agents,
        active_agents,
        idle_agents,
        total_tasks,
        completed_tasks,
        in_progress_tasks,
        pending_tasks,
        blocked_tasks,
        total_messages: messages.len(),
        total_tool_calls: events.len(),
        total_tokens,
        estimated_cost_usd,
    }
}
