//! Integration tests for the localhost control API (no network needed:
//! requests go straight into the router).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sliderelay_lib::config::Settings;
use sliderelay_lib::control_api::{router, CONTROL_PORT};
use sliderelay_lib::pairing::Pairing;
use sliderelay_lib::state::{AppState, NoUi};
use std::sync::Arc;
use tower::ServiceExt;

const ORIGIN: &str = "https://mk-manishkandel.github.io";

fn app() -> (axum::Router, Arc<AppState>) {
    let state = AppState::new(Settings::default(), Pairing::load(None), Arc::new(NoUi));
    (router(state.clone(), &[CONTROL_PORT]), state)
}

fn req(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", format!("127.0.0.1:{CONTROL_PORT}"))
        .header("origin", ORIGIN)
}

async fn json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Runs the full pairing flow and returns a valid token.
async fn paired_token(app: &axum::Router, state: &AppState) -> String {
    let resp = app
        .clone()
        .oneshot(req("POST", "/v1/pair").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let id = json(resp).await["requestId"].as_str().unwrap().to_string();
    state.pairing.lock().unwrap().answer(&id, true).unwrap();
    let resp = app
        .clone()
        .oneshot(
            req("GET", &format!("/v1/pair/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json(resp).await;
    assert_eq!(body["state"], "approved");
    body["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn hello_reports_versions() {
    let (app, _) = app();
    let resp = app
        .oneshot(req("GET", "/v1/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json(resp).await;
    assert_eq!(body["app"], "SlideRelay");
    assert_eq!(body["apiVersion"], 1);
    assert_eq!(body["minClientApi"], 1);
}

#[tokio::test]
async fn foreign_origin_is_rejected() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/hello")
                .header("host", format!("127.0.0.1:{CONTROL_PORT}"))
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn dns_rebinding_host_is_rejected() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/hello")
                .header("host", format!("evil.example:{CONTROL_PORT}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn commands_need_a_token() {
    let (app, _) = app();
    for (method, path) in [("GET", "/v1/status"), ("POST", "/v1/broadcast/start")] {
        let resp = app
            .clone()
            .oneshot(req(method, path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{path}");
        let resp = app
            .clone()
            .oneshot(
                req(method, path)
                    .header("authorization", "Bearer not-a-real-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
}

#[tokio::test]
async fn pairing_needs_a_browser_origin() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pair")
                .header("host", format!("127.0.0.1:{CONTROL_PORT}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn paired_addin_can_start_pause_and_stop() {
    let (app, state) = app();
    let token = paired_token(&app, &state).await;
    let auth = format!("Bearer {token}");

    let send = |cmd: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            app.oneshot(
                req("POST", &format!("/v1/broadcast/{cmd}"))
                    .header("authorization", auth)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // Pausing before starting is a conflict.
    assert_eq!(send("pause").await.status(), StatusCode::CONFLICT);

    let started = json(send("start").await).await;
    assert_eq!(started["state"], "waiting");

    let paused = json(send("pause").await).await;
    assert_eq!(paused["paused"], true);

    let blanked = json(send("blank").await).await;
    assert_eq!(blanked["blanked"], true);

    let stopped = json(send("stop").await).await;
    assert_eq!(stopped["state"], "idle");
    assert_eq!(stopped["paused"], false);

    assert_eq!(send("explode").await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cors_preflight_allows_private_network() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            req("OPTIONS", "/v1/status")
                .header("access-control-request-method", "GET")
                .header("access-control-request-headers", "authorization")
                .header("access-control-request-private-network", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let h = resp.headers();
    assert_eq!(h["access-control-allow-origin"], ORIGIN);
    assert_eq!(h["access-control-allow-private-network"], "true");
}

#[tokio::test]
async fn http2_style_authority_is_accepted() {
    // HTTP/2 has no Host header; the host is in the URI instead.
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("https://localhost:{CONTROL_PORT}/v1/hello"))
                .header("origin", ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn foreign_http2_authority_is_rejected() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("https://evil.example:{CONTROL_PORT}/v1/hello"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn settings_need_a_token() {
    let (app, _) = app();
    let resp = app
        .oneshot(req("GET", "/v1/settings").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn paired_addin_can_read_and_change_settings() {
    let (app, state) = app();
    let token = paired_token(&app, &state).await;
    let auth = format!("Bearer {token}");

    let resp = app
        .clone()
        .oneshot(
            req("GET", "/v1/settings")
                .header("authorization", &auth)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = json(resp).await;
    assert_eq!(body["settings"]["quality"], "balanced");
    assert!(body["adapters"].is_array());

    // Change the PIN and quality.
    body["settings"]["pin"] = "4321".into();
    body["settings"]["quality"] = "high".into();
    let resp = app
        .clone()
        .oneshot(
            req("POST", "/v1/settings")
                .header("authorization", &auth)
                .header("content-type", "application/json")
                .body(Body::from(body["settings"].to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await["settings"]["pin"], "4321");
    assert_eq!(state.settings().pin.as_deref(), Some("4321"));

    // A bad PIN is refused with a plain-words reason, and nothing changes.
    body["settings"]["pin"] = "12".into();
    let resp = app
        .oneshot(
            req("POST", "/v1/settings")
                .header("authorization", &auth)
                .header("content-type", "application/json")
                .body(Body::from(body["settings"].to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(resp).await["error"], "The PIN must be 4 to 8 digits.");
    assert_eq!(state.settings().pin.as_deref(), Some("4321"));
}
