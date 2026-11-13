use super::*;
use openecraft_core::model::Project;

impl Autosave {
    pub fn set_pending_for_test(&mut self, writing: std::thread::JoinHandle<()>) { self.writing = Some(writing); }
    pub fn use_store_for_test(&mut self, store: Store) { self.store = store; }
}

fn store(name: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("openecraft-autosave-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Store::at(dir.display().to_string())
}

fn named(name: &str) -> Project {
    let mut prj = Project::default();
    prj.info.name = name.into();
    prj
}

#[test]
fn the_stored_project_and_parts_replace_the_start_up_ones() {
    let store = store("replace");
    let ctx = egui::Context::default();
    let mut parts = PartsDb::default();
    parts.parts.push(openecraft_parts::Part { number: "P1".into(), ..Default::default() });
    // A first run saves.
    let mut first = Autosave::start(store.clone(), &ctx, &PartsDb::default());
    let mut session = Session::new(named("stored"), None);
    let mut start_parts = PartsDb::default();
    assert!(!first.save(&session, &parts), "nothing is written before the stored values arrived");
    first.poll(&mut session, &mut start_parts);
    assert!(first.save(&session, &parts));

    // The next run starts with another project and gets the stored one.
    let mut next = Autosave::start(store, &ctx, &PartsDb::default());
    let mut session = Session::new(named("start"), None);
    let mut db = PartsDb::default();
    assert!(next.poll(&mut session, &mut db));
    assert_eq!(session.prj.info.name, "stored");
    assert!(db.parts.iter().any(|p| p.number == "P1"));
}

#[test]
fn values_arriving_after_a_change_do_not_replace_it() {
    let store = store("late");
    let ctx = egui::Context::default();
    let saved = Session::new(named("stored"), None);
    store.store(PROJECT, openecraft_core::archive::pack(&saved.prj).into_bytes());
    store.store(PARTS, b"{}".to_vec());
    let mut autosave = Autosave::start(store, &ctx, &PartsDb::default());
    let mut session = Session::new(named("start"), None);
    session.prj.info.name = "edited".into();
    session.commit(true);
    let mut parts = PartsDb::default();
    parts.parts.push(openecraft_parts::Part { number: "new".into(), ..Default::default() });
    assert!(!autosave.poll(&mut session, &mut parts));
    assert_eq!(session.prj.info.name, "edited");
    assert!(parts.parts.iter().any(|p| p.number == "new"));
}

#[test]
fn unsaved_changes_of_the_last_session_stay_unsaved() {
    let store = store("unsaved");
    let ctx = egui::Context::default();
    store.store(PROJECT, openecraft_core::archive::pack(&named("stored")).into_bytes());
    let mut autosave = Autosave::start(store, &ctx, &PartsDb::default());
    let mut session = Session::new(named("start"), Some("plant.oec".into()));
    // The shell restores the flag stored at the last close.
    session.mark_unsaved();
    assert!(autosave.poll(&mut session, &mut PartsDb::default()));
    assert_eq!(session.prj.info.name, "stored");
    assert!(session.dirty(), "the file on disk does not hold the restored changes");
}

#[test]
fn values_arriving_after_a_replacement_do_not_replace_it() {
    let store = store("replaced");
    let ctx = egui::Context::default();
    store.store(PROJECT, openecraft_core::archive::pack(&named("stored")).into_bytes());
    let mut autosave = Autosave::start(store, &ctx, &PartsDb::default());
    let mut session = Session::new(named("start"), None);
    session.replace(named("opened"), Some("opened.oec".into()), None);
    assert!(!autosave.poll(&mut session, &mut PartsDb::default()));
    assert_eq!(session.prj.info.name, "opened");
}

#[test]
fn later_saves_are_written_on_another_thread() {
    let store = store("thread");
    let ctx = egui::Context::default();
    let mut autosave = Autosave::start(store.clone(), &ctx, &PartsDb::default());
    let mut session = Session::new(named("first"), None);
    autosave.poll(&mut session, &mut PartsDb::default());
    assert!(autosave.save(&session, &PartsDb::default()), "the first write is made at once");
    session.prj.info.name = "second".into();
    assert!(autosave.save(&session, &PartsDb::default()));
    session.prj.info.name = "third".into();
    autosave.save(&session, &PartsDb::default());
    autosave.finish();
    // The next run gets the last state saved.
    let mut next = Autosave::start(store, &ctx, &PartsDb::default());
    let mut restored = Session::new(named("start"), None);
    assert!(next.poll(&mut restored, &mut PartsDb::default()));
    assert_eq!(restored.prj.info.name, "third");
}
