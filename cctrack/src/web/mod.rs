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
