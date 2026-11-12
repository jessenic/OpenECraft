use super::*;
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
struct MemoryStore {
    values: RefCell<HashMap<String, String>>,
    failure: Option<CredentialError>,
}
impl CredentialStore for MemoryStore {
    fn load(&self, url: &str) -> Result<Option<String>, CredentialError> {
        if let Some(error) = self.failure { return Err(error); }
        Ok(target_name(url).and_then(|key| self.values.borrow().get(&key).cloned()))
    }
    fn save(&self, url: &str, token: &str) -> Result<bool, CredentialError> {
        if let Some(error) = self.failure { return Err(error); }
        if let Some(key) = target_name(url) { self.values.borrow_mut().insert(key, token.into()); return Ok(true); }
        Ok(false)
    }
    fn forget(&self, url: &str) -> Result<bool, CredentialError> {
        Ok(target_name(url).and_then(|key| self.values.borrow_mut().remove(&key)).is_some())
    }
}

const URL: &str = "https://dataportal.eplan.com/api";
fn token(connection: Connection) -> String {
    match connection { Connection::Eplan(e) => e.token, Connection::Parts(e) => e.token }
}

#[test]
fn a_gui_verified_pat_is_reused_and_rotated_without_a_mcp_token_argument() {
    let store = MemoryStore::default();
    let endpoint = EplanEndpoint { url: URL.into(), token: "first-pat".into() };
    remember_verified(Ok(ServerInfo::default()), Some(&endpoint), &store).unwrap();
    assert_eq!(token(connection_with_store(PortalKind::Eplan, URL, "", &store).unwrap()), "first-pat");
    store.save(URL, "replacement-pat").unwrap();
    assert_eq!(token(connection_with_store(PortalKind::Eplan, URL, "", &store).unwrap()), "replacement-pat");
    store.forget(URL).unwrap();
    assert_eq!(token(connection_with_store(PortalKind::Eplan, URL, "", &store).unwrap()), "");
}

#[test]
fn failed_checks_never_overwrite_the_working_pat() {
    let store = MemoryStore::default();
    store.save(URL, "working").unwrap();
    let endpoint = EplanEndpoint { url: URL.into(), token: "bad".into() };
    assert!(remember_verified(Err("denied".into()), Some(&endpoint), &store).is_err());
    assert_eq!(store.load(URL).unwrap().as_deref(), Some("working"));
    remember_verified(Ok(ServerInfo::default()), None, &store).unwrap();
    assert_eq!(store.load(URL).unwrap().as_deref(), Some("working"));
}

#[test]
fn saved_tokens_do_not_follow_hosts_paths_or_http_and_explicit_tokens_win() {
    let store = MemoryStore::default();
    store.save(URL, "saved-pat").unwrap();
    for url in ["https://dataportal.eplan.com.evil.test/api", "https://other.test/api", "https://dataportal.eplan.com/another-api", "http://dataportal.eplan.com/api"] {
        assert_eq!(token(connection_with_store(PortalKind::Eplan, url, "", &store).unwrap()), "");
    }
    assert_eq!(token(connection_with_store(PortalKind::OpenECraft, URL, "", &store).unwrap()), "");
    assert_eq!(token(connection_with_store(PortalKind::Eplan, URL, "explicit", &store).unwrap()), "explicit");
    assert_eq!(target_name(URL), target_name("HTTPS://DATAPORTAL.EPLAN.COM:443/api/"));
    for url in ["https://user:password@dataportal.eplan.com/api", "https://dataportal.eplan.com/api?token=secret", "https://dataportal.eplan.com/api#secret"] { assert!(target_name(url).is_none()); }
}

#[test]
fn credential_failures_are_propagated_and_debug_does_not_reveal_secrets() {
    let store = MemoryStore { failure: Some(CredentialError { operation: Operation::Read, code: 1312 }), ..Default::default() };
    let error = connection_with_store(PortalKind::Eplan, URL, "", &store).unwrap_err();
    assert!(error.contains("1312"));
    let connection = connection_with_store(PortalKind::Eplan, URL, "do-not-print-this-pat", &store).unwrap();
    assert!(!format!("{connection:?}").contains("do-not-print-this-pat"));
}

#[cfg(windows)]
#[test]
fn windows_store_roundtrip_uses_a_unique_dummy_entry_and_removes_it() {
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let url = format!("https://openecraft-credential-test-{}-{unique}.invalid/api", std::process::id());
    struct Cleanup(String);
    impl Drop for Cleanup { fn drop(&mut self) { let _ = SystemCredentialStore.forget(&self.0); } }
    let _cleanup = Cleanup(url.clone());
    assert!(SystemCredentialStore.load(&url).unwrap().is_none());
    assert!(SystemCredentialStore.save(&url, "dummy-pat").unwrap());
    assert_eq!(SystemCredentialStore.load(&url).unwrap().as_deref(), Some("dummy-pat"));
    assert!(SystemCredentialStore.save(&url, "rotated-dummy-pat").unwrap());
    assert_eq!(SystemCredentialStore.load(&url).unwrap().as_deref(), Some("rotated-dummy-pat"));
    assert!(SystemCredentialStore.forget(&url).unwrap());
    assert!(SystemCredentialStore.load(&url).unwrap().is_none());
    assert!(!SystemCredentialStore.forget(&url).unwrap());
}

#[test]
fn optional_save_failure_does_not_invalidate_a_working_connection() {
    let store = MemoryStore { failure: Some(CredentialError { operation: Operation::Save, code: 1312 }), ..Default::default() };
    let endpoint = EplanEndpoint { url: URL.into(), token: "session-fixture".into() };
    let (info, warning) = remember_verified(Ok(ServerInfo { parts: 42, ..Default::default() }), Some(&endpoint), &store).unwrap();
    assert_eq!(info.parts, 42);
    assert!(warning.unwrap().contains("Could not save"));
    // No consent (None) means that the failing store is never called.
    let (_, warning) = remember_verified(Ok(ServerInfo::default()), None, &store).unwrap();
    assert!(warning.is_none());
}
