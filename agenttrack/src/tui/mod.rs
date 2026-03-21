pub mod theme;
pub mod app_state;
pub mod layout;
pub mod top_bar;
pub mod agents_panel;
pub mod tasks_panel;
pub mod activity_panel;
pub mod messages_panel;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use tokio::sync::watch;

use crate::store::event::StoreSnapshot;
use app_state::AppState;

/// Run the TUI event loop. Blocks until user quits.
pub async fn run_tui(
    snapshot_rx: watch::Receiver<StoreSnapshot>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new();
    let mut last_snapshot = snapshot_rx.borrow().clone();

    loop {
        // Check for new snapshot
        if snapshot_rx.has_changed().unwrap_or(false) {
            last_snapshot = snapshot_rx.borrow().clone();
        }

        // Draw
        terminal.draw(|frame| {
            render(frame, &last_snapshot, &app);
        })?;

        // Handle input (poll with 100ms timeout)
        if event::poll(Duration::from_millis(100))? {
            if let CEvent::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let agent_count = last_snapshot
                    .teams
                    .get(app.selected_team_index)
                    .or(last_snapshot.teams.first())
                    .map(|t| t.agents.len())
                    .unwrap_or(0);

                let team_count = last_snapshot.teams.len();

                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('j') | KeyCode::Down => {
                        let max = panel_item_count(&last_snapshot, &app);
                        app.scroll_down(max);
                    }
                    KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
                    KeyCode::Left => app.prev_agent(agent_count),
                    KeyCode::Right => app.next_agent(agent_count),
                    KeyCode::Tab => app.next_team(team_count),
                    KeyCode::BackTab => app.prev_team(team_count), // Shift+Tab
                    KeyCode::Char('1') => app.select_panel(app_state::Panel::Agents),
                    KeyCode::Char('2') => app.select_panel(app_state::Panel::Tasks),
                    KeyCode::Char('3') => app.select_panel(app_state::Panel::Activity),
                    KeyCode::Char('4') => app.select_panel(app_state::Panel::Messages),
                    KeyCode::Char('w') => {
                        let _ = open::that("http://localhost:7891");
                    }
                    KeyCode::Char('t') => {
                        let current = theme::is_light_mode();
                        theme::set_light_mode(!current);
                    }
                    KeyCode::Char('?') => app.show_help = !app.show_help,
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn panel_item_count(snapshot: &StoreSnapshot, app: &AppState) -> usize {
    let team = match snapshot.teams.get(app.selected_team_index).or(snapshot.teams.first()) {
        Some(t) => t,
        None => return 0,
    };
    match app.active_panel {
        app_state::Panel::Agents => team.agents.len(),
        app_state::Panel::Tasks => team.tasks.len(),
        app_state::Panel::Activity => team.tool_events.len(),
        app_state::Panel::Messages => team.messages.len(),
    }
}

/// Main render entry point -- called once per frame by the TUI event loop.
pub fn render(frame: &mut Frame, snapshot: &StoreSnapshot, app: &AppState) {
    // Paint background
    let bg_widget = ratatui::widgets::Block::default().style(theme::bg());
    frame.render_widget(bg_widget, frame.area());

    let areas = layout::build_layout(frame.area());

    // Top tab bar (always visible)
    top_bar::render(frame, areas.top_bar, app, snapshot);

    if let Some(team) = snapshot.teams.get(app.selected_team_index).or(snapshot.teams.first()) {
        agents_panel::render(frame, areas.agents, team, app);
        tasks_panel::render(frame, areas.tasks, team, app);
        activity_panel::render(frame, areas.activity, team, app);
        messages_panel::render(frame, areas.messages, team, app);
    } else {
        let placeholder = Paragraph::new("Waiting for sessions...")
            .style(theme::dim())
            .alignment(Alignment::Center);
        frame.render_widget(placeholder, areas.activity);
    }

    // Status bar at the bottom
    render_status_bar(frame, areas.help_bar);
}

/// Render the bottom status bar: branding + keybindings only.
fn render_status_bar(frame: &mut Frame, area: ratatui::layout::Rect) {
    let spans = vec![
        Span::styled(" cctrack@2026", theme::title()),
        Span::styled(" \u{2502} ", theme::border()),
        Span::styled("Tab", Style::new().fg(Color::Cyan)),
        Span::styled(" switch  ", theme::dim()),
        Span::styled("\u{2190}\u{2192}", Style::new().fg(Color::Cyan)),
        Span::styled(" select  ", theme::dim()),
        Span::styled("t", Style::new().fg(Color::Cyan)),
        Span::styled(" theme  ", theme::dim()),
        Span::styled("q", Style::new().fg(Color::Cyan)),
        Span::styled(" quit", theme::dim()),
    ];

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}
