use ratatui::style::{Color, Modifier, Style};

use crate::store::models::AgentStatus;

// ─── Constants ───

pub const BORDER: Style = Style::new().fg(Color::DarkGray);
pub const SELECTED: Style = Style::new().add_modifier(Modifier::REVERSED);
pub const HEADER: Style = Style::new().fg(Color::White).add_modifier(Modifier::BOLD);
pub const TITLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const ACTIVE: Color = Color::Green;
pub const IDLE: Color = Color::Blue;
pub const SHUTDOWN: Color = Color::DarkGray;

// ─── Agent status helpers ───

/// Return a style coloured by agent status.
pub fn status_style(status: &AgentStatus) -> Style {
    match status {
        AgentStatus::Active => Style::new().fg(ACTIVE),
        AgentStatus::Idle => Style::new().fg(IDLE),
        AgentStatus::Shutdown => Style::new().fg(SHUTDOWN),
        AgentStatus::Unknown => Style::new().fg(Color::Yellow),
    }
}

/// Unicode symbol for each agent status.
pub fn status_symbol(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active => "\u{25cf}", // ●
        AgentStatus::Idle => "\u{25cb}",   // ○
        AgentStatus::Shutdown => "\u{2716}", // ✖
        AgentStatus::Unknown => "?",
    }
}

// ─── Task status helpers ───

/// Unicode symbol for each task status string.
pub fn task_status_symbol(status: &str) -> &'static str {
    match status {
        "completed" => "\u{2713}", // ✓
        "in_progress" => "\u{25cf}", // ●
        "pending" => "\u{25cb}",   // ○
        "blocked" => "\u{2298}",   // ⊘
        _ => "?",
    }
}

/// Style coloured by task status string.
pub fn task_status_style(status: &str) -> Style {
    match status {
        "completed" => Style::new().fg(Color::Green),
        "in_progress" => Style::new().fg(Color::Yellow),
        "pending" => Style::new().fg(Color::White),
        "blocked" => Style::new().fg(Color::Red),
        _ => Style::new().fg(Color::DarkGray),
    }
}

// ─── Tool colouring ───

/// Assign a colour to each tool type so they stand out in the activity feed.
pub fn tool_style(tool_name: &str) -> Style {
    let lower = tool_name.to_lowercase();
    if lower.contains("read") || lower.contains("cat") {
        Style::new().fg(Color::Cyan)
    } else if lower.contains("write") || lower.contains("edit") || lower.contains("patch") {
        Style::new().fg(Color::Yellow)
    } else if lower.contains("bash") || lower.contains("exec") || lower.contains("run") {
        Style::new().fg(Color::Magenta)
    } else if lower.contains("search") || lower.contains("grep") || lower.contains("glob") {
        Style::new().fg(Color::Blue)
    } else if lower.contains("list") || lower.contains("ls") {
        Style::new().fg(Color::Green)
    } else {
        Style::new().fg(Color::White)
    }
}
