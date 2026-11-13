//! OpenECraft parts server: the data portal of all OpenECraft installations (desktop
//! and web). Hosts manufacturer catalogs with faceted search, part history and
//! lifecycle, attachments (datasheets, models, macros), users with roles and a web
//! page for browsing. Runs independently of the editor.
//!
//! ```text
//! openecraft-parts-server --bind 0.0.0.0:8090 --data parts-db.sqlite --token <secret>
//! openecraft-parts-server --storage json --data parts-db.json
//! openecraft-parts-server --install-catalogs all
//! openecraft-parts-server --config parts-server.toml
//! ```
//!
//! `--config` reads the settings of the options from a TOML file (`settings`,
//! `parts-server.example.toml`); options on the command line win.
//!
//! Without `--token` (or `OPENECRAFT_TOKEN`), user accounts and directories the
//! server is read-only. Users sign in with a local account or with a directory
//! (Active Directory, LDAP) of the access settings (`--access`, default
//! `<data>.access.json`); their rights per manufacturer come from their role and the
//! grants to them and their groups (of the directory or defined on the server).

mod auth;
mod bundled;
mod groups;
mod routes;
mod settings;
mod storage;
mod web;

use clap::Parser;
use openecraft_parts::portal::{Role, User};
use storage::{StorageKind, Store};

fn fail(msg: String) -> ! {
    log::error!("{msg}");
    std::process::exit(1);
}

fn migrate(from: &std::path::Path, kind: StorageKind, to: &std::path::Path) {
    use storage::open_backend;
    let source = open_backend(StorageKind::resolve(None, from), from).unwrap_or_else(|e| fail(format!("cannot open {}: {e}", from.display())));
    let mut target = open_backend(kind, to).unwrap_or_else(|e| fail(format!("cannot open {}: {e}", to.display())));
    if target.revision() != 0 {
        fail(format!("{} is not empty: migrate into a new database", to.display()));
    }
    match storage::migrate(source.as_ref(), target.as_mut()) {
        Ok(n) => log::info!("copied {n} parts from {} to {}", from.display(), to.display()),
        Err(e) => fail(format!("migration failed: {e}")),
    }
}

fn add_admin(store: &mut Store, name: &str) {
    let password = std::env::var("OPENECRAFT_ADMIN_PASSWORD").unwrap_or_default();
    if password.is_empty() {
        fail("--admin-user needs the password in OPENECRAFT_ADMIN_PASSWORD".into());
    }
    if let Err(e) = openecraft_auth::check_password(&password) {
        fail(format!("--admin-user: {e}"));
    }
    let mut account = store.account(name).cloned().unwrap_or_default();
    account.user = User { name: name.trim().into(), role: Role::Admin, manufacturer: String::new(), ..Default::default() };
    account.password = auth::hash_password(&password).unwrap_or_else(|e| fail(e));
    account.sessions.clear();
    if let Err(e) = store.put_account(account) {
        fail(format!("cannot save user {name}: {e}"));
    }
    log::info!("administrator {name} ready");
}

fn install_catalogs(store: &mut Store, ids: &[String]) {
    let ids: Vec<String> = if ids.iter().any(|i| i.eq_ignore_ascii_case("all")) { bundled::ids().into_iter().map(String::from).collect() } else { ids.to_vec() };
    for id in ids {
        let bundle = bundled::load(id.trim()).unwrap_or_else(|e| fail(e));
        let name = bundle.catalog.name.clone();
        match store.install(bundle) {
            Ok(r) => {
                log::info!("catalog {name}: {} parts added, {} updated, {} kept", r.added, r.updated, r.kept.len());
                for number in &r.kept {
                    log::info!("  kept {number} (changed on this server)");
                }
            }
            Err(e) => fail(format!("cannot install catalog {name}: {e}")),
        }
    }
}

/// The groups local accounts named become groups of the server ([`groups::adopt`]).
fn adopt_groups(store: &mut Store, access: &mut openecraft_auth::AccessFile<openecraft_parts::portal::PartsRight>) {
    let Some((accounts, defined)) = groups::adopt(store.accounts(), &access.config.groups) else { return };
    access.set_groups(defined).unwrap_or_else(|e| fail(e));
    let n = accounts.len();
    for a in accounts {
        if let Err(e) = store.put_account(a) {
            fail(format!("cannot save the groups of the users: {e}"));
        }
    }
    log::info!("the groups of {n} users are now groups of the server");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = settings::Settings::new(settings::Args::parse()).unwrap_or_else(|e| fail(e));
    let kind = args.storage;
    if let Some(from) = &args.migrate_from {
        migrate(from, kind, &args.data);
    }
    let mut store = Store::open(kind, &args.data).unwrap_or_else(|e| fail(format!("cannot open parts database: {e}")));
    if let Some(name) = &args.admin_user {
        add_admin(&mut store, name);
    }
    install_catalogs(&mut store, &args.install_catalogs);
    match store.collect_garbage() {
        Ok(0) => {}
        Ok(n) => log::info!("removed {n} unused files"),
        Err(e) => log::warn!("unused files not removed: {e}"),
    }
    log::info!(
        "{} parts loaded from {} ({}, revision {})",
        store.count().unwrap_or(0),
        args.data.display(),
        store.kind(),
        store.revision
    );
    let mut access = openecraft_auth::AccessFile::open(&args.access, routes::default_access()).unwrap_or_else(|e| fail(format!("cannot read the access settings: {e}")));
    for d in &access.config.directories {
        log::info!("sign-in with directory {} ({})", d.id, d.url);
    }
    adopt_groups(&mut store, &mut access);
    let token = args.token;
    if openecraft_auth::server_config::secret_on_command_line(std::env::args(), "--token") {
        log::warn!("--token on the command line can be read by other users of this machine: use OPENECRAFT_TOKEN or the configuration file");
    }
    if token.is_none() && store.accounts().is_empty() && access.config.directories.is_empty() {
        log::warn!("no access token, users or directories: the server is read-only");
    }
    let registration = args.registration.unwrap_or(access.config.registration);
    if registration {
        log::info!("users may register themselves{}", if args.registration.is_some() { " (start options)" } else { "" });
    }
    if let Some(shown) = args.dashboard {
        log::info!("dashboard {} (start options)", if shown { "shown" } else { "hidden" });
    }
    let state = routes::AppState::new(store, token, access).with_registration(args.registration).with_dashboard(args.dashboard);
    let listener = tokio::net::TcpListener::bind(args.bind).await.unwrap_or_else(|e| fail(format!("cannot listen on {}: {e}", args.bind)));
    log::info!("listening on http://{}", args.bind);
    let shutdown = async {
        tokio::signal::ctrl_c().await.ok();
        log::info!("shutting down");
    };
    let app = routes::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>();
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown).await {
        fail(format!("server error: {e}"));
    }
}
