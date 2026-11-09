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
/// After a failed check (offline, GitHub slow), try again sooner.
const RETRY_AFTER: Duration = Duration::from_secs(30 * 60);
/// Asking GitHub "is there a newer version?" Some networks take ~20 s just
/// to connect to GitHub's download servers, and there are two hops.
const CHECK_TIMEOUT: Duration = Duration::from_secs(90);
/// Downloading the update itself (about 5-12 MB) on slow classroom Wi-Fi.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Whether this build can update itself (set at startup).
pub struct Updates {
    enabled: bool,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    /// What the updater is doing now, for the SlideRelay window.
    phase: Mutex<UpdateCheck>,
    /// One check at a time (background and the "Check for updates" button).
    checking: tokio::sync::Mutex<()>,
}

impl Updates {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ready: Mutex::new(None),
            phase: Mutex::new(UpdateCheck::Idle),
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

    fn set_phase(&self, phase: UpdateCheck) -> UpdateCheck {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = phase.clone();
        phase
    }

    fn phase(&self) -> UpdateCheck {
        if !self.enabled {
            return UpdateCheck::Unavailable;
        }
        self.phase.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// What the updater is doing, for the "Check for updates" link.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum UpdateCheck {
    /// Test builds made from source don't update themselves.
    Unavailable,
    /// Not checked yet.
    Idle,
    Checking,
    Downloading {
        version: String,
        /// 0-100, when GitHub says how big the file is.
        percent: Option<u8>,
    },
    UpToDate,
    /// Downloaded; installs when SlideRelay quits (or "Install and restart").
    Ready {
        version: String,
    },
    /// Offline, GitHub unreachable, bad signature...
    Failed {
        message: String,
    },
}

impl UpdateCheck {
    /// Still working (the window keeps asking for progress).
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading { .. })
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

/// What the updater is doing right now.
pub fn status(app: &AppHandle) -> UpdateCheck {
    app.state::<Updates>().phase()
}

/// The "Check for updates" link: starts a check in the background (unless
/// one is already running) and answers straight away. The window then asks
/// for `status` until it's finished.
pub fn start_check(app: &AppHandle) -> UpdateCheck {
    let updates = app.state::<Updates>();
    let now = updates.phase();
    if now == UpdateCheck::Unavailable || now.is_busy() || matches!(now, UpdateCheck::Ready { .. })
    {
        return now;
    }
    let shown = updates.set_phase(UpdateCheck::Checking);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        check_now(&app).await;
    });
    shown
}

/// Looks for a newer version and downloads it (once).
async fn check_now(app: &AppHandle) -> UpdateCheck {
    let updates = app.state::<Updates>();
    if !updates.enabled {
        return UpdateCheck::Unavailable;
    }
    let Ok(_one_at_a_time) = updates.checking.try_lock() else {
        // Already checking: that check reports its own progress.
        return updates.phase();
    };
    if let Some(version) = updates.ready_version() {
        return updates.set_phase(UpdateCheck::Ready { version });
    }
    updates.set_phase(UpdateCheck::Checking);
    let update = match app.updater_builder().timeout(CHECK_TIMEOUT).build() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let mut update = match update {
        Ok(Some(update)) => update,
        Ok(None) => {
            tracing::info!("SlideRelay is up to date");
            return updates.set_phase(UpdateCheck::UpToDate);
        }
        Err(e) => {
            // Offline classrooms are normal: just try next time.
            tracing::info!("could not check for updates: {e}");
            return updates.set_phase(UpdateCheck::Failed {
                message: "Couldn't reach GitHub. Check the internet connection and try again."
                    .into(),
            });
        }
    };
    let version = update.version.clone();
    tracing::info!("downloading update {version}");
    updates.set_phase(UpdateCheck::Downloading {
        version: version.clone(),
        percent: None,
    });
    update.timeout = Some(DOWNLOAD_TIMEOUT);
    let mut received: u64 = 0;
    let mut last_percent: Option<u8> = None;
    let progress_app = app.clone();
    let progress_version = version.clone();
    let result = update
        .download(
            move |chunk, total| {
                received += chunk as u64;
                let percent = total
                    .filter(|t| *t > 0)
                    .map(|t| ((received * 100) / t).min(100) as u8);
                if percent != last_percent {
                    last_percent = percent;
                    progress_app
                        .state::<Updates>()
                        .set_phase(UpdateCheck::Downloading {
                            version: progress_version.clone(),
                            percent,
                        });
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            tracing::info!("update {version} ready; installs after the broadcast");
            *updates.ready.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
            updates.set_phase(UpdateCheck::Ready { version })
        }
        Err(e) => {
            tracing::warn!("update download failed: {e}");
            updates.set_phase(UpdateCheck::Failed {
                message: "The update didn't download completely. Try again in a few minutes."
                    .into(),
            })
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
            tracing::info!("checking for updates");
            let wait = match check_now(&app).await {
                UpdateCheck::Ready { .. } => return,
                UpdateCheck::Failed { .. } => RETRY_AFTER,
                _ => CHECK_EVERY,
            };
            tokio::time::sleep(wait).await;
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
