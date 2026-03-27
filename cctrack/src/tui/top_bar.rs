use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::store::event::StoreSnapshot;
use crate::store::models::AgentStatus;
use super::app_state::AppState;
use super::theme;

/// Truncate a string to at most `max_width` display columns.
/// Handles CJK double-width chars correctly.
fn truncate_display(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > max_width {
            result.push('\u{2026}'); // …
            break;
        }
        result.push(ch);
        width += cw;
    }
    result
}

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

            let is_session_tab = t.name.starts_with("session:");
            let has_active = if is_session_tab {
                // Session tab dot follows the parent session's status
                // Parent is the first agent (or check agent_type != "subagent")
                t.agents.iter()
                    .find(|a| a.agent_type.as_deref() != Some("subagent"))
                    .map(|a| a.status == AgentStatus::Active)
                    .unwrap_or(false)
            } else {
                t.agents.iter().any(|a| a.status == AgentStatus::Active)
            };
            let dot_char = if has_active { "\u{25cf}" } else { "\u{25cb}" }; // ● or ○

            // Tab label: strip "session:" prefix for display
            let display_name = t.name.strip_prefix("session:").unwrap_or(&t.name);
            let raw_label = display_name.to_uppercase();
            // Truncate long tab labels (max 24 display chars)
            let tab_label = truncate_display(&raw_label, 24);
            // No count badge on tabs — keeps it clean

            if is_selected {
                let tab_style = theme::tab_selected();
                let dot_style = theme::tab_dot_selected(has_active);

                spans.push(Span::styled(" ", tab_style));
                spans.push(Span::styled(format!("{} ", dot_char), dot_style));
                spans.push(Span::styled(format!("{} ", tab_label), tab_style));
            } else {
                let dot_style = if has_active {
                    theme::status_style(&AgentStatus::Active)
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
