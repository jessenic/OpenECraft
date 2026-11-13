use super::*;

fn args(cli: &[&str]) -> Args {
    Args::try_parse_from(std::iter::once("openecraft-parts-server").chain(cli.iter().copied())).unwrap()
}

fn file(text: &str) -> File {
    server_config::parse(text).unwrap()
}

#[test]
fn without_options_the_defaults_apply() {
    let s = Settings::merge(args(&["--token", " "]), File::default());
    assert_eq!(s.bind, SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT)));
    assert_eq!(s.data, PathBuf::from("parts-db.sqlite"));
    assert_eq!(s.storage, StorageKind::Sqlite);
    assert_eq!(s.access, PathBuf::from("parts-db.access.json"));
    assert_eq!(s.token, None, "an empty token is no token");
    assert!(s.install_catalogs.is_empty());
    assert_eq!(s.registration, None, "administrators decide");
}

#[test]
fn registration_is_fixed_by_the_options() {
    assert_eq!(Settings::merge(args(&[]), file("registration = true\n")).registration, Some(true));
    assert_eq!(Settings::merge(args(&["--registration", "off"]), file("registration = true\n")).registration, Some(false));
    assert_eq!(Settings::merge(args(&["--registration", "yes"]), File::default()).registration, Some(true));
}

#[test]
fn the_file_sets_what_the_command_line_leaves_open() {
    let f = file("bind = \"0.0.0.0:9000\"\ndata = \"/srv/parts.json\"\ntoken = \"secret\"\nadmin_user = \"admin\"\ninstall_catalogs = [\"all\"]\n");
    let s = Settings::merge(args(&["--bind", "127.0.0.1:9100"]), f);
    assert_eq!(s.bind, "127.0.0.1:9100".parse().unwrap(), "the command line wins");
    assert_eq!(s.data, PathBuf::from("/srv/parts.json"));
    assert_eq!(s.storage, StorageKind::Json, "from the extension of the data file");
    assert_eq!(s.access, PathBuf::from("/srv/parts.access.json"));
    assert_eq!(s.token.as_deref(), Some("secret"));
    assert_eq!(s.admin_user.as_deref(), Some("admin"));
    assert_eq!(s.install_catalogs, vec!["all"]);
    let s = Settings::merge(args(&["--install-catalogs", "siemens", "--storage", "sqlite"]), file("install_catalogs = [\"all\"]\nstorage = \"json\"\n"));
    assert_eq!(s.install_catalogs, vec!["siemens"]);
    assert_eq!(s.storage, StorageKind::Sqlite);
}

#[test]
fn paths_in_the_file_are_relative_to_it() {
    let dir = std::env::temp_dir().join(format!("openecraft-parts-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("parts-server.toml");
    std::fs::write(&config, "data = \"db/parts.sqlite\"\naccess = \"access.json\"\n").unwrap();
    let s = Settings::new(args(&["--config", config.to_str().unwrap()])).unwrap();
    assert_eq!(s.data, dir.join("db/parts.sqlite"));
    assert_eq!(s.access, dir.join("access.json"));
    // Paths on the command line stay relative to the working directory.
    let s = Settings::new(args(&["--config", config.to_str().unwrap(), "--data", "here.sqlite"])).unwrap();
    assert_eq!(s.data, PathBuf::from("here.sqlite"));
    std::fs::write(&config, "migrate_from = \"old.json\"\n").unwrap();
    assert!(Settings::new(args(&["--config", config.to_str().unwrap()])).unwrap_err().contains("migrate_from"), "only on the command line");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_example_configuration_is_valid() {
    let f = file(include_str!("../../parts-server.example.toml"));
    assert!(f.bind.is_some() && f.data.is_some());
}

#[test]
fn the_admin_user_can_come_from_the_environment() {
    use clap::CommandFactory;
    let command = Args::command();
    let arg = command.get_arguments().find(|a| a.get_id() == "admin_user").unwrap();
    assert_eq!(arg.get_env(), Some(std::ffi::OsStr::new("OPENECRAFT_ADMIN_USER")));
    let s = Settings::merge(args(&["--admin-user", " "]), File::default());
    assert_eq!(s.admin_user, None, "an empty name is no administrator");
}

#[test]
fn the_dashboard_is_switched_by_the_options() {
    use clap::CommandFactory;
    assert_eq!(Settings::merge(args(&[]), File::default()).dashboard, None, "administrators decide");
    assert_eq!(Settings::merge(args(&[]), file("dashboard = false\n")).dashboard, Some(false));
    assert_eq!(Settings::merge(args(&["--dashboard", "on"]), file("dashboard = false\n")).dashboard, Some(true));
    let command = Args::command();
    let arg = command.get_arguments().find(|a| a.get_id() == "dashboard").unwrap();
    assert_eq!(arg.get_env(), Some(std::ffi::OsStr::new("OPENECRAFT_DASHBOARD")));
}
