use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Extract HH:MM:SS from an ISO-8601 or similar timestamp string.
/// Falls back to the raw string if parsing fails.
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

/// Render the live activity (tool events) panel with auto-scroll.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &mut AppState) {
    let is_focused = app.active_panel == Panel::Activity;

    // Determine selected agent name
    let selected_agent_name = team
        .agents
        .get(app.selected_agent_index)
        .map(|a| a.name.as_str());

    let is_all = team.name == "all";

    let panel_title = if is_all {
        " Live Activity ".to_string()
    } else {
        match selected_agent_name {
            Some(name) => format!(" Live Activity ({}) ", name),
            None => " Live Activity ".to_string(),
        }
    };

    // Filter tool events: hide internal startup_scan events, optionally filter by agent
    let all_visible: Vec<_> = team.tool_events.iter()
        .filter(|e| e.tool_name != "startup_scan")
        .collect();

    let events: Vec<&crate::store::models::ToolEvent> = if let Some(agent_name) = selected_agent_name {
        let filtered: Vec<_> = all_visible.iter()
            .filter(|e| e.agent_name == agent_name)
            .copied()
            .collect();
        if filtered.is_empty() { all_visible } else { filtered }
    } else {
        all_visible
    };

    let items: Vec<ListItem> = if events.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  Listening...",
            theme::dim(),
        )]))]
    } else {
        events
            .iter()
            .map(|evt| {
                let time = extract_time(&evt.timestamp);
                let tool_sty = theme::tool_style(&evt.tool_name);

                let duration = evt
                    .duration_ms
                    .map(|d| format!(" {}ms", d))
                    .unwrap_or_default();

                let summary_text = if evt.summary.is_empty() {
                    String::new()
                } else {
                    let s = &evt.summary;
                    if s.len() > 60 {
                        format!(" {:.57}...", s)
                    } else {
                        format!(" {}", s)
                    }
                };

                // Show agent name: always on ALL tab, or when multiple agents
                let agent_label = if is_all || team.agents.len() > 1 {
                    let short = evt.cwd.as_deref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .and_then(|f| f.to_str())
                        .unwrap_or_else(|| {
                            if evt.agent_name.len() > 10 { &evt.agent_name[..10] } else { &evt.agent_name }
                        });
                    format!("{} ", short)
                } else {
                    String::new()
                };

                let line = Line::from(vec![
                    Span::styled(time, theme::dim()),
                    Span::raw(" "),
                    Span::styled(agent_label, theme::project_name()),
                    Span::styled(format!("{:<6}", evt.tool_name), tool_sty),
                    Span::styled(summary_text, theme::text()),
                    Span::styled(duration, theme::dim()),
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
        .title(Span::styled(panel_title, theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let highlight = ratatui::style::Style::new()
        .bg(ratatui::style::Color::Blue)
        .fg(ratatui::style::Color::White)
        .add_modifier(ratatui::style::Modifier::BOLD);

    let list = List::new(items.clone())
        .block(block)
        .highlight_style(highlight);

    // Auto-scroll to bottom when following tail (selected = None)
    if app.activity_state.selected().is_none() && !items.is_empty() {
        app.activity_state.select(Some(items.len() - 1));
        frame.render_stateful_widget(list, area, &mut app.activity_state);
        // Reset to None so next frame continues following
        app.activity_state.select(None);
    } else {
        // Clamp manual selection to valid range
        if let Some(sel) = app.activity_state.selected() {
            if sel >= items.len() && !items.is_empty() {
                app.activity_state.select(Some(items.len() - 1));
            }
        }
        frame.render_stateful_widget(list, area, &mut app.activity_state);
    }
}
