//! Connection to a source of parts: the OpenECraft parts server or the EPLAN Data
//! Portal (through [`EplanEndpoint`]). Both answer the reading requests in the types
//! of the parts server API; the other functions only exist on the parts server.

use crate::eplan::{self, EplanEndpoint};
use crate::{check, decode, decode_found, Endpoint};
use openecraft_i18n::tr;
use openecraft_parts::api::{Accessories, PartQuery, ServerInfo};
use openecraft_parts::portal::SearchResult;
use openecraft_parts::Part;
use serde::{Deserialize, Serialize};

pub type Decoder<T> = Box<dyn FnOnce(ehttp::Result<ehttp::Response>) -> Result<T, String> + Send>;
type Follow<T> = Box<dyn FnOnce(ehttp::Result<ehttp::Response>) -> Result<Call<T>, String> + Send>;

/// A request and the decoding of its answer.
pub struct Call<T> {
    pub request: ehttp::Request,
    handler: Handler<T>,
}

enum Handler<T> {
    Decode(Decoder<T>),
    Follow(Follow<T>),
}

impl<T: 'static> Call<T> {
    pub fn new(request: ehttp::Request, decode: impl FnOnce(ehttp::Result<ehttp::Response>) -> Result<T, String> + Send + 'static) -> Call<T> {
        Call { request, handler: Handler::Decode(Box::new(decode)) }
    }

    /// Choose a follow-up request from the answer (shared by blocking and GUI clients).
    pub fn then(request: ehttp::Request, follow: impl FnOnce(ehttp::Result<ehttp::Response>) -> Result<Call<T>, String> + Send + 'static) -> Call<T> {
        Call { request, handler: Handler::Follow(Box::new(follow)) }
    }

    /// Sends the request with `fetch` (blocking) and decodes the answer.
    pub fn run(mut self, mut fetch: impl FnMut(&ehttp::Request) -> ehttp::Result<ehttp::Response>) -> Result<T, String> {
        loop {
            let response = fetch(&self.request);
            match self.handler {
                Handler::Decode(decode) => return decode(response),
                Handler::Follow(follow) => self = follow(response)?,
            }
        }
    }

    /// Send asynchronously, including any dependent requests, and deliver one result.
    pub fn fetch(self, done: impl FnOnce(Result<T, String>) + Send + 'static) {
        ehttp::fetch(self.request, move |response| match self.handler {
            Handler::Decode(decode) => done(decode(response)),
            Handler::Follow(follow) => match follow(response) {
                Ok(next) => next.fetch(done),
                Err(error) => done(Err(error)),
            },
        });
    }
}

/// Kind of the server a parts connection talks to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortalKind {
    /// `openecraft-parts-server`.
    #[default]
    OpenECraft,
    /// The EPLAN Data Portal (read-only).
    Eplan,
}

impl PortalKind {
    pub const ALL: [PortalKind; 2] = [PortalKind::OpenECraft, PortalKind::Eplan];

    pub fn title(self) -> &'static str {
        match self {
            PortalKind::OpenECraft => "OpenECraft parts server",
            PortalKind::Eplan => "EPLAN Data Portal",
        }
    }

    pub fn default_url(self) -> String {
        match self {
            PortalKind::OpenECraft => Endpoint::default().url,
            PortalKind::Eplan => EplanEndpoint::default().url,
        }
    }

    /// Only searching and reading parts and their files: no changes, accounts,
    /// favorites, history or download of the whole database.
    pub fn read_only(self) -> bool {
        self == PortalKind::Eplan
    }

    /// The local parts an update check compares: on the EPLAN Data Portal the parts
    /// that were loaded from there.
    pub fn update_candidates(self, local: &[Part]) -> Vec<Part> {
        local.iter().filter(|p| self == PortalKind::OpenECraft || !p.portal_id.is_empty()).cloned().collect()
    }
}

/// A parts server of either kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Connection {
    Parts(Endpoint),
    Eplan(EplanEndpoint),
}

impl Connection {
    pub fn new(kind: PortalKind, url: &str, token: &str) -> Connection {
        let (url, token) = (url.to_string(), token.to_string());
        match kind {
            PortalKind::OpenECraft => Connection::Parts(Endpoint { url, token }),
            PortalKind::Eplan => Connection::Eplan(EplanEndpoint { url, token }),
        }
    }

    pub fn kind(&self) -> PortalKind {
        match self {
            Connection::Parts(_) => PortalKind::OpenECraft,
            Connection::Eplan(_) => PortalKind::Eplan,
        }
    }

    /// The parts server for the functions only it offers.
    pub fn parts_server(&self) -> Result<&Endpoint, String> {
        match self {
            Connection::Parts(e) => Ok(e),
            Connection::Eplan(_) => Err(tr("The EPLAN Data Portal only offers searching and reading parts").into()),
        }
    }

    pub fn info(&self) -> Call<ServerInfo> {
        match self {
            Connection::Parts(e) => Call::new(e.info(), decode),
            Connection::Eplan(e) => e.connection_check(),
        }
    }

    /// Faceted search; a query with part numbers loads these parts (the EPLAN Data
    /// Portal only the first one, see [`Connection::find`]).
    pub fn search(&self, query: &PartQuery) -> Call<SearchResult> {
        match self {
            Connection::Parts(e) => Call::new(e.search(query), decode),
            Connection::Eplan(_) if !query.numbers.is_empty() => self.find(&query.numbers, &[]),
            Connection::Eplan(e) => {
                let q = query.clone();
                Call::new(e.search(query), move |r| eplan::decode_search(&q, r))
            }
        }
    }

    /// The parts with these numbers. The EPLAN Data Portal finds several parts only by
    /// their portal ids (`ids`, from [`Part::portal_id`]); without ids it loads the
    /// first number.
    pub fn find(&self, numbers: &[String], ids: &[String]) -> Call<SearchResult> {
        let query = PartQuery { numbers: numbers.to_vec(), ..Default::default() };
        match self {
            Connection::Parts(e) => Call::new(e.search(&query), decode),
            Connection::Eplan(e) => Call::new(e.lookup(numbers, ids), move |r| eplan::decode_search(&query, r)),
        }
    }

    /// The part, `None` if the server does not know it.
    pub fn part(&self, number: &str) -> Call<Option<Part>> {
        match self {
            Connection::Parts(e) => Call::new(e.part(number), decode_found),
            Connection::Eplan(e) => {
                let n = number.to_string();
                Call::new(e.part(number), move |r| eplan::decode_part(&n, r))
            }
        }
    }

    /// The accessories of a part and the parts it fits. The EPLAN Data Portal only
    /// names the accessories the part lists (by number).
    pub fn accessories(&self, number: &str) -> Call<Accessories> {
        match self {
            Connection::Parts(e) => Call::new(e.accessories(number), decode),
            Connection::Eplan(e) => {
                let n = number.to_string();
                Call::new(e.part(number), move |r| {
                    let part = eplan::decode_part(&n, r)?.unwrap_or_default();
                    let accessories = part.accessories.iter().map(|a| Part { number: a.clone(), ..Default::default() }).collect();
                    Ok(Accessories { accessories, fits: Vec::new() })
                })
            }
        }
    }

    /// A file of a part ([`openecraft_parts::portal::Attachment::id`]).
    pub fn file(&self, id: &str) -> Call<Vec<u8>> {
        match self {
            Connection::Parts(e) => Call::new(e.file(id), |r| check(r).map(|r| r.bytes)),
            Connection::Eplan(e) => Call::new(e.file(id), eplan::decode_file),
        }
    }

    /// Address of a file for the web browser; `None` if loading it needs the token.
    pub fn file_link(&self, id: &str) -> Option<String> {
        match self {
            Connection::Parts(e) => Some(e.file_url(id)),
            Connection::Eplan(e) => (!e.needs_token(id)).then(|| e.file_url(id)),
        }
    }
}

#[cfg(test)]
mod tests;
