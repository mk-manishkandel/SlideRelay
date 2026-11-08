//! SlideRelay companion app.
//!
//! Module map:
//! - `session`     — broadcast state machine (pure logic, unit-tested)
//! - `pairing`     — one-time approval of the PowerPoint add-in (tokens)
//! - `config`      — teacher settings saved as JSON
//! - `state`       — shared app state + commands (Start/Stop/Pause/...)
//! - `control_api` — localhost HTTP/WebSocket API used by the add-in
//! - `local_tls`   — macOS only: HTTPS for the add-in (PowerPoint for Mac refuses
//!   plain http to 127.0.0.1)
//! - `capture`     — finds the slideshow window (never Presenter View), captures it
//! - `broadcaster` — keeps capture in sync with the session
//! - `stream`      — encodes frames once and shares them with all students
//! - `student_server` — the web page + live stream students open
//! - `network`     — picks the classroom network address
//! - `qr`          — QR code for the student link
//! - `toolbar`     — floating toolbar, projector QR and keyboard shortcuts
//! - this file     — Tauri glue: windows, tray icon, logging, startup

pub mod broadcaster;
pub mod capture;
pub mod config;
pub mod control_api;
#[cfg(target_os = "macos")]
pub mod local_tls;
pub mod network;
pub mod pairing;
pub mod qr;
pub mod session;
pub mod state;
pub mod stream;
pub mod student_server;
pub mod toolbar;

use config::{Paths, Settings};
use pairing::Pairing;
use session::Status;
use state::{AppState, Command, Ui};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// Passed by the OS autostart entry so we start quietly in the tray.
const AUTOSTART_ARG: &str = "--autostart";

// ---------- The real UI, backed by Tauri windows ----------

struct TauriUi {
    app: AppHandle,
}

impl Ui for TauriUi {
    fn show_pairing_prompt(&self, request_id: &str) {
        // If the prompt is already open, hand it the new request instead of
        // closing and reopening it (closing is asynchronous, so reopening
        // straight away fails and would leave a stale, expired prompt).
        if let Some(existing) = self.app.get_webview_window("pair") {
            let _ = existing.emit("pair-request", request_id);
            let _ = existing.show();
            let _ = existing.set_focus();
            return;
        }
        let url = format!("pair.html?id={request_id}");
        let result = WebviewWindowBuilder::new(&self.app, "pair", WebviewUrl::App(url.into()))
            .title("Connect PowerPoint")
            .inner_size(380.0, 300.0)
            .resizable(false)
            .minimizable(false)
            .always_on_top(true)
            .center()
            .focused(true)
            .build();
        if let Err(e) = result {
            tracing::error!("could not open pairing window: {e}");
        }
    }

    fn show_qr(&self) {
        toolbar::show_qr(&self.app);
    }

    fn settings_changed(&self, settings: &Settings) {
        apply_launch_at_login(&self.app, settings.launch_at_login);
        toolbar::reposition_toolbar(&self.app);
        // A new network choice shows up in the link straight away.
        if let Some(state) = self.app.try_state::<Arc<AppState>>() {
            refresh_link(&state);
        }
    }

    fn broadcast_changed(&self, broadcasting: bool) {
        tracing::info!(
            "broadcast {}",
            if broadcasting { "started" } else { "stopped" }
        );
    }
}

/// Turns "Start SlideRelay when I log in" on or off in the operating system.
fn apply_launch_at_login(app: &AppHandle, on: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    if launcher.is_enabled().unwrap_or(false) == on {
        return;
    }
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(e) = result {
        tracing::warn!("could not change launch at login: {e}");
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

// ---------- Commands callable from SlideRelay's own windows ----------

type SharedState<'a> = tauri::State<'a, Arc<AppState>>;

#[tauri::command]
fn get_status(state: SharedState) -> Status {
    state.status()
}

#[tauri::command]
fn run_command(state: SharedState, command: String) -> Result<Status, String> {
    let cmd = Command::parse(&command).ok_or("Unknown command.")?;
    state
        .run(cmd)
        .map_err(|_| "Start a broadcast first.".to_string())
}

#[tauri::command]
fn answer_pairing(state: SharedState, id: String, allow: bool) -> Result<(), String> {
    state
        .pairing
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .answer(&id, allow)
        .map_err(|_| "This request has expired. Click Connect in PowerPoint again.".to_string())
}

#[derive(serde::Serialize)]
struct Qr {
    url: String,
    svg: String,
}

/// The student link and its QR code (only while broadcasting).
#[tauri::command]
fn get_qr(state: SharedState) -> Result<Qr, String> {
    let url = state
        .status()
        .url
        .ok_or("No broadcast is running, or no network was found.")?;
    let svg = qr::svg_for(&url).map_err(|e| e.to_string())?;
    Ok(Qr { url, svg })
}

/// The toolbar's buttons: run the command, then hand the keyboard back to
/// PowerPoint so arrow keys keep changing slides.
#[tauri::command]
fn toolbar_command(app: AppHandle, state: SharedState, command: String) -> Result<Status, String> {
    let result = match command.as_str() {
        "toggle-qr" => {
            toolbar::toggle_qr(&app);
            Ok(state.status())
        }
        // "Keep going" after Stop: nothing to do but give the keyboard back.
        "refocus" => Ok(state.status()),
        _ => run_command(state, command),
    };
    toolbar::refocus_powerpoint(&app);
    result
}

/// The projector QR code hides itself when clicked.
#[tauri::command]
fn hide_qr(app: AppHandle) {
    toolbar::hide_qr(&app);
    toolbar::refocus_powerpoint(&app);
}

#[tauri::command]
fn screen_permission_needed() -> bool {
    !capture::has_permission()
}

/// Opens the right System Settings page (macOS) for Screen Recording.
#[tauri::command]
fn open_screen_permission_settings() {
    capture::request_permission();
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// True when the one-time "let PowerPoint connect" step is still needed
/// (macOS only; always false on Windows).
#[tauri::command]
async fn connection_setup_needed(app: AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        let cert = app.state::<local_tls::LocalCert>().inner().clone();
        tauri::async_runtime::spawn_blocking(move || !cert.is_trusted())
            .await
            .unwrap_or(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        false
    }
}

/// Runs the one-time step: macOS asks for the teacher's password to trust
/// SlideRelay's local certificate.
#[tauri::command]
async fn setup_connection(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let cert = app.state::<local_tls::LocalCert>().inner().clone();
        tauri::async_runtime::spawn_blocking(move || cert.trust())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| {
                tracing::warn!("trusting local certificate failed: {e}");
                "macOS didn't allow it. Click the button again and enter your Mac password."
                    .to_string()
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

// ---------- Startup ----------

fn app_paths(app: &AppHandle) -> anyhow::Result<Paths> {
    let config = app.path().app_config_dir()?;
    Ok(Paths {
        settings: config.join("settings.json"),
        tokens: config.join("paired-addins.json"),
        tls: config.join("tls"),
        logs: app.path().app_log_dir()?,
    })
}

/// Logs go to a daily-rotated file in the app's log folder (kept 7 days).
/// Returns a guard that must live as long as the app.
fn init_logging(dir: &std::path::Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    // The folder doesn't exist on first run.
    std::fs::create_dir_all(dir).ok()?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("sliderelay")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("SLIDERELAY_LOG")
        .unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Some(guard)
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open SlideRelay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit SlideRelay", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("SlideRelay")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Windows convention: left-click the tray icon opens the app.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if !cfg!(target_os = "macos") {
                    show_main_window(tray.app_handle());
                }
            }
        });

    // macOS menu bar: monochrome "template" icon that adapts to light/dark.
    #[cfg(target_os = "macos")]
    {
        let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
        tray = tray.icon(icon).icon_as_template(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app.default_window_icon() {
            tray = tray.icon(icon.clone());
        }
    }
    tray.build(app)?;
    Ok(())
}

/// Starts the students' web server (restarting it if the teacher changes
/// the port) and keeps the link's network address up to date (e.g. if the
/// laptop moves to another Wi-Fi).
fn start_student_server(state: Arc<AppState>) {
    let server_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let state = server_state;
        let mut port_rx = state.watch_port();
        loop {
            let preferred = *port_rx.borrow_and_update();
            let listener = match student_server::bind(preferred).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("student server could not start: {e}");
                    state.update(|s| {
                        s.set_problem(Some(
                            "SlideRelay couldn't open a network port. Restart SlideRelay and try again."
                                .into(),
                        ))
                    });
                    if port_rx.changed().await.is_err() {
                        return;
                    }
                    continue;
                }
            };
            let port = listener.local_addr().map(|a| a.port()).ok();
            *state
                .student_port
                .write()
                .unwrap_or_else(|e| e.into_inner()) = port;
            refresh_link(&state);
            tokio::select! {
                result = student_server::serve(listener, state.clone()) => {
                    if let Err(e) = result {
                        tracing::error!("student server stopped: {e}");
                    }
                    return;
                }
                changed = port_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    tracing::info!("student port setting changed; restarting the student server");
                }
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            refresh_link(&state);
        }
    });
}

/// Works out the student link from the current network and port.
fn refresh_link(state: &AppState) {
    let preferred_ip = state.settings().lan_ip;
    let port = *state.student_port.read().unwrap_or_else(|e| e.into_inner());
    let ip = network::choose_lan_ip(&network::adapters(), preferred_ip.as_deref());
    let url = ip.zip(port).map(|(ip, port)| network::base_url(ip, port));
    let changed = {
        let mut base = state.base_url.write().unwrap_or_else(|e| e.into_inner());
        let changed = *base != url;
        *base = url.clone();
        changed
    };
    if changed {
        tracing::info!("student link base: {url:?}");
        state.update(|_| ());
    }
}

/// Shows the toolbar and takes the shortcuts while broadcasting; moves the
/// toolbar off the projector once the slideshow is found.
fn on_broadcast_state(app: &AppHandle, from: session::BroadcastState, to: session::BroadcastState) {
    use session::BroadcastState::Idle;
    match (from, to) {
        (Idle, _) => {
            toolbar::show_toolbar(app);
            toolbar::register_shortcuts(app);
        }
        (_, Idle) => {
            toolbar::hide_toolbar(app);
            toolbar::hide_qr(app);
            toolbar::unregister_shortcuts(app);
        }
        _ => toolbar::reposition_toolbar(app),
    }
}

/// True if a one-time setup step is still needed (shown in the main window).
fn needs_attention(app: &AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        if !capture::has_permission() {
            return true;
        }
        if let Some(cert) = app.try_state::<local_tls::LocalCert>() {
            if !cert.is_trusted() {
                return true;
            }
        }
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch (or a sliderelay:// link) just
        // brings the running app to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toolbar::on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .invoke_handler(tauri::generate_handler![
            get_status,
            run_command,
            answer_pairing,
            app_version,
            connection_setup_needed,
            setup_connection,
            get_qr,
            toolbar_command,
            hide_qr,
            screen_permission_needed,
            open_screen_permission_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let paths = app_paths(&handle)?;
            if let Some(guard) = init_logging(&paths.logs) {
                // Keep the log writer alive for the whole run.
                app.manage(guard);
            }
            tracing::info!("SlideRelay {} starting", env!("CARGO_PKG_VERSION"));

            // A menu-bar app on macOS: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings = Settings::load(&paths.settings);
            let pairing = Pairing::load(Some(paths.tokens.clone()));
            let ui = Arc::new(TauriUi {
                app: handle.clone(),
            });
            let state = AppState::new(settings, pairing, ui);
            state.set_settings_path(paths.settings.clone());
            app.manage(state.clone());
            app.manage(paths.clone());

            // Forward every status change to SlideRelay's own windows.
            let mut rx = state.subscribe();
            let emit_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut last = session::BroadcastState::Idle;
                while rx.changed().await.is_ok() {
                    let status = rx.borrow_and_update().clone();
                    let _ = emit_handle.emit("status", &status);
                    if status.state != last {
                        on_broadcast_state(&emit_handle, last, status.state);
                        last = status.state;
                    }
                }
            });

            // Start the localhost API for the add-in.
            #[cfg(target_os = "macos")]
            let ports = [control_api::CONTROL_PORT, local_tls::TLS_PORT];
            #[cfg(not(target_os = "macos"))]
            let ports = [control_api::CONTROL_PORT];
            let api = control_api::router(state.clone(), &ports);

            let http_api = api.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = control_api::serve(http_api).await {
                    tracing::error!("control API failed to start: {e}");
                }
            });

            // macOS: the same API over HTTPS for PowerPoint for Mac.
            #[cfg(target_os = "macos")]
            {
                let cert = local_tls::LocalCert::in_dir(&paths.tls);
                match cert.ensure() {
                    Ok(()) => {
                        let https_cert = cert.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = local_tls::serve(&https_cert, api).await {
                                tracing::error!("HTTPS control API failed to start: {e}");
                            }
                        });
                    }
                    Err(e) => tracing::error!("could not create local certificate: {e}"),
                }
                app.manage(cert);
            }

            // Capture + streaming.
            broadcaster::start(state.clone());
            start_student_server(state.clone());

            build_tray(&handle)?;

            // SlideRelay lives in the menu bar / system tray. Only open its
            // window when the teacher has something to do there (a one-time
            // setup step); otherwise start quietly, so there's no window to
            // close (on some Macs closing a window force-quits the app).
            let autostart = std::env::args().any(|a| a == AUTOSTART_ARG);
            if !autostart && needs_attention(&handle) {
                show_main_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps SlideRelay running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building SlideRelay");

    app.run(|_app, event| {
        // Stay alive in the tray when no windows are open.
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
