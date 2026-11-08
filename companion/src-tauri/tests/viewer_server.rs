//! End-to-end tests of the viewers' server over real sockets: many viewers,
//! pause/blank/stop, old links and the PIN.

use futures_util::{SinkExt, StreamExt};
use sliderelay_lib::config::Settings;
use sliderelay_lib::pairing::Pairing;
use sliderelay_lib::state::{AppState, Command, NoUi};
use sliderelay_lib::viewer_server;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn server(settings: Settings) -> (Arc<AppState>, u16) {
    let state = AppState::new(settings, Pairing::load(None), Arc::new(NoUi));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let s = state.clone();
    tokio::spawn(async move { viewer_server::serve(listener, s).await });
    (state, port)
}

async fn connect(port: u16, code: &str) -> Ws {
    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws/{code}"))
        .await
        .unwrap();
    ws
}

/// Next meaningful message (skips pings), with a timeout.
async fn next(ws: &mut Ws) -> Message {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timed out waiting for a message")
            .expect("connection closed")
            .unwrap();
        if !matches!(msg, Message::Ping(_) | Message::Pong(_)) {
            return msg;
        }
    }
}

fn mode_of(msg: &Message) -> String {
    let Message::Text(t) = msg else {
        panic!("expected text, got {msg:?}")
    };
    let v: serde_json::Value = serde_json::from_str(t).unwrap();
    v["mode"].as_str().unwrap_or_default().to_string()
}

fn code(state: &AppState) -> String {
    state.session.lock().unwrap().code().unwrap().to_string()
}

#[tokio::test]
async fn sixty_viewers_get_the_same_frame() {
    let (state, port) = server(Settings::default()).await;
    state.run(Command::Start).unwrap();
    state.update(|s| s.set_slideshow_found(true));
    let code = code(&state);

    let mut viewers = Vec::new();
    for _ in 0..60 {
        let mut ws = connect(port, &code).await;
        assert_eq!(mode_of(&next(&mut ws).await), "live");
        viewers.push(ws);
    }
    // The count reaches the presenter.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(state.status().viewers, 60);

    state.hub.push(vec![0xFF, 0xD8, 1, 2, 3]);
    for ws in &mut viewers {
        assert_eq!(
            next(ws).await,
            Message::Binary(vec![0xFF, 0xD8, 1, 2, 3].into())
        );
    }

    // Leaving updates the count.
    for mut ws in viewers.drain(..30) {
        ws.close(None).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(state.status().viewers, 30);
}

#[tokio::test]
async fn pause_blank_and_stop_reach_viewers() {
    let (state, port) = server(Settings::default()).await;
    state.run(Command::Start).unwrap();
    let code = code(&state);
    let mut ws = connect(port, &code).await;
    assert_eq!(mode_of(&next(&mut ws).await), "waiting");

    state.update(|s| s.set_slideshow_found(true));
    assert_eq!(mode_of(&next(&mut ws).await), "live");
    state.run(Command::Pause).unwrap();
    assert_eq!(mode_of(&next(&mut ws).await), "paused");
    state.run(Command::Blank).unwrap();
    assert_eq!(mode_of(&next(&mut ws).await), "blank");
    state.run(Command::Stop).unwrap();
    assert_eq!(mode_of(&next(&mut ws).await), "ended");
}

#[tokio::test]
async fn old_and_wrong_links_end_immediately() {
    let (state, port) = server(Settings::default()).await;
    state.run(Command::Start).unwrap();
    let old = code(&state);
    state.run(Command::Stop).unwrap();
    state.run(Command::Start).unwrap();
    assert_ne!(old, code(&state));

    let mut ws = connect(port, &old).await;
    assert_eq!(mode_of(&next(&mut ws).await), "ended");
    let mut ws = connect(port, "NOPE").await;
    assert_eq!(mode_of(&next(&mut ws).await), "ended");
}

#[tokio::test]
async fn pin_is_required_when_set() {
    let settings = Settings {
        pin: Some("4321".into()),
        ..Settings::default()
    };
    let (state, port) = server(settings).await;
    state.run(Command::Start).unwrap();
    let mut ws = connect(port, &code(&state)).await;

    assert_eq!(
        next(&mut ws).await,
        Message::Text(r#"{"type":"pin"}"#.into())
    );
    ws.send(Message::Text(r#"{"type":"pin","pin":"0000"}"#.into()))
        .await
        .unwrap();
    assert_eq!(
        next(&mut ws).await,
        Message::Text(r#"{"type":"pin-wrong"}"#.into())
    );
    ws.send(Message::Text(r#"{"type":"pin","pin":"4321"}"#.into()))
        .await
        .unwrap();
    assert_eq!(mode_of(&next(&mut ws).await), "waiting");
}

#[tokio::test]
async fn serves_the_viewer_page_for_any_code() {
    let (_state, port) = server(Settings::default()).await;
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream
        .write_all(b"GET /K7PQ HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut body = String::new();
    stream.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.1 200"));
    assert!(body.contains("SlideRelay"));
    assert!(body.to_lowercase().contains("content-security-policy"));
}
