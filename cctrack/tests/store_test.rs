use cctrack::store::event::{Event, StoreSnapshot};
use cctrack::store::models::*;
use cctrack::store::state::Store;
use tokio::sync::{mpsc, watch};

/// Helper: spin up the Store event loop, send events, then drop the sender
/// to shut down the loop. Returns the final snapshot.
async fn run_events(events: Vec<Event>) -> StoreSnapshot {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (snapshot_tx, snapshot_rx) = watch::channel(StoreSnapshot::default());

    let handle = tokio::spawn(Store::process_events(event_rx, snapshot_tx));

    for event in events {
        event_tx.send(event).await.expect("send event");
    }

    // Drop the sender so the loop exits
    drop(event_tx);
    handle.await.expect("store loop panicked");

    let snap = snapshot_rx.borrow().clone();
    snap
}

fn make_team_config() -> TeamConfig {
    let json = include_str!("fixtures/team_config.json");
    serde_json::from_str(json).expect("parse team config")
}

/// Find a team by name in the snapshot. teams[0] is always ALL.
fn find_team<'a>(snap: &'a StoreSnapshot, name: &str) -> &'a cctrack::store::event::TeamSnapshot {
    snap.teams.iter().find(|t| t.name == name).expect(&format!("team '{}' not found", name))
}

#[tokio::test]
async fn team_update_creates_agents() {
    let config = make_team_config();

    let snap = run_events(vec![Event::TeamUpdate {
        team_name: "my-project".into(),
        config,
    }])
    .await;

    // teams[0] = ALL, then my-project (may have restored sessions too)
    assert!(snap.teams.len() >= 2);
    assert_eq!(snap.teams[0].name, "all"); // ALL tab always first
    let team = find_team(&snap, "my-project");
    assert_eq!(team.description, "Working on feature X");
    assert_eq!(team.agents.len(), 2);

    // Agents should exist (timeout may mark them Shutdown after loop exits)
    let lead = team.agents.iter().find(|a| a.name == "team-lead").unwrap();
    assert_eq!(lead.agent_type.as_deref(), Some("team-leader"));
    assert!(matches!(lead.status, AgentStatus::Active | AgentStatus::Shutdown));

    let brainstormer = team
        .agents
        .iter()
        .find(|a| a.name == "brainstormer")
        .unwrap();
    assert_eq!(brainstormer.color.as_deref(), Some("blue"));
    assert!(matches!(brainstormer.status, AgentStatus::Active | AgentStatus::Shutdown));
}

#[tokio::test]
async fn task_update_tracks_status() {
    let config = make_team_config();
    let task1: TaskFile =
        serde_json::from_str(include_str!("fixtures/task_1.json")).expect("parse task 1");
    let task2: TaskFile =
        serde_json::from_str(include_str!("fixtures/task_2.json")).expect("parse task 2");

    let snap = run_events(vec![
        Event::TeamUpdate {
            team_name: "my-project".into(),
            config,
        },
        Event::TaskUpdate {
            team_name: "my-project".into(),
            task: task1,
        },
        Event::TaskUpdate {
            team_name: "my-project".into(),
            task: task2,
        },
    ])
    .await;

    let team = find_team(&snap, "my-project");
    assert_eq!(team.tasks.len(), 2);

    let t1 = team.tasks.iter().find(|t| t.id == "1").unwrap();
    assert_eq!(t1.status.as_deref(), Some("completed"));

    let t2 = team.tasks.iter().find(|t| t.id == "2").unwrap();
    assert_eq!(t2.status.as_deref(), Some("in_progress"));
    assert_eq!(t2.blocked_by, vec!["1"]);

    // Metrics should reflect task statuses
    assert_eq!(team.metrics.total_tasks, 2);
    assert_eq!(team.metrics.completed_tasks, 1);
    assert_eq!(team.metrics.in_progress_tasks, 1);
}

#[tokio::test]
async fn message_update_derives_to_field() {
    let config = make_team_config();
    let inbox_messages: Vec<InboxMessage> =
        serde_json::from_str(include_str!("fixtures/inbox_brainstormer.json"))
            .expect("parse inbox");

    let snap = run_events(vec![
        Event::TeamUpdate {
            team_name: "my-project".into(),
            config,
        },
        Event::MessageUpdate {
            team_name: "my-project".into(),
            agent_name: "brainstormer".into(),
            messages: inbox_messages,
        },
    ])
    .await;

    let team = find_team(&snap, "my-project");
    assert_eq!(team.messages.len(), 1);

    let msg = &team.messages[0];
    // The inbox belongs to "brainstormer", so "to" should be "brainstormer"
    assert_eq!(msg.to, "brainstormer");
    assert_eq!(msg.from, "team-lead");
    assert!(msg.text.contains("brainstorming"));
    assert_eq!(msg.msg_type, MessageType::DirectMessage);
    assert!(msg.read);
}

#[tokio::test]
async fn idle_notification_updates_agent_status() {
    let config = make_team_config();
    let inbox_messages: Vec<InboxMessage> =
        serde_json::from_str(include_str!("fixtures/inbox_team_lead.json")).expect("parse inbox");

    let snap = run_events(vec![
        Event::TeamUpdate {
            team_name: "my-project".into(),
            config,
        },
        Event::MessageUpdate {
            team_name: "my-project".into(),
            agent_name: "team-lead".into(),
            messages: inbox_messages,
        },
    ])
    .await;

    let team = find_team(&snap, "my-project");

    // The brainstormer sent an idle_notification, status should be Idle or Shutdown (timeout)
    let brainstormer = team
        .agents
        .iter()
        .find(|a| a.name == "brainstormer")
        .unwrap();
    assert!(matches!(brainstormer.status, AgentStatus::Idle | AgentStatus::Shutdown));

    // The team-lead: Active or Shutdown (timeout)
    let lead = team.agents.iter().find(|a| a.name == "team-lead").unwrap();
    assert!(matches!(lead.status, AgentStatus::Active | AgentStatus::Shutdown));
}

#[tokio::test]
async fn metrics_computed_correctly() {
    let config = make_team_config();
    let task1: TaskFile =
        serde_json::from_str(include_str!("fixtures/task_1.json")).expect("parse task 1");
    let task2: TaskFile =
        serde_json::from_str(include_str!("fixtures/task_2.json")).expect("parse task 2");
    let inbox_messages: Vec<InboxMessage> =
        serde_json::from_str(include_str!("fixtures/inbox_team_lead.json")).expect("parse inbox");

    let snap = run_events(vec![
        Event::TeamUpdate {
            team_name: "my-project".into(),
            config,
        },
        Event::TaskUpdate {
            team_name: "my-project".into(),
            task: task1,
        },
        Event::TaskUpdate {
            team_name: "my-project".into(),
            task: task2,
        },
        Event::MessageUpdate {
            team_name: "my-project".into(),
            agent_name: "team-lead".into(),
            messages: inbox_messages,
        },
    ])
    .await;

    let metrics = &find_team(&snap, "my-project").metrics;
    assert_eq!(metrics.total_agents, 2);
    // idle_agents may be 0 or 1 depending on timeout behavior
    assert!(metrics.idle_agents <= 1);
    assert_eq!(metrics.total_tasks, 2);
    assert_eq!(metrics.completed_tasks, 1);
    assert_eq!(metrics.in_progress_tasks, 1);
    // task_2 is blockedBy ["1"] and is in_progress (not completed) => blocked
    assert_eq!(metrics.blocked_tasks, 1);
    // 2 inbox messages: one DM + one idle notification
    // The idle notification has no text so it still creates a Message entry
    assert_eq!(metrics.total_messages, 2);
    assert_eq!(metrics.total_tool_calls, 0);
}
