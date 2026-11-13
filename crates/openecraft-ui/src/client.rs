//! Client for the global parts server (`openecraft-parts-server`, the data portal) or
//! the EPLAN Data Portal.
//! Requests run in the background (thread on desktop, fetch in the browser); results
//! are collected with [`ServerClient::poll`]. The requests themselves are built by
//! `openecraft_client`.

use openecraft_i18n::{tr, tr_fmt};
use openecraft_client::{check, decode, decode_found, ehttp, Call, Connection, Endpoint, PortalKind};
use openecraft_client::credentials::{self, CredentialStore, SystemCredentialStore};
use openecraft_parts::api::{Accessories, ImportResult, PartList, PartQuery, ServerInfo, Session, UserUpdate};
use openecraft_parts::portal::{AttachmentKind, Catalog, HistoryEntry, SearchResult, User};
use openecraft_parts::Part;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Result of a finished request.
#[derive(Debug)]
pub enum Reply {
    Info(Result<ServerInfo, String>),
    Checked(Box<CheckedConnection>),
    Parts(Result<PartList, String>),
    Imported(Result<ImportResult, String>),
    /// A part was saved, or a file attached to / removed from it.
    Saved(Result<Box<Part>, String>),
    /// Part number and result.
    Deleted(String, Result<(), String>),
    Search(Result<SearchResult, String>),
    /// Server versions of local parts (update check).
    Versions(Result<SearchResult, String>),
    /// Part number and its changes.
    History(String, Result<Vec<HistoryEntry>, String>),
    /// Part number and the whole part (with its models) for the details of the data
    /// portal; `None` if the server does not know it.
    Detail(String, Result<Option<Box<Part>>, String>),
    /// Part number, its accessories and the parts it fits.
    Accessories(String, Result<Accessories, String>),
    Catalogs(Result<Vec<Catalog>, String>),
    CatalogSaved(Result<Catalog, String>),
    CatalogDeleted(String, Result<(), String>),
    SignedIn(Result<Session, String>),
    SignedOut,
    Favorites(Result<Vec<String>, String>),
    Users(Result<Vec<User>, String>),
    UserSaved(Result<User, String>),
    UserDeleted(String, Result<(), String>),
    /// File name, purpose given with the request and the content.
    File(String, FileUse, Result<Vec<u8>, String>),
    /// Name and JSON text of a library; `None` if the server has none of that name.
    Library(String, Result<Option<String>, String>),
    LibrarySaved(String, Result<(), String>),
}

/// A background connection check and its endpoint-specific session credential.
/// Credentials are excluded from Debug, for both kinds of server.
pub struct CheckedConnection {
    selection: Connection,
    generation: u64,
    result: Result<ServerInfo, String>,
    warning: Option<String>,
}

impl std::fmt::Debug for CheckedConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckedConnection").field("generation", &self.generation)
            .field("result", &self.result).field("warning", &self.warning).finish_non_exhaustive()
    }
}

/// What a downloaded file is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileUse {
    /// Save it as a file.
    Save,
    /// Merge it into the macro library.
    Macros,
}

impl Reply {
    pub fn error(&self) -> Option<&str> {
        match self {
            Reply::Checked(check) => check.result.as_ref().err().map(String::as_str),
            Reply::Info(Err(e))
            | Reply::Parts(Err(e))
            | Reply::Imported(Err(e))
            | Reply::Saved(Err(e))
            | Reply::Deleted(_, Err(e))
            | Reply::Search(Err(e))
            | Reply::Versions(Err(e))
            | Reply::History(_, Err(e))
            | Reply::Detail(_, Err(e))
            | Reply::Accessories(_, Err(e))
            | Reply::Catalogs(Err(e))
            | Reply::CatalogSaved(Err(e))
            | Reply::CatalogDeleted(_, Err(e))
            | Reply::SignedIn(Err(e))
            | Reply::Favorites(Err(e))
            | Reply::Users(Err(e))
            | Reply::UserSaved(Err(e))
            | Reply::UserDeleted(_, Err(e))
            | Reply::File(_, _, Err(e))
            | Reply::Library(_, Err(e))
            | Reply::LibrarySaved(_, Err(e)) => Some(e),
            _ => None,
        }
    }

    /// Text for a status line.
    pub fn describe(&self) -> String {
        if let Some(e) = self.error() {
            return tr_fmt!("Parts server: {}", e);
        }
        match self {
            Reply::Checked(check) => {
                let status = Reply::Info(check.result.clone()).describe();
                match &check.warning { Some(warning) => format!("{status}; {warning}"), None => status }
            }
            Reply::Info(Ok(info)) => {
                let access = match &info.user {
                    Some(u) => tr_fmt!("signed in as {} ({})", u.name, tr(u.role.title())),
                    None if info.writable => tr("changes with token").to_string(),
                    None => tr("read-only").to_string(),
                };
                tr_fmt!("Connected: {} {} – {} parts, revision {}, {}", info.name, info.version, info.parts, info.revision, access)
            }
            Reply::Parts(Ok(list)) => tr_fmt!("Loaded {} parts (revision {})", list.parts.len(), list.revision),
            Reply::Imported(Ok(r)) => tr_fmt!("Uploaded {} parts, server revision {}", r.imported, r.revision),
            Reply::Saved(Ok(p)) => tr_fmt!("Saved part {}", p.number),
            Reply::Deleted(number, Ok(())) => tr_fmt!("Deleted part {}", number),
            Reply::Search(Ok(r)) => tr_fmt!("{} parts found", r.total),
            Reply::Versions(Ok(r)) => tr_fmt!("Compared with {} parts of the server", r.total),
            Reply::History(number, Ok(h)) => tr_fmt!("{} changes of part {}", h.len(), number),
            Reply::Catalogs(Ok(c)) => tr_fmt!("{} catalogs", c.len()),
            Reply::CatalogSaved(Ok(c)) => tr_fmt!("Saved catalog {}", c.name),
            Reply::CatalogDeleted(id, Ok(())) => tr_fmt!("Deleted catalog {}", id),
            Reply::SignedIn(Ok(s)) => tr_fmt!("Signed in as {} ({})", s.user.name, tr(s.user.role.title())),
            Reply::SignedOut => tr("Signed out").to_string(),
            Reply::Favorites(Ok(f)) => tr_fmt!("{} favorites", f.len()),
            Reply::Users(Ok(u)) => tr_fmt!("{} users", u.len()),
            Reply::UserSaved(Ok(u)) => tr_fmt!("Saved user {}", u.name),
            Reply::UserDeleted(name, Ok(())) => tr_fmt!("Deleted user {}", name),
            Reply::File(name, _, Ok(data)) => tr_fmt!("Downloaded {} ({} bytes)", name, data.len()),
            Reply::Library(name, Ok(Some(_))) => tr_fmt!("Loaded the library {}", name),
            Reply::Library(name, Ok(None)) => tr_fmt!("The parts server has no library {} yet", name),
            Reply::LibrarySaved(name, Ok(())) => tr_fmt!("Saved the library {}", name),
            _ => String::new(),
        }
    }
}

/// One lazy credential lookup shared by concurrent requests for this selection.
/// Only background workers initialize it, including the Credential Manager read.
#[derive(Clone)]
struct CredentialCache {
    selection: Connection,
    resolved: Arc<OnceLock<Result<Connection, String>>>,
}

impl CredentialCache {
    fn resolve(&self, store: &impl CredentialStore) -> Result<Connection, String> {
        let Connection::Eplan(endpoint) = &self.selection else { return Ok(self.selection.clone()); };
        if !endpoint.token.trim().is_empty() { return Ok(self.selection.clone()); }
        self.resolved.get_or_init(|| credentials::connection_with_store(PortalKind::Eplan, &endpoint.url, &endpoint.token, store)).clone()
    }

    fn call<T: 'static>(&self, store: &impl CredentialStore, build: impl FnOnce(Connection) -> Call<T>) -> Result<Call<T>, String> {
        self.resolve(store).map(build)
    }
}

pub struct ServerClient {
    /// Kind of the server: the OpenECraft parts server or the EPLAN Data Portal (read
    /// through `openecraft_client::eplan`, so the part handling works the same).
    pub kind: PortalKind,
    pub url: String,
    /// Access token for changes: the administrator token or the token of a session;
    /// for EPLAN an explicit PAT. Empty uses the endpoint's saved system credential.
    pub token: String,
    /// Explicit opt-in; never persisted with application settings.
    pub remember_pat: bool,
    cached_connection: Option<CredentialCache>,
    check_generation: u64,
    pub info: Option<ServerInfo>,
    pub status: String,
    /// Address and token of the other kind of server.
    other: (String, String),
    /// Portal ids of the parts the EPLAN Data Portal sent, by number (it loads
    /// several parts only by id).
    portal_ids: HashMap<String, String>,
    /// The last finished request failed (`status` shows why).
    failed: bool,
    pending: usize,
    inbox: Arc<Mutex<Vec<Reply>>>,
}

impl Default for ServerClient {
    fn default() -> Self {
        let Endpoint { url, token } = Endpoint::default();
        ServerClient {
            kind: PortalKind::OpenECraft,
            url,
            token,
            remember_pat: false,
            cached_connection: None,
            check_generation: 0,
            info: None,
            status: String::new(),
            other: (PortalKind::Eplan.default_url(), String::new()),
            portal_ids: HashMap::new(),
            failed: false,
            pending: 0,
            inbox: Arc::default(),
        }
    }
}

impl ServerClient {
    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// True if the last connection check found a server that accepts changes.
    pub fn writable(&self) -> bool {
        self.info.as_ref().is_some_and(|i| i.writable)
    }

    /// The server only offers searching and reading parts (EPLAN Data Portal).
    pub fn read_only(&self) -> bool {
        self.kind.read_only()
    }

    /// The signed in user (as of the last connection check or sign-in).
    pub fn user(&self) -> Option<&User> {
        self.info.as_ref().and_then(|i| i.user.as_ref())
    }

    pub fn endpoint(&self) -> Endpoint {
        Endpoint { url: self.url.clone(), token: self.token.clone() }
    }

    pub fn connection(&self) -> Connection {
        Connection::new(self.kind, &self.url, &self.token)
    }

    fn request_credentials(&mut self) -> CredentialCache {
        let selection = self.connection();
        if self.cached_connection.as_ref().is_none_or(|cache| cache.selection != selection) {
            self.cached_connection = Some(CredentialCache { selection, resolved: Arc::default() });
        }
        self.cached_connection.as_ref().unwrap().clone()
    }

    fn refresh_credentials(&mut self) -> CredentialCache {
        self.cached_connection = None;
        self.request_credentials()
    }

    /// Remove only this portal's saved PAT and its explicit GUI value.
    pub fn forget_saved_pat(&mut self) {
        if self.kind != PortalKind::Eplan { return; }
        self.forget_saved_pat_with(&SystemCredentialStore);
    }

    fn forget_saved_pat_with(&mut self, store: &impl CredentialStore) {
        self.check_generation += 1;
        match store.forget(&self.url) {
            Ok(removed) => {
                self.cached_connection = None;
                self.remember_pat = false;
                self.token.clear();
                self.info = None;
                self.status = if removed { tr("Saved Data Portal token removed") } else { tr("No saved Data Portal token for this endpoint") }.into();
                self.failed = false;
            }
            Err(error) => { self.status = error.to_string(); self.failed = true; }
        }
    }

    /// Switches the kind of server; the address and token of each kind are kept.
    pub fn set_kind(&mut self, kind: PortalKind) {
        if kind == self.kind {
            return;
        }
        let current = (std::mem::take(&mut self.url), std::mem::take(&mut self.token));
        (self.url, self.token) = std::mem::replace(&mut self.other, current);
        self.kind = kind;
        self.cached_connection = None;
        self.remember_pat = false;
        self.check_generation += 1;
        self.info = None;
        self.status.clear();
        self.failed = false;
    }

    /// The address of a kind of server (stored with the settings).
    pub fn url_of(&self, kind: PortalKind) -> &str {
        if kind == self.kind {
            &self.url
        } else {
            &self.other.0
        }
    }

    pub fn set_url_of(&mut self, kind: PortalKind, url: String) {
        if kind == self.kind {
            self.url = url;
        } else {
            self.other.0 = url;
        }
    }

    /// Address of a file for the web browser (`None`: it needs the access token).
    pub fn file_link(&self, id: &str) -> Option<String> {
        self.connection().file_link(id)
    }

    /// Sends the call; a refused call (`Err`) is answered at once.
    fn send<T: 'static>(
        &mut self,
        ctx: &egui::Context,
        call: Result<Call<T>, String>,
        status: String,
        wrap: impl 'static + Send + FnOnce(Result<T, String>) -> Reply,
    ) {
        let deliver = self.begin_request(ctx, status);
        match call {
            Ok(call) => call.fetch(move |result| deliver(wrap(result))),
            Err(e) => deliver(wrap(Err(e))),
        }
    }

    fn begin_request(&mut self, ctx: &egui::Context, status: String) -> impl FnOnce(Reply) + Send + 'static {
        self.status = status;
        self.pending += 1;
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        move |reply| {
            if let Ok(mut inbox) = inbox.lock() {
                inbox.push(reply);
            }
            ctx.request_repaint();
        }
    }

    /// Resolve credentials and construct this very request off the UI thread.
    fn send_portal<T: 'static>(
        &mut self,
        ctx: &egui::Context,
        build: impl FnOnce(Connection) -> Call<T> + Send + 'static,
        status: String,
        wrap: impl FnOnce(Result<T, String>) -> Reply + Send + 'static,
    ) {
        let cache = self.request_credentials();
        let deliver = self.begin_request(ctx, status);
        crate::io::background(move || {
            match cache.call(&SystemCredentialStore, build) {
                Ok(call) => call.fetch(move |result| deliver(wrap(result))),
                Err(error) => deliver(wrap(Err(error))),
            }
        });
    }

    /// A function only the OpenECraft parts server offers.
    fn server<T: 'static>(
        &self,
        request: impl FnOnce(&Endpoint) -> ehttp::Request,
        decode: impl FnOnce(ehttp::Result<ehttp::Response>) -> Result<T, String> + Send + 'static,
    ) -> Result<Call<T>, String> {
        let connection = self.connection();
        Ok(Call::new(request(connection.parts_server()?), decode))
    }

    /// Checks the connection (server info).
    pub fn check(&mut self, ctx: &egui::Context) {
        let selection = self.connection();
        let kind = self.kind;
        let url = self.url.clone();
        let token = self.token.clone();
        let remember = self.remember_pat;
        self.check_generation += 1;
        let generation = self.check_generation;
        let cache = self.refresh_credentials();
        let deliver = self.begin_request(ctx, tr("Connecting…").into());
        // Explicit checks refresh the same cache used by ordinary requests.
        crate::io::background(move || {
            let connection = cache.resolve(&SystemCredentialStore);
            let explicit = (remember && kind == PortalKind::Eplan && !token.trim().is_empty())
                .then_some(openecraft_client::EplanEndpoint { url, token });
            let finish = move |result| {
                let (result, warning) = match credentials::remember_verified(result, explicit.as_ref(), &SystemCredentialStore) {
                    Ok((info, warning)) => (Ok(info), warning),
                    Err(error) => (Err(error), None),
                };
                let check = CheckedConnection { selection, generation, result, warning };
                deliver(Reply::Checked(Box::new(check)));
            };
            match connection { Ok(connection) => connection.info().fetch(finish), Err(error) => finish(Err(error)) }
        });
    }

    /// Loads all parts.
    pub fn download(&mut self, ctx: &egui::Context) {
        let call = self.server(|e| e.list(&PartQuery::default()), decode);
        self.send(ctx, call, tr("Downloading parts…").into(), Reply::Parts);
    }

    /// Loads the parts with these numbers (e.g. to add them to the project).
    pub fn fetch(&mut self, ctx: &egui::Context, numbers: &[String]) {
        let ids: Vec<String> = numbers.iter().filter_map(|n| self.portal_ids.get(n)).cloned().collect();
        let numbers = numbers.to_vec();
        self.send_portal(ctx, move |c| c.find(&numbers, &ids), tr("Downloading parts…").into(), |r| {
            Reply::Parts(r.map(|s| PartList { revision: s.revision, total: s.total, offset: s.offset, parts: s.parts }))
        });
    }

    /// Faceted search (part lists without models).
    pub fn search(&mut self, ctx: &egui::Context, query: &PartQuery) {
        let query = PartQuery { brief: true, ..query.clone() };
        self.send_portal(ctx, move |c| c.search(&query), tr("Searching…").into(), Reply::Search);
    }

    /// Loads the server versions of local parts for an update check; on the EPLAN Data
    /// Portal of the parts loaded from there ([`PortalKind::update_candidates`]).
    pub fn versions(&mut self, ctx: &egui::Context, local: &[Part]) {
        let local = self.kind.update_candidates(local);
        let numbers: Vec<String> = local.iter().map(|p| p.number.clone()).collect();
        let ids: Vec<String> = local.iter().map(|p| p.portal_id.clone()).filter(|id| !id.is_empty()).collect();
        let status = tr("Checking for part updates…").into();
        if self.read_only() && ids.is_empty() {
            self.send(ctx, Err(tr("No part of the database comes from the EPLAN Data Portal").to_string()), status, Reply::Versions);
        } else {
            self.send_portal(ctx, move |c| c.find(&numbers, &ids), status, Reply::Versions);
        }
    }

    /// Uploads parts as JSON (needs the access token).
    pub fn upload(&mut self, ctx: &egui::Context, parts: &[Part]) {
        let call = self.server(|e| e.import(serde_json::to_vec(parts).unwrap_or_default(), "application/json"), decode);
        self.send(ctx, call, tr_fmt!("Uploading {} parts…", parts.len()), Reply::Imported);
    }

    /// Uploads the content of a CSV or JSON parts file.
    pub fn upload_file(&mut self, ctx: &egui::Context, content: Vec<u8>) {
        let call = self.server(|e| e.import(content, "text/plain; charset=utf-8"), decode);
        self.send(ctx, call, tr("Uploading parts file…").into(), Reply::Imported);
    }

    /// Creates or replaces a part.
    pub fn save(&mut self, ctx: &egui::Context, part: &Part) {
        let call = self.server(|e| e.save(part), decode);
        self.send(ctx, call, tr_fmt!("Saving {}…", part.number), |r| Reply::Saved(r.map(Box::new)));
    }

    pub fn delete(&mut self, ctx: &egui::Context, number: &str) {
        let call = self.server(|e| e.delete(number), |r| check(r).map(|_| ()));
        let n = number.to_string();
        self.send(ctx, call, tr_fmt!("Deleting {}…", number), move |r| Reply::Deleted(n, r));
    }

    pub fn history(&mut self, ctx: &egui::Context, number: &str) {
        let call = self.server(|e| e.history(number), decode);
        let n = number.to_string();
        self.send(ctx, call, tr("Loading the history…").into(), move |r| Reply::History(n, r));
    }

    /// The whole part with its models, for the details (the search sends parts without
    /// them); the status line stays.
    pub fn detail(&mut self, ctx: &egui::Context, number: &str) {
        let number = number.to_string();
        let n = number.clone();
        let status = self.status.clone();
        self.send_portal(ctx, move |c| c.part(&number), status, move |r| Reply::Detail(n, r.map(|p| p.map(Box::new))));
    }

    /// The accessories of a part and the parts it fits, for the details; the status
    /// line stays.
    pub fn accessories(&mut self, ctx: &egui::Context, number: &str) {
        let number = number.to_string();
        let n = number.clone();
        let status = self.status.clone();
        self.send_portal(ctx, move |c| c.accessories(&number), status, move |r| Reply::Accessories(n, r));
    }

    /// Stores a file with a part.
    pub fn attach(&mut self, ctx: &egui::Context, number: &str, name: &str, kind: Option<AttachmentKind>, data: Vec<u8>) {
        let call = self.server(|e| e.attach(number, name, kind, data), decode);
        self.send(ctx, call, tr_fmt!("Uploading {}…", name), |r| Reply::Saved(r.map(Box::new)));
    }

    pub fn detach(&mut self, ctx: &egui::Context, number: &str, id: &str) {
        let call = self.server(|e| e.detach(number, id), decode);
        self.send(ctx, call, tr("Removing the file…").into(), |r| Reply::Saved(r.map(Box::new)));
    }

    pub fn download_file(&mut self, ctx: &egui::Context, id: &str, name: &str, purpose: FileUse) {
        let id = id.to_string();
        let n = name.to_string();
        self.send_portal(ctx, move |c| c.file(&id), tr_fmt!("Downloading {}…", name), move |r| Reply::File(n, purpose, r));
    }

    pub fn catalogs(&mut self, ctx: &egui::Context) {
        let call = self.server(Endpoint::catalogs, decode);
        self.send(ctx, call, tr("Loading catalogs…").into(), Reply::Catalogs);
    }

    pub fn save_catalog(&mut self, ctx: &egui::Context, catalog: &Catalog) {
        let call = self.server(|e| e.save_catalog(catalog), decode);
        self.send(ctx, call, tr_fmt!("Saving {}…", catalog.name), Reply::CatalogSaved);
    }

    pub fn delete_catalog(&mut self, ctx: &egui::Context, id: &str) {
        let call = self.server(|e| e.delete_catalog(id), |r| check(r).map(|_| ()));
        let id = id.to_string();
        self.send(ctx, call, tr_fmt!("Deleting {}…", id), move |r| Reply::CatalogDeleted(id, r));
    }

    /// Signs in; the session's token replaces [`ServerClient::token`].
    pub fn sign_in(&mut self, ctx: &egui::Context, name: &str, password: &str) {
        let call = self.server(|e| e.login(name, password), decode);
        self.send(ctx, call, tr("Signing in…").into(), Reply::SignedIn);
    }

    pub fn sign_out(&mut self, ctx: &egui::Context) {
        let call = self.server(Endpoint::logout, |_| Ok(()));
        self.send(ctx, call, tr("Signing out…").into(), |_| Reply::SignedOut);
    }

    /// Loads a library (JSON) shared by all projects, e.g. the company's forms.
    pub fn library(&mut self, ctx: &egui::Context, name: &str) {
        let call = self.server(|e| e.library(name), |r| decode_found::<serde_json::Value>(r).map(|v| v.map(|v| v.to_string())));
        let n = name.to_string();
        self.send(ctx, call, tr_fmt!("Loading {}…", name), move |r| Reply::Library(n, r));
    }

    pub fn save_library(&mut self, ctx: &egui::Context, name: &str, json: &str) {
        let call = self.server(|e| e.save_library(name, json), |r| check(r).map(|_| ()));
        let n = name.to_string();
        self.send(ctx, call, tr_fmt!("Saving {}…", name), move |r| Reply::LibrarySaved(n, r));
    }

    pub fn favorites(&mut self, ctx: &egui::Context) {
        let call = self.server(Endpoint::favorites, decode);
        self.send(ctx, call, tr("Loading favorites…").into(), Reply::Favorites);
    }

    pub fn set_favorites(&mut self, ctx: &egui::Context, numbers: &[String]) {
        let call = self.server(|e| e.set_favorites(numbers), decode);
        self.send(ctx, call, tr("Saving favorites…").into(), Reply::Favorites);
    }

    pub fn users(&mut self, ctx: &egui::Context) {
        let call = self.server(Endpoint::users, decode);
        self.send(ctx, call, tr("Loading users…").into(), Reply::Users);
    }

    pub fn save_user(&mut self, ctx: &egui::Context, name: &str, update: &UserUpdate) {
        let call = self.server(|e| e.save_user(name, update), decode);
        self.send(ctx, call, tr_fmt!("Saving {}…", name), Reply::UserSaved);
    }

    pub fn delete_user(&mut self, ctx: &egui::Context, name: &str) {
        let call = self.server(|e| e.delete_user(name), |r| check(r).map(|_| ()));
        let n = name.to_string();
        self.send(ctx, call, tr_fmt!("Deleting {}…", name), move |r| Reply::UserDeleted(n, r));
    }

    /// Takes the finished requests and updates `status`, `info` and the token.
    pub fn poll(&mut self) -> Vec<Reply> {
        let replies: Vec<Reply> = self.inbox.lock().map(|mut i| std::mem::take(&mut *i)).unwrap_or_default();
        for reply in &replies {
            self.pending = self.pending.saturating_sub(1);
            if let Reply::Checked(check) = reply {
                // Ignore metadata from checks superseded by endpoint/token edits
                // or a later check/forget action.
                if check.generation != self.check_generation || check.selection != self.connection() { continue; }
                self.info = check.result.as_ref().ok().cloned();
            }
            let found = match reply {
                Reply::Search(Ok(r)) | Reply::Versions(Ok(r)) => r.parts.as_slice(),
                Reply::Parts(Ok(list)) => list.parts.as_slice(),
                _ => &[],
            };
            for p in found.iter().filter(|p| !p.portal_id.is_empty()) {
                self.portal_ids.insert(p.number.clone(), p.portal_id.clone());
            }
            match reply {
                Reply::Info(Ok(info)) => self.info = Some(info.clone()),
                Reply::Info(Err(_)) => self.info = None,
                Reply::SignedIn(Ok(session)) => {
                    self.token = session.token.clone();
                    if let Some(info) = &mut self.info {
                        info.user = Some(session.user.clone());
                    }
                }
                Reply::SignedOut => {
                    self.token.clear();
                    if let Some(info) = &mut self.info {
                        info.user = None;
                    }
                }
                _ => {}
            }
            if !matches!(reply, Reply::Detail(_, Ok(_))) {
                self.status = reply.describe();
                self.failed = reply.error().is_some();
            }
        }
        // Keep the public Info notification used by the parts manager to reload
        // parts and catalogs; credential details stay inside this client.
        replies.into_iter().filter_map(|reply| match reply {
            Reply::Checked(check) if check.generation == self.check_generation && check.selection == self.connection() => Some(Reply::Info(check.result)),
            Reply::Checked(_) => None,
            reply => Some(reply),
        }).collect()
    }

    /// True if the status line shows an error.
    pub fn failed(&self) -> bool {
        self.failed
    }
}

#[cfg(test)]
mod tests;
