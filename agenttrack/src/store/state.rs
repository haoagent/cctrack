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
}

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
        });
        true
    }

    /// Append a tool event to the ring buffer.
    fn push_tool_event(&mut self, event: ToolEvent) {
        if self.tool_events.len() >= TOOL_EVENT_RING_SIZE {
            self.tool_events.remove(0);
        }
        self.tool_events.push(event);
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
        let estimated_cost_usd: f64 = self.agents.iter().map(|a| a.tokens.estimated_cost_usd()).sum();

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
}

impl UnregisteredSession {
    fn push_tool_event(&mut self, event: ToolEvent) {
        if self.tool_events.len() >= TOOL_EVENT_RING_SIZE {
            self.tool_events.remove(0);
        }
        self.tool_events.push(event);
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
        // Track only top-level sessions (session_id → Agent info)
        // Excludes: team members, Agent-tool subagents
        let mut all_sessions: HashMap<String, Agent> = HashMap::new();
        // Global todos: session_id → latest todo list (for ALL tab)
        let mut global_todos: HashMap<String, Vec<TodoItem>> = HashMap::new();
        // Track pending Agent tool spawns: (cwd, timestamp) for subagent detection
        let mut pending_spawns: Vec<(String, std::time::Instant)> = Vec::new();
        // Known child session_ids (spawned by Agent tool)
        let mut child_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();

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

                    // 1. Match by agent name/id in existing teams
                    for state in teams.values_mut() {
                        if state.agents.iter().any(|a| a.name == *session_id || a.agent_id == *session_id) {
                            state.push_tool_event(tool_event.clone());
                            found = true;
                            break;
                        }
                    }

                    // 2. Match by lead_session_id
                    if !found {
                        for state in teams.values_mut() {
                            if state.config.lead_session_id.as_deref() == Some(session_id.as_str()) {
                                state.ensure_agent(session_id, tool_event.cwd.as_deref(), tool_event.transcript_path.as_deref());
                                state.push_tool_event(tool_event.clone());
                                found = true;
                                break;
                            }
                        }
                    }

                    // 3. Route to unregistered sessions
                    if !found {
                        let existing = unregistered.iter_mut().find(|s| s.agent.agent_id == *session_id);
                        if let Some(session) = existing {
                            session.push_tool_event(tool_event.clone());
                        } else {
                            // Detect subagent: if an Agent tool was called from same cwd recently
                            let is_subagent = if let Some(cwd) = tool_event.cwd.as_deref() {
                                let now = std::time::Instant::now();
                                // Clean up old spawns (> 60s)
                                pending_spawns.retain(|(_, ts)| now.duration_since(*ts).as_secs() < 60);
                                pending_spawns.iter().any(|(spawn_cwd, _)| spawn_cwd == cwd)
                            } else {
                                false
                            };

                            if is_subagent {
                                child_sessions.insert(session_id.to_string());
                            }

                            // Session name: CWD directory name > truncated session_id
                            let base_name = tool_event.cwd.as_deref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .and_then(|f| f.to_str())
                                .map(String::from)
                                .unwrap_or_else(|| {
                                    if session_id.len() > 8 {
                                        format!("session-{}", &session_id[..8])
                                    } else {
                                        session_id.to_string()
                                    }
                                });

                            // Deduplicate name among unregistered + team agents
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
                                },
                                tool_events: Vec::new(),
                                todos: Vec::new(),
                            };
                            session.push_tool_event(tool_event.clone());
                            unregistered.push(session);
                        }

                        // Track in all_sessions — only top-level sessions (not team members, not subagents)
                        if !child_sessions.contains(session_id) && !all_sessions.contains_key(session_id) {
                            if let Some(agent) = unregistered.iter().find(|s| s.agent.agent_id == *session_id).map(|s| s.agent.clone()) {
                                all_sessions.insert(session_id.to_string(), agent);
                            }
                        }
                    }

                    // Record Agent tool calls for subagent detection
                    if tool_event.tool_name == "Agent" {
                        if let Some(cwd) = tool_event.cwd.as_deref() {
                            pending_spawns.push((cwd.to_string(), std::time::Instant::now()));
                        }
                    }

                    // Always push to global events
                    push_global_event(&mut global_events, tool_event);
                }
                Event::TokenUpdate { session_id, usage } => {
                    let mut found = false;
                    // Check teams (by agent_id/name or lead_session_id)
                    for state in teams.values_mut() {
                        if let Some(agent) = state.agents.iter_mut().find(|a| {
                            a.agent_id == session_id || a.name == session_id
                        }) {
                            agent.tokens = usage.clone();
                            found = true;
                            break;
                        }
                        if state.config.lead_session_id.as_deref() == Some(&session_id) {
                            if let Some(agent) = state.agents.iter_mut().find(|a| a.agent_id == session_id) {
                                agent.tokens = usage.clone();
                                found = true;
                                break;
                            }
                        }
                    }
                    // Check unregistered sessions
                    if !found {
                        if let Some(session) = unregistered.iter_mut().find(|s| s.agent.agent_id == session_id) {
                            session.agent.tokens = usage.clone();
                        }
                    }
                    // Sync to all_sessions
                    if let Some(s) = all_sessions.get_mut(&session_id) {
                        s.tokens = usage;
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
            }

            // Build ALL snapshot — only sessions that have actually sent events
            let all_agents: Vec<Agent> = all_sessions.values().cloned().collect();

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

            // Build per-team snapshots (sorted by name)
            let mut team_snapshots: Vec<TeamSnapshot> = teams.values()
                .map(|ts| ts.snapshot())
                .collect();
            team_snapshots.sort_by(|a, b| a.name.cmp(&b.name));

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
    let estimated_cost_usd: f64 = agents.iter().map(|a| a.tokens.estimated_cost_usd()).sum();

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
