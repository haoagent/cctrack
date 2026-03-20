use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::store::event::StoreSnapshot;
use crate::store::models::AgentStatus;
use super::app_state::AppState;
use super::theme;

/// Render the top tab bar.
///
/// Selected tab: solid background + white text (high contrast).
/// Active teams get a green dot ●, idle get dim ○.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState, snapshot: &StoreSnapshot) {
    let mut spans: Vec<Span> = Vec::new();

    spans.push(Span::styled(" ", theme::dim())); // left padding

    if snapshot.teams.is_empty() {
        spans.push(Span::styled("Waiting for sessions...", theme::dim()));
    } else {
        for (i, t) in snapshot.teams.iter().enumerate() {
            let is_selected = i == app.selected_team_index
                || (app.selected_team_index >= snapshot.teams.len() && i == 0);

            let has_active = t.agents.iter().any(|a| a.status == AgentStatus::Active);
            let dot_char = if has_active { "\u{25cf}" } else { "\u{25cb}" }; // ● or ○

            if is_selected {
                // Selected: solid bg + white bold text
                let tab_bg = if theme::is_light_mode() {
                    Color::Rgb(60, 60, 80)
                } else {
                    Color::Rgb(80, 90, 120)
                };
                let tab_style = Style::new()
                    .bg(tab_bg)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
                let dot_style = Style::new()
                    .bg(tab_bg)
                    .fg(if has_active { Color::Rgb(120, 220, 120) } else { Color::Rgb(150, 150, 150) });

                spans.push(Span::styled(" ", tab_style));
                spans.push(Span::styled(format!("{} ", dot_char), dot_style));
                spans.push(Span::styled(format!("{} ", t.name), tab_style));
            } else {
                // Inactive: no bg, dim text
                let dot_style = if has_active {
                    Style::new().fg(Color::Rgb(158, 206, 106))
                } else {
                    theme::dim()
                };
                spans.push(Span::styled("  ", theme::dim()));
                spans.push(Span::styled(format!("{} ", dot_char), dot_style));
                spans.push(Span::styled(format!("{} ", t.name), theme::dim()));
            }
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);

    frame.render_widget(paragraph, area);
}
