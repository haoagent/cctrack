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
// Design principles:
//   1. ANSI 0-15 only → terminal theme controls the actual RGB values
//   2. Three-tier grayscale: White(bold) > Gray(normal) > DarkGray(dim)
//   3. Color = semantic signal only: green=ok, yellow=attention, red=error
//   4. Max 3-4 hues on screen at once → each color carries stronger meaning
//   5. Selection via REVERSED → follows terminal's own fg/bg
//   6. bg always Reset → never fight the terminal background

pub const SELECTED: Style = Style::new().add_modifier(Modifier::REVERSED);

// ─── Grayscale hierarchy (most of the UI should be these) ───

pub fn bg() -> Style {
    Style::new().bg(Color::Reset)
}

pub fn border() -> Style {
    if is_light() {
        Style::new().fg(Color::Gray)         // ANSI 7
    } else {
        Style::new().fg(Color::DarkGray)     // ANSI 8
    }
}

/// Bright — column headers, important labels.
pub fn header() -> Style {
    if is_light() {
        Style::new().fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
    }
}

/// Normal — body text, data values. Follows terminal's default foreground.
pub fn text() -> Style {
    Style::new().fg(Color::Reset)
}

/// Subdued — timestamps, borders, secondary info.
pub fn dim() -> Style {
    if is_light() {
        Style::new().fg(Color::Gray)
    } else {
        Style::new().fg(Color::DarkGray)
    }
}

// ─── Semantic colors (used sparingly for signals) ───

/// Panel titles — the only decorative color; keeps the UI from being all gray.
pub fn title() -> Style {
    if is_light() {
        Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD)
    }
}

/// Project/session name in agent list — bold text, no extra color.
pub fn project_name() -> Style {
    text().add_modifier(Modifier::BOLD)
}

/// Keybinding hints in status bar.
pub fn accent() -> Style {
    if is_light() {
        Style::new().fg(Color::Blue)
    } else {
        Style::new().fg(Color::LightBlue)
    }
}

/// Cost/money values — green signals "measurable spend".
pub fn cost_style() -> Style {
    Style::new().fg(Color::Green)
}

// ─── Tab selection ───

/// Selected tab — ANSI Blue bg (follows terminal's blue definition).
pub fn tab_selected() -> Style {
    Style::new().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
}

/// Dot on selected tab — same Blue bg, green/dim fg. No separate block.
pub fn tab_dot_selected(has_active: bool) -> Style {
    if has_active {
        Style::new().bg(Color::Blue).fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(Color::Blue).fg(Color::DarkGray)
    }
}

// ─── Agent status (traffic-light: green / yellow / gray / red) ───

pub fn status_style(status: &AgentStatus) -> Style {
    match status {
        AgentStatus::Active   => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        AgentStatus::Idle     => Style::new().fg(Color::Yellow),
        AgentStatus::Shutdown => dim(),
        AgentStatus::Unknown  => Style::new().fg(Color::Red),
    }
}

pub fn status_symbol(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active   => "\u{25cf}", // ●
        AgentStatus::Idle     => "\u{25cb}", // ○
        AgentStatus::Shutdown => "\u{00b7}", // ·
        AgentStatus::Unknown  => "?",
    }
}

// ─── Task status (same traffic-light logic) ───

pub fn task_status_symbol(status: &str) -> &'static str {
    match status {
        "completed"   => "\u{2713}", // ✓
        "in_progress" => "\u{25cf}", // ●
        "pending"     => "\u{25cb}", // ○
        "blocked"     => "\u{2298}", // ⊘
        _ => "?",
    }
}

pub fn task_status_style(status: &str) -> Style {
    match status {
        "completed"   => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        "in_progress" => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        "pending"     => dim(),
        "blocked"     => Style::new().fg(Color::Red),
        _ => dim(),
    }
}

// ─── Message type (only color the sender label) ───

pub fn message_type_style(msg_type: &crate::store::models::MessageType) -> Style {
    use crate::store::models::MessageType;
    match msg_type {
        MessageType::TaskCompleted        => Style::new().fg(Color::Green),
        MessageType::PlanApproval         => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        MessageType::ShutdownNotification => dim(),
        MessageType::Broadcast            => text().add_modifier(Modifier::BOLD),
        MessageType::DirectMessage        => text(),
        MessageType::IdleNotification     => dim(),
    }
}

// ─── Tool names (minimal color — just dim text, bold for mutations) ───
//
// No rainbow. Tool names are metadata, not signals.
// Only distinguish: read-only (dim) vs mutating (bold) vs execution (yellow).

pub fn tool_style(tool_name: &str) -> Style {
    let lower = tool_name.to_lowercase();
    match lower.as_str() {
        // Read-only tools — dim, not important
        "read" | "grep" | "glob" | "websearch" | "webfetch" => dim(),

        // Mutating tools — bold to signal "something changed"
        "edit" | "write" | "notebookedit" => text().add_modifier(Modifier::BOLD),

        // Execution — yellow = pay attention
        "bash" => Style::new().fg(Color::Yellow),

        // Orchestration — bold, these spawn work
        "agent" => text().add_modifier(Modifier::BOLD),

        // Meta — invisible
        "todowrite" | "skill" => dim(),

        _ => text(),
    }
}
