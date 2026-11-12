//! Shared EPLAN PATs. Only the OS credential store holds persistent secrets;
//! project files, settings, MCP replies and URLs never contain the saved PAT.

use crate::{Connection, EplanEndpoint, PortalKind};
use openecraft_i18n::tr_fmt;
use openecraft_parts::api::{encode_component, ServerInfo};

#[cfg(windows)]
#[path = "credentials/windows.rs"]
mod native;

#[cfg(not(windows))]
mod native {
    use super::CredentialError;
    pub fn load(_: &str) -> Result<Option<String>, CredentialError> { Ok(None) }
    pub fn save(_: &str, _: &str) -> Result<bool, CredentialError> { Ok(false) }
    pub fn forget(_: &str) -> Result<bool, CredentialError> { Ok(false) }
}

/// Errors deliberately contain no credential, URL or OS-provided error detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialError {
    pub operation: Operation,
    pub code: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Read, Save, Forget }

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self.operation {
            Operation::Read => tr_fmt!("Could not read the shared Data Portal token (Windows error {})", self.code),
            Operation::Save => tr_fmt!("Could not save the shared Data Portal token (Windows error {})", self.code),
            Operation::Forget => tr_fmt!("Could not remove the shared Data Portal token (Windows error {})", self.code),
        };
        f.write_str(&message)
    }
}

impl std::error::Error for CredentialError {}

/// Backend abstraction also keeps tests independent of the user's saved credentials.
pub trait CredentialStore {
    fn load(&self, url: &str) -> Result<Option<String>, CredentialError>;
    fn save(&self, url: &str, token: &str) -> Result<bool, CredentialError>;
    fn forget(&self, url: &str) -> Result<bool, CredentialError>;
}

/// Windows Credential Manager, current user, this computer. Other platforms retain
/// session-only tokens; there is deliberately no plaintext fallback.
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    pub fn available_for(url: &str) -> bool { cfg!(windows) && target_name(url).is_some() }
}

impl CredentialStore for SystemCredentialStore {
    fn load(&self, url: &str) -> Result<Option<String>, CredentialError> {
        match target_name(url) { Some(target) => native::load(&target), None => Ok(None) }
    }
    fn save(&self, url: &str, token: &str) -> Result<bool, CredentialError> {
        match target_name(url).filter(|_| !token.trim().is_empty()) {
            Some(target) => native::save(&target, token.trim()),
            None => Ok(false),
        }
    }
    fn forget(&self, url: &str) -> Result<bool, CredentialError> {
        match target_name(url) { Some(target) => native::forget(&target), None => Ok(false) }
    }
}

/// Endpoint-specific names prevent a saved PAT following URL changes. Scheme and
/// host case, an explicit :443 and trailing slashes are equivalent; paths are not.
fn target_name(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, host, port) = crate::origin(url)?;
    if scheme != "https" || url.contains(['?', '#', '\\', '\0']) { return None; }
    let (_, rest) = url.split_once("://")?;
    let path = rest.find('/').map_or("", |i| &rest[i..]).trim_end_matches('/');
    Some(format!("OpenECraft/EPLAN/PAT/v1/{}/{port}/{}", encode_component(&host), encode_component(path)))
}

/// Explicit session/CLI/environment credentials win. Otherwise read the shared
/// value afresh on each request, so GUI replacements and removals take effect.
pub fn connection_with_store(kind: PortalKind, url: &str, token: &str, store: &(impl CredentialStore + ?Sized)) -> Result<Connection, String> {
    if kind != PortalKind::Eplan || !token.trim().is_empty() || target_name(url).is_none() {
        return Ok(Connection::new(kind, url, token));
    }
    let saved = store.load(url).map_err(|e| e.to_string())?.unwrap_or_default();
    Ok(Connection::new(kind, url, &saved))
}

/// A successful connection remains successful if optional persistence fails.
/// The GUI shows the warning separately; only an explicit opt-in calls this helper.
pub fn remember_verified(info: Result<ServerInfo, String>, endpoint: Option<&EplanEndpoint>, store: &impl CredentialStore) -> Result<(ServerInfo, Option<String>), String> {
    let info = info?;
    let warning = endpoint.filter(|e| !e.token.trim().is_empty())
        .and_then(|e| store.save(&e.url, &e.token).err()).map(|e| e.to_string());
    Ok((info, warning))
}

#[cfg(test)]
mod tests;
