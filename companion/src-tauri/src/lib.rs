//! SlideRelay companion app.
//!
//! Module map:
//! - `addin_install` — "Add to PowerPoint" (installs the add-in manifest)
//! - `session`     — broadcast state machine (pure logic, unit-tested)
//! - `pairing`     — one-time approval of the PowerPoint add-in (tokens)
//! - `config`      — presenter settings saved as JSON
//! - `state`       — shared app state + commands (Start/Stop/Pause/...)
//! - `control_api` — localhost HTTP/WebSocket API used by the add-in
//! - `local_tls`   — macOS only: HTTPS for the add-in (PowerPoint for Mac refuses
//!   plain http to 127.0.0.1)
//! - `capture`     — finds the slideshow window (never Presenter View), captures it
//! - `broadcaster` — keeps capture in sync with the session
//! - `stream`      — encodes frames once and shares them with all viewers
//! - `viewer_server` — the web page + live stream viewers open
//! - `network`     — picks the classroom network address
//! - `qr`          — QR code for the viewer link
//! - `toolbar`     — floating toolbar, projector QR and keyboard shortcuts
//! - `updates`     — signed automatic updates (release builds only)
//! - this file     — Tauri glue: windows, tray icon, logging, startup

pub mod addin_install;
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
pub mod toolbar;
pub mod updates;
pub mod viewer_server;

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
        toolbar::position_setting_changed(&self.app, &settings.toolbar_position);
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

/// The viewer link and its QR code (only while broadcasting).
#[tauri::command]
fn get_qr(state: SharedState) -> Result<Qr, String> {
    let url = state
        .status()
        .url
        .ok_or("No broadcast is running, or no network was found.")?;
    let svg = qr::svg_for(&url).map_err(|e| e.to_string())?;
    Ok(Qr { url, svg })
}

// ---------- First-run setup guide (main window) ----------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupState {
    /// "macos" or "windows".
    platform: &'static str,
    addin_installed: bool,
    /// macOS: PowerPoint can't connect until the local certificate is trusted.
    connection_needed: bool,
    /// macOS: Screen Recording permission is missing.
    screen_permission_needed: bool,
    setup_done: bool,
}

fn data_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("SlideRelay"))
}

#[tauri::command]
async fn setup_state(app: AppHandle, state: SharedState<'_>) -> Result<SetupState, String> {
    let connection_needed = connection_setup_needed(app.clone()).await;
    let addin_installed = addin_install::is_installed(&data_dir(&app));
    let screen_permission_needed = !capture::has_permission();
    // Every step done: setup is finished (no "Done" button to click), so the
    // window won't open by itself at the next start.
    let all_done = addin_installed
        && (cfg!(not(target_os = "macos")) || (!connection_needed && !screen_permission_needed));
    if all_done && !state.settings().setup_done {
        state.mark_setup_done();
    }
    Ok(SetupState {
        platform: if cfg!(target_os = "macos") {
            "macos"
        } else {
            "windows"
        },
        addin_installed,
        connection_needed,
        screen_permission_needed,
        setup_done: state.settings().setup_done,
    })
}

#[tauri::command]
async fn install_addin(app: AppHandle) -> Result<(), String> {
    let dir = data_dir(&app);
    tauri::async_runtime::spawn_blocking(move || addin_install::install(&dir))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            tracing::warn!("could not add SlideRelay to PowerPoint: {e}");
            "SlideRelay couldn't add itself to PowerPoint. Follow the manual steps in the install guide."
                .to_string()
        })
}

/// Opens SlideRelay's help (the GitHub README) in the default browser.
#[tauri::command]
fn open_help(app: AppHandle) {
    use tauri_plugin_opener::OpenerExt;
    let url = "https://github.com/mk-manishkandel/SlideRelay#readme";
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        tracing::warn!("could not open help: {e}");
    }
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
    capture::open_permission_settings();
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The "Check for updates" link in the SlideRelay window.
#[tauri::command]
fn check_for_updates(app: AppHandle) -> updates::UpdateCheck {
    updates::start_check(&app)
}

/// Progress of the current update check, for the SlideRelay window.
#[tauri::command]
fn update_status(app: AppHandle) -> updates::UpdateCheck {
    updates::status(&app)
}

/// The presenter started dragging the toolbar: stop placing it for them.
#[tauri::command]
fn toolbar_moved() {
    toolbar::user_moved();
}

/// "Install and restart": only when no broadcast is running.
#[tauri::command]
fn install_update_now(app: AppHandle, state: SharedState) -> Result<(), String> {
    if state.status().state != session::BroadcastState::Idle {
        return Err("Stop the broadcast first. The update installs in a few seconds.".into());
    }
    if !updates::has_ready(&app) {
        return Err("No update is ready yet.".into());
    }
    // Windows: the installer closes and reopens SlideRelay itself.
    updates::install_if_ready(&app);
    app.restart();
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

/// Runs the one-time step: macOS asks for the presenter's password to trust
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
    let open = MenuItem::with_id(app, "open", "Setup and status", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit SlideRelay", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("SlideRelay")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => {
                // Never mid-broadcast: the menu only quits, and installs a
                // waiting update first.
                let idle =
                    app.state::<Arc<AppState>>().status().state == session::BroadcastState::Idle;
                if idle {
                    updates::install_if_ready(app);
                }
                app.exit(0);
            }
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

/// Starts the viewers' web server (restarting it if the presenter changes
/// the port) and keeps the link's network address up to date (e.g. if the
/// laptop moves to another Wi-Fi).
fn start_viewer_server(state: Arc<AppState>) {
    let server_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let state = server_state;
        let mut port_rx = state.watch_port();
        loop {
            let preferred = *port_rx.borrow_and_update();
            let listener = match viewer_server::bind(preferred).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("viewer server could not start: {e}");
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
            *state.viewer_port.write().unwrap_or_else(|e| e.into_inner()) = port;
            refresh_link(&state);
            tokio::select! {
                result = viewer_server::serve(listener, state.clone()) => {
                    if let Err(e) = result {
                        tracing::error!("viewer server stopped: {e}");
                    }
                    return;
                }
                changed = port_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    tracing::info!("viewer port setting changed; restarting the viewer server");
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

/// Works out the viewer link from the current network and port.
fn refresh_link(state: &AppState) {
    let preferred_ip = state.settings().lan_ip;
    let port = *state.viewer_port.read().unwrap_or_else(|e| e.into_inner());
    let ip = network::choose_lan_ip(&network::adapters(), preferred_ip.as_deref());
    let url = ip.zip(port).map(|(ip, port)| network::base_url(ip, port));
    let changed = {
        let mut base = state.base_url.write().unwrap_or_else(|e| e.into_inner());
        let changed = *base != url;
        *base = url.clone();
        changed
    };
    if changed {
        tracing::info!("viewer link base: {url:?}");
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
            quit_after_stop(app);
        }
        _ => toolbar::reposition_toolbar(app),
    }
}

/// Quits SlideRelay shortly after a broadcast stops (if the presenter
/// wants that), once viewers have had time to see "The broadcast has ended"
/// and the panel has its answer.
fn quit_after_stop(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if !state.settings().quit_on_stop {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        // Started again in the meantime? Then stay.
        if state.status().state == session::BroadcastState::Idle {
            tracing::info!("broadcast stopped; quitting as set in Settings");
            updates::install_if_ready(&app);
            app.exit(0);
        }
    });
}

/// Handles sliderelay:// links from the panel: "start" starts a broadcast,
/// "open" (or anything else) just makes sure the app is running.
fn handle_links(app: &AppHandle, urls: &[tauri::Url]) {
    for url in urls {
        tracing::info!("opened with link {url}");
        if url.host_str() == Some("start") {
            let state = app.state::<Arc<AppState>>().inner().clone();
            if state.status().state == session::BroadcastState::Idle {
                let _ = state.run(Command::Start);
            }
        }
    }
    if needs_attention(app) {
        show_main_window(app);
    }
}

/// True if the main window must open at startup: the first-run setup
/// guide hasn't been finished, or (Mac) PowerPoint can't reach SlideRelay
/// yet. Everything else, like Screen Recording permission, is shown and
/// fixed from the PowerPoint panel, so SlideRelay otherwise starts silently.
fn needs_attention(app: &AppHandle) -> bool {
    let setup_done = app
        .try_state::<Arc<AppState>>()
        .map(|s| s.settings().setup_done)
        .unwrap_or(true);
    if !setup_done {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        app.try_state::<local_tls::LocalCert>()
            .map(|cert| !cert.is_trusted())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let can_update = updates::configured(context.config());
    let mut builder = tauri::Builder::default()
        // Must be first: a second launch goes to the running app instead.
        // sliderelay:// links arrive through the deep-link plugin below.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let has_link = args.iter().any(|a| a.starts_with("sliderelay:"));
            if !has_link && needs_attention(app) {
                show_main_window(app);
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
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
        .manage(updates::Updates::new(can_update));
    if can_update {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    let app = builder
        .invoke_handler(tauri::generate_handler![
            get_status,
            run_command,
            answer_pairing,
            app_version,
            check_for_updates,
            update_status,
            install_update_now,
            connection_setup_needed,
            setup_connection,
            get_qr,
            toolbar_command,
            toolbar_moved,
            hide_qr,
            screen_permission_needed,
            open_screen_permission_settings,
            setup_state,
            install_addin,
            open_help
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
            start_viewer_server(state.clone());

            build_tray(&handle)?;
            updates::check_in_background(&handle);

            // After an update: give PowerPoint this version's add-in manifest.
            {
                let dir = data_dir(&handle);
                tauri::async_runtime::spawn_blocking(move || {
                    match addin_install::refresh_if_outdated(&dir) {
                        Ok(true) => tracing::info!("updated the PowerPoint add-in manifest"),
                        Ok(false) => {}
                        Err(e) => tracing::warn!("could not update the add-in manifest: {e}"),
                    }
                });
            }

            // sliderelay://start and sliderelay://open, at launch and later.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let link_handle = handle.clone();
                app.deep_link().on_open_url(move |event| {
                    handle_links(&link_handle, &event.urls());
                });
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    handle_links(&handle, &urls);
                }
            }

            // SlideRelay lives in the menu bar / system tray. Only open its
            // window when the presenter has something to do there (a one-time
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
        .build(context)
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
