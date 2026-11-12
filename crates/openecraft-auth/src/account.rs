//! Password hashes, session tokens (stored as SHA-256 with their start) and content ids.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// Sessions kept per user; the oldest is dropped when a new one starts.
pub const MAX_SESSIONS: usize = 20;

/// A session ends this long after the sign-in (30 days).
pub const SESSION_LIFETIME_SECS: u64 = 30 * 24 * 3600;

/// Shortest password the servers accept for local accounts.
pub const MIN_PASSWORD_LEN: usize = 8;

/// A password a server may set for a local account: long enough, not only blanks.
pub fn check_password(password: &str) -> Result<(), String> {
    if password.trim().chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("the password needs at least {MIN_PASSWORD_LEN} characters"));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

/// Checks the password of an account; without account (`None`) a password is checked
/// all the same and fails, so that the answer takes as long and does not tell which
/// user names exist.
pub fn verify_account_password(password: &str, hash: Option<&str>) -> bool {
    static DUMMY: OnceLock<String> = OnceLock::new();
    match hash {
        Some(hash) => verify_password(password, hash),
        None => {
            let dummy = DUMMY.get_or_init(|| hash_password(&new_token()).unwrap_or_default());
            verify_password(password, dummy);
            false
        }
    }
}

/// A random token (256 bit, hex).
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("no random numbers available");
    hex(&bytes)
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Seconds since 1970.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A session of an account: the hash of its token and when it started. Sessions
/// stored as a plain hash (before sessions ended) count as expired.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredSession")]
pub struct SessionKey {
    pub hash: String,
    /// Seconds since 1970.
    pub started: u64,
}

impl SessionKey {
    /// The session has not expired at `now`.
    pub fn alive(&self, now: u64) -> bool {
        now.saturating_sub(self.started) < SESSION_LIFETIME_SECS
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredSession {
    Hash(String),
    Key { hash: String, started: u64 },
}

impl From<StoredSession> for SessionKey {
    fn from(s: StoredSession) -> Self {
        match s {
            StoredSession::Hash(hash) => SessionKey { hash, started: 0 },
            StoredSession::Key { hash, started } => SessionKey { hash, started },
        }
    }
}

/// Starts a session in a list of sessions at `now` (seconds since 1970); drops the
/// expired ones. Returns the token for the client.
pub fn start_session(sessions: &mut Vec<SessionKey>, now: u64) -> String {
    let token = new_token();
    sessions.retain(|s| s.alive(now));
    sessions.push(SessionKey { hash: sha256_hex(token.as_bytes()), started: now });
    if sessions.len() > MAX_SESSIONS {
        sessions.remove(0);
    }
    token
}

/// The token belongs to a session that has not expired at `now`.
pub fn has_session(sessions: &[SessionKey], token: &str, now: u64) -> bool {
    let hash = sha256_hex(token.as_bytes());
    sessions.iter().any(|s| openecraft_parts::api::token_matches(&s.hash, &hash) && s.alive(now))
}

pub fn end_session(sessions: &mut Vec<SessionKey>, token: &str) -> bool {
    let hash = sha256_hex(token.as_bytes());
    let before = sessions.len();
    sessions.retain(|s| !openecraft_parts::api::token_matches(&s.hash, &hash));
    sessions.len() != before
}

#[cfg(test)]
mod tests;
