use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Render the tasks table.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &AppState) {
    let is_focused = app.active_panel == Panel::Tasks;

    // Header
    let header = Row::new(vec![
        Cell::from(Span::styled("ID", theme::header())),
        Cell::from(Span::styled("STATUS", theme::header())),
        Cell::from(Span::styled("SUBJECT", theme::header())),
    ])
    .height(1);

    // Data rows
    let rows: Vec<Row> = team
        .tasks
        .iter()
        .map(|task| {
            let raw_status = task.status.as_deref().unwrap_or("unknown");

            // If blocked_by is non-empty and not completed, show blocked status
            let (display_status, sym, sty) = if !task.blocked_by.is_empty()
                && raw_status != "completed"
            {
                let blocker = task.blocked_by.first().map(|b| format!("(by #{})", b)).unwrap_or_default();
                let label = format!("{} blocked {}", theme::task_status_symbol("blocked"), blocker);
                (label, theme::task_status_symbol("blocked"), theme::task_status_style("blocked"))
            } else {
                let sym = theme::task_status_symbol(raw_status);
                let sty = theme::task_status_style(raw_status);
                (format!("{} {}", sym, raw_status), sym, sty)
            };
            let _ = sym; // used inside display_status already

            let subject = task.subject.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(Span::styled(&task.id, theme::dim())),
                Cell::from(Span::styled(display_status, sty)),
                Cell::from(Span::styled(subject, theme::text())),
            ])
        })
        .collect();

    let border_style = if is_focused {
        ratatui::style::Style::new().fg(ratatui::style::Color::Cyan)
    } else {
        theme::border()
    };

    let block = Block::default()
        .title(Span::styled(" Tasks ", theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let widths = [
        Constraint::Length(8),
        Constraint::Percentage(30),
        Constraint::Percentage(60),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme::SELECTED);

    let mut state = TableState::default();
    if is_focused {
        state.select(Some(app.selected_rows[Panel::Tasks as usize]));
    }

    frame.render_stateful_widget(table, area, &mut state);
}
