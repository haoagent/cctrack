use std::fs;
use tempfile::TempDir;
use tokio::sync::mpsc;
use agenttrack::collector::file_watcher;
use agenttrack::store::event::Event;

#[tokio::test]
async fn discovers_existing_team_on_startup() {
    let tmp = TempDir::new().unwrap();
    let teams_dir = tmp.path().join("teams/test-team/inboxes");
    let tasks_dir = tmp.path().join("tasks/test-team");
    fs::create_dir_all(&teams_dir).unwrap();
    fs::create_dir_all(&tasks_dir).unwrap();

    // Write fixture config
    let config_json = fs::read_to_string("tests/fixtures/team_config.json").unwrap();
    fs::write(tmp.path().join("teams/test-team/config.json"), &config_json).unwrap();

    // Write fixture task
    let task_json = fs::read_to_string("tests/fixtures/task_1.json").unwrap();
    fs::write(tasks_dir.join("1.json"), &task_json).unwrap();

    let (tx, mut rx) = mpsc::channel(32);

    let watcher_path = tmp.path().to_path_buf();
    let handle = tokio::spawn(async move {
        file_watcher::run(watcher_path, tx).await;
    });

    // Should receive TeamUpdate from initial scan
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(3), rx.recv()
    ).await.unwrap().unwrap();
    assert!(matches!(event, Event::TeamUpdate { .. }));

    handle.abort();
}

#[tokio::test]
async fn detects_new_task_file_creation() {
    let tmp = TempDir::new().unwrap();
    let teams_dir = tmp.path().join("teams/test-team/inboxes");
    let tasks_dir = tmp.path().join("tasks/test-team");
    fs::create_dir_all(&teams_dir).unwrap();
    fs::create_dir_all(&tasks_dir).unwrap();

    let config_json = fs::read_to_string("tests/fixtures/team_config.json").unwrap();
    fs::write(tmp.path().join("teams/test-team/config.json"), &config_json).unwrap();

    let (tx, mut rx) = mpsc::channel(32);
    let watcher_path = tmp.path().to_path_buf();
    let handle = tokio::spawn(async move {
        file_watcher::run(watcher_path, tx).await;
    });

    // Drain initial scan events
    while let Ok(Some(_)) = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {}

    // Give the watcher time to fully register on macOS (FSEvents latency)
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Now create a new task file
    let task_json = fs::read_to_string("tests/fixtures/task_2.json").unwrap();
    fs::write(tasks_dir.join("2.json"), &task_json).unwrap();

    // Should receive TaskUpdate (allow extra time for macOS FSEvents + 500ms debounce)
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(5), rx.recv()
    ).await.unwrap().unwrap();
    assert!(matches!(event, Event::TaskUpdate { .. }));

    handle.abort();
}
