//! Broadcast session state.
//!
//! This is plain logic with no networking or UI, so it is easy to test. The
//! rest of the app asks the session what to do and shows its `Status`.

use rand::Rng;
use serde::Serialize;

/// Letters and digits that can't be confused when typed from a projector
/// (no 0/O, 1/I/L, 5/S, 2/Z).
const CODE_ALPHABET: &[u8] = b"346789ABCDEFGHJKMNPQRTUVWXY";
pub const CODE_LEN: usize = 4;

/// What the companion is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastState {
    /// Not broadcasting.
    Idle,
    /// Broadcasting, but no slideshow window is open yet.
    Waiting,
    /// Streaming the slideshow window.
    Live,
}

/// Mirrors `Status` in `shared/protocol.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: BroadcastState,
    pub paused: bool,
    pub blanked: bool,
    pub viewers: usize,
    pub url: Option<String>,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// Pause/blank/etc. only make sense while broadcasting.
    NotBroadcasting,
}

#[derive(Debug, Default)]
pub struct Session {
    /// The random code in the student link, e.g. "K7PQ". `None` when idle.
    code: Option<String>,
    slideshow_found: bool,
    paused: bool,
    blanked: bool,
    viewers: usize,
    problem: Option<String>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_broadcasting(&self) -> bool {
        self.code.is_some()
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Checks a code from a student's URL. Comparison ignores case so a
    /// typed "k7pq" still works.
    pub fn code_matches(&self, candidate: &str) -> bool {
        match &self.code {
            Some(code) => code.eq_ignore_ascii_case(candidate),
            None => false,
        }
    }

    /// Starts a broadcast with a fresh random code. Starting while already
    /// broadcasting keeps the current code (so the QR code stays valid).
    pub fn start(&mut self) {
        if self.code.is_none() {
            self.code = Some(random_code(&mut rand::rng()));
            self.paused = false;
            self.blanked = false;
        }
    }

    /// Ends the broadcast. The old code stops working immediately.
    pub fn stop(&mut self) {
        *self = Session {
            slideshow_found: self.slideshow_found,
            ..Session::default()
        };
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<(), CommandError> {
        self.require_broadcasting()?;
        self.paused = paused;
        Ok(())
    }

    pub fn set_blanked(&mut self, blanked: bool) -> Result<(), CommandError> {
        self.require_broadcasting()?;
        self.blanked = blanked;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn is_blanked(&self) -> bool {
        self.blanked
    }

    /// Called by the capture code when the slideshow window appears/disappears.
    pub fn set_slideshow_found(&mut self, found: bool) {
        self.slideshow_found = found;
    }

    pub fn set_viewers(&mut self, viewers: usize) {
        self.viewers = viewers;
    }

    /// A plain-words problem for the teacher (e.g. missing permission).
    pub fn set_problem(&mut self, problem: Option<String>) {
        self.problem = problem;
    }

    /// Builds the status shown in the add-in, toolbar and app window.
    /// `base_url` is like "http://192.168.1.20:8080".
    pub fn status(&self, base_url: Option<&str>) -> Status {
        let state = match (&self.code, self.slideshow_found) {
            (None, _) => BroadcastState::Idle,
            (Some(_), false) => BroadcastState::Waiting,
            (Some(_), true) => BroadcastState::Live,
        };
        let url = match (&self.code, base_url) {
            (Some(code), Some(base)) => Some(format!("{base}/{code}")),
            _ => None,
        };
        Status {
            state,
            paused: self.paused,
            blanked: self.blanked,
            viewers: if self.code.is_some() { self.viewers } else { 0 },
            url,
            problem: self.problem.clone(),
        }
    }

    fn require_broadcasting(&self) -> Result<(), CommandError> {
        if self.is_broadcasting() {
            Ok(())
        } else {
            Err(CommandError::NotBroadcasting)
        }
    }
}

/// Makes a random session code like "K7PQ".
pub fn random_code(rng: &mut impl Rng) -> String {
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        let s = Session::new();
        let st = s.status(Some("http://10.0.0.2:8080"));
        assert_eq!(st.state, BroadcastState::Idle);
        assert_eq!(st.url, None);
        assert!(!s.is_broadcasting());
    }

    #[test]
    fn start_creates_code_and_url() {
        let mut s = Session::new();
        s.start();
        let code = s.code().unwrap().to_string();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        let st = s.status(Some("http://10.0.0.2:8080"));
        assert_eq!(st.state, BroadcastState::Waiting);
        assert_eq!(st.url.unwrap(), format!("http://10.0.0.2:8080/{code}"));
    }

    #[test]
    fn start_twice_keeps_code() {
        let mut s = Session::new();
        s.start();
        let first = s.code().unwrap().to_string();
        s.start();
        assert_eq!(s.code().unwrap(), first);
    }

    #[test]
    fn live_only_when_slideshow_found() {
        let mut s = Session::new();
        s.set_slideshow_found(true);
        assert_eq!(s.status(None).state, BroadcastState::Idle);
        s.start();
        assert_eq!(s.status(None).state, BroadcastState::Live);
        s.set_slideshow_found(false);
        assert_eq!(s.status(None).state, BroadcastState::Waiting);
    }

    #[test]
    fn stop_invalidates_code_and_resets_flags() {
        let mut s = Session::new();
        s.start();
        let code = s.code().unwrap().to_string();
        s.set_paused(true).unwrap();
        s.set_blanked(true).unwrap();
        s.set_viewers(12);
        s.stop();
        assert!(!s.code_matches(&code));
        let st = s.status(None);
        assert_eq!(st.state, BroadcastState::Idle);
        assert!(!st.paused && !st.blanked);
        assert_eq!(st.viewers, 0);
    }

    #[test]
    fn new_broadcast_gets_new_code() {
        // Codes are random, so check that many restarts don't all repeat.
        let mut s = Session::new();
        let mut codes = std::collections::HashSet::new();
        for _ in 0..20 {
            s.start();
            codes.insert(s.code().unwrap().to_string());
            s.stop();
        }
        assert!(codes.len() > 1);
    }

    #[test]
    fn code_match_ignores_case() {
        let mut s = Session::new();
        s.start();
        let lower = s.code().unwrap().to_lowercase();
        assert!(s.code_matches(&lower));
        assert!(!s.code_matches("ZZZZZ"));
    }

    #[test]
    fn pause_and_blank_need_a_broadcast() {
        let mut s = Session::new();
        assert_eq!(s.set_paused(true), Err(CommandError::NotBroadcasting));
        assert_eq!(s.set_blanked(true), Err(CommandError::NotBroadcasting));
        s.start();
        assert!(s.set_paused(true).is_ok());
        assert!(s.status(None).paused);
        assert!(s.set_paused(false).is_ok());
        assert!(!s.status(None).paused);
    }
}
