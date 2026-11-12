use super::*;
use openecraft_parts::access::{Grant, Subject};
use openecraft_parts::portal::PartsRight;

fn dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("oec-access-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn missing_files_give_the_default_and_changes_are_stored() {
    let dir = dir();
    let path = dir.join("access.json");
    let _ = std::fs::remove_file(&path);
    let default = AccessConfig::<PartsRight> { policy: Policy { anonymous: "reader".into(), ..Default::default() }, ..Default::default() };
    let mut file = AccessFile::open(&path, default).unwrap();
    assert_eq!(file.config.policy.anonymous, "reader");
    assert!(!path.exists(), "written on the first change only");
    let policy = Policy { grants: vec![Grant::new(Subject::Group("corp:Purchasing".into()), "editor", "ABB")], ..Default::default() };
    file.set_policy(policy.clone()).unwrap();
    let back = AccessFile::<PartsRight>::open(&path, AccessConfig::default()).unwrap();
    assert_eq!(back.config.policy, policy);
    let bad = Policy { grants: vec![Grant::new(Subject::User("x".into()), "nobody", "*")], ..Default::default() };
    assert!(file.set_policy(bad).is_err());
    assert_eq!(file.config.policy, policy, "unchanged");
    assert!(!std::fs::read_to_string(&path).unwrap().contains("registration"), "closed is not written");
    assert!(file.config.registration_approval, "approval by default");
    file.set_registration(true, true).unwrap();
    let back = AccessFile::<PartsRight>::open(&path, AccessConfig::default()).unwrap();
    assert!(back.config.registration && back.config.registration_approval);
    assert!(!std::fs::read_to_string(&path).unwrap().contains("approval"), "the default is not written");
    file.set_registration(true, false).unwrap();
    let back = AccessFile::<PartsRight>::open(&path, AccessConfig::default()).unwrap();
    assert!(!back.config.registration_approval);
    assert_eq!(back.config.policy, policy, "the policy stays");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_are_checked() {
    let dir = dir();
    let path = dir.join("bad.json");
    let d = r#"{"id":"corp","url":"ldaps://dc","bind_dn":"CN=svc","base_dn":"DC=corp"}"#;
    std::fs::write(&path, format!(r#"{{"directories":[{d}, {d}]}}"#)).unwrap();
    let err = AccessFile::<PartsRight>::open(&path, AccessConfig::default()).unwrap_err();
    assert!(err.to_string().contains("defined twice"), "{err}");
    std::fs::write(&path, r#"{"grants":[{"subject":{"user":"a"},"role":"nobody"}]}"#).unwrap();
    assert!(AccessFile::<PartsRight>::open(&path, AccessConfig::default()).is_err());
    std::fs::write(&path, "not json").unwrap();
    assert!(AccessFile::<PartsRight>::open(&path, AccessConfig::default()).is_err());
    std::fs::write(&path, format!(r#"{{"directories":[{d}],"anonymous":"reader","roles":[{{"name":"buyer","rights":["read"]}}]}}"#)).unwrap();
    let file = AccessFile::<PartsRight>::open(&path, AccessConfig::default()).unwrap();
    let settings = file.settings();
    assert_eq!(settings.providers.len(), 1);
    assert_eq!(settings.policy.roles[0].name, "buyer");
    assert!(!serde_json::to_string(&settings).unwrap().contains("CN=svc"), "directory settings stay on the server");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn private_files_are_replaced_whole() {
    let dir = dir().join("private");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("users.json");
    write_private(&path, b"first").unwrap();
    write_private(&path, b"second").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    assert!(!dir.join("users.json.tmp").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
