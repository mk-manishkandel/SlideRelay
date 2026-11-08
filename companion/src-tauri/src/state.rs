//! The app's shared state, used by the control API, the Tauri windows and
//! (later) the capture and viewer servers.
//!
//! It doesn't depend on Tauri: anything UI-related goes through the `Ui`
//! trait, so tests can swap in a fake UI.

use crate::config::Settings;
use crate::pairing::Pairing;
use crate::session::{BroadcastState, CommandError, Session, Status};
use crate::stream::{EncodeSettings, Hub, ViewerMode};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::watch;

/// The things the core logic needs the UI to do.
pub trait Ui: Send + Sync {
    /// Ask the presenter "Allow PowerPoint to control SlideRelay?".
    fn show_pairing_prompt(&self, request_id: &str);
    /// Show (or bring to front) the QR code window.
    fn show_qr(&self);
    /// The broadcast started or stopped (to show/hide the toolbar, etc.).
    fn broadcast_changed(&self, broadcasting: bool);
    /// The presenter saved new settings (launch at login, toolbar position...).
    fn settings_changed(&self, _settings: &Settings) {}
}

/// Why new settings were not saved, in plain words for the presenter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError(pub String);

/// A command from the add-in, toolbar, tray or keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Start,
    Stop,
    Pause,
    Resume,
    Blank,
    Unblank,
    ShowQr,
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "start" => Self::Start,
            "stop" => Self::Stop,
            "pause" => Self::Pause,
            "resume" => Self::Resume,
            "blank" => Self::Blank,
            "unblank" => Self::Unblank,
            "show-qr" => Self::ShowQr,
            _ => return None,
        })
    }
}

pub struct AppState {
    pub session: Mutex<Session>,
    pub pairing: Mutex<Pairing>,
    pub settings: RwLock<Settings>,
    /// "http://<lan-ip>:<port>" once the viewer server is running.
    pub base_url: RwLock<Option<String>>,
    /// The port the viewer server is actually using.
    pub viewer_port: RwLock<Option<u16>>,
    /// Where settings are saved (None in tests).
    settings_path: Mutex<Option<PathBuf>>,
    /// The viewer port the presenter chose; the server restarts when it changes.
    port_tx: watch::Sender<u16>,
    /// Shares frames and the viewer mode with all viewers.
    pub hub: Arc<Hub>,
    /// Scaling/quality used by the encoder (from Settings → Quality).
    pub encode_settings: Arc<Mutex<EncodeSettings>>,
    /// Where the slideshow window is (screen points), for "Show QR on projector".
    pub slideshow_bounds: Mutex<Option<(f64, f64, f64, f64)>>,
    /// Wakes the broadcaster thread right away after a command.
    pub wake: crate::broadcaster::Wake,
    status_tx: watch::Sender<Status>,
    ui: Arc<dyn Ui>,
}

impl AppState {
    pub fn new(settings: Settings, pairing: Pairing, ui: Arc<dyn Ui>) -> Arc<Self> {
        let session = Session::new();
        let (status_tx, _) = watch::channel(session.status(None));
        let encode = EncodeSettings::from(&settings.quality);
        let (port_tx, _) = watch::channel(settings.port);
        Arc::new(Self {
            session: Mutex::new(session),
            pairing: Mutex::new(pairing),
            settings: RwLock::new(settings),
            base_url: RwLock::new(None),
            viewer_port: RwLock::new(None),
            settings_path: Mutex::new(None),
            port_tx,
            hub: Arc::new(Hub::new()),
            encode_settings: Arc::new(Mutex::new(encode)),
            slideshow_bounds: Mutex::new(None),
            wake: crate::broadcaster::Wake::default(),
            status_tx,
            ui,
        })
    }

    pub fn ui(&self) -> &dyn Ui {
        self.ui.as_ref()
    }

    /// Save settings to this file from now on.
    pub fn set_settings_path(&self, path: PathBuf) {
        *self.settings_path.lock().unwrap_or_else(|e| e.into_inner()) = Some(path);
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The port setting, watched by the viewer server.
    pub fn watch_port(&self) -> watch::Receiver<u16> {
        self.port_tx.subscribe()
    }

    /// Checks, saves and applies new settings. Most take effect straight
    /// away (quality and speed on the next frame, the PIN for the next
    /// viewer who joins); a new port restarts the viewer server, so it
    /// can't change mid-broadcast.
    pub fn save_settings(&self, new: Settings) -> Result<Settings, SettingsError> {
        let mut new = new.validated().map_err(SettingsError)?;
        let old = self.settings();
        // Only the setup wizard changes this.
        new.setup_done = old.setup_done;
        if new.port != old.port && self.status().state != BroadcastState::Idle {
            return Err(SettingsError(
                "Stop the broadcast before changing the port.".into(),
            ));
        }
        let path = self
            .settings_path
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(path) = path {
            new.save(&path).map_err(|e| {
                tracing::error!("could not save settings: {e}");
                SettingsError("SlideRelay couldn't save your settings.".into())
            })?;
        }
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = new.clone();
        *self
            .encode_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = EncodeSettings::from(&new.quality);
        self.port_tx.send_if_modified(|port| {
            let changed = *port != new.port;
            *port = new.port;
            changed
        });
        self.wake.notify();
        self.ui.settings_changed(&new);
        Ok(new)
    }

    /// Marks the first-run setup as finished (saved straight away).
    pub fn mark_setup_done(&self) {
        let mut s = self.settings();
        if s.setup_done {
            return;
        }
        s.setup_done = true;
        let path = self
            .settings_path
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(path) = path {
            if let Err(e) = s.save(&path) {
                tracing::error!("could not save settings: {e}");
            }
        }
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = s;
    }

    pub fn status(&self) -> Status {
        self.status_tx.borrow().clone()
    }

    /// Live status updates (for the add-in WebSocket and the app windows).
    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.status_tx.subscribe()
    }

    /// Changes the session and notifies everyone if the status changed.
    pub fn update<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        let (result, status, mode) = {
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let result = f(&mut session);
            let base = self.base_url.read().unwrap_or_else(|e| e.into_inner());
            let status = session.status(base.as_deref());
            let mode = viewer_mode(&status);
            (result, status, mode)
        };
        // Viewers' screens follow the session (Live/Paused/Blank/Ended).
        self.hub.set_mode(mode);
        self.status_tx.send_if_modified(|current| {
            if *current != status {
                *current = status;
                true
            } else {
                false
            }
        });
        result
    }

    /// Runs a presenter command and returns the new status.
    pub fn run(&self, command: Command) -> Result<Status, CommandError> {
        let was_broadcasting = self.status().state != crate::session::BroadcastState::Idle;
        match command {
            Command::Start => self.update(|s| s.start()),
            Command::Stop => self.update(|s| s.stop()),
            Command::Pause => self.update(|s| s.set_paused(true))?,
            Command::Resume => self.update(|s| s.set_paused(false))?,
            Command::Blank => self.update(|s| s.set_blanked(true))?,
            Command::Unblank => self.update(|s| s.set_blanked(false))?,
            Command::ShowQr => {
                if !self
                    .session
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_broadcasting()
                {
                    return Err(CommandError::NotBroadcasting);
                }
                self.ui.show_qr();
            }
        }
        self.wake.notify();
        let status = self.status();
        let now_broadcasting = status.state != crate::session::BroadcastState::Idle;
        if was_broadcasting != now_broadcasting {
            self.ui.broadcast_changed(now_broadcasting);
        }
        Ok(status)
    }
}

/// A UI that does nothing — for tests and headless use.
#[derive(Default)]
pub struct NoUi;

impl Ui for NoUi {
    fn show_pairing_prompt(&self, _request_id: &str) {}
    fn show_qr(&self) {}
    fn broadcast_changed(&self, _broadcasting: bool) {}
}

/// What viewers should see for a given presenter status.
pub fn viewer_mode(status: &Status) -> ViewerMode {
    match status.state {
        BroadcastState::Idle => ViewerMode::Ended,
        _ if status.blanked => ViewerMode::Blank,
        // Paused means "freeze on the last slide", even if the presenter then
        // leaves the slideshow to edit something.
        _ if status.paused => ViewerMode::Paused,
        BroadcastState::Waiting => ViewerMode::Waiting,
        BroadcastState::Live => ViewerMode::Live,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::pairing::Pairing;

    fn state() -> Arc<AppState> {
        AppState::new(Settings::default(), Pairing::load(None), Arc::new(NoUi))
    }

    #[test]
    fn viewers_follow_the_presenter() {
        let s = state();
        assert_eq!(s.hub.mode(), ViewerMode::Ended);
        s.run(Command::Start).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Waiting);
        s.update(|x| x.set_slideshow_found(true));
        assert_eq!(s.hub.mode(), ViewerMode::Live);
        s.run(Command::Pause).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Paused);
        s.run(Command::Blank).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Blank);
        s.run(Command::Unblank).unwrap();
        s.run(Command::Resume).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Live);
        s.run(Command::Stop).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Ended);
    }

    #[test]
    fn blank_works_before_the_slideshow_starts() {
        let s = state();
        s.run(Command::Start).unwrap();
        s.run(Command::Blank).unwrap();
        assert_eq!(s.hub.mode(), ViewerMode::Blank);
    }

    #[test]
    fn settings_apply_and_port_is_locked_while_live() {
        let s = state();
        let mut new = s.settings();
        new.pin = Some("1234".into());
        new.quality = crate::config::Quality::High;
        s.save_settings(new).unwrap();
        assert_eq!(s.settings().pin.as_deref(), Some("1234"));
        assert_eq!(
            *s.encode_settings.lock().unwrap(),
            EncodeSettings::from(&crate::config::Quality::High)
        );

        let port_rx = s.watch_port();
        s.run(Command::Start).unwrap();
        let mut moved = s.settings();
        moved.port = 9123;
        assert!(s.save_settings(moved.clone()).is_err());
        s.run(Command::Stop).unwrap();
        s.save_settings(moved).unwrap();
        assert_eq!(*port_rx.borrow(), 9123);

        let mut bad = s.settings();
        bad.pin = Some("12".into());
        assert!(s.save_settings(bad).is_err());
        assert_eq!(s.settings().pin.as_deref(), Some("1234"));
    }

    #[test]
    fn pause_stays_paused_when_the_slideshow_is_left() {
        let s = state();
        s.run(Command::Start).unwrap();
        s.update(|x| x.set_slideshow_found(true));
        s.hub.push(vec![7]);
        s.run(Command::Pause).unwrap();
        // Presenter leaves the slideshow to use PowerPoint.
        s.update(|x| x.set_slideshow_found(false));
        assert_eq!(s.hub.mode(), ViewerMode::Paused);
        let (frames, _) = s.hub.subscribe();
        assert_eq!(frames.borrow().as_ref().unwrap().jpeg.as_ref(), &[7]);
    }
}
