use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::store::event::TeamSnapshot;
use crate::store::models::MessageType;
use super::app_state::{AppState, Panel};
use super::theme;

/// Extract HH:MM:SS from an ISO-8601 or similar timestamp string.
fn extract_time(ts: &str) -> &str {
    if let Some(t_pos) = ts.find('T') {
        let after_t = &ts[t_pos + 1..];
        if after_t.len() >= 8 {
            return &after_t[..8];
        }
    }
    if ts.len() >= 8 && ts.as_bytes().get(2) == Some(&b':') && ts.as_bytes().get(5) == Some(&b':')
    {
        return &ts[..8];
    }
    ts
}

/// Render the messages panel with auto-scroll.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &mut AppState) {
    let is_focused = app.active_panel == Panel::Messages;

    // Filter out IdleNotification messages
    let filtered: Vec<_> = team
        .messages
        .iter()
        .filter(|m| m.msg_type != MessageType::IdleNotification)
        .collect();

    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No messages yet",
            theme::dim(),
        )]))]
    } else {
        filtered
            .iter()
            .map(|msg| {
                let time = extract_time(&msg.timestamp);
                let summary = if msg.summary.is_empty() {
                    let t = &msg.text;
                    if t.len() > 60 {
                        format!("{}...", &t[..57])
                    } else {
                        t.clone()
                    }
                } else {
                    msg.summary.clone()
                };

                let from_style = theme::message_type_style(&msg.msg_type);

                let line = Line::from(vec![
                    Span::styled(time, theme::dim()),
                    Span::raw("  "),
                    Span::styled(&msg.from, from_style),
                    Span::styled(" \u{2192} ", theme::dim()), // →
                    Span::styled(&msg.to, theme::text().add_modifier(Modifier::BOLD)),
                    Span::styled(": ", theme::dim()),
                    Span::styled(summary, theme::text()),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let border_style = if is_focused {
        theme::accent()
    } else {
        theme::border()
    };
    let block = Block::default()
        .title(Span::styled(" Messages ", theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let highlight = ratatui::style::Style::new()
        .bg(ratatui::style::Color::Black)
        .fg(ratatui::style::Color::White)
        .add_modifier(ratatui::style::Modifier::BOLD);

    let list = List::new(items.clone())
        .block(block)
        .highlight_style(highlight);

    // Auto-scroll to bottom when following tail (selected = None)
    if app.messages_state.selected().is_none() && !items.is_empty() {
        app.messages_state.select(Some(items.len() - 1));
        frame.render_stateful_widget(list, area, &mut app.messages_state);
        app.messages_state.select(None);
    } else {
        if let Some(sel) = app.messages_state.selected() {
            if sel >= items.len() && !items.is_empty() {
                app.messages_state.select(Some(items.len() - 1));
            }
        }
        frame.render_stateful_widget(list, area, &mut app.messages_state);
    }
}
