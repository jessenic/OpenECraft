//! Test helpers: an in-memory parts server and sessions / files for the tool tests.

use crate::parts_server::{Server, Transport};
use crate::session::Session;
use openecraft_client::{ehttp, Endpoint, PortalKind};
use openecraft_parts::access::{Policy, Settings};
use openecraft_parts::api::{ApiError, ImportResult, Login, PartQuery, Registration, ServerInfo, Session as ApiSession, UserUpdate, API_PREFIX};
use openecraft_parts::portal::{Attachment, AttachmentKind, Catalog, ChangeKind, HistoryEntry, PartsRight, Role, User};
use openecraft_parts::{import, to_csv, Part};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub const URL: &str = "http://parts.test";
pub const TOKEN: &str = "secret";

/// Parts server with the routes of `openecraft-parts-server`, holding its data in
/// memory. The user `ann` (password `pw`) signs in with the token [`TOKEN`].
#[derive(Clone, Default)]
pub struct FakeServer {
    pub parts: Rc<RefCell<Vec<Part>>>,
    pub catalogs: Rc<RefCell<Vec<Catalog>>>,
    pub users: Rc<RefCell<Vec<User>>>,
    pub favorites: Rc<RefCell<Vec<String>>>,
    pub files: Rc<RefCell<Vec<FakeFile>>>,
    pub policy: Rc<RefCell<Policy<PartsRight>>>,
    /// Libraries by name (JSON text).
    pub libraries: Rc<RefCell<std::collections::BTreeMap<String, String>>>,
    pub registration: Rc<RefCell<Registration>>,
}

/// Attachment file: id, name and content.
pub type FakeFile = (String, String, Vec<u8>);

fn response(status: u16, bytes: Vec<u8>) -> ehttp::Response {
    ehttp::Response {
        url: String::new(),
        ok: (200..300).contains(&status),
        status,
        status_text: String::new(),
        headers: Default::default(),
        bytes,
    }
}

fn json(value: &impl serde::Serialize) -> ehttp::Response {
    response(200, serde_json::to_vec(value).unwrap())
}

fn error(status: u16, text: &str) -> ehttp::Response {
    response(status, serde_json::to_vec(&ApiError { error: text.into() }).unwrap())
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

impl Transport for FakeServer {
    fn fetch(&self, req: &ehttp::Request) -> Result<ehttp::Response, String> {
        let rest = req.url.strip_prefix(URL).ok_or("host not found")?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let path = path.strip_prefix(API_PREFIX).ok_or("no API path")?;
        let authorized = req.headers.get("Authorization") == Some(format!("Bearer {TOKEN}").as_str());
        let write = !matches!(req.method, ehttp::Method::GET) && path != "/login" && path != "/search";
        if write && !authorized {
            return Ok(error(401, "invalid token"));
        }
        if path == "/login" {
            let login: Login = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
            return Ok(if login.name == "ann" && login.password == "pw" {
                json(&ApiSession { token: TOKEN.into(), user: User { name: "ann".into(), role: Role::Editor, manufacturer: String::new(), ..Default::default() }, ..Default::default() })
            } else {
                error(401, "wrong user name or password")
            });
        }
        if let Some(rest) = path.strip_prefix("/parts/").filter(|r| r.contains('/')) {
            return Ok(self.part_route(rest, query, req));
        }
        let segments: Vec<String> = path.split('/').skip(1).map(decode).collect();
        match (segments.iter().map(String::as_str).collect::<Vec<_>>().as_slice(), &req.method) {
            (["search"], _) => {
                let q: PartQuery = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                return Ok(json(&q.search(&self.parts.borrow(), 1)));
            }
            (["catalogs"], _) => return Ok(json(&*self.catalogs.borrow())),
            (["catalogs", id], ehttp::Method::PUT) => {
                let c: Catalog = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                self.catalogs.borrow_mut().retain(|x| x.id != *id);
                self.catalogs.borrow_mut().push(c.clone());
                return Ok(json(&c));
            }
            (["catalogs", id], ehttp::Method::DELETE) => {
                self.catalogs.borrow_mut().retain(|x| x.id != *id);
                return Ok(response(204, Vec::new()));
            }
            (["favorites"], ehttp::Method::GET) if !authorized => return Ok(error(401, "invalid token")),
            (["favorites"], ehttp::Method::GET) => return Ok(json(&*self.favorites.borrow())),
            (["favorites"], _) => {
                *self.favorites.borrow_mut() = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                return Ok(json(&*self.favorites.borrow()));
            }
            (["users"], _) => return Ok(json(&*self.users.borrow())),
            (["users", name], ehttp::Method::PUT) => {
                let u: UserUpdate = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                let user = User { name: name.to_string(), role: u.role, manufacturer: u.manufacturer, ..Default::default() };
                self.users.borrow_mut().retain(|x| x.name != *name);
                self.users.borrow_mut().push(user.clone());
                return Ok(json(&user));
            }
            (["users", name, "approval"], ehttp::Method::POST) => {
                let mut users = self.users.borrow_mut();
                let Some(user) = users.iter_mut().find(|u| u.name == *name) else { return Ok(error(404, "user not found")) };
                user.pending = false;
                return Ok(json(&*user));
            }
            (["users", name], ehttp::Method::DELETE) => {
                self.users.borrow_mut().retain(|x| x.name != *name);
                return Ok(response(204, Vec::new()));
            }
            (["access"], method) => {
                if !authorized {
                    return Ok(error(401, "invalid token"));
                }
                if *method == ehttp::Method::PUT {
                    let policy: Policy<PartsRight> = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                    if let Err(e) = policy.validate() {
                        return Ok(error(400, &e));
                    }
                    *self.policy.borrow_mut() = policy;
                }
                return Ok(json(&Settings { policy: self.policy.borrow().clone(), ..Default::default() }));
            }
            (["libraries", name], ehttp::Method::GET) => {
                return Ok(match self.libraries.borrow().get(*name) {
                    Some(j) => response(200, j.clone().into_bytes()),
                    None => error(404, "library not found"),
                });
            }
            (["libraries", name], ehttp::Method::PUT) => {
                self.libraries.borrow_mut().insert(name.to_string(), String::from_utf8(req.body.clone()).map_err(|e| e.to_string())?);
                return Ok(response(204, Vec::new()));
            }
            (["registration"], method) => {
                if !authorized {
                    return Ok(error(401, "invalid token"));
                }
                if *method == ehttp::Method::PUT {
                    *self.registration.borrow_mut() = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                }
                return Ok(json(&*self.registration.borrow()));
            }
            (["files", id], _) => {
                let files = self.files.borrow();
                return Ok(match files.iter().find(|f| f.0 == *id) {
                    Some(f) => response(200, f.2.clone()),
                    None => error(404, "file not found"),
                });
            }
            _ => {}
        }
        let mut parts = self.parts.borrow_mut();
        Ok(match (path, &req.method) {
            ("/info", _) => json(&ServerInfo { name: "fake".into(), parts: parts.len(), writable: true, ..Default::default() }),
            ("/parts", _) => {
                let mut q = PartQuery::default();
                for (k, v) in query.split('&').filter_map(|kv| kv.split_once('=')) {
                    match k {
                        "q" => q.q = Some(decode(v)),
                        "group" => q.group = Some(decode(v)),
                        "manufacturer" => q.manufacturer = Some(decode(v)),
                        "offset" => q.offset = v.parse().ok(),
                        "limit" => q.limit = v.parse().ok(),
                        _ => {}
                    }
                }
                let (total, page) = q.apply(&parts);
                json(&openecraft_parts::api::PartList { total, offset: q.offset.unwrap_or(0), parts: page, revision: 1 })
            }
            ("/export.csv", _) => response(200, to_csv(&parts).into_bytes()),
            ("/import", _) => match import(std::str::from_utf8(&req.body).unwrap()) {
                Ok(new) => {
                    let imported = new.len();
                    for p in new {
                        parts.retain(|x| x.number != p.number);
                        parts.push(p);
                    }
                    json(&ImportResult { imported, revision: 2 })
                }
                Err(e) => error(400, &e),
            },
            (p, method) => {
                let number = decode(p.strip_prefix("/parts/").ok_or("unknown path")?);
                let at = parts.iter().position(|x| x.number == number);
                match (method, at) {
                    (ehttp::Method::GET, Some(i)) => json(&parts[i]),
                    // Like axum's JSON extractor: the first content type counts.
                    (ehttp::Method::PUT, _) if req.headers.get("Content-Type") != Some("application/json") => {
                        error(415, "expected application/json")
                    }
                    (ehttp::Method::PUT, _) => {
                        let part: Part = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                        parts.retain(|x| x.number != number);
                        parts.push(part);
                        response(200, Vec::new())
                    }
                    (ehttp::Method::DELETE, Some(i)) => {
                        parts.remove(i);
                        response(204, Vec::new())
                    }
                    _ => error(404, "part not found"),
                }
            }
        })
    }
}

impl FakeServer {
    /// `/parts/{number}/history[/{revision}]` and `/parts/{number}/attachments[/{id}]`.
    fn part_route(&self, rest: &str, query: &str, req: &ehttp::Request) -> ehttp::Response {
        let segments: Vec<String> = rest.split('/').map(decode).collect();
        let mut parts = self.parts.borrow_mut();
        let Some(part) = parts.iter_mut().find(|p| p.number == segments[0]) else { return error(404, "part not found") };
        match (segments[1].as_str(), segments.get(2)) {
            ("history", None) => json(&vec![HistoryEntry { revision: 1, time: 0, user: "ann".into(), change: ChangeKind::Created }]),
            ("history", Some(rev)) if rev == "1" => json(&*part),
            ("history", Some(_)) => error(404, "no such version"),
            ("attachments", None) => {
                let param = |key: &str| query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| decode(v));
                let name = param("name").unwrap_or_default();
                let kind = param("kind").and_then(|k| AttachmentKind::ALL.into_iter().find(|x| format!("{x:?}") == k)).unwrap_or_else(|| AttachmentKind::guess(&name));
                let id = format!("f{}", self.files.borrow().len() + 1);
                part.attachments.push(Attachment { id: id.clone(), name: name.clone(), kind, size: req.body.len() as u64 });
                self.files.borrow_mut().push((id, name, req.body.clone()));
                json(&*part)
            }
            ("attachments", Some(id)) => {
                part.attachments.retain(|a| a.id != *id);
                json(&*part)
            }
            _ => error(404, "unknown path"),
        }
    }
}

pub fn part(number: &str, group: &str) -> Part {
    Part { number: number.into(), manufacturer: "ACME".into(), group: group.into(), ..Default::default() }
}

/// Session connected to `server` (with the write token).
pub fn session_with(server: &FakeServer) -> Session {
    Session::new(Server { endpoint: Endpoint { url: URL.into(), token: TOKEN.into() }, kind: PortalKind::OpenECraft, credentials: Box::new(NoCredentials), transport: Box::new(server.clone()) })
}

pub fn session() -> Session {
    session_with(&FakeServer::default())
}

/// Empty directory for the files of one test.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openecraft-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs a tool or a command of a plugin with JSON arguments.
pub fn run(s: &mut Session, tool: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let entry = crate::tools::lookup(s, tool).unwrap_or_else(|| panic!("no tool or command {tool}"));
    crate::tools::call(s, entry, args.as_object().unwrap())
}

/// Session with a new, empty project.
pub fn new_session() -> Session {
    let mut s = session();
    run(&mut s, "new_project", serde_json::json!({ "name": "Test" })).unwrap();
    s
}

pub const VAULT_URL: &str = "http://vault.test";
pub const VAULT_TOKEN: &str = "V:session";

/// A file of the fake vault: master id, folder, name, versions (id, content) and
/// whether it is checked out.
#[derive(Clone, Debug)]
pub struct VaultFile {
    pub id: String,
    pub folder: String,
    pub name: String,
    pub versions: Vec<(String, Vec<u8>)>,
    pub checked_out: bool,
}

/// Autodesk Vault with the routes of the Vault Data API the tools use, holding its
/// data in memory. The user `anna` (password `pw`) signs in to the vault `Vault`
/// (id 1) with the token [`VAULT_TOKEN`]; uploads come in parts of 4 bytes.
#[derive(Clone, Default)]
pub struct FakeVault {
    /// Folder id, parent id, name.
    pub folders: Rc<RefCell<Vec<(String, String, String)>>>,
    pub files: Rc<RefCell<Vec<VaultFile>>>,
    pub items: Rc<RefCell<Vec<serde_json::Value>>>,
    uploads: Rc<RefCell<Vec<Vec<u8>>>>,
    next_id: Rc<RefCell<u32>>,
}

impl FakeVault {
    fn id(&self) -> String {
        let mut n = self.next_id.borrow_mut();
        *n += 1;
        format!("{}", 100 + *n)
    }

    /// Adds a file with one version to a folder; returns the file id.
    pub fn add(&self, folder: &str, name: &str, content: &[u8]) -> String {
        let (id, version) = (self.id(), self.id());
        self.files.borrow_mut().push(VaultFile { id: id.clone(), folder: folder.into(), name: name.into(), versions: vec![(version, content.to_vec())], checked_out: false });
        id
    }

    fn version_json(f: &VaultFile, index: usize) -> serde_json::Value {
        serde_json::json!({
            "entityType": "FileVersion",
            "id": f.versions[index].0,
            "name": f.name,
            "version": index + 1,
            "revision": "A",
            "state": "Work in Progress",
            "isCheckedOut": f.checked_out,
            "parentFolderId": f.folder,
            "file": { "id": f.id },
        })
    }

    fn contents(&self, folder: &str, q: Option<&str>) -> serde_json::Value {
        let folder = if folder == "root" { "1" } else { folder };
        let mut results: Vec<serde_json::Value> = Vec::new();
        if q.is_none() {
            for (id, _, name) in self.folders.borrow().iter().filter(|f| f.1 == folder) {
                results.push(serde_json::json!({ "entityType": "Folder", "id": id, "name": name, "fullName": format!("$/{name}") }));
            }
        }
        for f in self.files.borrow().iter() {
            let hit = match q {
                Some(q) => f.name.to_lowercase().contains(&q.to_lowercase()),
                None => f.folder == folder,
            };
            if hit {
                results.push(Self::version_json(f, f.versions.len() - 1));
            }
        }
        serde_json::json!({ "pagination": { "limit": 1000, "totalResults": results.len() }, "results": results })
    }

    fn route(&self, method: &ehttp::Method, path: &str, query: &str, req: &ehttp::Request) -> Result<ehttp::Response, String> {
        let param = |key: &str| query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| decode(v));
        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
        let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut files = self.files.borrow_mut();
        let version = |files: &[VaultFile], vid: &str| files.iter().find_map(|f| f.versions.iter().position(|v| v.0 == vid).map(|i| (f.clone(), i)));
        Ok(match (method, segments.as_slice()) {
            (ehttp::Method::GET, ["sessions", "@current"]) => json(&serde_json::json!({ "accessToken": VAULT_TOKEN, "vaultInformation": { "id": "1", "name": "Vault" }, "userInformation": { "name": "anna" } })),
            (ehttp::Method::GET, ["vaults", "1", "folders", folder, "contents"]) => {
                drop(files);
                json(&self.contents(folder, param("q").as_deref()))
            }
            (ehttp::Method::GET, ["vaults", "1", "search-results"]) => {
                drop(files);
                json(&self.contents("", Some(&param("q").unwrap_or_default())))
            }
            (ehttp::Method::GET, ["vaults", "1", "item-versions"]) => {
                let q = param("q").unwrap_or_default();
                let hits: Vec<serde_json::Value> = self.items.borrow().iter().filter(|i| i.to_string().contains(&q)).cloned().collect();
                json(&serde_json::json!({ "results": hits }))
            }
            (ehttp::Method::GET, ["vaults", "1", "file-versions", vid]) => match version(&files, vid) {
                Some((f, i)) => json(&Self::version_json(&f, i)),
                None => vault_error(404, "File version not found"),
            },
            (ehttp::Method::GET, ["vaults", "1", "file-versions", vid, "content"]) => match version(&files, vid) {
                Some((f, i)) => response(200, f.versions[i].1.clone()),
                None => vault_error(404, "File version not found"),
            },
            (ehttp::Method::GET, ["vaults", "1", "files", id]) => match files.iter().find(|f| f.id == *id) {
                Some(f) => json(&serde_json::json!({ "id": f.id, "fileVersion": Self::version_json(f, f.versions.len() - 1) })),
                None => vault_error(404, "File not found"),
            },
            (ehttp::Method::POST, ["vaults", "1", "files"]) => {
                let ticket = body["uploadTicket"].as_str().unwrap_or_default().to_string();
                let content = ticket.parse::<usize>().ok().and_then(|i| self.uploads.borrow().get(i).cloned()).ok_or("unknown ticket")?;
                let folder = body["folderId"].as_i64().unwrap_or(1).to_string();
                let name = body["name"].as_str().unwrap_or_default().to_string();
                if files.iter().any(|f| f.folder == folder && f.name.eq_ignore_ascii_case(&name)) {
                    return Ok(vault_error(409, "A file of this name exists"));
                }
                let (id, vid) = (self.id(), self.id());
                let f = VaultFile { id: id.clone(), folder, name, versions: vec![(vid, content)], checked_out: false };
                let out = serde_json::json!({ "id": id, "fileVersion": Self::version_json(&f, 0) });
                files.push(f);
                let mut r = json(&out);
                r.status = 201;
                r
            }
            (ehttp::Method::POST, ["vaults", "1", "files", action]) => {
                let (id, verb) = action.split_once(':').ok_or("no action")?;
                let Some(f) = files.iter_mut().find(|f| f.id == id) else { return Ok(vault_error(404, "File not found")) };
                match verb {
                    "checkout" if f.checked_out => vault_error(409, "The file is checked out"),
                    "checkout" => {
                        f.checked_out = true;
                        json(&Self::version_json(f, f.versions.len() - 1))
                    }
                    "undo-checkout" => {
                        f.checked_out = false;
                        json(&Self::version_json(f, f.versions.len() - 1))
                    }
                    "checkin" if !f.checked_out => vault_error(409, "The file is not checked out"),
                    "checkin" => {
                        let ticket = body["uploadTicket"].as_str().unwrap_or_default();
                        let content = ticket.parse::<usize>().ok().and_then(|i| self.uploads.borrow().get(i).cloned()).ok_or("unknown ticket")?;
                        let vid = self.id();
                        f.versions.push((vid, content));
                        f.checked_out = body["keepCheckedOut"].as_bool().unwrap_or(false);
                        json(&Self::version_json(f, f.versions.len() - 1))
                    }
                    _ => vault_error(404, "unknown action"),
                }
            }
            (ehttp::Method::POST, ["vaults", "1", "file-uploads"]) => {
                let mut uploads = self.uploads.borrow_mut();
                uploads.push(Vec::new());
                json(&serde_json::json!({ "uploadId": (uploads.len() - 1).to_string(), "uploadSessionToken": "s0", "maxPartSize": 4 }))
            }
            (ehttp::Method::PUT, ["vaults", "1", "file-uploads", upload, "parts", index]) => {
                let i: usize = index.parse().map_err(|_| "bad part")?;
                if req.headers.get("X-Vault-Upload-Session") != Some(format!("s{i}").as_str()) {
                    return Ok(vault_error(400, "Wrong upload session"));
                }
                let mut uploads = self.uploads.borrow_mut();
                let u: usize = upload.parse().map_err(|_| "bad upload")?;
                uploads[u].extend_from_slice(&req.body);
                json(&serde_json::json!({ "uploadId": upload, "uploadSessionToken": format!("s{}", i + 1), "maxPartSize": 4 }))
            }
            (ehttp::Method::POST, ["vaults", "1", "file-uploads", action]) => {
                let upload = action.strip_suffix(":complete").ok_or("no action")?;
                json(&serde_json::json!({ "uploadId": upload, "state": "completed", "uploadTicket": upload }))
            }
            _ => vault_error(404, "unknown path"),
        })
    }
}

fn vault_error(status: u16, detail: &str) -> ehttp::Response {
    response(status, serde_json::to_vec(&serde_json::json!({ "statusCode": status, "errorCode": "Error", "title": "Vault", "detail": detail })).unwrap())
}

impl Transport for FakeVault {
    fn fetch(&self, req: &ehttp::Request) -> Result<ehttp::Response, String> {
        let rest = req.url.strip_prefix(VAULT_URL).ok_or("host not found")?;
        let rest = rest.strip_prefix(openecraft_client::vault::API_PATH).ok_or("no API path")?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        match (&req.method, path) {
            (ehttp::Method::GET, "/server-info") => return Ok(json(&serde_json::json!({ "name": "Vault Professional", "productVersion": "31.0.0" }))),
            (ehttp::Method::POST, "/sessions") => {
                let body: serde_json::Value = serde_json::from_slice(&req.body).map_err(|e| e.to_string())?;
                let input = &body["input"];
                return Ok(if input["userName"] == "anna" && input["password"] == "pw" && input["vault"] == "Vault" {
                    json(&serde_json::json!({ "id": "9", "accessToken": VAULT_TOKEN, "vaultInformation": { "id": "1", "name": "Vault" }, "userInformation": { "name": "anna" } }))
                } else {
                    vault_error(401, "The user name or password is wrong.")
                });
            }
            _ => {}
        }
        if req.headers.get("Authorization") != Some(format!("Bearer {VAULT_TOKEN}").as_str()) {
            return Ok(vault_error(401, "Not signed in"));
        }
        self.route(&req.method, path, query, req)
    }
}

/// Session with the fake vault as its Vault (not signed in yet).
pub fn session_with_vault(vault: &FakeVault) -> Session {
    let mut s = session();
    s.vault = crate::vault::Vault::new(Box::new(vault.clone()));
    s
}

/// Isolates HTTP-only tests from the real user's credentials.
pub struct NoCredentials;
impl openecraft_client::credentials::CredentialStore for NoCredentials {
    fn load(&self, _: &str) -> Result<Option<String>, openecraft_client::credentials::CredentialError> { Ok(None) }
    fn save(&self, _: &str, _: &str) -> Result<bool, openecraft_client::credentials::CredentialError> { Ok(false) }
    fn forget(&self, _: &str) -> Result<bool, openecraft_client::credentials::CredentialError> { Ok(false) }
}
