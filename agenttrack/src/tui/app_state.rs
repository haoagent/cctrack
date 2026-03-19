/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Agents,
    Tasks,
    Activity,
    Messages,
}

impl Panel {
    /// Cycle to the next panel in tab order: Agents -> Tasks -> Activity -> Messages -> Agents.
    fn next(self) -> Self {
        match self {
            Panel::Agents => Panel::Tasks,
            Panel::Tasks => Panel::Activity,
            Panel::Activity => Panel::Messages,
            Panel::Messages => Panel::Agents,
        }
    }

    /// Index into the `selected_rows` array.
    fn index(self) -> usize {
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
    /// Index of the currently selected agent (used by activity panel).
    pub selected_agent_index: usize,
    /// One scroll-row cursor per panel.
    pub selected_rows: [usize; 4],
    /// Set to `true` to quit the event loop.
    pub should_quit: bool,
    /// Toggle the help overlay.
    pub show_help: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_panel: Panel::Agents,
            selected_agent_index: 0,
            selected_rows: [0; 4],
            should_quit: false,
            show_help: false,
        }
    }

    /// Cycle to the next panel (Tab key).
    pub fn next_panel(&mut self) {
        self.active_panel = self.active_panel.next();
    }

    /// Jump directly to a specific panel.
    pub fn select_panel(&mut self, p: Panel) {
        self.active_panel = p;
    }

    /// Select the next agent (right arrow).
    pub fn next_agent(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        self.selected_agent_index = (self.selected_agent_index + 1) % max;
    }

    /// Select the previous agent (left arrow).
    pub fn prev_agent(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        if self.selected_agent_index == 0 {
            self.selected_agent_index = max - 1;
        } else {
            self.selected_agent_index -= 1;
        }
    }

    /// Scroll down in the active panel (j / down-arrow).
    pub fn scroll_down(&mut self, max: usize) {
        let idx = self.active_panel.index();
        if max == 0 {
            return;
        }
        if self.selected_rows[idx] + 1 < max {
            self.selected_rows[idx] += 1;
        }
    }

    /// Scroll up in the active panel (k / up-arrow).
    pub fn scroll_up(&mut self) {
        let idx = self.active_panel.index();
        if self.selected_rows[idx] > 0 {
            self.selected_rows[idx] -= 1;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
