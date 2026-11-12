//! Blocking client for the parts server (`openecraft-parts-server`) or the EPLAN Data
//! Portal (searching and reading only). The requests come from `openecraft_client`;
//! the transport is a trait so that the tools can be tested against an in-memory server.

use crate::args::Options;
use openecraft_client::{check, decode, decode_found, ehttp, text, Call, Connection, Endpoint, PortalKind};
use openecraft_client::credentials::{CredentialStore, SystemCredentialStore};
use openecraft_parts::access::{Policy, Settings};
use openecraft_parts::api::{ImportResult, PartQuery, ServerInfo, Session, UserUpdate};
use openecraft_parts::portal::{AttachmentKind, Catalog, HistoryEntry, PartsRight, SearchResult, User};
use openecraft_parts::Part;

pub trait Transport {
    fn fetch(&self, request: &ehttp::Request) -> ehttp::Result<ehttp::Response>;
}

pub struct Http;

impl Transport for Http {
    fn fetch(&self, request: &ehttp::Request) -> ehttp::Result<ehttp::Response> {
        ehttp::fetch_blocking(request)
    }
}

pub struct Server {
    /// Address and token; for the EPLAN Data Portal its address and personal access token.
    pub endpoint: Endpoint,
    pub kind: PortalKind,
    pub transport: Box<dyn Transport>,
    pub credentials: Box<dyn CredentialStore>,
}

impl Server {
    /// Server from `--server` / `--token`, otherwise from the environment
    /// (`OPENECRAFT_SERVER`, `OPENECRAFT_TOKEN`), otherwise the local default.
    pub fn from_options(o: &Options) -> Server {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let default = Endpoint::default();
        let endpoint = Endpoint {
            url: o.server.clone().or_else(|| env("OPENECRAFT_SERVER")).unwrap_or(default.url),
            token: o.token.clone().or_else(|| env("OPENECRAFT_TOKEN")).unwrap_or_default(),
        };
        Server { endpoint, kind: PortalKind::OpenECraft, transport: Box::new(Http), credentials: Box::new(SystemCredentialStore) }
    }

    fn fetch(&self, request: ehttp::Request) -> ehttp::Result<ehttp::Response> {
        self.transport.fetch(&request)
    }

    pub fn connection(&self) -> Result<Connection, String> {
        openecraft_client::credentials::connection_with_store(self.kind, &self.endpoint.url, &self.endpoint.token, self.credentials.as_ref())
    }

    fn call<T: 'static>(&self, call: Call<T>) -> Result<T, String> {
        call.run(|req| self.transport.fetch(req))
    }

    /// The parts server for the functions the EPLAN Data Portal does not offer.
    fn full(&self) -> Result<&Endpoint, String> {
        Connection::new(self.kind, &self.endpoint.url, &self.endpoint.token).parts_server()?;
        Ok(&self.endpoint)
    }

    pub fn info(&self) -> Result<ServerInfo, String> {
        self.call(self.connection()?.info())
    }

    /// The part, `None` if the server does not know it.
    pub fn get(&self, number: &str) -> Result<Option<Part>, String> {
        self.call(self.connection()?.part(number))
    }

    pub fn put(&self, part: &Part) -> Result<(), String> {
        check(self.fetch(self.full()?.save(part))).map(|_| ())
    }

    pub fn delete(&self, number: &str) -> Result<(), String> {
        check(self.fetch(self.full()?.delete(number))).map(|_| ())
    }

    /// Uploads the content of a parts file (CSV, JSON, BMEcat, EPLAN XML).
    pub fn import(&self, content: Vec<u8>) -> Result<ImportResult, String> {
        decode(self.fetch(self.full()?.import(content, "text/plain; charset=utf-8")))
    }

    /// All parts as CSV.
    pub fn export(&self) -> Result<String, String> {
        text(self.fetch(self.full()?.export()))
    }

    pub fn search(&self, query: &PartQuery) -> Result<SearchResult, String> {
        self.call(self.connection()?.search(query))
    }

    /// The server versions of local parts (update check); the EPLAN Data Portal finds
    /// them by their portal ids.
    pub fn versions(&self, local: &[Part]) -> Result<Vec<Part>, String> {
        let local = self.kind.update_candidates(local);
        let numbers: Vec<String> = local.iter().map(|p| p.number.clone()).collect();
        let ids: Vec<String> = local.iter().map(|p| p.portal_id.clone()).filter(|id| !id.is_empty()).collect();
        if numbers.is_empty() || (self.kind.read_only() && ids.is_empty()) {
            return Ok(Vec::new());
        }
        Ok(self.call(self.connection()?.find(&numbers, &ids))?.parts)
    }

    pub fn history(&self, number: &str) -> Result<Vec<HistoryEntry>, String> {
        decode(self.fetch(self.full()?.history(number)))
    }

    pub fn history_part(&self, number: &str, revision: u64) -> Result<Option<Part>, String> {
        decode_found(self.fetch(self.full()?.history_part(number, revision)))
    }

    pub fn attach(&self, number: &str, name: &str, kind: Option<AttachmentKind>, data: Vec<u8>) -> Result<Part, String> {
        decode(self.fetch(self.full()?.attach(number, name, kind, data)))
    }

    pub fn detach(&self, number: &str, id: &str) -> Result<Part, String> {
        decode(self.fetch(self.full()?.detach(number, id)))
    }

    pub fn file(&self, id: &str) -> Result<Vec<u8>, String> {
        self.call(self.connection()?.file(id))
    }

    pub fn catalogs(&self) -> Result<Vec<Catalog>, String> {
        decode(self.fetch(self.full()?.catalogs()))
    }

    pub fn save_catalog(&self, catalog: &Catalog) -> Result<Catalog, String> {
        decode(self.fetch(self.full()?.save_catalog(catalog)))
    }

    pub fn delete_catalog(&self, id: &str) -> Result<(), String> {
        check(self.fetch(self.full()?.delete_catalog(id))).map(|_| ())
    }

    /// Signs in and uses the session's token from now on.
    pub fn login(&mut self, name: &str, password: &str) -> Result<Session, String> {
        let session: Session = decode(self.fetch(self.full()?.login(name, password)))?;
        self.endpoint.token = session.token.clone();
        Ok(session)
    }

    pub fn favorites(&self) -> Result<Vec<String>, String> {
        decode(self.fetch(self.full()?.favorites()))
    }

    pub fn set_favorites(&self, numbers: &[String]) -> Result<Vec<String>, String> {
        decode(self.fetch(self.full()?.set_favorites(numbers)))
    }

    pub fn users(&self) -> Result<Vec<User>, String> {
        decode(self.fetch(self.full()?.users()))
    }

    pub fn save_user(&self, name: &str, update: &UserUpdate) -> Result<User, String> {
        decode(self.fetch(self.full()?.save_user(name, update)))
    }

    pub fn delete_user(&self, name: &str) -> Result<(), String> {
        check(self.fetch(self.full()?.delete_user(name))).map(|_| ())
    }

    pub fn access(&self) -> Result<Settings<PartsRight>, String> {
        decode(self.fetch(self.full()?.access()))
    }

    pub fn save_access(&self, policy: &Policy<PartsRight>) -> Result<Settings<PartsRight>, String> {
        decode(self.fetch(self.full()?.save_access(policy)))
    }
}
