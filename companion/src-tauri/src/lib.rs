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
//! - this file     — Tauri glue: windows, tray icon, logging, startup

pub mod config;
pub mod control_api;
#[cfg(target_os = "macos")]
pub mod local_tls;
pub mod pairing;
pub mod session;
pub mod state;

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
        // The dedicated QR window arrives in milestone 2; for now the main
        // window shows the link.
        show_main_window(&self.app);
    }

    fn broadcast_changed(&self, broadcasting: bool) {
        tracing::info!(
            "broadcast {}",
            if broadcasting { "started" } else { "stopped" }
        );
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch (or a sliderelay:// link) just
        // brings the running app to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            run_command,
            answer_pairing,
            app_version,
            connection_setup_needed,
            setup_connection
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
            app.manage(state.clone());
            app.manage(paths.clone());

            // Forward every status change to SlideRelay's own windows.
            let mut rx = state.subscribe();
            let emit_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                while rx.changed().await.is_ok() {
                    let status = rx.borrow_and_update().clone();
                    let _ = emit_handle.emit("status", status);
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

            build_tray(&handle)?;

            let quiet = std::env::args().any(|a| a == AUTOSTART_ARG);
            if !quiet {
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
