use std::path::{Path, PathBuf};

use notify::{Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::warn;

use crate::store::event::Event;
use crate::store::models::*;

/// Run the file watcher collector loop.
///
/// 1. On startup, scans `claude_home/teams/` for existing team dirs.
/// 2. For each team dir: reads config.json, tasks, and inbox messages.
/// 3. Sets up a recursive `notify` watcher on teams/ and tasks/ dirs.
/// 4. On file change events, debounces 500ms then processes the changed file.
pub async fn run(claude_home: PathBuf, event_tx: mpsc::Sender<Event>) {
    // Canonicalize to resolve symlinks (e.g., /var -> /private/var on macOS)
    // so that paths from notify match our stored paths.
    let claude_home = std::fs::canonicalize(&claude_home).unwrap_or(claude_home);
    let teams_dir = claude_home.join("teams");
    let tasks_dir = claude_home.join("tasks");

    // Ensure watched directories exist
    if let Err(e) = std::fs::create_dir_all(&teams_dir) {
        warn!("Failed to create teams directory: {}", e);
    }
    if let Err(e) = std::fs::create_dir_all(&tasks_dir) {
        warn!("Failed to create tasks directory: {}", e);
    }

    // Phase 1: Initial scan of existing team directories
    initial_scan(&teams_dir, &tasks_dir, &event_tx).await;

    // Phase 2: Set up file watcher for ongoing changes
    watch_for_changes(&claude_home, event_tx).await;
}

/// Scan existing team directories and emit events for everything found.
async fn initial_scan(
    teams_dir: &Path,
    tasks_dir: &Path,
    event_tx: &mpsc::Sender<Event>,
) {
    let entries = match std::fs::read_dir(teams_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read teams directory: {}", e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let team_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Read config.json
        let config_path = path.join("config.json");
        if config_path.exists() {
            if let Some(config) = read_team_config(&config_path) {
                let _ = event_tx
                    .send(Event::TeamUpdate {
                        team_name: team_name.clone(),
                        config,
                    })
                    .await;
            }
        }

        // Read task files from tasks/<team>/
        let team_tasks_dir = tasks_dir.join(&team_name);
        if team_tasks_dir.exists() {
            scan_task_files(&team_tasks_dir, &team_name, event_tx).await;
        }

        // Read inbox files from teams/<team>/inboxes/
        let inboxes_dir = path.join("inboxes");
        if inboxes_dir.exists() {
            scan_inbox_files(&inboxes_dir, &team_name, event_tx).await;
        }
    }
}

/// Scan all .json task files in a team's tasks directory.
async fn scan_task_files(
    tasks_dir: &Path,
    team_name: &str,
    event_tx: &mpsc::Sender<Event>,
) {
    let entries = match std::fs::read_dir(tasks_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read tasks directory {:?}: {}", tasks_dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_json_file(&path) {
            continue;
        }

        if let Some(task) = read_task_file(&path) {
            let _ = event_tx
                .send(Event::TaskUpdate {
                    team_name: team_name.to_string(),
                    task,
                })
                .await;
        }
    }
}

/// Scan all .json inbox files in a team's inboxes directory.
async fn scan_inbox_files(
    inboxes_dir: &Path,
    team_name: &str,
    event_tx: &mpsc::Sender<Event>,
) {
    let entries = match std::fs::read_dir(inboxes_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read inboxes directory {:?}: {}", inboxes_dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_json_file(&path) {
            continue;
        }

        let agent_name = match path.file_stem().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        if let Some(messages) = read_inbox_file(&path) {
            let _ = event_tx
                .send(Event::MessageUpdate {
                    team_name: team_name.to_string(),
                    agent_name,
                    messages,
                })
                .await;
        }
    }
}

/// Set up a notify watcher and process file change events with debouncing.
async fn watch_for_changes(claude_home: &Path, event_tx: mpsc::Sender<Event>) {
    let (fs_tx, mut fs_rx) = mpsc::channel::<PathBuf>(256);

    let teams_dir = claude_home.join("teams");
    let tasks_dir = claude_home.join("tasks");

    // Set up the notify watcher
    let fs_tx_clone = fs_tx.clone();
    let mut watcher: RecommendedWatcher =
        match notify::recommended_watcher(move |res: Result<NotifyEvent, notify::Error>| {
            if let Ok(event) = res {
                // Only process Create, Modify, and Remove events
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in event.paths {
                            let _ = fs_tx_clone.blocking_send(path);
                        }
                    }
                    _ => {}
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                warn!("Failed to create file watcher: {}", e);
                // Keep the task alive so it doesn't terminate
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        };

    // Watch both directories recursively
    if let Err(e) = watcher.watch(&teams_dir, RecursiveMode::Recursive) {
        warn!("Failed to watch teams directory: {}", e);
    }
    if let Err(e) = watcher.watch(&tasks_dir, RecursiveMode::Recursive) {
        warn!("Failed to watch tasks directory: {}", e);
    }

    // Keep watcher alive by holding it in scope
    let _watcher = watcher;

    // Process file system events with debouncing
    loop {
        match fs_rx.recv().await {
            Some(first_path) => {
                // Debounce: wait 500ms to let rapid writes settle
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                // Collect all unique paths (including the first one)
                let mut paths = vec![first_path];
                while let Ok(extra_path) = fs_rx.try_recv() {
                    if !paths.contains(&extra_path) {
                        paths.push(extra_path);
                    }
                }

                // Process each unique changed path
                for path in paths {
                    process_file_change(&path, &teams_dir, &tasks_dir, &event_tx).await;
                }
            }
            None => {
                // Channel closed, watcher dropped
                break;
            }
        }
    }
}

/// Process a single file change event based on its path.
async fn process_file_change(
    path: &Path,
    teams_dir: &Path,
    tasks_dir: &Path,
    event_tx: &mpsc::Sender<Event>,
) {
    // Skip non-JSON files and .lock files
    if !is_json_file(path) {
        return;
    }

    // Try to determine what kind of file changed based on path segments
    let path_str = path.to_string_lossy();

    if path_str.contains("/teams/") {
        // Could be config.json or an inbox file
        if path.file_name().and_then(|n| n.to_str()) == Some("config.json") {
            // config.json changed -> TeamUpdate
            if let Some(team_name) = extract_team_name_from_teams_path(path, teams_dir) {
                if let Some(config) = read_team_config(path) {
                    let _ = event_tx
                        .send(Event::TeamUpdate { team_name, config })
                        .await;
                }
            }
        } else if path_str.contains("/inboxes/") {
            // inboxes/*.json changed -> MessageUpdate
            if let Some((team_name, agent_name)) =
                extract_team_and_agent_from_inbox_path(path, teams_dir)
            {
                if let Some(messages) = read_inbox_file(path) {
                    let _ = event_tx
                        .send(Event::MessageUpdate {
                            team_name,
                            agent_name,
                            messages,
                        })
                        .await;
                }
            }
        }
    } else if path_str.contains("/tasks/") {
        // tasks/<team>/*.json changed -> TaskUpdate
        if let Some(team_name) = extract_team_name_from_tasks_path(path, tasks_dir) {
            if let Some(task) = read_task_file(path) {
                let _ = event_tx
                    .send(Event::TaskUpdate { team_name, task })
                    .await;
            }
        }
    }
}

// ─── Path Helpers ───────────────────────────────────────────────────────────

/// Check if a path points to a .json file (not a .lock file).
fn is_json_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("json"))
}

/// Extract team name from a path under teams/<team>/...
fn extract_team_name_from_teams_path(path: &Path, teams_dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(teams_dir).ok()?;
    let first_component = relative.components().next()?;
    Some(first_component.as_os_str().to_str()?.to_string())
}

/// Extract team name and agent name from an inbox path: teams/<team>/inboxes/<agent>.json
fn extract_team_and_agent_from_inbox_path(
    path: &Path,
    teams_dir: &Path,
) -> Option<(String, String)> {
    let team_name = extract_team_name_from_teams_path(path, teams_dir)?;
    let agent_name = path.file_stem().and_then(|n| n.to_str())?.to_string();
    Some((team_name, agent_name))
}

/// Extract team name from a path under tasks/<team>/...
fn extract_team_name_from_tasks_path(path: &Path, tasks_dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(tasks_dir).ok()?;
    let first_component = relative.components().next()?;
    Some(first_component.as_os_str().to_str()?.to_string())
}

// ─── File Reading Helpers ───────────────────────────────────────────────────

/// Read and parse a team config.json file. Returns None on failure.
fn read_team_config(path: &Path) -> Option<TeamConfig> {
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<TeamConfig>(&contents) {
            Ok(config) => Some(config),
            Err(e) => {
                warn!("Failed to parse team config {:?}: {}", path, e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to read team config {:?}: {}", path, e);
            None
        }
    }
}

/// Read and parse a task .json file. Returns None on failure.
fn read_task_file(path: &Path) -> Option<TaskFile> {
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<TaskFile>(&contents) {
            Ok(task) => Some(task),
            Err(e) => {
                warn!("Failed to parse task file {:?}: {}", path, e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to read task file {:?}: {}", path, e);
            None
        }
    }
}

/// Read and parse an inbox .json file (array of InboxMessage). Returns None on failure.
fn read_inbox_file(path: &Path) -> Option<Vec<InboxMessage>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Vec<InboxMessage>>(&contents) {
            Ok(messages) => Some(messages),
            Err(e) => {
                warn!("Failed to parse inbox file {:?}: {}", path, e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to read inbox file {:?}: {}", path, e);
            None
        }
    }
}
