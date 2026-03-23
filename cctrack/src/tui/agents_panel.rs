use ratatui::{
    Frame,
    layout::{Constraint, Rect},
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
    let is_all = team.name == "all";

    // On ALL tab, filter out shutdown sessions
    let agents: Vec<_> = if is_all {
        team.agents.iter().filter(|a| a.status != crate::store::models::AgentStatus::Shutdown).collect()
    } else {
        team.agents.iter().collect()
    };

    let panel_title = if is_all {
        format!(" Sessions ({}) ", agents.len())
    } else {
        let active = team.agents.iter()
            .filter(|a| a.status == crate::store::models::AgentStatus::Active)
            .count();
        format!(" Agents ({}/{}) ", active, team.agents.len())
    };
    let block = Block::default()
        .title(Span::styled(&panel_title, theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    if is_all {
        // ALL tab: ● NAME, TOKENS, COST (status dot inline with name)
        let header = Row::new(vec![
            Cell::from(Span::styled("NAME", theme::header())),
            Cell::from(Span::styled("TOKENS", theme::header())),
            Cell::from(Span::styled("COST", theme::header())),
        ])
        .height(1);

        let rows: Vec<Row> = agents.iter().map(|agent| {
            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);
            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_for_model(agent.model.as_deref()))
            } else {
                "-".to_string()
            };

            // "● Project: title" — dot=status color, project=cyan, title=normal
            let name_line = if let Some((project, title)) = agent.name.split_once(": ") {
                ratatui::text::Line::from(vec![
                    Span::styled(format!("{} ", status_sym), status_sty),
                    Span::styled(format!("{}: ", project), theme::project_name()),
                    Span::styled(title.to_string(), theme::text()),
                ])
            } else {
                ratatui::text::Line::from(vec![
                    Span::styled(format!("{} ", status_sym), status_sty),
                    Span::styled(agent.name.clone(), theme::project_name()),
                ])
            };

            Row::new(vec![
                Cell::from(name_line),
                Cell::from(Span::styled(tokens, theme::dim())),
                Cell::from(Span::styled(cost, theme::cost_style())),
            ])
        }).collect();

        let widths = [
            Constraint::Percentage(60),
            Constraint::Percentage(18),
            Constraint::Percentage(18),
        ];

        let table = Table::new(rows, widths).header(header).block(block);
        frame.render_widget(table, area);
    } else {
        // Team tab: NAME, MODEL, STATUS, TOKENS, COST
        let header = Row::new(vec![
            Cell::from(Span::styled("NAME", theme::header())),
            Cell::from(Span::styled("MODEL", theme::header())),
            Cell::from(Span::styled("STATUS", theme::header())),
            Cell::from(Span::styled("TOKENS", theme::header())),
            Cell::from(Span::styled("COST", theme::header())),
        ])
        .height(1);

        let rows: Vec<Row> = agents.iter().map(|agent| {
            let model_str = agent.model.as_deref().map(short_model).unwrap_or("-");
            let status_sym = theme::status_symbol(&agent.status);
            let status_sty = theme::status_style(&agent.status);
            let tokens = format_tokens(agent.tokens.total());
            let cost = if agent.tokens.total() > 0 {
                format!("${:.2}", agent.tokens.estimated_cost_for_model(agent.model.as_deref()))
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Cell::from(Span::styled(agent.name.clone(), theme::project_name())),
                Cell::from(Span::styled(model_str, theme::dim())),
                Cell::from(Span::styled(format!("{} {}", status_sym, agent.status.label()), status_sty)),
                Cell::from(Span::styled(tokens, theme::dim())),
                Cell::from(Span::styled(cost, theme::cost_style())),
            ])
        }).collect();

        let widths = [
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ];

        let table = Table::new(rows, widths).header(header).block(block);
        frame.render_widget(table, area);
    }
}
