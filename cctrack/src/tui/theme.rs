use std::sync::atomic::{AtomicBool, Ordering};
use ratatui::style::{Color, Modifier, Style};

use crate::store::models::AgentStatus;

// ─── Theme mode ───

static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_light_mode(v: bool) {
    LIGHT_MODE.store(v, Ordering::Relaxed);
}

pub fn is_light_mode() -> bool {
    LIGHT_MODE.load(Ordering::Relaxed)
}

fn is_light() -> bool {
    is_light_mode()
}

// ─── Terminal-native ANSI colors ───
//
// Use ANSI color names instead of RGB hex values.
// This lets the terminal (Ghostty, iTerm, etc.) render with ITS OWN palette.
// Result: always looks native and consistent with user's terminal theme.
//
// ANSI base (0-7):   Black Red Green Yellow Blue Magenta Cyan White
// ANSI bright (8-15): same but brighter variants

pub const SELECTED: Style = Style::new().add_modifier(Modifier::REVERSED);

pub fn bg() -> Style {
    Style::new().bg(Color::Reset) // follow terminal's own background
}

pub fn border() -> Style {
    Style::new().fg(Color::DarkGray) // ANSI 8 — subtle but visible
}

pub fn header() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD) // ANSI 15
}

pub fn title() -> Style {
    Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD) // ANSI 12
}

pub fn dim() -> Style {
    Style::new().fg(Color::DarkGray) // ANSI 8
}

pub fn text() -> Style {
    Style::new().fg(Color::Gray) // ANSI 7 — terminal's normal white/gray
}

pub fn cost_style() -> Style {
    Style::new().fg(Color::Green) // ANSI 2
}

/// Highlight for project/session names.
pub fn project_name() -> Style {
    Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD) // ANSI 4
}

/// Accent for keybindings, links.
pub fn accent() -> Style {
    Style::new().fg(Color::LightBlue) // ANSI 12
}

// ─── Agent status ───

pub fn status_style(status: &AgentStatus) -> Style {
    match status {
        AgentStatus::Active => Style::new()
            .fg(Color::LightGreen)              // ANSI 10
            .add_modifier(Modifier::BOLD),
        AgentStatus::Idle => Style::new()
            .fg(Color::Blue),                   // ANSI 4
        AgentStatus::Shutdown => dim(),
        AgentStatus::Unknown => Style::new()
            .fg(Color::Yellow),                 // ANSI 3
    }
}

pub fn status_symbol(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active => "\u{25cf}", // ●
        AgentStatus::Idle => "\u{25cb}",   // ○
        AgentStatus::Shutdown => "\u{25cb}", // ○
        AgentStatus::Unknown => "?",
    }
}

// ─── Task status ───

pub fn task_status_symbol(status: &str) -> &'static str {
    match status {
        "completed" => "\u{2713}", // ✓
        "in_progress" => "\u{25cf}", // ●
        "pending" => "\u{25cb}",   // ○
        "blocked" => "\u{2298}",   // ⊘
        _ => "?",
    }
}

pub fn task_status_style(status: &str) -> Style {
    match status {
        "completed" => Style::new()
            .fg(Color::LightGreen)               // ANSI 10
            .add_modifier(Modifier::BOLD),
        "in_progress" => Style::new()
            .fg(Color::Yellow)                   // ANSI 3
            .add_modifier(Modifier::BOLD),
        "pending" => dim(),
        "blocked" => Style::new()
            .fg(Color::LightRed),                // ANSI 9
        _ => dim(),
    }
}

// ─── Message type colors ───

pub fn message_type_style(msg_type: &crate::store::models::MessageType) -> Style {
    use crate::store::models::MessageType;
    match msg_type {
        MessageType::TaskCompleted => Style::new().fg(Color::Green),
        MessageType::PlanApproval => Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        MessageType::ShutdownNotification => dim(),
        MessageType::Broadcast => Style::new().fg(Color::Magenta),
        MessageType::DirectMessage => Style::new().fg(Color::Cyan),
        MessageType::IdleNotification => dim(),
    }
}

// ─── Tool colours (ANSI native) ───

pub fn tool_style(tool_name: &str) -> Style {
    let lower = tool_name.to_lowercase();
    match lower.as_str() {
        "read"       => Style::new().fg(Color::LightBlue),   // ANSI 12
        "edit"       => Style::new().fg(Color::Yellow),       // ANSI 3
        "write"      => Style::new().fg(Color::Cyan),         // ANSI 6
        "bash"       => Style::new().fg(Color::LightGreen),   // ANSI 10
        "grep" | "glob" => Style::new().fg(Color::Magenta),   // ANSI 5
        "agent"      => Style::new().fg(Color::LightYellow),  // ANSI 11
        "todowrite"  => dim(),
        "websearch" | "webfetch" => Style::new().fg(Color::LightBlue),
        "skill"      => Style::new().fg(Color::Cyan),
        "notebookedit" => Style::new().fg(Color::Yellow),
        _            => text(),
    }
}
