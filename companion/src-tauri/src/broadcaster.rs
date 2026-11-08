//! Keeps capture in sync with the session.
//!
//! A background thread checks once a second (or immediately after a teacher
//! command): is a broadcast running? Is PowerPoint's slideshow open? Then it
//! starts, re-attaches or stops the window capture. This handles the
//! slideshow starting late, ending, and being recreated (e.g. the teacher
//! exits and restarts the slideshow).

use crate::capture::{self, ActiveCapture, Detection};
use crate::state::AppState;
use crate::stream::{self, EncodeSettings, FrameSink};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const CHECK_EVERY: Duration = Duration::from_secs(1);

/// Lets other code wake the broadcaster without waiting for the next tick.
#[derive(Default)]
pub struct Wake {
    flag: Mutex<bool>,
    cv: Condvar,
}

impl Wake {
    pub fn notify(&self) {
        *self.flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }

    fn wait(&self, timeout: Duration) {
        let guard = self.flag.lock().unwrap_or_else(|e| e.into_inner());
        let (mut guard, _) = self
            .cv
            .wait_timeout_while(guard, timeout, |woken| !*woken)
            .unwrap_or_else(|e| e.into_inner());
        *guard = false;
    }
}

/// Problems shown to the teacher, in plain words.
pub const PROBLEM_PERMISSION: &str =
    "SlideRelay needs Screen Recording permission. Open SlideRelay and click Fix this.";
pub const PROBLEM_PRESENTER_ONLY: &str =
    "Presenter View is on with only one screen, so students can't see your slides. Turn off Presenter View in the Slide Show tab.";
pub const PROBLEM_CAPTURE_FAILED: &str =
    "SlideRelay can't capture the slideshow. Try ending and restarting the slideshow.";

pub fn start(state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("sliderelay-broadcaster".into())
        .spawn(move || run(state))
        .expect("could not start broadcaster thread");
}

fn run(state: Arc<AppState>) {
    let mut current: Option<Box<dyn ActiveCapture>> = None;
    let mut sink: Option<FrameSink> = None;
    let mut capture_params: Option<(u8, EncodeSettings)> = None;
    let mut asked_permission = false;
    let mut last_logged_windows = String::new();

    loop {
        let broadcasting = state
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_broadcasting();

        if !broadcasting {
            if let Some(c) = current.take() {
                c.stop();
            }
            // Dropping the last sink ends the encoder thread.
            sink = None;
            asked_permission = false;
            state.update(|s| {
                s.set_slideshow_found(false);
                s.set_problem(None);
            });
            state.wake.wait(CHECK_EVERY);
            continue;
        }

        // Current settings (the teacher may change them mid-broadcast).
        let (max_fps, encode) = {
            let settings = state.settings.read().unwrap_or_else(|e| e.into_inner());
            (
                settings.max_fps.clamp(1, 30),
                EncodeSettings::from(&settings.quality),
            )
        };
        *state
            .encode_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = encode;

        if !capture::has_permission() {
            if !asked_permission {
                capture::request_permission();
                asked_permission = true;
            }
            set_found(&state, false, Some(PROBLEM_PERMISSION));
            state.wake.wait(CHECK_EVERY);
            continue;
        }

        let detection = match capture::find_powerpoint_windows() {
            Ok(windows) => {
                // Log what we see when it changes — invaluable for support.
                let summary = format!("{windows:?}");
                if summary != last_logged_windows {
                    tracing::info!("PowerPoint windows: {summary}");
                    last_logged_windows = summary;
                }
                capture::choose(&windows)
            }
            Err(capture::CaptureError::NoPermission) => {
                set_found(&state, false, Some(PROBLEM_PERMISSION));
                state.wake.wait(CHECK_EVERY);
                continue;
            }
            Err(e) => {
                tracing::warn!("could not list windows: {e}");
                Detection::NoSlideshow
            }
        };

        match detection {
            Detection::Found(window) => {
                let params_changed = capture_params != Some((max_fps, encode));
                let needs_restart = match &current {
                    Some(c) => c.window_id() != window.id || !c.is_alive() || params_changed,
                    None => true,
                };
                if needs_restart {
                    if let Some(c) = current.take() {
                        c.stop();
                    }
                    let frame_sink = sink
                        .get_or_insert_with(|| {
                            stream::start_encoder(state.hub.clone(), state.encode_settings.clone())
                        })
                        .clone();
                    match capture::start_capture(&window, frame_sink, max_fps, encode.max_width) {
                        Ok(c) => {
                            current = Some(c);
                            capture_params = Some((max_fps, encode));
                        }
                        Err(e) => {
                            tracing::error!("capture failed to start: {e}");
                            set_found(&state, false, Some(PROBLEM_CAPTURE_FAILED));
                            state.wake.wait(CHECK_EVERY);
                            continue;
                        }
                    }
                }
                *state
                    .slideshow_bounds
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(window.bounds);
                set_found(&state, true, None);
            }
            Detection::PresenterOnly => {
                if let Some(c) = current.take() {
                    c.stop();
                }
                set_found(&state, false, Some(PROBLEM_PRESENTER_ONLY));
            }
            Detection::NoSlideshow => {
                if let Some(c) = current.take() {
                    tracing::info!("slideshow ended");
                    c.stop();
                }
                set_found(&state, false, None);
            }
        }
        state.wake.wait(CHECK_EVERY);
    }
}

fn set_found(state: &AppState, found: bool, problem: Option<&str>) {
    state.update(|s| {
        s.set_slideshow_found(found);
        s.set_problem(problem.map(str::to_owned));
    });
}
