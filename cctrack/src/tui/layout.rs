use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Pre-computed rectangles for every panel in the TUI.
pub struct LayoutAreas {
    pub top_bar: Rect,
    pub agents: Rect,
    pub tasks: Rect,
    pub activity: Rect,
    pub messages: Option<Rect>,
    pub help_bar: Rect,
}

/// Build layout for ALL tab:
/// ```text
/// ┌─────────────── top_bar (3 rows) ───────────────┐
/// ├──── sessions (50%) ──┬──── stats (50%) ─────────┤  upper 55%
/// ├──────────── activity (remaining) ────────────────┤
/// └─────────────── help_bar (1 row) ────────────────┘
/// ```
pub fn build_layout_all(area: Rect) -> LayoutAreas {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),        // top_bar
            Constraint::Percentage(55),   // upper (sessions + stats)
            Constraint::Min(5),           // activity (fills remaining)
            Constraint::Length(2),        // help_bar
        ])
        .split(area);

    let upper = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(vertical[1]);

    LayoutAreas {
        top_bar: vertical[0],
        agents: upper[0],
        tasks: upper[1],
        activity: vertical[2],
        messages: None,
        help_bar: vertical[3],
    }
}

/// Build layout for team tabs:
/// ```text
/// ┌─────────────── top_bar (3 rows) ───────────────┐
/// ├──── agents (50%) ────┬──── todos (50%) ─────────┤  upper 40%
/// ├──────────── activity (30%) ─────────────────────┤
/// ├──────────── messages (remaining) ───────────────┤
/// └─────────────── help_bar (1 row) ────────────────┘
/// ```
pub fn build_layout_team(area: Rect) -> LayoutAreas {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),        // top_bar
            Constraint::Percentage(35),   // upper (agents + todos)
            Constraint::Percentage(30),   // activity
            Constraint::Percentage(20),   // messages
            Constraint::Length(2),        // help_bar
        ])
        .split(area);

    let upper = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(vertical[1]);

    LayoutAreas {
        top_bar: vertical[0],
        agents: upper[0],
        tasks: upper[1],
        activity: vertical[2],
        messages: Some(vertical[3]),
        help_bar: vertical[4],
    }
}
