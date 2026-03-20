use std::fs;
use std::path::Path;

/// Install the cctrack PostToolUse hook into Claude Code's settings.json.
///
/// 1. Read `claude_home/settings.json` (create `{}` if missing)
/// 2. Backup to `claude_home/settings.json.cctrack-backup`
/// 3. Parse as serde_json::Value
/// 4. Navigate to hooks.PostToolUse (create path if missing)
/// 5. Check if our entry already exists (command contains our port or "cctrack")
/// 6. If not present, append our entry
/// 7. Pretty-print and write back
pub fn install_hooks(claude_home: &Path, hook_port: u16) -> Result<(), String> {
    let settings_path = claude_home.join("settings.json");

    // Read existing or create empty
    let contents = if settings_path.exists() {
        fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings.json: {}", e))?
    } else {
        // Ensure parent dir exists
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        "{}".to_string()
    };

    // Backup
    let backup_path = claude_home.join("settings.json.cctrack-backup");
    fs::write(&backup_path, &contents)
        .map_err(|e| format!("Failed to write backup: {}", e))?;

    // Parse
    let mut root: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse settings.json: {}", e))?;

    // Ensure root is an object
    if !root.is_object() {
        return Err("settings.json root is not an object".to_string());
    }

    // Navigate to hooks.PostToolUse, creating path if missing
    if root.get("hooks").is_none() {
        root["hooks"] = serde_json::json!({});
    }
    if root["hooks"].get("PostToolUse").is_none() {
        root["hooks"]["PostToolUse"] = serde_json::json!([]);
    }

    let post_tool_use = root["hooks"]["PostToolUse"]
        .as_array_mut()
        .ok_or("hooks.PostToolUse is not an array")?;

    // Check if our entry already exists (handles both flat and nested structures)
    let our_marker = format!("localhost:{}/hook", hook_port);
    let already_installed = post_tool_use.iter().any(|entry| {
        // Check flat structure: {"command": "curl ..."}
        if let Some(cmd) = entry.get("command").and_then(|v| v.as_str()) {
            if cmd.contains(&our_marker) || cmd.contains("cctrack") {
                return true;
            }
        }
        // Check nested structure: {"matcher": "*", "hooks": [{"command": "curl ..."}]}
        if let Some(hooks) = entry.get("hooks").and_then(|v| v.as_array()) {
            for hook in hooks {
                if let Some(cmd) = hook.get("command").and_then(|v| v.as_str()) {
                    if cmd.contains(&our_marker) || cmd.contains("cctrack") {
                        return true;
                    }
                }
            }
        }
        false
    });

    if !already_installed {
        // Claude Code pipes hook event JSON to stdin.
        // We use `curl -d @-` to read from stdin and POST to our hook server.
        let hook_command = format!(
            "curl -s -X POST http://localhost:{}/hook -H 'Content-Type: application/json' -d @-",
            hook_port
        );
        let entry = serde_json::json!({
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": hook_command
            }]
        });
        post_tool_use.push(entry);
    }

    // Pretty-print and write back
    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize settings.json: {}", e))?;
    fs::write(&settings_path, output)
        .map_err(|e| format!("Failed to write settings.json: {}", e))?;

    Ok(())
}

/// Remove cctrack PostToolUse hooks from Claude Code's settings.json.
///
/// Removes entries whose command contains "localhost:78XX/hook" (ports 7890-7899).
pub fn uninstall_hooks(claude_home: &Path) -> Result<(), String> {
    let settings_path = claude_home.join("settings.json");

    if !settings_path.exists() {
        return Ok(()); // Nothing to uninstall
    }

    let contents = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings.json: {}", e))?;

    let mut root: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse settings.json: {}", e))?;

    // Navigate to hooks.PostToolUse
    if let Some(hooks) = root.get_mut("hooks") {
        if let Some(post_tool_use) = hooks.get_mut("PostToolUse") {
            if let Some(arr) = post_tool_use.as_array_mut() {
                arr.retain(|entry| {
                    let check_cmd = |cmd: &str| -> bool {
                        (7890..=7899).any(|port| {
                            cmd.contains(&format!("localhost:{}/hook", port))
                        })
                    };
                    // Check flat: {"command": "curl ..."}
                    if let Some(cmd) = entry.get("command").and_then(|v| v.as_str()) {
                        if check_cmd(cmd) { return false; }
                    }
                    // Check nested: {"hooks": [{"command": "curl ..."}]}
                    if let Some(hooks) = entry.get("hooks").and_then(|v| v.as_array()) {
                        for hook in hooks {
                            if let Some(cmd) = hook.get("command").and_then(|v| v.as_str()) {
                                if check_cmd(cmd) { return false; }
                            }
                        }
                    }
                    true
                });
            }
        }
    }

    // Pretty-print and write back
    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize settings.json: {}", e))?;
    fs::write(&settings_path, output)
        .map_err(|e| format!("Failed to write settings.json: {}", e))?;

    Ok(())
}
