//! Aggregate usage statistics from Claude Code transcript files.
//!
//! Scans `~/.claude/projects/` for .jsonl transcripts, sums token usage,
//! and groups by time period and project.

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use chrono::{NaiveDate, Utc, Datelike};
use serde::Serialize;

/// Per-model pricing ($/MTok) — Anthropic API March 2026
/// Source: https://platform.claude.com/docs/en/about-claude/pricing
/// Tiered: tokens above 200k per-message are charged at higher rates.
struct ModelPricing {
    input: f64,
    output: f64,
    cache_write: f64,     // cache_creation (flat, or 5m ephemeral)
    cache_write_1h: f64,  // 2x input (ephemeral 1-hour)
    cache_read: f64,      // 0.1x input
    // Tiered rates for tokens above 200k per-message
    input_above_200k: f64,
    output_above_200k: f64,
    cache_write_above_200k: f64,
    cache_read_above_200k: f64,
}

/// Threshold for tiered pricing (per-message).
const TIERED_THRESHOLD: u64 = 200_000;

fn get_pricing(model: &str) -> ModelPricing {
    let m = model.to_lowercase();
    if m.contains("opus") {
        // Opus 4.5/4.6: $5/$25, above 200k: $10/$37.50
        ModelPricing {
            input: 5.0, output: 25.0, cache_write: 6.25, cache_write_1h: 10.0, cache_read: 0.50,
            input_above_200k: 10.0, output_above_200k: 37.50, cache_write_above_200k: 12.50, cache_read_above_200k: 1.00,
        }
    } else if m.contains("sonnet") {
        // Sonnet 4.5/4.6: $3/$15 (no tiered pricing)
        ModelPricing {
            input: 3.0, output: 15.0, cache_write: 3.75, cache_write_1h: 6.0, cache_read: 0.30,
            input_above_200k: 3.0, output_above_200k: 15.0, cache_write_above_200k: 3.75, cache_read_above_200k: 0.30,
        }
    } else if m.contains("haiku") {
        // Haiku 4.5: $1/$5 (no tiered pricing)
        ModelPricing {
            input: 1.0, output: 5.0, cache_write: 1.25, cache_write_1h: 2.0, cache_read: 0.10,
            input_above_200k: 1.0, output_above_200k: 5.0, cache_write_above_200k: 1.25, cache_read_above_200k: 0.10,
        }
    } else {
        // Default to Sonnet pricing
        ModelPricing {
            input: 3.0, output: 15.0, cache_write: 3.75, cache_write_1h: 6.0, cache_read: 0.30,
            input_above_200k: 3.0, output_above_200k: 15.0, cache_write_above_200k: 3.75, cache_read_above_200k: 0.30,
        }
    }
}

/// Calculate cost for a token count with tiered pricing.
/// Tokens up to TIERED_THRESHOLD use base_rate, tokens above use tiered_rate.
fn tiered_cost(tokens: u64, base_rate: f64, tiered_rate: f64) -> f64 {
    if tokens <= TIERED_THRESHOLD {
        tokens as f64 / 1_000_000.0 * base_rate
    } else {
        let below = TIERED_THRESHOLD as f64 / 1_000_000.0 * base_rate;
        let above = (tokens - TIERED_THRESHOLD) as f64 / 1_000_000.0 * tiered_rate;
        below + above
    }
}

/// Usage stats for a single transcript/session.
/// Cost is pre-computed per-message with tiered pricing, then accumulated.
#[derive(Debug, Clone, Default)]
struct SessionUsage {
    date: Option<NaiveDate>,
    project: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    /// Pre-computed cost accumulated from per-message tiered pricing.
    cost_usd: f64,
}

impl SessionUsage {
    fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens
            + self.cache_write_tokens
    }
}

/// Per-message token counts before aggregation.
struct MessageTokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    cache_write_1h: u64,
}

impl MessageTokens {
    /// Compute cost for this single message using tiered pricing.
    fn cost(&self, pricing: &ModelPricing) -> f64 {
        let input = tiered_cost(self.input, pricing.input, pricing.input_above_200k);
        let output = tiered_cost(self.output, pricing.output, pricing.output_above_200k);
        let cw = tiered_cost(self.cache_write, pricing.cache_write, pricing.cache_write_above_200k);
        let cw_1h = self.cache_write_1h as f64 / 1_000_000.0 * pricing.cache_write_1h;
        let cr = tiered_cost(self.cache_read, pricing.cache_read, pricing.cache_read_above_200k);
        input + output + cw + cw_1h + cr
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
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

impl UsageBucket {
    fn add(&mut self, s: &SessionUsage) {
        self.sessions += 1;
        self.input_tokens += s.input_tokens;
        self.output_tokens += s.output_tokens;
        self.cache_read_tokens += s.cache_read_tokens;
        self.cache_write_tokens += s.cache_write_tokens;
        self.total_tokens += s.total_tokens();
        self.cost_usd += s.cost_usd;
    }
}

/// A single day's aggregated usage — for time-series charts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DailyPoint {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub sessions: usize,
}

/// A 5-hour usage window for cap utilization tracking.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CapWindow {
    /// Window start time (ISO 8601).
    pub start: String,
    /// Window end time (ISO 8601).
    pub end: String,
    /// Output tokens consumed in this window.
    pub output_tokens: u64,
    /// Plan's 5h output token cap.
    pub cap: u64,
    /// Utilization percentage (0-100).
    pub utilization_pct: f64,
    /// Wasted tokens (cap - used, 0 if still open).
    pub waste: u64,
    /// Whether this window is currently active (not yet expired).
    pub is_current: bool,
}

/// Cap utilization summary.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CapSummary {
    /// Current 5h window.
    pub current: Option<CapWindow>,
    /// Recent completed windows (last 48h, newest first).
    pub history: Vec<CapWindow>,
    /// Average utilization % across completed windows.
    pub avg_utilization_pct: f64,
    /// Total wasted output tokens across completed windows.
    pub total_waste: u64,
    /// Plan tier name.
    pub plan: String,
    /// 5h cap value.
    pub cap_per_window: u64,
}

/// Full stats report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatsReport {
    pub today: UsageBucket,
    pub this_week: UsageBucket,
    pub this_month: UsageBucket,
    pub total: UsageBucket,
    pub by_project: Vec<UsageBucket>,
    /// Daily time-series for charts (last 30 days, sorted ascending).
    pub daily: Vec<DailyPoint>,
    /// 5h window cap utilization.
    pub cap: CapSummary,
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

            // Skip in-progress transcripts (modified within last 60 seconds)
            if let Ok(meta) = file_path.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(age) = SystemTime::now().duration_since(modified) {
                        if age.as_secs() < 60 {
                            continue;
                        }
                    }
                }
            }

            let usages = parse_transcript(&file_path, &project_name);
            sessions.extend(usages);
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

    // Build daily time-series (last 30 days)
    let cutoff = today - chrono::Duration::days(30);
    let mut daily_map: HashMap<NaiveDate, DailyPoint> = HashMap::new();
    for s in &sessions {
        if let Some(date) = s.date {
            if date >= cutoff {
                let dp = daily_map.entry(date).or_insert_with(|| DailyPoint {
                    date: date.format("%Y-%m-%d").to_string(),
                    ..Default::default()
                });
                dp.input_tokens += s.input_tokens;
                dp.output_tokens += s.output_tokens;
                dp.cache_tokens += s.cache_read_tokens + s.cache_write_tokens;
                dp.total_tokens += s.total_tokens();
                dp.cost_usd += s.cost_usd;
                dp.sessions += 1;
            }
        }
    }
    let mut daily: Vec<DailyPoint> = daily_map.into_values().collect();
    daily.sort_by(|a, b| a.date.cmp(&b.date));
    report.daily = daily;

    // Build 5h window cap utilization
    let plan = crate::config::Config::load().plan;
    report.cap = compute_cap_windows(&projects_dir, &plan);

    report
}

/// Compute 5h window cap utilization by scanning transcripts for output tokens.
fn compute_cap_windows(projects_dir: &Path, plan: &crate::config::PlanConfig) -> CapSummary {
    let cap = plan.output_cap_5h();
    let now = Utc::now();
    let cutoff = now - chrono::Duration::hours(48);

    // Collect all (timestamp, output_tokens) from recent transcripts
    let mut events: Vec<(chrono::DateTime<Utc>, u64)> = Vec::new();

    if let Ok(project_dirs) = std::fs::read_dir(projects_dir) {
        for project_entry in project_dirs.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() { continue; }
            if let Ok(files) = std::fs::read_dir(&project_path) {
                for file_entry in files.flatten() {
                    let file_path = file_entry.path();
                    if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
                    // Only scan recent files (modified within 48h)
                    if let Ok(meta) = file_path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified.elapsed().map(|d| d.as_secs() > 48 * 3600).unwrap_or(true) {
                                continue;
                            }
                        }
                    }
                    collect_output_events(&file_path, &cutoff, &mut events);
                }
            }
        }
    }

    events.sort_by_key(|e| e.0);
    if events.is_empty() {
        return CapSummary {
            plan: plan.tier.clone(),
            cap_per_window: cap,
            ..Default::default()
        };
    }

    // Build 5h windows: first event starts window 0, each window is 5h
    let window_duration = chrono::Duration::hours(5);
    let mut windows: Vec<CapWindow> = Vec::new();
    let mut window_start = events[0].0;
    let mut window_output: u64 = 0;

    for &(ts, output) in &events {
        // If this event is beyond the current window, close it and start new
        while ts >= window_start + window_duration {
            let window_end = window_start + window_duration;
            let is_current = now >= window_start && now < window_end;
            windows.push(CapWindow {
                start: window_start.to_rfc3339(),
                end: window_end.to_rfc3339(),
                output_tokens: window_output,
                cap,
                utilization_pct: (window_output as f64 / cap as f64 * 100.0).min(100.0),
                waste: if is_current { 0 } else { cap.saturating_sub(window_output) },
                is_current,
            });
            window_start = window_end;
            window_output = 0;
        }
        window_output += output;
    }

    // Close the current/last window
    let window_end = window_start + window_duration;
    let is_current = now >= window_start && now < window_end;
    windows.push(CapWindow {
        start: window_start.to_rfc3339(),
        end: window_end.to_rfc3339(),
        output_tokens: window_output,
        cap,
        utilization_pct: (window_output as f64 / cap as f64 * 100.0).min(100.0),
        waste: if is_current { 0 } else { cap.saturating_sub(window_output) },
        is_current,
    });

    // Split current vs history
    let current = windows.iter().find(|w| w.is_current).cloned();
    let history: Vec<CapWindow> = windows.iter().filter(|w| !w.is_current).cloned().collect();

    let completed_count = history.len();
    let avg_util = if completed_count > 0 {
        history.iter().map(|w| w.utilization_pct).sum::<f64>() / completed_count as f64
    } else { 0.0 };
    let total_waste: u64 = history.iter().map(|w| w.waste).sum();

    CapSummary {
        current,
        history,
        avg_utilization_pct: (avg_util * 10.0).round() / 10.0,
        total_waste,
        plan: plan.tier.clone(),
        cap_per_window: cap,
    }
}

/// Extract (timestamp, output_tokens) from a transcript file for cap tracking.
fn collect_output_events(
    path: &Path,
    cutoff: &chrono::DateTime<Utc>,
    events: &mut Vec<(chrono::DateTime<Utc>, u64)>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        if !line.contains("\"output_tokens\"") { continue; }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            let ts_str = val.get("timestamp").and_then(|v| v.as_str());
            let output = val.get("message")
                .and_then(|m| m.get("usage"))
                .and_then(|u| u.get("output_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if output == 0 { continue; }

            if let Some(ts) = ts_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) {
                let ts_utc = ts.with_timezone(&Utc);
                if ts_utc >= *cutoff {
                    events.push((ts_utc, output));
                }
            }
        }
    }
}

/// Parse a single transcript .jsonl file for usage data.
/// Returns multiple SessionUsage entries — one per day the session was active.
/// Cost is computed per-message with tiered pricing, then accumulated per-day.
fn parse_transcript(path: &Path, project_name: &str) -> Vec<SessionUsage> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut model = String::new();
    // Accumulate usage per-day: date → usage
    let mut daily: HashMap<NaiveDate, SessionUsage> = HashMap::new();

    for line in content.lines() {
        // Only process lines with usage or model data
        if !line.contains("\"usage\"") && !line.contains("\"model\"") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            // Extract model — update per message (can change mid-session)
            if let Some(m) = val.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                model = m.to_string();
            }

            // Extract timestamp for this message → determine which day
            let msg_date = val.get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|ts| if ts.len() >= 10 { NaiveDate::parse_from_str(&ts[..10], "%Y-%m-%d").ok() } else { None });

            if let Some(u) = val.get("message").and_then(|m| m.get("usage")) {
                let date = msg_date.unwrap_or_else(|| Utc::now().date_naive());

                // Extract per-message token counts
                let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let cache_read = u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

                let (cache_write, cache_write_1h) = if let Some(cc) = u.get("cache_creation") {
                    (
                        cc.get("ephemeral_5m_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        cc.get("ephemeral_1h_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                    )
                } else {
                    (u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0), 0)
                };

                // Compute per-message cost with tiered pricing
                let msg = MessageTokens { input, output, cache_read, cache_write, cache_write_1h };
                let pricing = get_pricing(&model);
                let msg_cost = msg.cost(&pricing);

                // Accumulate into daily bucket
                let day_usage = daily.entry(date).or_insert_with(|| SessionUsage {
                    date: Some(date),
                    project: project_name.to_string(),
                    ..Default::default()
                });

                day_usage.input_tokens += input;
                day_usage.output_tokens += output;
                day_usage.cache_read_tokens += cache_read;
                day_usage.cache_write_tokens += cache_write + cache_write_1h;
                day_usage.cost_usd += msg_cost;
            }
        }
    }

    // Return all daily entries (skip empty)
    daily.into_values()
        .filter(|u| u.total_tokens() > 0)
        .collect()
}

/// Convert a project directory name to a readable name.
/// e.g. "-Users-jerry-Documents-Clipal" → "Clipal"
fn project_dir_to_name(path: &Path) -> String {
    let dir_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Claude Code encodes paths with dashes: "-Users-jerry-Documents-Clipal"
    // Take the last non-empty segment
    let name = if dir_name.starts_with('-') {
        dir_name.rsplit('-')
            .find(|s| !s.is_empty())
            .unwrap_or("root")
            .to_string()
    } else if dir_name == "." || dir_name.is_empty() {
        "root".to_string()
    } else {
        dir_name.to_string()
    };

    // Final safety: never return empty
    if name.is_empty() { "root".to_string() } else { name }
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
