use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Extract HH:MM:SS from an ISO-8601 or similar timestamp string.
/// Falls back to the raw string if parsing fails.
fn extract_time(ts: &str) -> &str {
    // Try to find "HH:MM:SS" (8 chars with colons at positions 2 and 5)
    // Common formats: "2024-01-15T14:30:00Z", "14:30:00", epoch-based strings
    if let Some(t_pos) = ts.find('T') {
        let after_t = &ts[t_pos + 1..];
        if after_t.len() >= 8 {
            return &after_t[..8];
        }
    }
    // If the string itself looks like HH:MM:SS
    if ts.len() >= 8 && ts.as_bytes().get(2) == Some(&b':') && ts.as_bytes().get(5) == Some(&b':')
    {
        return &ts[..8];
    }
    ts
}

/// Render the live activity (tool events) panel.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState) {
    let is_focused = app.active_panel == Panel::Activity;

    // Determine selected agent name
    let selected_agent_name = team
        .agents
        .get(app.selected_agent_index)
        .map(|a| a.name.as_str());

    let panel_title = match selected_agent_name {
        Some(name) => format!(" Live Activity ({}) ", name),
        None => " Live Activity ".to_string(),
    };

    // Filter tool events by selected agent (or show all if no match)
    let events: Vec<&crate::store::models::ToolEvent> = if let Some(agent_name) = selected_agent_name {
        let filtered: Vec<_> = team
            .tool_events
            .iter()
            .filter(|e| e.agent_name == agent_name)
            .collect();
        if filtered.is_empty() {
            // Show all events if no match for selected agent
            team.tool_events.iter().collect()
        } else {
            filtered
        }
    } else {
        team.tool_events.iter().collect()
    };

    let items: Vec<ListItem> = if events.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  Enable hooks: cctrack hooks install",
            Style::new().fg(Color::DarkGray),
        )]))]
    } else {
        events
            .iter()
            .rev() // Most recent first
            .map(|evt| {
                let time = extract_time(&evt.timestamp);
                let tool_sty = theme::tool_style(&evt.tool_name);

                let line = Line::from(vec![
                    Span::styled(time, Style::new().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(&evt.tool_name, tool_sty),
                    Span::raw("  "),
                    Span::styled(
                        evt.duration_ms
                            .map(|d| format!("{}ms", d))
                            .unwrap_or_default(),
                        Style::new().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let border_style = if is_focused {
        ratatui::style::Style::new().fg(ratatui::style::Color::Cyan)
    } else {
        theme::BORDER
    };

    let block = Block::default()
        .title(Span::styled(panel_title, theme::TITLE))
        .borders(Borders::ALL)
        .border_style(border_style);

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::SELECTED);

    let mut state = ListState::default();
    if is_focused {
        state.select(Some(app.selected_rows[Panel::Activity as usize]));
    }

    frame.render_stateful_widget(list, area, &mut state);
}
