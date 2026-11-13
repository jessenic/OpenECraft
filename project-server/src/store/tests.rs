use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openecraft-project-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn create_save_and_reload() {
    let dir = temp_dir("reload");
    let mut store = Store::open(&dir).unwrap();
    assert!(store.projects.is_empty());
    let mut prj = Project::default();
    prj.info.name = "Plant".into();
    store.create("Plant 1", prj.clone()).unwrap();
    assert_eq!(store.get("Plant 1").unwrap().revision, 1);
    let first = store.reserve_ids("Plant 1").unwrap();
    assert_eq!(store.get("Plant 1").unwrap().next_id, first + ID_BLOCK);

    // Unchanged project: same revision.
    let same = store.get("Plant 1").unwrap().project.clone();
    assert_eq!(store.save("Plant 1", same).unwrap(), 1);
    prj.info.name = "Plant (changed)".into();
    assert_eq!(store.save("Plant 1", prj).unwrap(), 2);
    assert!(store.save("missing", Project::default()).is_err());

    let reloaded = Store::open(&dir).unwrap();
    let entry = reloaded.get("Plant 1").unwrap();
    assert_eq!(entry.revision, 2);
    assert_eq!(entry.next_id, first + ID_BLOCK);
    assert_eq!(entry.project.info.name, "Plant (changed)");
    // The folder of the project is a normal split project.
    let index = dir.join("Plant 1").join("Plant 1.oecp");
    assert_eq!(folder::open(&index).unwrap().0.info.name, "Plant (changed)");
    assert!(dir.join("Plant 1").join("pages").is_dir());
    assert!(!dir.join("Plant 1.oec").exists());

    let mut store = reloaded;
    store.delete("Plant 1").unwrap();
    assert!(store.delete("Plant 1").is_err());
    assert!(!dir.join("Plant 1").exists() && !dir.join("Plant 1.state.json").exists());
    assert!(Store::open(&dir).unwrap().projects.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignores_other_files() {
    let dir = temp_dir("other");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    std::fs::write(dir.join(".hidden.oec"), "{}").unwrap();
    std::fs::create_dir_all(dir.join("backup")).unwrap();
    std::fs::write(dir.join("backup").join("other.oecp"), "{}").unwrap();
    let store = Store::open(&dir).unwrap();
    assert!(store.projects.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_writes_only_the_files_that_changed() {
    let dir = temp_dir("changed");
    let mut store = Store::open(&dir).unwrap();
    store.create("p", openecraft_core::sample::demo_project()).unwrap();
    let folder = dir.join("p");
    let page = |store: &Store, i: usize| folder.join("pages").join(format!("{}.json", store.get("p").unwrap().project.pages[i].id));
    let modified = |path: &Path| std::fs::metadata(path).unwrap().modified().unwrap();
    let (first, second) = (page(&store, 0), page(&store, 1));
    let before = (modified(&first), modified(&second), std::fs::read(&second).unwrap());
    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut prj = store.get("p").unwrap().project.clone();
    prj.pages[0].description = "Changed".into();
    store.save("p", prj).unwrap();
    assert!(modified(&first) > before.0);
    assert_eq!((modified(&second), std::fs::read(&second).unwrap()), (before.1, before.2));
    // Reserving ids touches the id counter only.
    store.reserve_ids("p").unwrap();
    assert_eq!(modified(&second), before.1);
    assert!(Store::open(&dir).unwrap().get("p").unwrap() == store.get("p").unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_files_of_earlier_versions_are_converted() {
    let dir = temp_dir("convert");
    std::fs::create_dir_all(&dir).unwrap();
    let mut prj = openecraft_core::sample::demo_project();
    prj.info.name = "Old plant".into();
    std::fs::write(dir.join("Plant.oec"), prj.to_json()).unwrap();
    std::fs::write(dir.join("Plant.state.json"), r#"{ "revision": 7, "next_id": 5000000 }"#).unwrap();

    let store = Store::open(&dir).unwrap();
    let entry = store.get("Plant").unwrap();
    assert_eq!((entry.revision, entry.next_id), (7, 5_000_000));
    assert!(entry.project == prj);
    assert!(dir.join("Plant").join("Plant.oecp").is_file());
    // The project file is kept, but no longer read.
    assert!(!dir.join("Plant.oec").exists() && dir.join("Plant.oec.bak").is_file());
    let again = Store::open(&dir).unwrap();
    assert_eq!(again.projects.len(), 1);
    assert!(again.get("Plant").unwrap().project == prj);

    // A project file next to the folder of the project is left alone.
    std::fs::write(dir.join("Plant.oec"), Project::default().to_json()).unwrap();
    let store = Store::open(&dir).unwrap();
    assert_eq!(store.get("Plant").unwrap().project.info.name, "Old plant");
    assert!(dir.join("Plant.oec").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ids_never_go_below_the_project() {
    let mut store = Store::in_memory();
    let prj = Project { next_id: 500, ..Default::default() };
    store.create("p", prj).unwrap();
    let first = store.reserve_ids("p").unwrap();
    assert!(first >= 500);
    assert!(store.reserve_ids("p").unwrap() >= first + ID_BLOCK);
}

#[test]
fn earlier_revisions_are_kept_for_a_while() {
    let mut store = Store::in_memory();
    store.create("p", Project::default()).unwrap();
    for i in 0..KEPT_REVISIONS + 2 {
        let mut prj = store.get("p").unwrap().project.clone();
        prj.info.name = format!("v{i}");
        store.save("p", prj).unwrap();
    }
    let current = store.get("p").unwrap().revision;
    assert_eq!(store.at("p", current).unwrap().info.name, format!("v{}", KEPT_REVISIONS + 1));
    assert_eq!(store.at("p", current - 1).unwrap().info.name, format!("v{}", KEPT_REVISIONS));
    assert!(store.at("p", current - KEPT_REVISIONS as u64).is_some());
    assert!(store.at("p", 1).is_none(), "too old");
    store.delete("p").unwrap();
    assert!(store.at("p", current - 1).is_none());
}
