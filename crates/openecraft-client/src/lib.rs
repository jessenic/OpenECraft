//! Requests to the parts server (`openecraft-parts-server`), the project server
//! (`openecraft-project-server`) and Autodesk Vault ([`vault`]) and the decoding of
//! their answers. Shared by the background clients of the applications and the
//! blocking clients of the command line; only sending the request differs.
//! [`eplan`] wraps the API of the EPLAN Data Portal so that it can be read like a
//! parts server ([`Connection`]). [`update`] looks for new releases of OpenECraft on
//! GitHub.

mod connection;
pub mod credentials;
pub mod eplan;
mod endpoint;
mod projects;
pub mod update;
pub mod vault;

pub use connection::{Call, Connection, Decoder, PortalKind};
pub use ehttp;
pub use endpoint::{check, decode, decode_found, origin, same_origin, text, Endpoint};
pub(crate) use endpoint::reason;
pub use eplan::EplanEndpoint;
pub use projects::ProjectEndpoint;
