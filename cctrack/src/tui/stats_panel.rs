use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::stats::{StatsReport, format_tokens};
use super::theme;

/// Render the stats panel (replaces Messages on ALL tab).
pub fn render(frame: &mut Frame, area: Rect, report: &StatsReport) {
    let header = Row::new(vec![
        Cell::from(Span::styled("", theme::header())),
        Cell::from(Span::styled("SESSIONS", theme::header())),
        Cell::from(Span::styled("TOKENS", theme::header())),
        Cell::from(Span::styled("COST", theme::header())),
    ])
    .height(1);

    // Time period rows
    let periods = [&report.today, &report.this_week, &report.this_month, &report.total];
    let mut rows: Vec<Row> = periods.iter().map(|p| {
        let is_total = p.label == "Total";
        let label_style = if is_total {
            theme::text().add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            theme::text()
        };
        let cost_str = format!("${:.2}", p.cost_usd);

        Row::new(vec![
            Cell::from(Span::styled(&p.label, label_style)),
            Cell::from(Span::styled(format!("{}", p.sessions), theme::dim())),
            Cell::from(Span::styled(format_tokens(p.total_tokens), theme::dim())),
            Cell::from(Span::styled(cost_str, theme::cost_style())),
        ])
    }).collect();

    // Separator + project rows
    if !report.by_project.is_empty() {
        rows.push(Row::new(vec![
            Cell::from(Span::styled("By Project", theme::header())),
            Cell::from(Span::raw("")),
            Cell::from(Span::raw("")),
            Cell::from(Span::raw("")),
        ]));

        for p in report.by_project.iter().take(5) {
            let cost_str = format!("${:.2}", p.cost_usd);
            rows.push(Row::new(vec![
                Cell::from(Span::styled(&p.label, theme::dim())),
                Cell::from(Span::styled(format!("{}", p.sessions), theme::dim())),
                Cell::from(Span::styled(format_tokens(p.total_tokens), theme::dim())),
                Cell::from(Span::styled(cost_str, theme::cost_style())),
            ]));
        }
    }

    let block = Block::default()
        .title(Span::styled(" Stats ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block);

    frame.render_widget(table, area);
}
