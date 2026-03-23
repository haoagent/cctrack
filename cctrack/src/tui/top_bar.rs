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
            let is_all = i == 0; // First tab is always ALL

            let dot_char = if has_active { "\u{25cf}" } else { "\u{25cb}" }; // ● or ○

            // Tab label: "NAME (N)" where N = agent count (skip for ALL tab)
            let agent_count = t.agents.len();
            let tab_label = if is_all {
                t.name.to_uppercase()
            } else {
                format!("{} ({})", t.name.to_uppercase(), agent_count)
            };

            if is_selected {
                let tab_bg = if is_all {
                    Color::Rgb(30, 90, 100)
                } else {
                    Color::Rgb(65, 105, 225) // royal blue
                };
                let tab_style = Style::new()
                    .bg(tab_bg)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
                let dot_style = Style::new()
                    .bg(tab_bg)
                    .fg(if has_active { Color::LightGreen } else { Color::DarkGray });

                spans.push(Span::styled(" ", tab_style));
                spans.push(Span::styled(format!("{} ", dot_char), dot_style));
                spans.push(Span::styled(format!("{} ", tab_label), tab_style));
            } else {
                let dot_style = if has_active {
                    Style::new().fg(Color::LightGreen)
                } else {
                    theme::dim()
                };
                spans.push(Span::styled("  ", theme::dim()));
                spans.push(Span::styled(format!("{} ", dot_char), dot_style));
                spans.push(Span::styled(format!("{} ", tab_label), theme::dim()));
            }
        }
    }

    let lines = vec![
        Line::from(""),           // blank top line
        Line::from(spans),        // tab content
    ];
    let paragraph = Paragraph::new(lines);

    frame.render_widget(paragraph, area);
}
