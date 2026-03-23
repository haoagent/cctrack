//! Aggregate usage statistics from Claude Code transcript files.
//!
//! Scans `~/.claude/projects/` for .jsonl transcripts, sums token usage,
//! and groups by time period and project.

use std::collections::HashMap;
use std::path::Path;

use chrono::{NaiveDate, Utc, Datelike};
use serde::Serialize;

/// Usage stats for a single transcript/session.
#[derive(Debug, Clone, Default)]
struct SessionUsage {
    date: Option<NaiveDate>,
    project: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_create_tokens: u64,
}

impl SessionUsage {
    fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_create_tokens
    }

    fn estimated_cost_usd(&self) -> f64 {
        let input = (self.input_tokens + self.cache_create_tokens) as f64 / 1_000_000.0 * 15.0;
        let output = self.output_tokens as f64 / 1_000_000.0 * 75.0;
        let cache = self.cache_read_tokens as f64 / 1_000_000.0 * 1.5;
        input + output + cache
    }
}

/// Aggregated stats for a time period or project.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageBucket {
    pub label: String,
    pub sessions: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

impl UsageBucket {
    fn add(&mut self, s: &SessionUsage) {
        self.sessions += 1;
        self.input_tokens += s.input_tokens;
        self.output_tokens += s.output_tokens;
        self.cache_read_tokens += s.cache_read_tokens;
        self.cache_create_tokens += s.cache_create_tokens;
        self.total_tokens += s.total_tokens();
        self.cost_usd += s.estimated_cost_usd();
    }
}

/// Full stats report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatsReport {
    pub today: UsageBucket,
    pub this_week: UsageBucket,
    pub this_month: UsageBucket,
    pub total: UsageBucket,
    pub by_project: Vec<UsageBucket>,
}

/// Scan all Claude Code transcripts and compute usage stats.
pub fn compute_stats(claude_home: &Path) -> StatsReport {
    let projects_dir = claude_home.join("projects");
    if !projects_dir.exists() {
        return StatsReport::default();
    }

    let now = Utc::now();
    let today = now.date_naive();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);

    let mut sessions: Vec<SessionUsage> = Vec::new();

    // Scan all project directories
    let project_dirs = match std::fs::read_dir(&projects_dir) {
        Ok(entries) => entries,
        Err(_) => return StatsReport::default(),
    };

    for project_entry in project_dirs.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let project_name = project_dir_to_name(&project_path);

        // Scan .jsonl files in project dir
        let jsonl_files = match std::fs::read_dir(&project_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for file_entry in jsonl_files.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            if let Some(usage) = parse_transcript(&file_path, &project_name) {
                sessions.push(usage);
            }
        }
    }

    // Aggregate
    let mut report = StatsReport::default();
    report.today.label = "Today".to_string();
    report.this_week.label = "This week".to_string();
    report.this_month.label = format!("{}", today.format("%B"));
    report.total.label = "Total".to_string();

    let mut project_map: HashMap<String, UsageBucket> = HashMap::new();

    for s in &sessions {
        report.total.add(s);

        if let Some(date) = s.date {
            if date == today {
                report.today.add(s);
            }
            if date >= week_start {
                report.this_week.add(s);
            }
            if date >= month_start {
                report.this_month.add(s);
            }
        }

        let bucket = project_map.entry(s.project.clone()).or_insert_with(|| UsageBucket {
            label: s.project.clone(),
            ..Default::default()
        });
        bucket.add(s);
    }

    // Sort projects by cost descending
    let mut by_project: Vec<UsageBucket> = project_map.into_values().collect();
    by_project.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));
    report.by_project = by_project;

    report
}

/// Parse a single transcript .jsonl file for usage data.
fn parse_transcript(path: &Path, project_name: &str) -> Option<SessionUsage> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut usage = SessionUsage {
        project: project_name.to_string(),
        ..Default::default()
    };

    for line in content.lines() {
        // Extract date from first timestamp
        if usage.date.is_none() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(ts) = val.get("timestamp").and_then(|v| v.as_str()) {
                    // Parse ISO 8601: "2026-03-20T08:25:06.464Z"
                    if ts.len() >= 10 {
                        usage.date = NaiveDate::parse_from_str(&ts[..10], "%Y-%m-%d").ok();
                    }
                }
            }
        }

        // Sum token usage from assistant messages
        if !line.contains("\"usage\"") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(u) = val.get("message").and_then(|m| m.get("usage")) {
                usage.input_tokens += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.output_tokens += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.cache_read_tokens += u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.cache_create_tokens += u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            }
        }
    }

    // Skip sessions with no usage
    if usage.total_tokens() == 0 {
        return None;
    }

    Some(usage)
}

/// Convert a project directory name to a readable name.
/// e.g. "-Users-jerry-Documents-Clipal" → "Clipal"
fn project_dir_to_name(path: &Path) -> String {
    let dir_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Claude Code encodes paths with dashes: "-Users-jerry-Documents-Clipal"
    // Take the last segment
    if dir_name.starts_with('-') {
        dir_name.rsplit('-').next().unwrap_or(dir_name).to_string()
    } else if dir_name == "." || dir_name.is_empty() {
        "home".to_string()
    } else {
        dir_name.to_string()
    }
}

/// Format token count as compact string.
pub fn format_tokens(n: u64) -> String {
    if n == 0 {
        "0".to_string()
    } else if n < 1_000 {
        format!("{}", n)
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}

/// Print stats report to stdout.
pub fn print_stats(report: &StatsReport) {
    println!();
    println!("  \x1b[1;36mcctrack stats\x1b[0m");
    println!("  \x1b[90m─────────────────────────────────────────\x1b[0m");
    println!();

    // Time periods
    let periods = [&report.today, &report.this_week, &report.this_month, &report.total];
    println!("  \x1b[90m{:<14} {:>6}  {:>10}  {:>10}\x1b[0m", "", "sess", "tokens", "cost");
    for p in periods {
        let tokens = format_tokens(p.total_tokens);
        let cost = format!("${:.2}", p.cost_usd);
        let label_style = if p.label == "Total" { "\x1b[1;37m" } else { "\x1b[37m" };
        println!("  {}{:<14}\x1b[0m {:>6}  {:>10}  \x1b[32m{:>10}\x1b[0m",
            label_style, p.label, p.sessions, tokens, cost);
    }

    // By project
    if !report.by_project.is_empty() {
        println!();
        println!("  \x1b[90mBy Project\x1b[0m");
        for p in &report.by_project {
            let tokens = format_tokens(p.total_tokens);
            let cost = format!("${:.2}", p.cost_usd);
            println!("  {:<14} {:>6}  {:>10}  \x1b[32m{:>10}\x1b[0m",
                p.label, p.sessions, tokens, cost);
        }
    }

    println!();
}
