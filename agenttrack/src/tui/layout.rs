use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Pre-computed rectangles for every panel in the TUI.
pub struct LayoutAreas {
    pub top_bar: Rect,
    pub agents: Rect,
    pub tasks: Rect,
    pub activity: Rect,
    pub messages: Rect,
    pub help_bar: Rect,
}

/// Divide `area` into the standard cctrack layout.
///
/// ```text
/// ┌─────────────── top_bar (3 rows) ───────────────┐
/// ├──── agents (50%) ────┬──── tasks (50%) ─────────┤  upper 40%
/// ├──────────── activity (30%) ─────────────────────┤
/// ├──────────── messages (remaining) ───────────────┤
/// └─────────────── help_bar (1 row) ────────────────┘
/// ```
pub fn build_layout(area: Rect) -> LayoutAreas {
    // Top-level vertical split
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),        // top_bar (blank + tabs + blank)
            Constraint::Percentage(40),   // upper half (agents + tasks)
            Constraint::Percentage(30),   // activity
            Constraint::Min(3),           // messages (fills remaining)
            Constraint::Length(1),        // help_bar
        ])
        .split(area);

    // Upper half horizontal split
    let upper = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // agents
            Constraint::Percentage(50), // tasks
        ])
        .split(vertical[1]);

    LayoutAreas {
        top_bar: vertical[0],
        agents: upper[0],
        tasks: upper[1],
        activity: vertical[2],
        messages: vertical[3],
        help_bar: vertical[4],
    }
}
