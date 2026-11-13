use openecraft_i18n::{tr, tr_fmt};
use openecraft_parts::access::Policy;
use openecraft_parts::api::{self, PartQuery, API_PREFIX, DEFAULT_PORT};
use openecraft_parts::portal::{AttachmentKind, Catalog, PartsRight};
use openecraft_parts::Part;
use serde::de::DeserializeOwned;

/// Server address and access token; builds the requests of the API. Requests carry
/// the token as `Authorization: Bearer …`.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub url: String,
    pub token: String,
}

impl Default for Endpoint {
    fn default() -> Self {
        Endpoint { url: format!("http://127.0.0.1:{DEFAULT_PORT}"), token: String::new() }
    }
}

impl Endpoint {
    fn url(&self, path: &str) -> String {
        api::url(&self.url, path)
    }

    fn authorized(&self, mut req: ehttp::Request) -> ehttp::Request {
        if !self.token.trim().is_empty() {
            req.headers.insert("Authorization", format!("Bearer {}", self.token.trim()));
        }
        req
    }

    fn json<T: serde::Serialize + ?Sized>(&self, method: ehttp::Method, path: &str, body: &T) -> ehttp::Request {
        let mut req = ehttp::Request::post(self.url(path), serde_json::to_vec(body).unwrap_or_default());
        req.method = method;
        self.authorized(with_content_type(req, "application/json"))
    }

    /// GET /api/v1/info, with the token so that the answer names the user.
    pub fn info(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/info"))))
    }

    /// POST /api/v1/search: parts with facets.
    pub fn search(&self, query: &PartQuery) -> ehttp::Request {
        self.json(ehttp::Method::POST, &format!("{API_PREFIX}/search"), query)
    }

    /// GET /api/v1/parts/{number}/accessories: its accessories and the parts it fits.
    pub fn accessories(&self, number: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&api::accessories_path(number))))
    }

    /// GET /api/v1/parts/{number}/history
    pub fn history(&self, number: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{}/history", api::part_path(number)))))
    }

    /// GET /api/v1/parts/{number}/history/{revision}: the part as of a change.
    pub fn history_part(&self, number: &str, revision: u64) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{}/history/{revision}", api::part_path(number)))))
    }

    /// POST /api/v1/parts/{number}/attachments: stores a file with the part.
    pub fn attach(&self, number: &str, name: &str, kind: Option<AttachmentKind>, data: Vec<u8>) -> ehttp::Request {
        let mut path = format!("{}/attachments?name={}", api::part_path(number), api::encode_component(name));
        if let Some(k) = kind {
            path.push_str(&format!("&kind={k:?}"));
        }
        self.authorized(with_content_type(ehttp::Request::post(self.url(&path), data), "application/octet-stream"))
    }

    /// DELETE /api/v1/parts/{number}/attachments/{id}
    pub fn detach(&self, number: &str, id: &str) -> ehttp::Request {
        let path = format!("{}/attachments/{}", api::part_path(number), api::encode_component(id));
        self.authorized(ehttp::Request::delete(&self.url(&path)))
    }

    /// Address of an attachment's file (for downloads or the browser).
    pub fn file_url(&self, id: &str) -> String {
        self.url(&api::file_path(id))
    }

    /// GET /api/v1/files/{id}
    pub fn file(&self, id: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.file_url(id)))
    }

    /// GET /api/v1/catalogs
    pub fn catalogs(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/catalogs"))))
    }

    /// PUT /api/v1/catalogs/{id}
    pub fn save_catalog(&self, catalog: &Catalog) -> ehttp::Request {
        self.json(ehttp::Method::PUT, &api::catalog_path(&catalog.id), catalog)
    }

    /// DELETE /api/v1/catalogs/{id}
    pub fn delete_catalog(&self, id: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::delete(&self.url(&api::catalog_path(id))))
    }

    /// GET /api/v1/catalogs/{id}/logo: the image of the catalog's logo.
    pub fn catalog_logo(&self, id: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&api::catalog_logo_path(id))))
    }

    /// PUT /api/v1/catalogs/{id}/logo?name=: sets the logo (the file name gives its type).
    pub fn set_catalog_logo(&self, id: &str, name: &str, data: Vec<u8>) -> ehttp::Request {
        let path = format!("{}?name={}", api::catalog_logo_path(id), api::encode_component(name));
        let mut req = ehttp::Request::post(self.url(&path), data);
        req.method = ehttp::Method::PUT;
        self.authorized(with_content_type(req, "application/octet-stream"))
    }

    /// DELETE /api/v1/catalogs/{id}/logo
    pub fn delete_catalog_logo(&self, id: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::delete(&self.url(&api::catalog_logo_path(id))))
    }

    /// GET /api/v1/libraries/{name}: a library shared by all projects (404: none yet).
    pub fn library(&self, name: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&api::library_path(name))))
    }

    /// PUT /api/v1/libraries/{name} with the library (a JSON object).
    pub fn save_library(&self, name: &str, json: &str) -> ehttp::Request {
        let mut req = ehttp::Request::post(self.url(&api::library_path(name)), json.as_bytes().to_vec());
        req.method = ehttp::Method::PUT;
        self.authorized(with_content_type(req, "application/json"))
    }

    /// POST /api/v1/login (no token needed) → [`api::Session`]
    pub fn login(&self, name: &str, password: &str) -> ehttp::Request {
        let body = api::Login { name: name.trim().into(), password: password.into(), ..Default::default() };
        Endpoint { token: String::new(), ..self.clone() }.json(ehttp::Method::POST, &format!("{API_PREFIX}/login"), &body)
    }

    /// POST /api/v1/logout: ends the session of the token.
    pub fn logout(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::post(self.url(&format!("{API_PREFIX}/logout")), Vec::new()))
    }

    /// GET /api/v1/favorites
    pub fn favorites(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/favorites"))))
    }

    /// PUT /api/v1/favorites
    pub fn set_favorites(&self, numbers: &[String]) -> ehttp::Request {
        self.json(ehttp::Method::PUT, &format!("{API_PREFIX}/favorites"), numbers)
    }

    /// GET /api/v1/users (administrator)
    pub fn users(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/users"))))
    }

    /// PUT /api/v1/users/{name} (administrator)
    pub fn save_user(&self, name: &str, update: &api::UserUpdate) -> ehttp::Request {
        self.json(ehttp::Method::PUT, &api::user_path(name), update)
    }

    /// POST /api/v1/users/{name}/approval: lets a registered user sign in (administrator)
    pub fn approve_user(&self, name: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::post(self.url(&format!("{}/approval", api::user_path(name))), Vec::new()))
    }

    /// DELETE /api/v1/users/{name} (administrator)
    pub fn delete_user(&self, name: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::delete(&self.url(&api::user_path(name))))
    }

    /// GET /api/v1/access: roles, grants and directories (administrator)
    pub fn access(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/access"))))
    }

    /// GET /api/v1/registration: whether users may register themselves (administrator)
    pub fn registration(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/registration"))))
    }

    /// PUT /api/v1/registration: opens or closes the registration, with or without
    /// approval (administrator)
    pub fn save_registration(&self, registration: &api::Registration) -> ehttp::Request {
        self.json(ehttp::Method::PUT, &format!("{API_PREFIX}/registration"), registration)
    }

    /// PUT /api/v1/access: replaces roles and grants (administrator)
    pub fn save_access(&self, policy: &Policy<PartsRight>) -> ehttp::Request {
        self.json(ehttp::Method::PUT, &format!("{API_PREFIX}/access"), policy)
    }

    /// GET /api/v1/parts?…
    pub fn list(&self, query: &PartQuery) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/parts{}", query.to_query_string()))))
    }

    /// GET /api/v1/parts/{number}
    pub fn part(&self, number: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&api::part_path(number))))
    }

    /// PUT /api/v1/parts/{number}: creates or replaces a part.
    pub fn save(&self, part: &Part) -> ehttp::Request {
        let body = serde_json::to_vec(part).unwrap_or_default();
        self.authorized(with_content_type(ehttp::Request::put(self.url(&api::part_path(&part.number)), body), "application/json"))
    }

    /// DELETE /api/v1/parts/{number}
    pub fn delete(&self, number: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::delete(&self.url(&api::part_path(number))))
    }

    /// POST /api/v1/import with CSV or JSON parts.
    pub fn import(&self, body: Vec<u8>, content_type: &str) -> ehttp::Request {
        self.authorized(with_content_type(ehttp::Request::post(self.url(&format!("{API_PREFIX}/import")), body), content_type))
    }

    /// GET /api/v1/export.csv
    pub fn export(&self) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(&format!("{API_PREFIX}/export.csv"))))
    }
}

/// Sets the content type. `Headers::insert` would add a second one next to the
/// `text/plain` ehttp sets for requests with a body, and the server uses the first.
pub(crate) fn with_content_type(mut req: ehttp::Request, content_type: &str) -> ehttp::Request {
    req.headers.headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
    req.headers.insert("Content-Type", content_type);
    req
}

/// Why a request did not reach the server, in the program's language where the cause
/// is known (the operating system words its own errors in the system language).
pub(crate) fn reason(error: &str) -> String {
    let e = error.to_lowercase();
    let has = |codes: &[&str]| codes.iter().any(|c| e.contains(c));
    if has(&["os error 10061", "os error 111)", "os error 61)", "connection refused"]) {
        tr("the server refuses the connection (not started, or another port)").to_string()
    } else if has(&["os error 11001", "os error 11004", "failed to lookup", "name or service not known", "no such host", "dns error"]) {
        tr("the server name is unknown").to_string()
    } else if has(&["os error 10060", "timed out", "timeout"]) {
        tr("the server does not answer in time").to_string()
    } else if has(&["invalid uri", "relative url", "unknown scheme", "empty host", "url error", "invalid url"]) {
        tr("the address is not a valid URL (e.g. http://server:8090)").to_string()
    } else {
        // "io: ..." / "http: ..." of the HTTP library
        error.split_once(": ").filter(|(kind, _)| matches!(*kind, "io" | "http" | "transport")).map_or(error, |(_, rest)| rest).to_string()
    }
}

/// Scheme, host and port of an address (`https://Host:443/x` → `https`, `host`, 443);
/// None for an address without scheme or host.
pub fn origin(url: &str) -> Option<(String, String, u16)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Credentials in the address hide the real host.
    if authority.contains('@') || authority.is_empty() {
        return None;
    }
    // `[::1]:8080` (IPv6) or `host:8080`.
    let split = match authority.strip_prefix('[') {
        Some(v6) => v6.split_once(']').map(|(h, rest)| (h, rest.strip_prefix(':'))),
        None => Some(authority.split_once(':').map_or((authority, None), |(h, p)| (h, Some(p)))),
    };
    let (host, port) = split?;
    let port = match port {
        Some(p) => p.parse().ok()?,
        None if scheme == "https" => 443,
        None => 80,
    };
    (!host.is_empty()).then(|| (scheme, host.to_ascii_lowercase(), port))
}

/// Both addresses are on the same server (scheme, host and port): a token for one may
/// be sent to the other.
pub fn same_origin(a: &str, b: &str) -> bool {
    origin(a).is_some_and(|o| Some(o) == origin(b))
}

/// Successful response or a readable error (server message, HTTP status, connection error).
pub fn check(result: ehttp::Result<ehttp::Response>) -> Result<ehttp::Response, String> {
    match result {
        Err(e) => Err(tr_fmt!("connection failed: {}", crate::reason(&e))),
        Ok(resp) if resp.ok => Ok(resp),
        Ok(resp) => Err(api::error_text(resp.status, &resp.status_text, &resp.bytes)),
    }
}

/// Turns an HTTP result into the expected JSON type or a readable error.
pub fn decode<T: DeserializeOwned>(result: ehttp::Result<ehttp::Response>) -> Result<T, String> {
    let resp = check(result)?;
    serde_json::from_slice(&resp.bytes).map_err(|e| tr_fmt!("unexpected answer: {}", e))
}

/// Like [`decode`], but `None` if the server does not know the resource (404).
pub fn decode_found<T: DeserializeOwned>(result: ehttp::Result<ehttp::Response>) -> Result<Option<T>, String> {
    match result {
        Ok(resp) if resp.status == 404 => Ok(None),
        result => decode(result).map(Some),
    }
}

/// The answer as text (CSV export).
pub fn text(result: ehttp::Result<ehttp::Response>) -> Result<String, String> {
    String::from_utf8(check(result)?.bytes).map_err(|_| tr("The server sent no text").to_string())
}

#[cfg(test)]
mod tests;
