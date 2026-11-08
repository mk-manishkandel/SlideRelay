//! The web server students connect to on the local network.
//!
//! ```text
//! GET /<code>       the viewer page (also any other path: the page itself
//!                   explains a wrong or old link)
//! GET /assets/...   the viewer's script and styles (built into the app)
//! GET /ws/<code>    WebSocket: JSON state messages + binary JPEG frames
//! ```
//!
//! Viewers are strictly read-only: the server ignores everything they send
//! except a PIN answer. Nothing is recorded or stored.

use crate::state::AppState;
use crate::stream::ViewerMode;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The built viewer (`viewer/dist`), embedded in the app binary.
#[derive(rust_embed::RustEmbed)]
#[folder = "../../viewer/dist"]
struct ViewerFiles;

/// Protects the computer if something floods it with connections.
const MAX_VIEWERS: usize = 400;
const MAX_PIN_TRIES: u32 = 5;
/// A viewer that can't take a frame within this time is disconnected
/// (it will reconnect by itself).
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct Ctx {
    state: Arc<AppState>,
    viewers: Arc<AtomicUsize>,
}

pub fn router(state: Arc<AppState>) -> Router {
    let ctx = Ctx {
        state,
        viewers: Arc::new(AtomicUsize::new(0)),
    };
    Router::new()
        .route("/ws/{code}", get(ws))
        .route("/assets/{*file}", get(asset))
        .route("/favicon.svg", get(favicon))
        .fallback(get(page))
        .with_state(ctx)
}

/// Tries port 80 (shortest link), then common alternatives, then any free
/// port. A port set in Settings is tried first.
pub async fn bind(preferred: u16) -> std::io::Result<tokio::net::TcpListener> {
    let mut ports = vec![];
    if preferred != 0 {
        ports.push(preferred);
    }
    ports.extend([80, 8080, 8000, 8888, 0]);
    let mut last_err = None;
    for port in ports {
        match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("no free port")))
}

pub async fn serve(listener: tokio::net::TcpListener, state: Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("student server listening on {}", listener.local_addr()?);
    axum::serve(listener, router(state)).await?;
    Ok(())
}

// ---------- Static files ----------

fn with_security_headers(mut resp: Response, cache: &'static str) -> Response {
    let h = resp.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' blob: data:; connect-src 'self'; style-src 'self'; script-src 'self'; frame-ancestors 'none'",
        ),
    );
    resp
}

fn embedded(path: &str, cache: &'static str) -> Response {
    match ViewerFiles::get(path) {
        Some(file) => {
            let mime = file.metadata.mimetype().to_owned();
            let resp = Response::builder()
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(file.data.into_owned()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            with_security_headers(resp, cache)
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn page() -> Response {
    embedded("index.html", "no-cache")
}

async fn asset(Path(file): Path<String>) -> Response {
    // Vite gives assets content-hashed names, so they can be cached forever.
    embedded(
        &format!("assets/{file}"),
        "public, max-age=31536000, immutable",
    )
}

async fn favicon() -> Response {
    embedded("favicon.svg", "public, max-age=86400")
}

// ---------- Live stream ----------

async fn ws(State(ctx): State<Ctx>, Path(code): Path<String>, ws: WebSocketUpgrade) -> Response {
    if ctx.viewers.load(Ordering::Relaxed) >= MAX_VIEWERS {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    ws.max_message_size(1024)
        .on_upgrade(move |socket| viewer(ctx, code, socket))
}

fn state_msg(mode: ViewerMode) -> Message {
    Message::Text(
        serde_json::json!({ "type": "state", "mode": mode })
            .to_string()
            .into(),
    )
}

fn code_ok(ctx: &Ctx, code: &str) -> bool {
    ctx.state
        .session
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .code_matches(code)
}

/// Sends "ended" and closes the connection properly. Just dropping the
/// socket makes Windows reset it, which can discard the "ended" message
/// before the phone reads it (it would then show "Reconnecting…" forever).
async fn end(socket: &mut WebSocket) {
    if !send(socket, state_msg(ViewerMode::Ended)).await {
        return;
    }
    let _ = send(socket, Message::Close(None)).await;
    // Wait (briefly) for the browser's close reply.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(Ok(msg)) = socket.recv().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    })
    .await;
}

async fn send(socket: &mut WebSocket, msg: Message) -> bool {
    matches!(
        tokio::time::timeout(SEND_TIMEOUT, socket.send(msg)).await,
        Ok(Ok(()))
    )
}

/// Asks for the PIN if the teacher set one. Returns false to disconnect.
async fn check_pin(ctx: &Ctx, socket: &mut WebSocket) -> bool {
    let pin = ctx
        .state
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .pin
        .clone()
        .filter(|p| !p.is_empty());
    let Some(pin) = pin else { return true };

    if !send(socket, Message::Text(r#"{"type":"pin"}"#.into())).await {
        return false;
    }
    for _ in 0..MAX_PIN_TRIES {
        let msg = tokio::time::timeout(Duration::from_secs(300), socket.recv()).await;
        let Ok(Some(Ok(Message::Text(text)))) = msg else {
            return false;
        };
        let answer = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["pin"].as_str().map(str::to_owned))
            .unwrap_or_default();
        if answer.trim() == pin {
            return true;
        }
        // Slow down guessing.
        tokio::time::sleep(Duration::from_millis(800)).await;
        if !send(socket, Message::Text(r#"{"type":"pin-wrong"}"#.into())).await {
            return false;
        }
    }
    false
}

async fn viewer(ctx: Ctx, code: String, mut socket: WebSocket) {
    // Old or wrong link: tell the page the broadcast has ended.
    if !code_ok(&ctx, &code) {
        end(&mut socket).await;
        return;
    }
    if !check_pin(&ctx, &mut socket).await {
        return;
    }

    let count = ctx.viewers.fetch_add(1, Ordering::Relaxed) + 1;
    ctx.state.update(|s| s.set_viewers(count));

    stream_to(&ctx, &code, &mut socket).await;

    let count = ctx.viewers.fetch_sub(1, Ordering::Relaxed) - 1;
    ctx.state.update(|s| s.set_viewers(count));
}

async fn stream_to(ctx: &Ctx, code: &str, socket: &mut WebSocket) {
    let (mut frames, mut modes) = ctx.state.hub.subscribe();

    // Current state, then the current slide (if live).
    let mode = *modes.borrow_and_update();
    if !send(socket, state_msg(mode)).await {
        return;
    }
    let first = frames.borrow_and_update().clone();
    if let (ViewerMode::Live | ViewerMode::Paused, Some(f)) = (mode, first) {
        if !send(socket, Message::Binary(f.jpeg.clone())).await {
            return;
        }
    }

    let mut ping = tokio::time::interval(Duration::from_secs(20));
    loop {
        tokio::select! {
            changed = modes.changed() => {
                if changed.is_err() { return; }
                let mode = *modes.borrow_and_update();
                // A quick Stop→Start makes a new code: old links end here.
                let mode = if code_ok(ctx, code) { mode } else { ViewerMode::Ended };
                if mode == ViewerMode::Ended {
                    end(socket).await;
                    return;
                }
                if !send(socket, state_msg(mode)).await {
                    return;
                }
            }
            changed = frames.changed() => {
                if changed.is_err() { return; }
                let frame = frames.borrow_and_update().clone();
                if let Some(f) = frame {
                    if !send(socket, Message::Binary(f.jpeg.clone())).await { return; }
                }
            }
            _ = ping.tick() => {
                if !send(socket, Message::Ping(Default::default())).await { return; }
            }
            incoming = socket.recv() => match incoming {
                // Viewers are read-only: ignore anything they send.
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            },
        }
    }
}
