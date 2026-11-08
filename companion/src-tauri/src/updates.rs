//! Automatic updates, signed with the project's updater key.
//!
//! Only release builds made by GitHub Actions have the updater configured
//! (see `tauri.release.conf.json`); development builds never update
//! themselves. An update is downloaded quietly in the background and only
//! installed when no broadcast is running: when the presenter stops (if
//! SlideRelay quits after a broadcast) or quits from the menu.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// How often a long-running SlideRelay looks again (after the first check).
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Whether this build can update itself (set at startup).
pub struct Updates {
    enabled: bool,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    /// One check at a time (background and the "Check for updates" button).
    checking: tokio::sync::Mutex<()>,
}

impl Updates {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ready: Mutex::new(None),
            checking: tokio::sync::Mutex::new(()),
        }
    }

    fn ready_version(&self) -> Option<String> {
        self.ready
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|(u, _)| u.version.clone())
    }
}

/// The result of a check, for the "Check for updates" button.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum UpdateCheck {
    /// Test builds made from source don't update themselves.
    Unavailable,
    UpToDate,
    /// Downloaded; installs when SlideRelay quits (or "Install now").
    Ready {
        version: String,
    },
    /// Offline, GitHub unreachable, bad signature...
    Failed {
        message: String,
    },
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

/// Looks for a newer version and downloads it (once).
pub async fn check_now(app: &AppHandle) -> UpdateCheck {
    let updates = app.state::<Updates>();
    if !updates.enabled {
        return UpdateCheck::Unavailable;
    }
    let _one_at_a_time = updates.checking.lock().await;
    if let Some(version) = updates.ready_version() {
        return UpdateCheck::Ready { version };
    }
    let update = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let update = match update {
        Ok(Some(update)) => update,
        Ok(None) => {
            tracing::info!("SlideRelay is up to date");
            return UpdateCheck::UpToDate;
        }
        Err(e) => {
            // Offline classrooms are normal: just try next time.
            tracing::info!("could not check for updates: {e}");
            return UpdateCheck::Failed {
                message: "Couldn't reach GitHub. Check the internet connection and try again."
                    .into(),
            };
        }
    };
    tracing::info!("downloading update {}", update.version);
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            let version = update.version.clone();
            tracing::info!("update {version} ready; installs after the broadcast");
            *updates.ready.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
            UpdateCheck::Ready { version }
        }
        Err(e) => {
            tracing::warn!("update download failed: {e}");
            UpdateCheck::Failed {
                message: "The update didn't download completely. Try again later.".into(),
            }
        }
    }
}

/// Checks a little after startup, then every few hours, and downloads.
pub fn check_in_background(app: &AppHandle) {
    if !app.state::<Updates>().enabled {
        tracing::info!("updates are off in this build (made from source)");
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let startup finish first; a classroom laptop may be slow.
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            if matches!(check_now(&app).await, UpdateCheck::Ready { .. }) {
                return;
            }
            tokio::time::sleep(CHECK_EVERY).await;
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

/// True if a downloaded update is waiting.
pub fn has_ready(app: &AppHandle) -> bool {
    app.try_state::<Updates>()
        .is_some_and(|u| u.ready_version().is_some())
}
