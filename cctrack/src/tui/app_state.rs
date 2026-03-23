use ratatui::widgets::{TableState, ListState};

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Agents,
    Tasks,
    Activity,
    Messages,
}

impl Panel {
    /// Index for generic per-panel operations.
    pub fn index(self) -> usize {
        match self {
            Panel::Agents => 0,
            Panel::Tasks => 1,
            Panel::Activity => 2,
            Panel::Messages => 3,
        }
    }
}

/// Application-level UI state (keyboard focus, selection cursors, flags).
pub struct AppState {
    /// The panel that currently has keyboard focus.
    pub active_panel: Panel,
    /// Index of the currently selected team (for multi-team switching).
    pub selected_team_index: usize,
    /// Index of the currently selected agent (drives activity filter).
    pub selected_agent_index: usize,

    // ─── Per-panel scroll state (persists across frames) ───
    pub agents_state: TableState,
    pub tasks_state: TableState,
    pub activity_state: ListState,
    pub messages_state: ListState,

    /// Set to `true` to quit the event loop.
    pub should_quit: bool,
    /// Toggle the help overlay.
    pub show_help: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_panel: Panel::Agents,
            selected_team_index: 0,
            selected_agent_index: 0,
            agents_state: TableState::default().with_selected(0),
            tasks_state: TableState::default().with_selected(0),
            activity_state: ListState::default(),  // None = follow tail
            messages_state: ListState::default(),   // None = follow tail
            should_quit: false,
            show_help: false,
        }
    }

    /// Cycle to the next team.
    pub fn next_team(&mut self, max: usize) {
        if max == 0 { return; }
        self.selected_team_index = (self.selected_team_index + 1) % max;
        self.reset_selections();
    }

    /// Cycle to the previous team.
    pub fn prev_team(&mut self, max: usize) {
        if max == 0 { return; }
        if self.selected_team_index == 0 {
            self.selected_team_index = max - 1;
        } else {
            self.selected_team_index -= 1;
        }
        self.reset_selections();
    }

    /// Jump directly to a specific panel.
    pub fn select_panel(&mut self, p: Panel) {
        self.active_panel = p;
    }

    /// Cycle to next panel (→). Skip Messages on ALL tab.
    pub fn next_panel(&mut self, has_messages: bool) {
        self.active_panel = match self.active_panel {
            Panel::Agents   => Panel::Tasks,
            Panel::Tasks    => Panel::Activity,
            Panel::Activity => if has_messages { Panel::Messages } else { Panel::Agents },
            Panel::Messages => Panel::Agents,
        };
    }

    /// Cycle to previous panel (←). Skip Messages on ALL tab.
    pub fn prev_panel(&mut self, has_messages: bool) {
        self.active_panel = match self.active_panel {
            Panel::Agents   => if has_messages { Panel::Messages } else { Panel::Activity },
            Panel::Tasks    => Panel::Agents,
            Panel::Activity => Panel::Tasks,
            Panel::Messages => Panel::Activity,
        };
    }

    /// Scroll down in the active panel (j / down-arrow).
    pub fn scroll_down(&mut self, max: usize) {
        if max == 0 { return; }
        match self.active_panel {
            Panel::Agents => {
                let cur = self.agents_state.selected().unwrap_or(0);
                if cur + 1 < max {
                    self.agents_state.select(Some(cur + 1));
                    self.selected_agent_index = cur + 1;
                }
            }
            Panel::Tasks => {
                let cur = self.tasks_state.selected().unwrap_or(0);
                if cur + 1 < max {
                    self.tasks_state.select(Some(cur + 1));
                }
            }
            Panel::Activity => {
                let cur = self.activity_state.selected().unwrap_or(0);
                if cur + 1 < max {
                    self.activity_state.select(Some(cur + 1));
                } else {
                    // At the bottom → resume tail-follow
                    self.activity_state.select(None);
                }
            }
            Panel::Messages => {
                let cur = self.messages_state.selected().unwrap_or(0);
                if cur + 1 < max {
                    self.messages_state.select(Some(cur + 1));
                } else {
                    self.messages_state.select(None);
                }
            }
        }
    }

    /// Scroll up in the active panel (k / up-arrow).
    pub fn scroll_up(&mut self, max: usize) {
        match self.active_panel {
            Panel::Agents => {
                let cur = self.agents_state.selected().unwrap_or(0);
                if cur > 0 {
                    self.agents_state.select(Some(cur - 1));
                    self.selected_agent_index = cur - 1;
                }
            }
            Panel::Tasks => {
                let cur = self.tasks_state.selected().unwrap_or(0);
                if cur > 0 {
                    self.tasks_state.select(Some(cur - 1));
                }
            }
            Panel::Activity => {
                // If following tail (None), pin to last item - 1
                if self.activity_state.selected().is_none() {
                    if max > 1 {
                        self.activity_state.select(Some(max - 2));
                    }
                } else {
                    let cur = self.activity_state.selected().unwrap_or(0);
                    if cur > 0 {
                        self.activity_state.select(Some(cur - 1));
                    }
                }
            }
            Panel::Messages => {
                if self.messages_state.selected().is_none() {
                    if max > 1 {
                        self.messages_state.select(Some(max - 2));
                    }
                } else {
                    let cur = self.messages_state.selected().unwrap_or(0);
                    if cur > 0 {
                        self.messages_state.select(Some(cur - 1));
                    }
                }
            }
        }
    }

    /// Clamp agent selection to valid range after data changes.
    pub fn clamp_agent_index(&mut self, max: usize) {
        if max == 0 {
            self.selected_agent_index = 0;
            self.agents_state.select(None);
        } else if self.selected_agent_index >= max {
            self.selected_agent_index = max - 1;
            self.agents_state.select(Some(self.selected_agent_index));
        }
    }

    fn reset_selections(&mut self) {
        self.selected_agent_index = 0;
        self.agents_state = TableState::default().with_selected(0);
        self.tasks_state = TableState::default().with_selected(0);
        self.activity_state = ListState::default();
        self.messages_state = ListState::default();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
