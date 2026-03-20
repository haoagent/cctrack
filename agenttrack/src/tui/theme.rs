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

// ─── Ghostty-inspired palette (soft pastels on dark, high contrast on light) ───
//
// Dark mode: #1a1b26 background feel, muted pastels for accents
// 256-color indexed values for precise control:
//   242 = medium gray (borders)
//   249 = light gray (dim text)
//   252 = near-white (normal text)

pub const SELECTED: Style = Style::new().add_modifier(Modifier::REVERSED);

pub fn bg() -> Style {
    if is_light() {
        Style::new()
    } else {
        Style::new().bg(Color::Rgb(26, 27, 38)) // dark navy-black
    }
}

pub fn border() -> Style {
    if is_light() {
        Style::new().fg(Color::Indexed(245))
    } else {
        Style::new().fg(Color::Indexed(242))
    }
}

pub fn header() -> Style {
    if is_light() {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Indexed(252)).add_modifier(Modifier::BOLD)
    }
}

pub fn title() -> Style {
    if is_light() {
        Style::new().fg(Color::Rgb(86, 156, 214)).add_modifier(Modifier::BOLD) // soft blue
    } else {
        Style::new().fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD) // sky blue
    }
}

pub fn dim() -> Style {
    if is_light() {
        Style::new().fg(Color::Indexed(245))
    } else {
        Style::new().fg(Color::Indexed(245)) // visible gray on dark
    }
}

pub fn text() -> Style {
    if is_light() {
        Style::new()
    } else {
        Style::new().fg(Color::Indexed(252))
    }
}

// ─── Agent status ───

pub fn status_style(status: &AgentStatus) -> Style {
    match status {
        AgentStatus::Active => Style::new()
            .fg(Color::Rgb(158, 206, 106)) // soft green
            .add_modifier(Modifier::BOLD),
        AgentStatus::Idle => Style::new()
            .fg(Color::Rgb(122, 162, 247)), // soft blue
        AgentStatus::Shutdown => Style::new()
            .fg(Color::Rgb(247, 118, 142)), // soft red/pink
        AgentStatus::Unknown => Style::new()
            .fg(Color::Rgb(224, 175, 104)), // soft amber
    }
}

pub fn status_symbol(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active => "\u{25cf}", // ●
        AgentStatus::Idle => "\u{25cb}",   // ○
        AgentStatus::Shutdown => "\u{2716}", // ✖
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
            .fg(Color::Rgb(158, 206, 106))
            .add_modifier(Modifier::BOLD),
        "in_progress" => Style::new()
            .fg(Color::Rgb(224, 175, 104))
            .add_modifier(Modifier::BOLD),
        "pending" => dim(),
        "blocked" => Style::new().fg(Color::Rgb(247, 118, 142)),
        _ => dim(),
    }
}

// ─── Tool colours ───

pub fn tool_style(tool_name: &str) -> Style {
    let lower = tool_name.to_lowercase();
    if lower.contains("read") || lower.contains("cat") {
        Style::new().fg(Color::Rgb(125, 207, 255)) // sky blue
    } else if lower.contains("write") || lower.contains("edit") || lower.contains("patch") {
        Style::new().fg(Color::Rgb(224, 175, 104)).add_modifier(Modifier::BOLD) // amber
    } else if lower.contains("bash") || lower.contains("exec") || lower.contains("run") {
        Style::new().fg(Color::Rgb(187, 154, 247)) // lavender
    } else if lower.contains("search") || lower.contains("grep") || lower.contains("glob") {
        Style::new().fg(Color::Rgb(122, 162, 247)) // soft blue
    } else if lower.contains("list") || lower.contains("ls") {
        Style::new().fg(Color::Rgb(158, 206, 106)) // soft green
    } else if lower.contains("agent") || lower.contains("todo") {
        Style::new().fg(Color::Rgb(255, 167, 196)) // pink
    } else {
        text()
    }
}
