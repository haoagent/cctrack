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

fn short_model(model: &str) -> &str {
    let lower = model.to_lowercase();
    if lower.contains("opus") { "opus" }
    else if lower.contains("sonnet") { "sonnet" }
    else if lower.contains("haiku") { "haiku" }
    else if model.len() > 12 { model.rsplit('-').next().unwrap_or(model) }
    else { model }
}

fn format_tokens(n: u64) -> String {
    if n == 0 { "-".to_string() }
    else if n < 1_000 { format!("{}", n) }
    else if n < 1_000_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { format!("{:.1}M", n as f64 / 1_000_000.0) }
}

pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &mut AppState) {
    let is_focused = app.active_panel == Panel::Agents;
    let is_all = team.name == "all";
    let is_session_tab = team.name.starts_with("session:");

    let agents: Vec<_> = team.agents.iter().collect();
    app.clamp_agent_index(agents.len());

    let active = agents.iter().filter(|a| a.status == crate::store::models::AgentStatus::Active).count();
    let total = agents.len();
    let panel_title = if is_all {
        format!(" Sessions ({}/{}) ", active, total)
    } else {
        format!(" Agents ({}/{}) ", active, total)
    };

    let border_style = if is_focused { theme::accent() } else { theme::border() };
    let block = Block::default()
        .title(Span::styled(&panel_title, theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    // Black bg + white fg — system default highlight, no colored patches
    let highlight = ratatui::style::Style::new()
        .bg(ratatui::style::Color::Black)
        .fg(ratatui::style::Color::White)
        .add_modifier(Modifier::BOLD);

    if is_all {
        let header = Row::new(vec![
            Cell::from(Span::styled("NAME", theme::header())),
            Cell::from(Span::styled("TOKENS", theme::header())),
            Cell::from(Span::styled("COST", theme::header())),
        ]).height(1);

        let rows: Vec<Row> = agents.iter().map(|agent| {
            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);
            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_for_model(agent.model.as_deref()))
            } else { "-".to_string() };

            let sub_count_suffix = agent.sub_agent_count
                .filter(|&n| n > 0).map(|n| format!(" ({})", n)).unwrap_or_default();

            let name_line = if let Some((project, title)) = agent.name.split_once(": ") {
                ratatui::text::Line::from(vec![
                    Span::styled(format!("{} ", status_sym), status_sty),
                    Span::styled(format!("{}: ", project), theme::project_name()),
                    Span::styled(title.to_string(), theme::text()),
                    Span::styled(sub_count_suffix, theme::dim()),
                ])
            } else {
                ratatui::text::Line::from(vec![
                    Span::styled(format!("{} ", status_sym), status_sty),
                    Span::styled(agent.name.clone(), theme::project_name()),
                    Span::styled(sub_count_suffix, theme::dim()),
                ])
            };

            Row::new(vec![
                Cell::from(name_line),
                Cell::from(Span::styled(tokens, theme::dim())),
                Cell::from(Span::styled(cost, theme::cost_style())),
            ])
        }).collect();

        let widths = [Constraint::Percentage(60), Constraint::Percentage(18), Constraint::Percentage(18)];
        let table = Table::new(rows, widths).header(header).block(block)
            .row_highlight_style(highlight);
        frame.render_stateful_widget(table, area, &mut app.agents_state);

    } else if is_session_tab {
        let header = Row::new(vec![
            Cell::from(Span::styled("NAME", theme::header())),
            Cell::from(Span::styled("MODEL", theme::header())),
            Cell::from(Span::styled("TOKENS", theme::header())),
            Cell::from(Span::styled("COST", theme::header())),
        ]).height(1);

        let rows: Vec<Row> = agents.iter().map(|agent| {
            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);
            let model_str = agent.model.as_deref().map(short_model).unwrap_or("-");
            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_for_model(agent.model.as_deref()))
            } else { "-".to_string() };

            let name_line = ratatui::text::Line::from(vec![
                Span::styled(format!("{} ", status_sym), status_sty),
                Span::styled(agent.name.clone(), theme::text()),
            ]);

            Row::new(vec![
                Cell::from(name_line),
                Cell::from(Span::styled(model_str, theme::dim())),
                Cell::from(Span::styled(tokens, theme::dim())),
                Cell::from(Span::styled(cost, theme::cost_style())),
            ])
        }).collect();

        let widths = [Constraint::Percentage(40), Constraint::Percentage(15), Constraint::Percentage(20), Constraint::Percentage(20)];
        let table = Table::new(rows, widths).header(header).block(block)
            .row_highlight_style(highlight);
        frame.render_stateful_widget(table, area, &mut app.agents_state);

    } else {
        let header = Row::new(vec![
            Cell::from(Span::styled("NAME", theme::header())),
            Cell::from(Span::styled("MODEL", theme::header())),
            Cell::from(Span::styled("STATUS", theme::header())),
            Cell::from(Span::styled("TOKENS", theme::header())),
            Cell::from(Span::styled("COST", theme::header())),
        ]).height(1);

        let rows: Vec<Row> = agents.iter().map(|agent| {
            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);
            let model_str = agent.model.as_deref().map(short_model).unwrap_or("-");
            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_for_model(agent.model.as_deref()))
            } else { "-".to_string() };

            Row::new(vec![
                Cell::from(Span::styled(agent.name.clone(), theme::project_name())),
                Cell::from(Span::styled(model_str, theme::dim())),
                Cell::from(ratatui::text::Line::from(vec![
                    Span::styled(format!("{} ", status_sym), status_sty),
                    Span::styled(agent.status.label(), theme::text()),
                ])),
                Cell::from(Span::styled(tokens, theme::dim())),
                Cell::from(Span::styled(cost, theme::cost_style())),
            ])
        }).collect();

        let widths = [Constraint::Percentage(30), Constraint::Percentage(15), Constraint::Percentage(20), Constraint::Percentage(15), Constraint::Percentage(15)];
        let table = Table::new(rows, widths).header(header).block(block)
            .row_highlight_style(highlight);
        frame.render_stateful_widget(table, area, &mut app.agents_state);
    }
}
