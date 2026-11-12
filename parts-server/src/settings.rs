//! Settings of the parts server: the command line options (and their environment
//! variables) win over the configuration file (`--config`, TOML with the option names as
//! keys, see `parts-server.example.toml`), the file over the defaults.

use crate::storage::StorageKind;
use clap::Parser;
use openecraft_auth::server_config;
use openecraft_parts::api::DEFAULT_PORT;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Data portal (global parts database) server for OpenECraft")]
pub struct Args {
    /// Configuration file (TOML) with the settings of the options below, e.g.
    /// `bind = "0.0.0.0:8090"`; options given on the command line win.
    #[arg(long, env = "OPENECRAFT_PARTS_SERVER_CONFIG")]
    config: Option<PathBuf>,
    /// Address to listen on [default: 127.0.0.1:8090].
    #[arg(long)]
    bind: Option<SocketAddr>,
    /// Storage backend. Default: JSON for a `.json` data file, SQLite otherwise.
    #[arg(long, value_enum)]
    storage: Option<StorageKind>,
    /// Database file (created with the example and the bundled catalogs if missing)
    /// [default: parts-db.sqlite].
    #[arg(long)]
    data: Option<PathBuf>,
    /// Copy all data of this database into a new, empty `--data` database first
    /// (e.g. `--migrate-from parts-db.json` when changing to SQLite).
    #[arg(long)]
    migrate_from: Option<PathBuf>,
    /// Administrator token. Without a token changes need user accounts.
    #[arg(long, env = "OPENECRAFT_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// Create this administrator account (or reset its password) at startup; the
    /// password is read from OPENECRAFT_ADMIN_PASSWORD.
    #[arg(long, env = "OPENECRAFT_ADMIN_USER")]
    admin_user: Option<String>,
    /// Install or update bundled manufacturer catalogs at startup: `all` or ids
    /// (e.g. `siemens`). Parts changed by users are kept.
    #[arg(long, value_delimiter = ',')]
    install_catalogs: Vec<String>,
    /// Access settings: directories (Active Directory, LDAP), roles and grants.
    /// Default: `<data>.access.json` next to the database.
    #[arg(long)]
    access: Option<PathBuf>,
    /// Let users register themselves (`true`) or not (`false`) as readers; without it
    /// administrators switch it on the web page.
    #[arg(long, env = "OPENECRAFT_REGISTRATION", value_parser = clap::builder::BoolishValueParser::new())]
    registration: Option<bool>,
}

/// The configuration file: the options without `--config` and `--migrate-from`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct File {
    bind: Option<SocketAddr>,
    storage: Option<StorageKind>,
    data: Option<PathBuf>,
    token: Option<String>,
    admin_user: Option<String>,
    install_catalogs: Vec<String>,
    access: Option<PathBuf>,
    registration: Option<bool>,
}

/// The settings the server runs with.
#[derive(Debug, PartialEq)]
pub struct Settings {
    pub bind: SocketAddr,
    pub storage: StorageKind,
    pub data: PathBuf,
    pub migrate_from: Option<PathBuf>,
    /// Administrator token (not empty).
    pub token: Option<String>,
    /// Administrator to create (not empty).
    pub admin_user: Option<String>,
    pub install_catalogs: Vec<String>,
    pub access: PathBuf,
    /// Registration of users fixed by the start options; `None`: as administrators set it.
    pub registration: Option<bool>,
}

impl Settings {
    /// The settings of the command line and of its configuration file.
    pub fn new(args: Args) -> Result<Settings, String> {
        let file = match &args.config {
            Some(path) => server_config::load(path)?,
            None => File::default(),
        };
        Ok(Settings::merge(args, file))
    }

    fn merge(args: Args, file: File) -> Settings {
        let in_file = |p: PathBuf| match &args.config {
            Some(config) => server_config::relative_to(config, p),
            None => p,
        };
        let data = args.data.or(file.data.map(in_file)).unwrap_or_else(|| PathBuf::from("parts-db.sqlite"));
        let access = args.access.or(file.access.map(in_file)).unwrap_or_else(|| data.with_extension("access.json"));
        Settings {
            bind: args.bind.or(file.bind).unwrap_or(SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT))),
            storage: StorageKind::resolve(args.storage.or(file.storage), &data),
            migrate_from: args.migrate_from,
            token: args.token.or(file.token).filter(|t| !t.trim().is_empty()),
            admin_user: args.admin_user.or(file.admin_user).filter(|n| !n.trim().is_empty()),
            install_catalogs: if args.install_catalogs.is_empty() { file.install_catalogs } else { args.install_catalogs },
            registration: args.registration.or(file.registration),
            data,
            access,
        }
    }
}

#[cfg(test)]
mod tests;
