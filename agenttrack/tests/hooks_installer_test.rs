use agenttrack::collector::hooks_installer::{install_hooks, uninstall_hooks};
use std::fs;
use tempfile::TempDir;

#[test]
fn install_into_empty_settings() {
    let tmp = TempDir::new().unwrap();
    let claude_home = tmp.path();

    // No settings.json exists
    install_hooks(claude_home, 7890).expect("install should succeed");

    // Verify settings.json was created
    let settings_path = claude_home.join("settings.json");
    assert!(settings_path.exists(), "settings.json should be created");

    let contents = fs::read_to_string(&settings_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Verify hooks.PostToolUse has our entry
    let post_tool_use = &parsed["hooks"]["PostToolUse"];
    assert!(post_tool_use.is_array());
    let arr = post_tool_use.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let cmd = arr[0]["command"].as_str().unwrap();
    assert!(cmd.contains("localhost:7890/hook"));
}

#[test]
fn install_preserves_existing_hooks() {
    let tmp = TempDir::new().unwrap();
    let claude_home = tmp.path();

    // Write settings with a PreToolUse hook
    let existing = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "type": "command",
                    "command": "echo pre-tool"
                }
            ]
        },
        "someOtherSetting": true
    });
    fs::write(
        claude_home.join("settings.json"),
        serde_json::to_string_pretty(&existing).unwrap(),
    )
    .unwrap();

    install_hooks(claude_home, 7891).expect("install should succeed");

    let contents = fs::read_to_string(claude_home.join("settings.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // PreToolUse should still be there
    let pre_tool_use = &parsed["hooks"]["PreToolUse"];
    assert!(pre_tool_use.is_array());
    assert_eq!(pre_tool_use.as_array().unwrap().len(), 1);
    assert_eq!(
        pre_tool_use[0]["command"].as_str().unwrap(),
        "echo pre-tool"
    );

    // PostToolUse should have our entry
    let post_tool_use = &parsed["hooks"]["PostToolUse"];
    assert!(post_tool_use.is_array());
    let arr = post_tool_use.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0]["command"]
        .as_str()
        .unwrap()
        .contains("localhost:7891/hook"));

    // Other settings preserved
    assert_eq!(parsed["someOtherSetting"], true);
}

#[test]
fn install_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let claude_home = tmp.path();

    // Install twice
    install_hooks(claude_home, 7892).expect("first install should succeed");
    install_hooks(claude_home, 7892).expect("second install should succeed");

    let contents = fs::read_to_string(claude_home.join("settings.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    let post_tool_use = &parsed["hooks"]["PostToolUse"];
    let arr = post_tool_use.as_array().unwrap();

    // Should have exactly 1 entry, not 2
    assert_eq!(arr.len(), 1, "install should be idempotent");
}

#[test]
fn install_creates_backup() {
    let tmp = TempDir::new().unwrap();
    let claude_home = tmp.path();

    // Write initial settings
    let original = r#"{"existing": true}"#;
    fs::write(claude_home.join("settings.json"), original).unwrap();

    install_hooks(claude_home, 7893).expect("install should succeed");

    // Verify backup file was created with original content
    let backup_path = claude_home.join("settings.json.agenttrack-backup");
    assert!(backup_path.exists(), "backup file should be created");

    let backup_contents = fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup_contents, original);
}

#[test]
fn uninstall_removes_hook() {
    let tmp = TempDir::new().unwrap();
    let claude_home = tmp.path();

    // Install first
    install_hooks(claude_home, 7894).expect("install should succeed");

    // Verify it's there
    let contents = fs::read_to_string(claude_home.join("settings.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(
        parsed["hooks"]["PostToolUse"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Uninstall
    uninstall_hooks(claude_home).expect("uninstall should succeed");

    // Verify it's gone
    let contents = fs::read_to_string(claude_home.join("settings.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    let post_tool_use = &parsed["hooks"]["PostToolUse"];
    let arr = post_tool_use.as_array().unwrap();
    assert_eq!(arr.len(), 0, "hook should be removed after uninstall");
}
