use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Truncate a model identifier to a short display name.
///
/// "claude-opus-4-6" -> "opus", "claude-sonnet-4-6" -> "sonnet", etc.
fn short_model(model: &str) -> &str {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        "opus"
    } else if lower.contains("sonnet") {
        "sonnet"
    } else if lower.contains("haiku") {
        "haiku"
    } else if model.len() > 12 {
        // Return last segment after the final dash
        model.rsplit('-').next().unwrap_or(model)
    } else {
        model
    }
}

/// Render the agents table.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState) {
    let is_focused = app.active_panel == Panel::Agents;

    // Header row
    let header = Row::new(vec![
        Cell::from(Span::styled("NAME", theme::HEADER)),
        Cell::from(Span::styled("MODEL", theme::HEADER)),
        Cell::from(Span::styled("STATUS", theme::HEADER)),
    ])
    .height(1);

    // Data rows
    let rows: Vec<Row> = team
        .agents
        .iter()
        .map(|agent| {
            let model_str = agent
                .model
                .as_deref()
                .map(short_model)
                .unwrap_or("-");

            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);

            Row::new(vec![
                Cell::from(Span::styled(
                    agent.name.clone(),
                    ratatui::style::Style::new().fg(ratatui::style::Color::White),
                )),
                Cell::from(Span::raw(model_str)),
                Cell::from(Span::styled(
                    format!("{} {:?}", status_sym, agent.status),
                    status_sty,
                )),
            ])
        })
        .collect();

    let border_style = if is_focused {
        ratatui::style::Style::new().fg(ratatui::style::Color::Cyan)
    } else {
        theme::BORDER
    };

    let block = Block::default()
        .title(Span::styled(" Agents ", theme::TITLE))
        .borders(Borders::ALL)
        .border_style(border_style);

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(25),
        Constraint::Percentage(35),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme::SELECTED);

    let mut state = TableState::default();
    if is_focused {
        state.select(Some(app.selected_rows[Panel::Agents as usize]));
    }

    frame.render_stateful_widget(table, area, &mut state);
}
