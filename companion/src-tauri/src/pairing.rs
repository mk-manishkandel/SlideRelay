//! Pairing: how the PowerPoint add-in earns the right to control the app.
//!
//! 1. The add-in calls `POST /v1/pair`. We create a pending request and the app
//!    shows an "Allow PowerPoint to control SlideRelay?" window.
//! 2. The presenter clicks Allow. We create a random 256-bit token.
//! 3. The add-in polls `GET /v1/pair/{id}` and receives the token exactly once.
//!    It sends it as `Authorization: Bearer <token>` from then on.
//!
//! We only store a SHA-256 hash of each token on disk, and compare hashes in
//! constant time, so the token file is useless to anyone who reads it.

use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// How long the presenter has to click Allow.
const REQUEST_TTL: Duration = Duration::from_secs(120);
/// Limits how many prompts other software could pop up at once.
const MAX_PENDING: usize = 3;
/// Only remember this many paired add-ins (oldest is forgotten first).
const MAX_TOKENS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PairState {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// Mirrors `PairPoll` in `shared/protocol.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairPoll {
    pub state: PairState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug)]
struct Pending {
    created: Instant,
    state: PairState,
    /// Plain token, kept only until the add-in collects it.
    token: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TokenFile {
    /// Hex SHA-256 hashes of approved tokens.
    hashes: Vec<String>,
}

#[derive(Debug)]
pub struct Pairing {
    pending: HashMap<String, Pending>,
    hashes: Vec<String>,
    /// Where approved token hashes are saved. `None` in tests.
    path: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PairError {
    TooManyRequests,
    UnknownRequest,
}

impl Pairing {
    /// Loads previously approved tokens from `path` (if it exists).
    pub fn load(path: Option<PathBuf>) -> Self {
        let hashes = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<TokenFile>(&s).ok())
            .map(|f| f.hashes)
            .unwrap_or_default();
        Self {
            pending: HashMap::new(),
            hashes,
            path,
        }
    }

    /// Starts a new pairing request and returns its id.
    pub fn request(&mut self) -> Result<String, PairError> {
        self.expire_old();
        let open = self
            .pending
            .values()
            .filter(|p| p.state == PairState::Pending)
            .count();
        if open >= MAX_PENDING {
            return Err(PairError::TooManyRequests);
        }
        let id = random_hex(16);
        self.pending.insert(
            id.clone(),
            Pending {
                created: Instant::now(),
                state: PairState::Pending,
                token: None,
            },
        );
        Ok(id)
    }

    /// The presenter's answer from the "Allow?" window.
    pub fn answer(&mut self, id: &str, allow: bool) -> Result<(), PairError> {
        self.expire_old();
        let pending = self.pending.get_mut(id).ok_or(PairError::UnknownRequest)?;
        if pending.state != PairState::Pending {
            return Err(PairError::UnknownRequest);
        }
        if allow {
            let token = random_hex(32);
            self.hashes.push(hash(&token));
            if self.hashes.len() > MAX_TOKENS {
                self.hashes.remove(0);
            }
            pending.state = PairState::Approved;
            pending.token = Some(token);
            self.save();
        } else {
            pending.state = PairState::Denied;
        }
        Ok(())
    }

    /// The add-in asks "did the presenter answer yet?". A token is handed out
    /// only once; after that the request is forgotten.
    pub fn poll(&mut self, id: &str) -> PairPoll {
        self.expire_old();
        let Some(pending) = self.pending.get_mut(id) else {
            return PairPoll {
                state: PairState::Expired,
                token: None,
            };
        };
        let poll = PairPoll {
            state: pending.state,
            token: pending.token.take(),
        };
        if poll.state != PairState::Pending {
            self.pending.remove(id);
        }
        poll
    }

    /// Ids of requests still waiting for the presenter.
    pub fn open_requests(&self) -> Vec<String> {
        self.pending
            .iter()
            .filter(|(_, p)| p.state == PairState::Pending)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// True if `token` was approved earlier. Constant-time comparison.
    pub fn verify(&self, token: &str) -> bool {
        let candidate = hash(token);
        let mut found = 0u8;
        for h in &self.hashes {
            found |= h.as_bytes().ct_eq(candidate.as_bytes()).unwrap_u8();
        }
        found == 1
    }

    /// Forgets every paired add-in (used by "Disconnect PowerPoint" in settings).
    pub fn forget_all(&mut self) {
        self.hashes.clear();
        self.save();
    }

    fn expire_old(&mut self) {
        let now = Instant::now();
        self.pending
            .retain(|_, p| now.duration_since(p.created) < REQUEST_TTL);
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let file = TokenFile {
            hashes: self.hashes.clone(),
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string(&file) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::error!("could not save pairing tokens: {e}");
                }
            }
            Err(e) => tracing::error!("could not encode pairing tokens: {e}"),
        }
    }
}

fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// `bytes` random bytes from the OS-seeded CSPRNG, as hex.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_flow_hands_out_token_once() {
        let mut p = Pairing::load(None);
        let id = p.request().unwrap();
        assert_eq!(p.poll(&id).state, PairState::Pending);
        p.answer(&id, true).unwrap();
        let poll = p.poll(&id);
        assert_eq!(poll.state, PairState::Approved);
        let token = poll.token.unwrap();
        assert_eq!(token.len(), 64);
        assert!(p.verify(&token));
        // Second poll: the request is gone, no token leaks again.
        assert_eq!(p.poll(&id).token, None);
    }

    #[test]
    fn deny_gives_no_token() {
        let mut p = Pairing::load(None);
        let id = p.request().unwrap();
        p.answer(&id, false).unwrap();
        let poll = p.poll(&id);
        assert_eq!(poll.state, PairState::Denied);
        assert_eq!(poll.token, None);
    }

    #[test]
    fn unknown_tokens_are_rejected() {
        let p = Pairing::load(None);
        assert!(!p.verify("nope"));
        assert!(!p.verify(""));
    }

    #[test]
    fn limits_pending_requests() {
        let mut p = Pairing::load(None);
        for _ in 0..MAX_PENDING {
            p.request().unwrap();
        }
        assert_eq!(p.request(), Err(PairError::TooManyRequests));
    }

    #[test]
    fn cannot_answer_twice_or_unknown() {
        let mut p = Pairing::load(None);
        let id = p.request().unwrap();
        p.answer(&id, false).unwrap();
        assert_eq!(p.answer(&id, true), Err(PairError::UnknownRequest));
        assert_eq!(p.answer("bogus", true), Err(PairError::UnknownRequest));
    }

    #[test]
    fn tokens_survive_restart_and_forget_all() {
        let dir = std::env::temp_dir().join(format!("sliderelay-test-{}", random_hex(4)));
        let path = dir.join("tokens.json");
        let token = {
            let mut p = Pairing::load(Some(path.clone()));
            let id = p.request().unwrap();
            p.answer(&id, true).unwrap();
            p.poll(&id).token.unwrap()
        };
        let mut reloaded = Pairing::load(Some(path.clone()));
        assert!(reloaded.verify(&token));
        // The file holds only hashes, never the token itself.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains(&token));
        reloaded.forget_all();
        assert!(!Pairing::load(Some(path)).verify(&token));
        let _ = std::fs::remove_dir_all(dir);
    }
}
