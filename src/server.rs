use axum::{
    response::{sse::{Event, KeepAlive, Sse}, Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, sync::atomic::{AtomicBool, Ordering}};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use std::net::SocketAddr;
use solana_vanity::{grind, validate_prefix, get_difficulty_estimate, GrinderEvent};

const HTML: &str = include_str!("dashboard.html");

#[derive(Clone)]
struct AppState {
    // Shared state if needed
}

#[derive(Deserialize)]
struct ValidateRequest {
    prefix: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    valid: bool,
    suggestion: String,
    difficulty: String,
    estimated_time: String,
}

#[derive(Deserialize)]
struct GrindRequest {
    prefix: String,
    count: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {};

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/validate", post(validate))
        .route("/api/grind", post(grind_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(HTML)
}

async fn validate(Json(payload): Json<ValidateRequest>) -> Json<ValidateResponse> {
    match validate_prefix(&payload.prefix) {
        Ok(_) => {
            let (time_est, difficulty_label) = get_difficulty_estimate(payload.prefix.len());
            Json(ValidateResponse {
                valid: true,
                suggestion: "".to_string(),
                difficulty: difficulty_label,
                estimated_time: time_est,
            })
        }
        Err(e) => {
            Json(ValidateResponse {
                valid: false,
                suggestion: e,
                difficulty: "".to_string(),
                estimated_time: "".to_string(),
            })
        }
    }
}

async fn grind_handler(
    Json(payload): Json<GrindRequest>,
) -> axum::response::Response {
    let (tx_sse, rx_sse) = mpsc::channel::<String>(512);

    let prefix = payload.prefix;
    let count = payload.count.max(1);

    if let Err(e) = validate_prefix(&prefix) {
        let err_json = serde_json::json!({
            "type": "error",
            "message": e
        });
        let _ = tx_sse.send(format!("data: {}\n\n", err_json)).await;
        return Sse::new(ReceiverStream::new(rx_sse)
            .map(|s| Ok::<_, Infallible>(Event::default().data(s.trim_start_matches("data: ").trim_end()))))
            .keep_alive(KeepAlive::default())
            .into_response();
    }

    let abort_signal = Arc::new(AtomicBool::new(false));
    let (tx_grind, rx_grind) = std::sync::mpsc::channel();

    // Start grinder in blocking task
    let abort_clone = abort_signal.clone();
    let prefix_clone = prefix.clone();
    tokio::task::spawn_blocking(move || {
        grind(prefix_clone, count, tx_grind, abort_clone);
    });

    // We process the grinder events asynchronously and send them via channel
    tokio::spawn(async move {
        // Since rx_grind is synchronous and blocking, we cannot just block here in the async task.
        // Wait, rx_grind.recv() would block tokio runtime. We should run a loop in spawn_blocking that sends to tx_sse via blocking_send.
        // Let's spawn another blocking thread to read from rx_grind and push to tx_sse.

        let _ = tokio::task::spawn_blocking(move || {
            for event in rx_grind {
                let json_str = match event {
                    GrinderEvent::Progress(p) => {
                        serde_json::json!({
                            "type": "progress",
                            "attempts": p.attempts,
                            "rate": p.rate as u64,
                            "elapsed": p.elapsed,
                            "found": p.found,
                            "needed": p.needed
                        }).to_string()
                    }
                    GrinderEvent::Match(m) => {
                        serde_json::json!({
                            "type": "match",
                            "public_key": m.public_key,
                            "private_key": m.private_key,
                            "attempts": m.attempts,
                            "count": count
                        }).to_string()
                    }
                    GrinderEvent::Done { total_attempts, elapsed, found } => {
                        serde_json::json!({
                            "type": "done",
                            "total_attempts": total_attempts,
                            "elapsed": elapsed,
                            "found": found
                        }).to_string()
                    }
                };

                if tx_sse.blocking_send(json_str).is_err() {
                    // client disconnected
                    abort_signal.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }).await;
    });

    let stream = ReceiverStream::new(rx_sse).map(|s| {
        Ok::<_, Infallible>(Event::default().data(s))
    });

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
