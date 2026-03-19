use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::store::event::TeamSnapshot;
use super::app_state::AppState;
use super::theme;

/// Render the top status bar.
///
/// ```text
/// AgentTrack -- team: {name} -- {n} agents -- {completed}/{total} tasks -- {events} events
/// ```
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, _app: &AppState) {
    let m = &team.metrics;

    let spans = vec![
        Span::styled(
            " AgentTrack",
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" \u{2500} ", Style::new().fg(Color::DarkGray)),
        Span::styled("team: ", Style::new().fg(Color::DarkGray)),
        Span::styled(
            &team.name,
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" \u{2500} ", Style::new().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", m.total_agents),
            Style::new().fg(Color::Green),
        ),
        Span::styled(" agents", Style::new().fg(Color::DarkGray)),
        Span::styled(" \u{2500} ", Style::new().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", m.completed_tasks, m.total_tasks),
            Style::new().fg(Color::Yellow),
        ),
        Span::styled(" tasks", Style::new().fg(Color::DarkGray)),
        Span::styled(" \u{2500} ", Style::new().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", m.total_tool_calls),
            Style::new().fg(Color::Magenta),
        ),
        Span::styled(" events", Style::new().fg(Color::DarkGray)),
    ];

    let line = Line::from(spans);
    let block = Block::default().borders(Borders::BOTTOM).border_style(theme::BORDER);
    let paragraph = Paragraph::new(line).block(block);

    frame.render_widget(paragraph, area);
}
