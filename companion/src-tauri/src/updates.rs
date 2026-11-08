//! Automatic updates, signed with the project's updater key.
//!
//! Only release builds made by GitHub Actions have the updater configured
//! (see `tauri.release.conf.json`); development builds never update
//! themselves. An update is downloaded quietly in the background and only
//! installed when no broadcast is running: when the presenter stops (if
//! SlideRelay quits after a broadcast) or quits from the menu.

use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Whether this build can update itself (set at startup).
pub struct Updates {
    enabled: bool,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
}

impl Updates {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ready: Mutex::new(None),
        }
    }
}

/// True if the build's config has an updater public key.
pub fn configured(config: &tauri::Config) -> bool {
    config
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

/// Checks for an update a little after startup and downloads it.
pub fn check_in_background(app: &AppHandle) {
    if !app.state::<Updates>().enabled {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let startup finish first; a classroom laptop may be slow.
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        let update = match app.updater() {
            Ok(updater) => updater.check().await,
            Err(e) => Err(e),
        };
        let update = match update {
            Ok(Some(update)) => update,
            Ok(None) => {
                tracing::info!("SlideRelay is up to date");
                return;
            }
            Err(e) => {
                // Offline classrooms are normal: just try next time.
                tracing::info!("could not check for updates: {e}");
                return;
            }
        };
        tracing::info!("downloading update {}", update.version);
        match update.download(|_, _| {}, || {}).await {
            Ok(bytes) => {
                tracing::info!(
                    "update {} ready; installs after the broadcast",
                    update.version
                );
                *app.state::<Updates>()
                    .ready
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
            }
            Err(e) => tracing::warn!("update download failed: {e}"),
        }
    });
}

/// Installs a downloaded update, if any. Call only when quitting and not
/// broadcasting. On Windows the installer closes SlideRelay itself.
pub fn install_if_ready(app: &AppHandle) {
    let Some(updates) = app.try_state::<Updates>() else {
        return;
    };
    let ready = updates
        .ready
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some((update, bytes)) = ready {
        tracing::info!("installing update {}", update.version);
        if let Err(e) = update.install(bytes) {
            tracing::error!("update install failed: {e}");
        }
    }
}
