use super::*;

#[test]
fn tablet_mode_of_a_wiring_start_is_not_stored() {
    let mut shell = ShellState { tablet_once: true, ..Default::default() };
    assert!(shell.tablet());
    let stored: ShellState = serde_json::from_str(&serde_json::to_string(&shell).unwrap()).unwrap();
    assert!(!stored.tablet(), "the next normal start has no tablet mode");
    // Switching it off ends it for the session too; switching it on is stored.
    shell.set_tablet(false);
    assert!(!shell.tablet());
    shell.set_tablet(true);
    let stored: ShellState = serde_json::from_str(&serde_json::to_string(&shell).unwrap()).unwrap();
    assert!(stored.tablet());
}

#[test]
fn the_unsaved_state_is_stored_with_the_shell() {
    let shell = ShellState { dirty: true, ..Default::default() };
    let stored: ShellState = serde_json::from_str(&serde_json::to_string(&shell).unwrap()).unwrap();
    assert!(stored.dirty);
    let old: ShellState = serde_json::from_str("{}").unwrap();
    assert!(!old.dirty, "stores written before have no flag");
}

#[test]
fn the_startup_setting_is_stored_with_the_shell() {
    use crate::startup::{ServerProject, Startup, Target};
    let startup = Startup::Project(Target::Server(ServerProject { url: "http://srv:8091".into(), name: "plant".into() }));
    let shell = ShellState { startup: startup.clone(), ..Default::default() };
    let stored: ShellState = serde_json::from_str(&serde_json::to_string(&shell).unwrap()).unwrap();
    assert_eq!(stored.startup, startup);
    let old: ShellState = serde_json::from_str("{}").unwrap();
    assert_eq!(old.startup, Startup::ReopenLast, "stores written before reopen the last project");
}

#[test]
fn counts_of_one_use_their_own_sentence() {
    openecraft_i18n::with_lang(openecraft_i18n::Lang::En, || {
        assert_eq!(errors_warnings(1, 1), "1 error, 1 warning");
        assert_eq!(errors_warnings(1, 3), "1 error, 3 warnings");
        assert_eq!(errors_warnings(2, 1), "2 errors, 1 warning");
        assert_eq!(errors_warnings(0, 0), "0 errors, 0 warnings");
        assert_eq!(counted(1, "1 note", "x".into()), "1 note");
        assert_eq!(counted(4, "1 note", "4 notes".into()), "4 notes");
    });
}
