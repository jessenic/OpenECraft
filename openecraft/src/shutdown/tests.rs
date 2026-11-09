use super::*;
use openecraft_core::model::Project;
use openecraft_ui::io::Store;
use std::sync::mpsc;

fn setup(name: &str) -> (Autosave, Session, Store, egui::Context) {
    let dir = std::env::temp_dir().join(format!("openecraft-shutdown-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::at(dir.display().to_string());
    let ctx = egui::Context::default();
    let mut autosave = Autosave::start(store.clone(), &ctx, &PartsDb::default());
    let mut project = Project::default();
    project.info.name = "recovery".into();
    let mut session = Session::new(project, None);
    autosave.poll(&mut session, &mut PartsDb::default());
    (autosave, session, store, ctx)
}

fn wait(shutdown: &mut Shutdown) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !shutdown.poll() && shutdown.error.is_none() {
        assert!(std::time::Instant::now() < deadline, "closing worker did not finish");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(shutdown.error.is_none(), "{:?}", shutdown.error);
}

#[test]
fn closing_waits_off_thread_and_restores_the_final_snapshot() {
    let (mut autosave, mut session, store, ctx) = setup("ordered");
    let (release, blocked) = mpsc::channel();
    let old = store.clone();
    autosave.set_pending_for_test(std::thread::spawn(move || {
        blocked.recv().unwrap();
        old.store("project", openecraft_core::archive::pack(&Project::default()).into_bytes());
    }));
    session.prj.info.name = "final edits".into();
    let mut parts = PartsDb::default();
    parts.parts.push(openecraft_parts::Part { number: "final part".into(), ..Default::default() });
    let mut macros = MacroLibrary::default();
    macros.macros.push(openecraft_core::macros::Macro { name: "final macro".into(), ..Default::default() });
    let mut closing = Shutdown::start(&mut autosave, &session, &parts, &macros, &ctx);
    assert!(!closing.poll(), "the UI can poll while the old write is blocked");
    assert_eq!(closing.step(), 0);
    release.send(()).unwrap();
    wait(&mut closing);
    let mut next = Autosave::start(store, &ctx, &PartsDb::default());
    let mut restored = Session::new(Project::default(), None);
    let mut restored_parts = PartsDb::default();
    assert!(next.poll(&mut restored, &mut restored_parts));
    assert_eq!(restored.prj.info.name, "final edits");
    assert_eq!(restored_parts, parts);
    let prepared = closing.prepared.as_ref().unwrap();
    assert_eq!(prepared.get_string("project_packed"), Some(String::new()));
    assert_eq!(eframe::get_value::<MacroLibrary>(prepared, "macros").unwrap(), macros);
}

#[test]
fn failed_store_keeps_the_recovery_fallback_and_macros() {
    let (mut autosave, session, _, ctx) = setup("fallback");
    autosave.use_store_for_test(Store::at("\0invalid-store"));
    let mut closing = Shutdown::start(&mut autosave, &session, &PartsDb::default(), &MacroLibrary::default(), &ctx);
    wait(&mut closing);
    let mut final_storage = Prepared::default();
    closing.prepared.as_ref().unwrap().apply(&mut final_storage);
    let recovered = openecraft_core::archive::unpack(&final_storage.get_string("project_packed").unwrap()).unwrap();
    assert_eq!(recovered.info.name, "recovery");
    assert!(eframe::get_value::<PartsDb>(&final_storage, "parts_db").is_some());
    assert!(eframe::get_value::<MacroLibrary>(&final_storage, "macros").is_some());
}
