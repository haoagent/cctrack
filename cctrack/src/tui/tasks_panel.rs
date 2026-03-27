use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::store::event::TeamSnapshot;
use super::app_state::{AppState, Panel};
use super::theme;

/// Map todo status to a display symbol.
fn todo_symbol(status: &str) -> &'static str {
    match status {
        "completed" => "\u{2713}",   // ✓
        "in_progress" => "\u{25cf}", // ●
        "pending" => "\u{25cb}",     // ○
        _ => "?",
    }
}

/// Map todo status to a display label.
fn todo_label(status: &str) -> &'static str {
    match status {
        "completed" => "done",
        "in_progress" => "running",
        "pending" => "pending",
        _ => "?",
    }
}

/// Render the todos panel with scroll support.
pub fn render(frame: &mut Frame, area: Rect, team: &TeamSnapshot, app: &mut AppState) {
    let is_focused = app.active_panel == Panel::Tasks;

    let header = Row::new(vec![
        Cell::from(Span::styled("STATUS", theme::header())),
        Cell::from(Span::styled("TODO", theme::header())),
    ])
    .height(1);

    let rows: Vec<Row> = if team.todos.is_empty() {
        vec![Row::new(vec![
            Cell::from(Span::styled("", theme::dim())),
            Cell::from(Span::styled("  No active todos", theme::dim())),
        ])]
    } else {
        team.todos
            .iter()
            .map(|todo| {
                let sym = todo_symbol(&todo.status);
                let label = todo_label(&todo.status);
                let sty = theme::task_status_style(&todo.status);

                let status_text = format!("{} {}", sym, label);

                // Show activeForm for in_progress, content for others
                let display = if todo.status == "in_progress" && !todo.active_form.is_empty() {
                    &todo.active_form
                } else {
                    &todo.content
                };

                Row::new(vec![
                    Cell::from(Span::styled(status_text, sty)),
                    Cell::from(Span::styled(display.clone(), theme::text())),
                ])
            })
            .collect()
    };

    let border_style = if is_focused {
        theme::accent()
    } else {
        theme::border()
    };
    let block = Block::default()
        .title(Span::styled(" Todos ", theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let highlight = ratatui::style::Style::new()
        .bg(ratatui::style::Color::Black)
        .fg(ratatui::style::Color::White)
        .add_modifier(ratatui::style::Modifier::BOLD);

    let widths = [
        Constraint::Length(12),
        Constraint::Percentage(80),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(highlight);

    frame.render_stateful_widget(table, area, &mut app.tasks_state);
}
