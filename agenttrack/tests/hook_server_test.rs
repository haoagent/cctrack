use agenttrack::collector::hook_server;
use agenttrack::store::event::Event;
use tokio::sync::mpsc;

#[tokio::test]
async fn accepts_valid_hook_payload() {
    let (tx, mut rx) = mpsc::channel::<Event>(16);

    // Start server on a high port to avoid conflicts
    let port = hook_server::run(18900, tx).await;
    assert!(port >= 18900 && port <= 18909);

    // POST a valid payload
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/hook", port))
        .json(&serde_json::json!({
            "session_id": "sess-abc",
            "tool_name": "Read",
            "input": {"file_path": "/home/user/src/main.rs"},
            "output": {"content": "fn main() {}"},
            "duration_ms": 42
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    // Verify Event::ToolCall was received
    let event = rx.recv().await.expect("expected an event");
    match event {
        Event::ToolCall(te) => {
            assert_eq!(te.agent_name, "sess-abc");
            assert_eq!(te.tool_name, "Read");
            assert_eq!(te.duration_ms, Some(42));
        }
        other => panic!("expected ToolCall, got {:?}", other),
    }
}

#[tokio::test]
async fn handles_unknown_fields_gracefully() {
    let (tx, mut rx) = mpsc::channel::<Event>(16);

    let port = hook_server::run(18910, tx).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/hook", port))
        .json(&serde_json::json!({
            "session_id": "sess-xyz",
            "tool_name": "Bash",
            "input": {"command": "ls -la"},
            "output": {},
            "duration_ms": 10,
            "unknown_field_1": "hello",
            "unknown_field_2": 42,
            "nested_unknown": {"a": "b"}
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let event = rx.recv().await.expect("expected an event");
    match event {
        Event::ToolCall(te) => {
            assert_eq!(te.agent_name, "sess-xyz");
            assert_eq!(te.tool_name, "Bash");
        }
        other => panic!("expected ToolCall, got {:?}", other),
    }
}

#[tokio::test]
async fn handles_empty_payload() {
    let (tx, mut rx) = mpsc::channel::<Event>(16);

    let port = hook_server::run(18920, tx).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/hook", port))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let event = rx.recv().await.expect("expected an event");
    match event {
        Event::ToolCall(te) => {
            // All fields should have defaults
            assert_eq!(te.agent_name, "");
            assert_eq!(te.tool_name, "");
            assert_eq!(te.duration_ms, None); // 0 maps to None
        }
        other => panic!("expected ToolCall, got {:?}", other),
    }
}
