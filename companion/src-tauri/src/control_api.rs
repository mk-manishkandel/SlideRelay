//! The localhost API the PowerPoint add-in talks to.
//!
//! Listens ONLY on 127.0.0.1 (never the network). Defence layers:
//! 1. Host header must be 127.0.0.1/localhost (blocks DNS-rebinding attacks).
//! 2. Browser requests must come from the add-in's own origin (CORS).
//! 3. Everything except `/v1/hello` and pairing needs a token the teacher
//!    approved in the app ("pairing").
//!
//! The JSON shapes mirror `shared/protocol.ts`. See that file for the
//! versioning rules.

use crate::pairing::PairError;
use crate::state::{AppState, Command};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// API version this companion speaks (see `shared/protocol.ts`).
pub const API_VERSION: u32 = 1;
/// Oldest add-in API version we still accept.
pub const MIN_CLIENT_API: u32 = 1;
/// Fixed loopback port; the add-in has it hard-coded.
pub const CONTROL_PORT: u16 = 47615;

/// Web origins allowed to call this API from a browser/webview.
pub const ALLOWED_ORIGINS: &[&str] = &[
    "https://mk-manishkandel.github.io", // production add-in (GitHub Pages)
    "https://localhost:3000",            // add-in dev server
];

#[derive(Clone)]
struct Ctx {
    state: Arc<AppState>,
    /// Ports we serve on; used for the Host-header check.
    ports: Arc<Vec<u16>>,
}

/// Builds the API. `ports` are the ports it is served on (Host-header check).
pub fn router(state: Arc<AppState>, ports: &[u16]) -> Router {
    let ctx = Ctx {
        state,
        ports: Arc::new(ports.to_vec()),
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            ALLOWED_ORIGINS.iter().map(|o| HeaderValue::from_static(o)),
        ))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        // Chromium's "Private Network Access": a public https page calling
        // 127.0.0.1 sends a preflight that must be answered with this.
        .allow_private_network(true)
        .max_age(Duration::from_secs(600));

    Router::new()
        .route("/v1/hello", get(hello))
        .route("/v1/pair", post(pair_start))
        .route("/v1/pair/{id}", get(pair_poll))
        .route("/v1/status", get(status))
        .route("/v1/broadcast/{command}", post(command))
        .route("/v1/events", get(events))
        .layer(middleware::from_fn_with_state(ctx.clone(), guard))
        .layer(cors)
        .with_state(ctx)
}

/// Rejects requests with a foreign Host header or a foreign Origin.
async fn guard(State(ctx): State<Ctx>, req: Request, next: Next) -> Response {
    tracing::debug!(
        "control API {} {} origin={:?}",
        req.method(),
        req.uri().path(),
        req.headers().get(header::ORIGIN)
    );
    // HTTP/1.1 sends a Host header; HTTP/2 (used over HTTPS) sends the
    // host as the request URI's authority instead.
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned)
        .or_else(|| req.uri().authority().map(|a| a.to_string()));
    let host_ok = host
        .map(|h| {
            ctx.ports
                .iter()
                .any(|p| h == format!("127.0.0.1:{p}") || h == format!("localhost:{p}"))
        })
        .unwrap_or(false);
    if !host_ok {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    }
    // Requests without an Origin come from non-browser software; they still
    // need a valid token for anything that matters.
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        let ok = origin
            .to_str()
            .map(|o| ALLOWED_ORIGINS.contains(&o))
            .unwrap_or(false);
        if !ok {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    next.run(req).await
}

fn authorized(ctx: &Ctx, headers: &HeaderMap) -> bool {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match token {
        Some(t) => ctx
            .state
            .pairing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .verify(t),
        None => false,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Hello {
    app: &'static str,
    version: &'static str,
    api_version: u32,
    min_client_api: u32,
}

async fn hello() -> Json<Hello> {
    Json(Hello {
        app: "SlideRelay",
        version: env!("CARGO_PKG_VERSION"),
        api_version: API_VERSION,
        min_client_api: MIN_CLIENT_API,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairStart {
    request_id: String,
}

async fn pair_start(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    // Only the add-in itself (a browser origin we know) may ask to pair.
    if headers.get(header::ORIGIN).is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = ctx
        .state
        .pairing
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .request();
    match result {
        Ok(id) => {
            ctx.state.ui().show_pairing_prompt(&id);
            Json(PairStart { request_id: id }).into_response()
        }
        Err(PairError::TooManyRequests) => StatusCode::TOO_MANY_REQUESTS.into_response(),
        Err(PairError::UnknownRequest) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn pair_poll(State(ctx): State<Ctx>, Path(id): Path<String>) -> Response {
    let poll = ctx
        .state
        .pairing
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .poll(&id);
    Json(poll).into_response()
}

async fn status(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if !authorized(&ctx, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(ctx.state.status()).into_response()
}

async fn command(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Path(command): Path<String>,
) -> Response {
    if !authorized(&ctx, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(cmd) = Command::parse(&command) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match ctx.state.run(cmd) {
        Ok(status) => Json(status).into_response(),
        Err(_) => (StatusCode::CONFLICT, Json(ctx.state.status())).into_response(),
    }
}

/// Live status feed. Browsers can't add headers to WebSockets, so the first
/// message must be `{"type":"auth","token":"..."}`.
async fn events(State(ctx): State<Ctx>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(4096)
        .on_upgrade(move |socket| events_loop(ctx, socket))
}

async fn events_loop(ctx: Ctx, mut socket: WebSocket) {
    // 1. Authenticate within 5 seconds.
    let first = tokio::time::timeout(Duration::from_secs(5), socket.recv()).await;
    let token = match first {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .filter(|v| v["type"] == "auth")
            .and_then(|v| v["token"].as_str().map(str::to_owned)),
        _ => None,
    };
    let ok = token
        .map(|t| {
            ctx.state
                .pairing
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .verify(&t)
        })
        .unwrap_or(false);
    if !ok {
        let _ = socket
            .send(Message::Text(
                r#"{"type":"error","message":"unauthorized"}"#.into(),
            ))
            .await;
        return;
    }

    // 2. Send the current status, then every change.
    let mut rx = ctx.state.subscribe();
    loop {
        let msg = serde_json::json!({ "type": "status", "status": *rx.borrow_and_update() });
        if socket
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
        tokio::select! {
            changed = rx.changed() => if changed.is_err() { return },
            incoming = socket.recv() => match incoming {
                // The add-in never needs to send more; any close/error ends it.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => return,
            },
        }
    }
}

/// Starts the API on 127.0.0.1:47615. Fails if another program holds the
/// port (usually a second SlideRelay — single-instance normally prevents it).
pub async fn serve(app: Router) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", CONTROL_PORT)).await?;
    tracing::info!("control API listening on 127.0.0.1:{CONTROL_PORT}");
    axum::serve(listener, app).await?;
    Ok(())
}
