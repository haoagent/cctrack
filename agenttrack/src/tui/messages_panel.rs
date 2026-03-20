use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
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

/// Render the messages panel.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState) {
    let _is_focused = app.active_panel == Panel::Messages;

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
        // Show messages in chronological order (most recent at bottom)
        filtered
            .iter()
            .map(|msg| {
                let time = extract_time(&msg.timestamp);
                let summary = if msg.summary.is_empty() {
                    // Fall back to truncated text
                    let t = &msg.text;
                    if t.len() > 60 {
                        format!("{}...", &t[..57])
                    } else {
                        t.clone()
                    }
                } else {
                    msg.summary.clone()
                };

                let from_style = match msg.msg_type {
                    MessageType::TaskCompleted => Style::new().fg(Color::Green),
                    MessageType::PlanApproval => Style::new()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    MessageType::ShutdownNotification => theme::dim(),
                    MessageType::Broadcast => Style::new().fg(Color::Magenta),
                    _ => Style::new().fg(Color::Cyan),
                };

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

    let block = Block::default()
        .title(Span::styled(" Messages ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let list = List::new(items).block(block);

    frame.render_widget(list, area);
}
