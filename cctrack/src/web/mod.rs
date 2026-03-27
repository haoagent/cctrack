use axum::{
    extract::State,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::get,
    Router,
};
use rust_embed::Embed;
use serde::Serialize;
use std::convert::Infallible;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;

use crate::config::Config;
use crate::stats;
use crate::store::event::StoreSnapshot;

#[derive(Embed)]
#[folder = "src/web/static/"]
struct StaticAssets;

type SharedState = watch::Receiver<StoreSnapshot>;

pub async fn run(port: u16, snapshot_rx: watch::Receiver<StoreSnapshot>) {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/sse", get(sse_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/cap", get(cap_handler))
        .route("/static/{*path}", get(static_handler))
        .with_state(snapshot_rx);

    // Try ports port..port+9
    for p in port..port + 10 {
        let addr = format!("0.0.0.0:{p}");
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                tracing::info!("Web UI at http://localhost:{p}");
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("Web server error: {e}");
                }
                return;
            }
            Err(_) => {
                tracing::warn!("Port {p} in use, trying next...");
                continue;
            }
        }
    }
    tracing::error!("Could not bind to any port in range {port}-{}", port + 9);
}

async fn index_handler() -> Html<String> {
    match StaticAssets::get("index.html") {
        Some(content) => Html(
            String::from_utf8_lossy(content.data.as_ref()).to_string(),
        ),
        None => Html("<h1>index.html not found</h1>".to_string()),
    }
}

async fn static_handler(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

async fn stats_handler() -> axum::Json<stats::StatsReport> {
    let claude_home = Config::claude_home();
    let report = stats::compute_stats(&claude_home);
    axum::Json(report)
}

async fn sse_handler(
    State(snapshot_rx): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = WatchStream::new(snapshot_rx);

    // Throttle to max 2 events/second
    let throttled = stream.throttle(std::time::Duration::from_millis(500));

    let event_stream = throttled.map(|snapshot| {
        let json = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
        Ok(SseEvent::default().data(json))
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

// ─── OAuth Cap ───

#[derive(Serialize, Default)]
struct CapResponse {
    ok: bool,
    plan: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<CapWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weekly: Option<CapWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct CapWindow {
    used_pct: f64,
    resets_at: String,
}

async fn cap_handler() -> axum::Json<CapResponse> {
    match fetch_cap().await {
        Ok(resp) => axum::Json(resp),
        Err(e) => axum::Json(CapResponse {
            ok: false,
            error: Some(e.to_string()),
            ..Default::default()
        }),
    }
}

async fn fetch_cap() -> anyhow::Result<CapResponse> {
    // Read OAuth token from macOS Keychain
    let output = tokio::process::Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("No Claude Code credentials in keychain. Login to Claude Code first.");
    }

    let creds_json = String::from_utf8_lossy(&output.stdout);
    let mut creds: serde_json::Value = serde_json::from_str(creds_json.trim())?;

    let oauth = creds
        .get("claudeAiOauth")
        .ok_or_else(|| anyhow::anyhow!("No claudeAiOauth in credentials"))?
        .clone();

    let mut token = oauth
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No accessToken"))?
        .to_string();

    let plan = oauth
        .get("rateLimitTier")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Check if token expired, refresh if needed
    let expires_at = oauth
        .get("expiresAt")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    if expires_at > 0 && now_ms > expires_at {
        // Token expired — try to refresh
        if let Some(refresh_token) = oauth.get("refreshToken").and_then(|v| v.as_str()) {
            let client = reqwest::Client::new();
            let resp = client
                .post("https://console.anthropic.com/v1/oauth/token")
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token),
                ])
                .send()
                .await?;

            if resp.status().is_success() {
                let refresh_data: serde_json::Value = resp.json().await?;
                if let Some(new_token) = refresh_data.get("access_token").and_then(|v| v.as_str())
                {
                    token = new_token.to_string();

                    // Update keychain with new token
                    let new_expires = refresh_data
                        .get("expires_in")
                        .and_then(|v| v.as_u64())
                        .map(|s| now_ms + s * 1000)
                        .unwrap_or(now_ms + 3600000);

                    if let Some(oauth_obj) = creds.get_mut("claudeAiOauth") {
                        oauth_obj["accessToken"] =
                            serde_json::Value::String(new_token.to_string());
                        oauth_obj["expiresAt"] =
                            serde_json::Value::Number(new_expires.into());
                    }

                    // Write back to keychain
                    let updated = serde_json::to_string(&creds)?;
                    let whoami = tokio::process::Command::new("whoami")
                        .output()
                        .await?;
                    let user = String::from_utf8_lossy(&whoami.stdout).trim().to_string();
                    let _ = tokio::process::Command::new("security")
                        .args([
                            "add-generic-password",
                            "-U",
                            "-s",
                            "Claude Code-credentials",
                            "-a",
                            &user,
                            "-w",
                            &updated,
                        ])
                        .output()
                        .await;
                }
            } else {
                anyhow::bail!(
                    "Token expired. Refresh failed ({}). Re-login to Claude Code.",
                    resp.status()
                );
            }
        } else {
            anyhow::bail!("Token expired and no refresh token available. Re-login to Claude Code.");
        }
    }

    // Call Anthropic usage API
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Usage API returned {}", resp.status());
    }

    let data: serde_json::Value = resp.json().await?;

    // Parse response — try multiple field name patterns
    let session = data.get("five_hour").or_else(|| data.get("current_session")).map(|w| CapWindow {
        used_pct: w.get("utilization").or_else(|| w.get("used_percentage"))
            .and_then(|v| v.as_f64()).unwrap_or(0.0)
            * if w.get("utilization").is_some() { 100.0 } else { 1.0 },
        resets_at: w.get("resets_at").or_else(|| w.get("reset_time"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string(),
    });

    let weekly = data.get("seven_day").or_else(|| data.get("weekly_limits")).map(|w| CapWindow {
        used_pct: w.get("utilization").or_else(|| w.get("used_percentage"))
            .and_then(|v| v.as_f64()).unwrap_or(0.0)
            * if w.get("utilization").is_some() { 100.0 } else { 1.0 },
        resets_at: w.get("resets_at").or_else(|| w.get("reset_time"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string(),
    });

    Ok(CapResponse {
        ok: true,
        plan,
        session,
        weekly,
        error: None,
    })
}
