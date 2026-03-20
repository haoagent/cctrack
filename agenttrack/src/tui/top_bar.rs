use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::store::event::StoreSnapshot;
use super::app_state::AppState;
use super::theme;

/// Render the top tab bar — purely team/session tabs.
///
/// ```text
///  ▌solo▐  delivery-media-cleanup   cctrack-eval
/// ```
pub fn render(frame: &mut Frame, area: Rect, app: &AppState, snapshot: &StoreSnapshot) {
    let mut spans: Vec<Span> = Vec::new();

    if snapshot.teams.is_empty() {
        spans.push(Span::styled(" No active sessions ", theme::dim()));
    } else {
        for (i, t) in snapshot.teams.iter().enumerate() {
            let is_selected = i == app.selected_team_index
                || (app.selected_team_index >= snapshot.teams.len() && i == 0);

            if is_selected {
                // Active tab: bright with indicator
                spans.push(Span::styled(
                    format!(" \u{258c}{}\u{2590} ", t.name), // ▌name▐
                    theme::title().add_modifier(Modifier::BOLD),
                ));
            } else {
                // Inactive tab
                spans.push(Span::styled(
                    format!("  {}  ", t.name),
                    theme::dim(),
                ));
            }
        }
    }

    let line = Line::from(spans);
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme::border());
    let paragraph = Paragraph::new(line).block(block);

    frame.render_widget(paragraph, area);
}
