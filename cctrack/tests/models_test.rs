use cctrack::store::models::*;

#[test]
fn parse_team_config() {
    let json = include_str!("fixtures/team_config.json");
    let config: TeamConfig = serde_json::from_str(json).expect("Failed to parse team config");

    assert_eq!(config.name, "my-project");
    assert_eq!(config.description, "Working on feature X");
    assert_eq!(config.members.len(), 2);

    let lead = &config.members[0];
    assert_eq!(lead.name, "team-lead");
    assert_eq!(lead.agent_id, "team-lead@my-project");
    assert_eq!(lead.agent_type.as_deref(), Some("team-leader"));
    assert_eq!(lead.model.as_deref(), Some("claude-sonnet-4-6"));
    // team-lead has no color field
    assert!(lead.color.is_none());

    let brainstormer = &config.members[1];
    assert_eq!(brainstormer.name, "brainstormer");
    assert_eq!(brainstormer.agent_type.as_deref(), Some("general-purpose"));
    assert_eq!(brainstormer.color.as_deref(), Some("blue"));
    assert_eq!(brainstormer.plan_mode_required, Some(false));
    assert_eq!(brainstormer.backend_type.as_deref(), Some("in-process"));
}

#[test]
fn parse_task_with_dependencies() {
    let json = include_str!("fixtures/task_2.json");
    let task: TaskFile = serde_json::from_str(json).expect("Failed to parse task");

    assert_eq!(task.id, "2");
    assert_eq!(task.subject.as_deref(), Some("spec-reviewer"));
    assert_eq!(task.description.as_deref(), Some("Review the spec document"));
    assert_eq!(task.status.as_deref(), Some("in_progress"));
    assert_eq!(task.blocks, vec!["3", "4"]);
    assert_eq!(task.blocked_by, vec!["1"]);
    assert!(task.metadata.is_some());
    assert_eq!(task.metadata.as_ref().unwrap().internal, Some(true));
}

#[test]
fn parse_inbox_with_mixed_message_types() {
    let json = include_str!("fixtures/inbox_team_lead.json");
    let messages: Vec<InboxMessage> =
        serde_json::from_str(json).expect("Failed to parse inbox messages");

    assert_eq!(messages.len(), 2);

    // First message: regular direct message
    let dm = &messages[0];
    assert_eq!(dm.from.as_deref(), Some("brainstormer"));
    assert!(dm.text.is_some());
    assert_eq!(dm.summary.as_deref(), Some("Brainstorming complete, spec ready"));
    assert_eq!(dm.read, Some(false));
    assert_eq!(dm.color.as_deref(), Some("blue"));
    assert!(dm.msg_type.is_none());
    assert_eq!(dm.classify_type(), MessageType::DirectMessage);

    // Second message: idle notification (embedded JSON in text field, matching real format)
    let idle = &messages[1];
    assert_eq!(idle.from.as_deref(), Some("brainstormer"));
    // In real Claude Code, idle_notification type is embedded inside the text field as JSON
    assert!(idle.text.as_ref().unwrap().contains("idle_notification"));
    assert_eq!(idle.classify_type(), MessageType::IdleNotification);
}

#[test]
fn parse_task_without_optional_fields() {
    // Minimal task JSON with only required field
    let json = r#"{"id": "99"}"#;
    let task: TaskFile = serde_json::from_str(json).expect("Failed to parse minimal task");

    assert_eq!(task.id, "99");
    assert!(task.subject.is_none());
    assert!(task.description.is_none());
    assert!(task.status.is_none());
    assert!(task.blocks.is_empty());
    assert!(task.blocked_by.is_empty());
    assert!(task.metadata.is_none());
}

#[test]
fn parse_member_without_optional_fields() {
    // Minimal member JSON with only required fields
    let json = r#"{
        "agentId": "test-agent@team",
        "name": "test-agent"
    }"#;
    let member: MemberConfig = serde_json::from_str(json).expect("Failed to parse minimal member");

    assert_eq!(member.name, "test-agent");
    assert_eq!(member.agent_id, "test-agent@team");
    assert!(member.agent_type.is_none());
    assert!(member.model.is_none());
    assert!(member.color.is_none());
    assert!(member.plan_mode_required.is_none());
    assert!(member.joined_at.is_none());
    assert!(member.tmux_pane_id.is_none());
    assert!(member.cwd.is_none());
    assert!(member.subscriptions.is_empty());
    assert!(member.backend_type.is_none());
}
