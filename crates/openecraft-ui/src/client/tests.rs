use super::*;

#[test]
fn poll_updates_status_and_info() {
    let mut client = ServerClient { pending: 2, ..Default::default() };
    {
        let mut inbox = client.inbox.lock().unwrap();
        inbox.push(Reply::Info(Ok(ServerInfo { name: "srv".into(), writable: true, ..Default::default() })));
        inbox.push(Reply::Deleted("A".into(), Ok(())));
    }
    let replies = client.poll();
    assert_eq!(replies.len(), 2);
    assert!(!client.busy());
    assert!(client.writable());
    assert_eq!(client.status, "Deleted part A");
    assert!(!client.failed());

    client.inbox.lock().unwrap().push(Reply::Info(Err("connection failed: refused".into())));
    client.poll();
    assert!(client.failed());
    assert!(client.info.is_none());

    client.inbox.lock().unwrap().push(Reply::Deleted("B".into(), Ok(())));
    client.poll();
    assert!(!client.failed(), "a later success clears the error");
}

#[test]
fn signing_in_and_out_changes_the_token() {
    use openecraft_parts::api::Session;
    use openecraft_parts::portal::{Role, User};
    let mut client = ServerClient { pending: 3, token: "old".into(), info: Some(ServerInfo::default()), ..Default::default() };
    let user = User { name: "ann".into(), role: Role::Editor, manufacturer: String::new(), ..Default::default() };
    client.inbox.lock().unwrap().push(Reply::SignedIn(Ok(Session { token: "session".into(), user: user.clone(), ..Default::default() })));
    client.poll();
    assert_eq!(client.token, "session");
    assert_eq!(client.user(), Some(&user));
    client.inbox.lock().unwrap().push(Reply::SignedIn(Err("wrong user name or password".into())));
    client.poll();
    assert_eq!(client.token, "session", "a failed sign-in keeps the session");
    assert!(client.failed());
    client.inbox.lock().unwrap().push(Reply::SignedOut);
    client.poll();
    assert!(client.token.is_empty() && client.user().is_none());
}

#[test]
fn each_kind_keeps_its_address_and_token() {
    let mut client = ServerClient { token: "admin".into(), info: Some(ServerInfo::default()), ..Default::default() };
    client.set_kind(PortalKind::Eplan);
    assert_eq!((client.url.as_str(), client.token.as_str()), (openecraft_client::eplan::EPLAN_URL, ""));
    assert!(client.read_only() && client.info.is_none());
    client.token = "pat".into();
    client.set_url_of(PortalKind::OpenECraft, "http://other:8090".into());
    assert_eq!(client.url_of(PortalKind::OpenECraft), "http://other:8090");
    client.set_kind(PortalKind::OpenECraft);
    assert_eq!((client.url.as_str(), client.token.as_str()), ("http://other:8090", "admin"));
    client.set_kind(PortalKind::Eplan);
    assert_eq!(client.token, "pat");
}

#[test]
fn the_eplan_data_portal_refuses_changes() {
    let ctx = egui::Context::default();
    let mut client = ServerClient::default();
    client.set_kind(PortalKind::Eplan);
    client.catalogs(&ctx);
    client.save(&ctx, &Part::default());
    let replies = client.poll();
    assert!(matches!(replies[0], Reply::Catalogs(Err(_))) && matches!(replies[1], Reply::Saved(Err(_))));
    assert!(client.failed() && !client.busy());
    assert_eq!(client.file_link("download/dxf_data/part/1"), None);
}

fn checked(client: &ServerClient, warning: Option<String>) -> Reply {
    Reply::Checked(Box::new(CheckedConnection {
        selection: client.connection(),
        generation: client.check_generation,
        result: Ok(ServerInfo { parts: 17, ..Default::default() }),
        warning,
    }))
}

const TEST_URL: &str = "https://portal.test/api";

#[derive(Default)]
struct MemoryCredentials {
    tokens: Mutex<HashMap<String, String>>,
    reads: std::sync::atomic::AtomicUsize,
    fail: std::sync::atomic::AtomicBool,
}
impl MemoryCredentials {
    fn saved() -> Self {
        let store = Self::default();
        store.tokens.lock().unwrap().insert(TEST_URL.into(), "saved-fixture".into());
        store
    }
    fn reads(&self) -> usize { self.reads.load(std::sync::atomic::Ordering::SeqCst) }
}
impl CredentialStore for MemoryCredentials {
    fn load(&self, url: &str) -> Result<Option<String>, credentials::CredentialError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(credentials::CredentialError { operation: credentials::Operation::Read, code: 1312 });
        }
        Ok(self.tokens.lock().unwrap().get(url).cloned())
    }
    fn save(&self, _: &str, _: &str) -> Result<bool, credentials::CredentialError> { panic!("automatic loading must not save") }
    fn forget(&self, _: &str) -> Result<bool, credentials::CredentialError> { panic!("automatic loading must not delete") }
}

fn portal_client() -> ServerClient {
    let mut client = ServerClient::default();
    client.set_kind(PortalKind::Eplan);
    client.url = TEST_URL.into();
    client
}

#[test]
fn session_cache_is_endpoint_specific_and_explicit_tokens_win() {
    let mut client = portal_client();
    let store = MemoryCredentials::saved();
    assert!(!client.remember_pat);
    let first = client.request_credentials();
    assert_eq!(store.reads(), 0, "preparing a request on the UI thread does not read credentials");
    let saved = first.resolve(&store).unwrap();
    assert_eq!(saved, Connection::new(client.kind, &client.url, "saved-fixture"));
    assert_eq!(client.request_credentials().resolve(&store).unwrap(), saved);
    assert_eq!(store.reads(), 1);
    client.token = "explicit-fixture".into();
    assert_eq!(client.request_credentials().resolve(&store).unwrap(), client.connection());
    assert_eq!(store.reads(), 1, "explicit tokens bypass the store");
    client.token.clear();
    client.url = "https://other.test/api".into();
    assert_eq!(client.request_credentials().resolve(&store).unwrap(), client.connection());
    assert_eq!(store.reads(), 2);
    assert_eq!(first.resolve(&store).unwrap(), saved, "old workers retain only their own selection");
    client.set_kind(PortalKind::OpenECraft);
    assert_eq!(client.request_credentials().resolve(&store).unwrap(), client.connection());
    assert_eq!(store.reads(), 2, "the OpenECraft server does not read EPLAN credentials");
}

#[test]
fn save_warning_keeps_info_and_a_usable_session_without_exposing_the_token() {
    let other = ServerClient { token: "private-fixture".into(), ..Default::default() };
    assert!(!format!("{:?}", checked(&other, None)).contains("private-fixture"));
    let mut client = ServerClient::default();
    client.set_kind(PortalKind::Eplan);
    client.token = "private-fixture".into();
    let reply = checked(&client, Some("Could not save the shared Data Portal token (Windows error 1312)".into()));
    assert!(!format!("{reply:?}").contains("private-fixture"));
    client.inbox.lock().unwrap().push(reply);
    let replies = client.poll();
    assert!(matches!(replies.as_slice(), [Reply::Info(Ok(_))]), "parts manager must still receive the Info notification");
    assert!(!client.failed());
    assert_eq!(client.info.as_ref().unwrap().parts, 17);
    assert!(client.status.contains("Connected:") && client.status.contains("Could not save"));
    assert!(!client.status.contains("private-fixture"));
    assert_eq!(client.request_credentials().resolve(&MemoryCredentials::default()).unwrap(), Connection::new(client.kind, &client.url, "private-fixture"));
}

struct ForgetStore(bool);
impl CredentialStore for ForgetStore {
    fn load(&self, _: &str) -> Result<Option<String>, credentials::CredentialError> { panic!("ordinary GUI requests must use the cache") }
    fn save(&self, _: &str, _: &str) -> Result<bool, credentials::CredentialError> { panic!("forget must not save") }
    fn forget(&self, _: &str) -> Result<bool, credentials::CredentialError> { Ok(self.0) }
}

#[test]
fn forgetting_reports_absence_and_invalidates_pending_checks() {
    for removed in [false, true] {
        let mut client = portal_client();
        let previous = client.request_credentials();
        previous.resolve(&MemoryCredentials::saved()).unwrap();
        let old = checked(&client, None);
        client.forget_saved_pat_with(&ForgetStore(removed));
        assert_eq!(client.status, if removed { "Saved Data Portal token removed" } else { "No saved Data Portal token for this endpoint" });
        client.inbox.lock().unwrap().push(old);
        previous.resolve(&MemoryCredentials::saved()).unwrap();
        client.poll();
        assert!(client.info.is_none() && client.cached_connection.is_none());
        assert_eq!(client.request_credentials().resolve(&MemoryCredentials::default()).unwrap(), client.connection());
    }
}

#[test]
fn pending_check_cannot_follow_a_changed_endpoint_or_explicit_token() {
    for change_token in [false, true] {
        let mut client = ServerClient::default();
        client.set_kind(PortalKind::Eplan);
        let old = checked(&client, None);
        if change_token { client.token = "replacement-fixture".into(); } else { client.url = "https://other.test/api".into(); }
        client.inbox.lock().unwrap().push(old);
        client.poll();
        assert!(client.info.is_none() && client.cached_connection.is_none());
    }
}

#[test]
fn missing_tokens_and_read_errors_are_cached_until_a_refresh() {
    for fail in [false, true] {
        let mut client = portal_client();
        let store = MemoryCredentials::default();
        store.fail.store(fail, std::sync::atomic::Ordering::SeqCst);
        let result = client.request_credentials().resolve(&store);
        if fail { assert!(result.as_ref().unwrap_err().contains("1312")); }
        else { assert_eq!(result.as_ref().unwrap(), &client.connection()); }
        assert_eq!(client.request_credentials().resolve(&store), result);
        assert_eq!(store.reads(), 1);
        store.fail.store(false, std::sync::atomic::Ordering::SeqCst);
        store.tokens.lock().unwrap().insert(TEST_URL.into(), "replacement-fixture".into());
        assert_eq!(client.request_credentials().resolve(&store), result);
        let refreshed = client.refresh_credentials().resolve(&store).unwrap();
        assert_eq!(refreshed, Connection::new(PortalKind::Eplan, TEST_URL, "replacement-fixture"));
        assert_eq!(store.reads(), 2);
    }
}

#[test]
fn the_first_download_waits_off_thread_and_concurrent_requests_share_one_lookup() {
    use std::sync::mpsc;
    use std::time::Duration;
    struct GatedStore {
        memory: MemoryCredentials,
        entered: mpsc::Sender<std::thread::ThreadId>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl CredentialStore for GatedStore {
        fn load(&self, url: &str) -> Result<Option<String>, credentials::CredentialError> {
            self.entered.send(std::thread::current().id()).unwrap();
            self.release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
            self.memory.load(url)
        }
        fn save(&self, _: &str, _: &str) -> Result<bool, credentials::CredentialError> { panic!("must not save") }
        fn forget(&self, _: &str) -> Result<bool, credentials::CredentialError> { panic!("must not forget") }
    }
    let mut client = portal_client();
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let store = Arc::new(GatedStore { memory: MemoryCredentials::saved(), entered, release: Mutex::new(gate) });
    let (done, completed) = mpsc::channel();
    for _ in 0..2 {
        let cache = client.request_credentials();
        let store = store.clone();
        let done = done.clone();
        crate::io::background(move || {
            let result = cache.call(store.as_ref(), |c| c.file("download/dxf_data/part/42"))
                .and_then(|call| call.run(|request| {
                    assert_eq!(request.method, ehttp::Method::POST);
                    assert_eq!(request.headers.get("Authorization"), Some("Bearer PAT:saved-fixture"));
                    assert_eq!(request.body, b"42");
                    Ok(ehttp::Response { url: request.url.clone(), ok: true, status: 200, status_text: "OK".into(), headers: Default::default(), bytes: b"fixture".to_vec() })
                }));
            done.send(result).unwrap();
        });
    }
    assert_ne!(started.recv_timeout(Duration::from_secs(5)).unwrap(), std::thread::current().id());
    // The UI can prepare another request while the OS read is deliberately blocked.
    let cache = client.request_credentials();
    assert!(completed.try_recv().is_err(), "no unauthenticated request may race ahead");
    release.send(()).unwrap();
    for _ in 0..2 { assert_eq!(completed.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), b"fixture"); }
    assert_eq!(store.memory.reads(), 1);
    assert_eq!(cache.resolve(store.as_ref()).unwrap(), Connection::new(PortalKind::Eplan, TEST_URL, "saved-fixture"));
    assert!(started.try_recv().is_err(), "the second request reuses the first lookup");
}

#[test]
fn credential_read_failure_prevents_building_an_unauthenticated_request() {
    let mut client = portal_client();
    let store = MemoryCredentials::default();
    store.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let error = client.request_credentials().call::<Vec<u8>>(&store, |_| panic!("must not construct a request after a credential error"));
    assert!(error.err().unwrap().contains("1312"));
}
