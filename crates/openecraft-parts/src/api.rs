//! HTTP API of the parts server (data portal), shared by the server and its clients.
//!
//! | Method | Path | |
//! | --- | --- | --- |
//! | GET | `/api/v1/info` | [`ServerInfo`] (with the caller's [`User`]) |
//! | GET | `/api/v1/parts?q=&group=&manufacturer=&catalog=&offset=&limit=` | [`PartList`] |
//! | POST | `/api/v1/search` | [`PartQuery`] JSON → [`SearchResult`] with facets |
//! | GET | `/api/v1/parts/{number}` | [`Part`] |
//! | PUT | `/api/v1/parts/{number}` | create / replace a part (editor) |
//! | DELETE | `/api/v1/parts/{number}` | delete a part (editor) |
//! | GET | `/api/v1/parts/{number}/accessories` | [`Accessories`]: its accessories and the parts it fits |
//! | GET | `/api/v1/parts/{number}/history` | [`HistoryEntry`] list, newest first |
//! | GET | `/api/v1/parts/{number}/history/{revision}` | the part as of that change |
//! | POST | `/api/v1/parts/{number}/attachments?name=&kind=` | file body → [`Part`] (editor) |
//! | DELETE | `/api/v1/parts/{number}/attachments/{id}` | → [`Part`] (editor) |
//! | GET | `/api/v1/files/{id}` | file of an attachment |
//! | POST | `/api/v1/import` | CSV, JSON or BMEcat body, upserts parts (editor) → [`ImportResult`] |
//! | GET | `/api/v1/export.csv` | all parts as CSV |
//! | GET | `/api/v1/export.json` | all parts as a JSON part list (with models), as `/import` reads it |
//! | GET | `/api/v1/backup` | the whole database as a SQLite file (administrator) |
//! | GET | `/api/v1/catalogs` | [`Catalog`] list |
//! | PUT / DELETE | `/api/v1/catalogs/{id}` | create, change, delete a catalog (editor) |
//! | POST | `/api/v1/login` | [`Login`] → [`Session`] |
//! | POST | `/api/v1/register` | [`Login`] → [`Session`]: a new reader account, if [`ServerInfo::registration`]; `202` and a session without token while it awaits approval |
//! | GET / PUT | `/api/v1/registration` | [`Registration`]: whether users may register themselves (administrator) |
//! | POST | `/api/v1/logout` | ends the session of the token |
//! | GET / PUT | `/api/v1/favorites` | part numbers of the caller (signed in) |
//! | GET | `/api/v1/users` | [`UserEntry`] list: users with their sessions (administrator) |
//! | PUT / DELETE | `/api/v1/users/{name}` | [`UserUpdate`] (administrator) |
//! | POST | `/api/v1/users/{name}/approval` | approves a registered user → [`User`] (administrator) |
//! | DELETE | `/api/v1/users/{name}/sessions` | signs the user out everywhere (administrator) |
//! | GET | `/api/v1/access` | [`Settings`](crate::access::Settings) of [`PartsRight`] (administrator) |
//! | PUT | `/api/v1/access` | [`Policy`](crate::access::Policy) of [`PartsRight`] (administrator) |
//! | GET | `/api/v1/bundled` | [`BundledCatalog`] list: catalogs shipped with the server (administrator) |
//! | POST | `/api/v1/bundled/{id}` | installs or updates a bundled catalog (administrator) → [`Installed`] |
//! | GET | `/api/v1/texts?lang=de` | translations of the web portal |
//!
//! Requests of a user carry `Authorization: Bearer <token>`: the token of a
//! [`Session`] or the server's administrator token (`--token`). Users sign in with a
//! local account or with a directory of the server (Active Directory, LDAP). What
//! they may do comes from their role and the grants of the server's policy
//! ([`crate::access`]); the scope of a right is the manufacturer. Errors are returned
//! as [`ApiError`] JSON.

use crate::access::{Access, SERVER};
use crate::portal::{Lifecycle, PartsRight, Role, Sort, User};
use crate::TopGroup;
use serde::{Deserialize, Serialize};

pub use crate::portal::{Catalog, HistoryEntry, SearchResult};

pub const API_PREFIX: &str = "/api/v1";
pub const DEFAULT_PORT: u16 = 8090;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub parts: usize,
    /// Incremented on every change of the database.
    pub revision: u64,
    /// False if nobody can change the database (no administrator token, no users).
    pub writable: bool,
    pub catalogs: usize,
    /// Storage backend (`sqlite`, `json`).
    pub storage: String,
    /// The user of the request's token (`None`: anonymous).
    pub user: Option<User>,
    /// What the caller may do (role and grants of the server's policy).
    pub access: Access<PartsRight>,
    /// Anybody may create an account (`POST /api/v1/register`).
    pub registration: bool,
}

impl ServerInfo {
    /// The caller's rights; from the user's role if the server sends none (older
    /// servers).
    pub fn rights(&self) -> Access<PartsRight> {
        match &self.user {
            Some(u) if self.access.entries.is_empty() => u.access(),
            _ => self.access.clone(),
        }
    }

    /// True if the caller has the right on parts or catalogs of this manufacturer.
    pub fn may(&self, right: PartsRight, manufacturer: &str) -> bool {
        self.rights().allows(manufacturer.trim(), right)
    }

    /// True if the caller may change parts of this manufacturer.
    pub fn may_edit(&self, manufacturer: &str) -> bool {
        self.may(PartsRight::EditParts, manufacturer)
    }

    /// True if the caller may change any parts at all.
    pub fn can_edit(&self) -> bool {
        self.rights().anywhere(PartsRight::EditParts)
    }

    /// True if the caller manages users, roles and grants.
    pub fn is_admin(&self) -> bool {
        self.rights().allows(SERVER, PartsRight::Administer)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartList {
    pub revision: u64,
    /// Number of matching parts (before offset / limit).
    pub total: usize,
    pub offset: usize,
    pub parts: Vec<crate::Part>,
}

/// The accessories of a part and, if it is an accessory itself, the parts it fits
/// (both sides of the pairs, see [`crate::accessory`]); parts without their models.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accessories {
    pub accessories: Vec<crate::Part>,
    pub fits: Vec<crate::Part>,
}

impl Accessories {
    /// The accessories and the parts `part` fits among `parts`.
    pub fn of(parts: &[crate::Part], part: &crate::Part) -> Accessories {
        let brief = |v: Vec<&crate::Part>| v.into_iter().map(crate::portal::brief).collect();
        Accessories { accessories: brief(crate::accessory::accessories_of(parts, part)), fits: brief(crate::accessory::mains_of(parts, part)) }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportResult {
    pub imported: usize,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiError {
    pub error: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Login {
    /// Also read as `user` (the field of the project server).
    #[serde(alias = "user")]
    pub name: String,
    pub password: String,
    /// Directory to sign in with; empty = the local accounts, then every directory.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub provider: String,
}

/// A signed in user and the token for its requests.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub token: String,
    pub user: User,
    /// What the user may do.
    pub access: Access<PartsRight>,
}

/// Creates or changes a user. An empty password keeps the current one (a new local
/// user needs one; users of a directory have none on the server).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserUpdate {
    pub role: Role,
    pub manufacturer: String,
    pub password: String,
    /// Local groups of the user; `None` keeps them. Groups of directory users come
    /// from the directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
}

/// Whether users may register themselves (`GET|PUT /api/v1/registration`; `PUT`
/// sets `enabled` and `approval`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Registration {
    pub enabled: bool,
    /// `enabled` is set by the start options of the server (`--registration`): not
    /// changeable here.
    pub fixed: bool,
    /// New accounts wait for the approval of an administrator before they can sign in.
    pub approval: bool,
}

impl Default for Registration {
    fn default() -> Self {
        Registration { enabled: false, fixed: false, approval: true }
    }
}

/// A user as administrators see it (`GET /api/v1/users`): the [`User`] fields and its
/// sessions. Clients that only know [`User`] read the list as users.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserEntry {
    #[serde(flatten)]
    pub user: User,
    /// Sessions that have not expired.
    pub sessions: usize,
    /// Unix time of the newest sign-in (0: none kept).
    pub signed_in: u64,
}

/// A manufacturer catalog shipped with the server (`GET /api/v1/bundled`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BundledCatalog {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub version: String,
    pub parts: usize,
    /// Version of the catalog on the server; empty if it is not installed.
    pub installed: String,
}

/// What installing a bundled catalog did (`POST /api/v1/bundled/{id}`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Installed {
    pub added: usize,
    pub updated: usize,
    /// Numbers of the parts left as they are (changed by users).
    pub kept: Vec<String>,
}

/// Query parameters of `GET /api/v1/parts`, body of `POST /api/v1/search`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartQuery {
    /// Free text (number, type, manufacturer, description, groups, catalog); every
    /// word must occur.
    pub q: Option<String>,
    pub top_group: Option<TopGroup>,
    /// Product group (exact, case insensitive).
    pub group: Option<String>,
    pub subgroup: Option<String>,
    /// Manufacturer (exact, case insensitive).
    pub manufacturer: Option<String>,
    /// Catalog id (exact, case insensitive).
    pub catalog: Option<String>,
    pub lifecycle: Option<Lifecycle>,
    /// Technical attributes (name, value) that must match (search only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<(String, String)>,
    /// Only these part numbers (search only; update checks).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub numbers: Vec<String>,
    /// Only parts changed after this server revision.
    pub changed_since: Option<u64>,
    pub sort: Sort,
    pub descending: bool,
    /// Leave out the models of the parts (lists).
    pub brief: bool,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

impl PartQuery {
    /// `?q=…&group=…` (empty if no parameter is set). Attributes and numbers are only
    /// sent with `POST /api/v1/search`.
    pub fn to_query_string(&self) -> String {
        let mut params = Vec::new();
        for (k, v) in [("q", &self.q), ("group", &self.group), ("subgroup", &self.subgroup), ("manufacturer", &self.manufacturer), ("catalog", &self.catalog)] {
            if let Some(v) = v {
                params.push(format!("{k}={}", encode_component(v)));
            }
        }
        if let Some(t) = self.top_group {
            params.push(format!("top_group={t:?}"));
        }
        if let Some(l) = self.lifecycle {
            params.push(format!("lifecycle={l:?}"));
        }
        if let Some(r) = self.changed_since {
            params.push(format!("changed_since={r}"));
        }
        if self.sort != Sort::Number {
            params.push(format!("sort={:?}", self.sort));
        }
        if self.descending {
            params.push("descending=true".into());
        }
        if self.brief {
            params.push("brief=true".into());
        }
        if let Some(o) = self.offset {
            params.push(format!("offset={o}"));
        }
        if let Some(l) = self.limit {
            params.push(format!("limit={l}"));
        }
        if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        }
    }
}

/// Readable message of a failed request: the server's [`ApiError`], otherwise the
/// HTTP status.
pub fn error_text(status: u16, status_text: &str, body: &[u8]) -> String {
    serde_json::from_slice::<ApiError>(body).map(|e| e.error).unwrap_or_else(|_| format!("{status} {status_text}"))
}

/// Percent-encodes everything except unreserved URL characters (RFC 3986).
pub fn encode_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Path of a single part, e.g. `/api/v1/parts/GEN.CT-9A-24DC`.
pub fn part_path(number: &str) -> String {
    format!("{API_PREFIX}/parts/{}", encode_component(number))
}

/// Path of the accessories of a part.
pub fn accessories_path(number: &str) -> String {
    format!("{}/accessories", part_path(number))
}

/// Path of an attachment's file.
pub fn file_path(id: &str) -> String {
    format!("{API_PREFIX}/files/{}", encode_component(id))
}

pub fn catalog_path(id: &str) -> String {
    format!("{API_PREFIX}/catalogs/{}", encode_component(id))
}

pub fn user_path(name: &str) -> String {
    format!("{API_PREFIX}/users/{}", encode_component(name))
}

/// Joins a server base URL (with or without trailing slash) and an API path.
pub fn url(base: &str, path: &str) -> String {
    format!("{}{path}", base.trim().trim_end_matches('/'))
}

/// The token of an `Authorization: Bearer <token>` header value.
pub fn bearer_token(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ").map(str::trim)
}

/// An address that may be opened or exported as a link: a web address (http, https)
/// or an e-mail address (mailto), never one that runs code (`javascript:`), opens
/// files or programs of the machine (`file:`, UNC paths) or other apps.
pub fn is_web_link(url: &str) -> bool {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once(':') else { return false };
    let web = matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") && rest.starts_with("//") && rest.len() > 2;
    let mail = scheme.eq_ignore_ascii_case("mailto") && !rest.is_empty();
    (web || mail) && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Compares a presented token with the expected one in constant time.
pub fn token_matches(presented: &str, expected: &str) -> bool {
    presented.len() == expected.len() && presented.bytes().zip(expected.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests;
