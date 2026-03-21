use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Truncate a model identifier to a short display name.
fn short_model(model: &str) -> &str {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        "opus"
    } else if lower.contains("sonnet") {
        "sonnet"
    } else if lower.contains("haiku") {
        "haiku"
    } else if model.len() > 12 {
        model.rsplit('-').next().unwrap_or(model)
    } else {
        model
    }
}

/// Format token count as compact string: 1.2K, 45K, 1.2M
fn format_tokens(n: u64) -> String {
    if n == 0 {
        "-".to_string()
    } else if n < 1_000 {
        format!("{}", n)
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}

/// Render the agents table.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState) {
    let _is_focused = app.active_panel == Panel::Agents;

    let header = Row::new(vec![
        Cell::from(Span::styled("NAME", theme::header())),
        Cell::from(Span::styled("MODEL", theme::header())),
        Cell::from(Span::styled("STATUS", theme::header())),
        Cell::from(Span::styled("TOKENS", theme::header())),
        Cell::from(Span::styled("COST", theme::header())),
    ])
    .height(1);

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

            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_usd())
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Cell::from(Span::styled(
                    agent.name.clone(),
                    theme::text().add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(model_str, theme::dim())),
                Cell::from(Span::styled(
                    format!("{} {}", status_sym, agent.status.label()),
                    status_sty,
                )),
                Cell::from(Span::styled(tokens, theme::text())),
                Cell::from(Span::styled(cost, theme::dim())),
            ])
        })
        .collect();

    let panel_title = if team.name == "all" { " Sessions " } else { " Agents " };
    let block = Block::default()
        .title(Span::styled(panel_title, theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block);

    frame.render_widget(table, area);
}
