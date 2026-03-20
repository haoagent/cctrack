use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::store::event::{StoreSnapshot, TeamSnapshot};
use super::app_state::AppState;
use super::theme;

/// Render the top status bar with team tabs.
///
/// ```text
/// cctrack ─ [solo] [team-a] [*team-b*] ─ 4 agents ─ 0/3 tasks ─ 12 events
/// ```
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState, snapshot: &StoreSnapshot) {
    let m = &team.metrics;

    let mut spans = vec![
        Span::styled(" cctrack", theme::title()),
        Span::styled(" ", theme::dim()),
    ];

    // Team tabs
    if snapshot.teams.len() > 1 {
        for (i, t) in snapshot.teams.iter().enumerate() {
            let is_selected = i == app.selected_team_index
                || (app.selected_team_index >= snapshot.teams.len() && i == 0);
            if is_selected {
                spans.push(Span::styled(
                    format!(" [{}] ", t.name),
                    theme::text().add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", t.name),
                    theme::dim(),
                ));
            }
        }
        spans.push(Span::styled(" \u{2500} ", theme::dim()));
    } else {
        spans.push(Span::styled(
            &team.name,
            theme::text().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" \u{2500} ", theme::dim()));
    }

    spans.extend_from_slice(&[
        Span::styled(
            format!("{}", m.total_agents),
            theme::status_style(&crate::store::models::AgentStatus::Active),
        ),
        Span::styled(" agents", theme::dim()),
        Span::styled(" \u{2500} ", theme::dim()),
        Span::styled(
            format!("{}/{}", m.completed_tasks, m.total_tasks),
            theme::task_status_style("in_progress"),
        ),
        Span::styled(" tasks", theme::dim()),
        Span::styled(" \u{2500} ", theme::dim()),
        Span::styled(
            format!("{}", m.total_tool_calls),
            theme::tool_style("Bash"),
        ),
        Span::styled(" events", theme::dim()),
    ]);

    let line = Line::from(spans);
    let block = Block::default().borders(Borders::BOTTOM).border_style(theme::border());
    let paragraph = Paragraph::new(line).block(block);

    frame.render_widget(paragraph, area);
}
