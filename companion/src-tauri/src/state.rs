//! The app's shared state, used by the control API, the Tauri windows and
//! (later) the capture and student servers.
//!
//! It doesn't depend on Tauri: anything UI-related goes through the `Ui`
//! trait, so tests can swap in a fake UI.

use crate::config::Settings;
use crate::pairing::Pairing;
use crate::session::{CommandError, Session, Status};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::watch;

/// The things the core logic needs the UI to do.
pub trait Ui: Send + Sync {
    /// Ask the teacher "Allow PowerPoint to control SlideRelay?".
    fn show_pairing_prompt(&self, request_id: &str);
    /// Show (or bring to front) the QR code window.
    fn show_qr(&self);
    /// The broadcast started or stopped (to show/hide the toolbar, etc.).
    fn broadcast_changed(&self, broadcasting: bool);
}

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
    /// "http://<lan-ip>:<port>" once the student server is running.
    pub base_url: RwLock<Option<String>>,
    status_tx: watch::Sender<Status>,
    ui: Arc<dyn Ui>,
}

impl AppState {
    pub fn new(settings: Settings, pairing: Pairing, ui: Arc<dyn Ui>) -> Arc<Self> {
        let session = Session::new();
        let (status_tx, _) = watch::channel(session.status(None));
        Arc::new(Self {
            session: Mutex::new(session),
            pairing: Mutex::new(pairing),
            settings: RwLock::new(settings),
            base_url: RwLock::new(None),
            status_tx,
            ui,
        })
    }

    pub fn ui(&self) -> &dyn Ui {
        self.ui.as_ref()
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
        let (result, status) = {
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let result = f(&mut session);
            let base = self.base_url.read().unwrap_or_else(|e| e.into_inner());
            (result, session.status(base.as_deref()))
        };
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

    /// Runs a teacher command and returns the new status.
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
        let status = self.status();
        let now_broadcasting = status.state != crate::session::BroadcastState::Idle;
        if was_broadcasting != now_broadcasting {
            self.ui.broadcast_changed(now_broadcasting);
            if now_broadcasting {
                // Show the QR code as soon as the broadcast starts.
                self.ui.show_qr();
            }
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
