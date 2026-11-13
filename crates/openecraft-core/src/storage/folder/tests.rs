use super::*;
use crate::storage::INDEX;
use crate::document::Document;
use crate::model::{Page, PageKind};
use crate::sample::demo_project;

/// Empty directory for the files of one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openecraft-core-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn with_document() -> Project {
    let mut prj = demo_project();
    let id = prj.new_id();
    let mut doc = Page::new(id, "8", "Data sheet", PageKind::Document);
    doc.document = Some(Document::new("sheet.pdf", b"%PDF-1.4 test"));
    prj.pages.push(doc);
    prj
}

#[test]
fn index_files_are_known_by_their_extension() {
    assert!(is_index(Path::new("plant/plant.oecp")));
    assert!(is_index(Path::new("PLANT.OECP")));
    assert!(!is_index(Path::new("plant.oec")));
    assert!(!is_index(Path::new("plant.oecz")));
    assert!(!is_index(Path::new("oecp")));
}

#[test]
fn projects_round_trip_through_a_folder() {
    let dir = temp_dir("folder-round-trip");
    let index = dir.join("plant").join("Plant.oecp");
    let prj = with_document();
    let written = save(&index, &prj).unwrap();
    assert_eq!(written, split(&prj).len());
    assert!(dir.join("plant/pages").join(format!("{}.json", prj.pages[0].id)).is_file());
    assert!(dir.join("plant/data/layout.json").is_file());
    assert_eq!(std::fs::read_dir(dir.join("plant/files")).unwrap().count(), 1);

    let (back, conv) = open(&index).unwrap();
    assert!(back == prj);
    assert!(!conv.converted());
    assert!(read(&index).unwrap() == split(&prj));
    // Saving the same project changes no file.
    assert_eq!(save(&index, &back).unwrap(), 0);
    // Git is told to leave the files as they are; a file of the user stays.
    let attributes = dir.join("plant").join(".gitattributes");
    assert!(std::fs::read_to_string(&attributes).unwrap().contains("*.oecp -text merge=openecraft"));
    std::fs::write(&attributes, "# mine\n").unwrap();
    save(&index, &back).unwrap();
    assert_eq!(std::fs::read_to_string(&attributes).unwrap(), "# mine\n");
    assert!(!stored(&dir.join("plant")).unwrap().iter().any(|(name, _)| name.ends_with(TMP)));
}

#[test]
fn saving_writes_only_what_changed_and_removes_what_is_gone() {
    let dir = temp_dir("folder-changes");
    let index = dir.join("p.oecp");
    let mut prj = with_document();
    save(&index, &prj).unwrap();
    // Files of others in the folder stay.
    let own = [dir.join("README.md"), dir.join("pages/.gitattributes"), dir.join("files/notes.txt"), dir.join("data/devices/readme.txt")];
    for path in &own {
        std::fs::write(path, "mine").unwrap();
    }
    // Left behind by a crash.
    let tmp = dir.join("pages/1.json.tmp");
    std::fs::write(&tmp, "{").unwrap();

    // One page changed: its file and nothing else.
    prj.pages[0].description = "Changed".into();
    assert_eq!(save(&index, &prj).unwrap(), 1);
    assert!(!tmp.exists());

    // The document page removed: its file and the embedded file go, the index changes.
    let page = prj.pages.pop().unwrap();
    assert_eq!(save(&index, &prj).unwrap(), 3);
    assert!(!dir.join("pages").join(format!("{}.json", page.id)).exists());
    assert!(!dir.join("files").read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".pdf")));
    // No device left: the folder goes too, unless it holds files of others.
    prj.devices.clear();
    prj.strips.clear();
    save(&index, &prj).unwrap();
    assert!(dir.join("data/devices/readme.txt").is_file());
    assert_eq!(dir.join("data/devices").read_dir().unwrap().count(), 1);
    assert!(!dir.join("data/strips").exists());
    for path in &own {
        assert_eq!(std::fs::read_to_string(path).unwrap(), "mine");
    }
    assert!(open(&index).unwrap().0 == prj);
}

#[test]
fn copies_of_a_project_give_new_objects_different_ids() {
    let dir = temp_dir("folder-ids");
    let index = dir.join("p.oecp");
    let prj = demo_project();
    save(&index, &prj).unwrap();
    // Two people open the same state of the project and each add objects.
    let (mut a, _) = open(&index).unwrap();
    let (mut b, _) = open(&index).unwrap();
    assert!(a == prj && a.next_id == prj.next_id, "nothing changes before the first new object");
    let ids = |p: &mut Project| -> Vec<u64> { (0..100).map(|_| p.new_id()).collect() };
    let (ids_a, ids_b) = (ids(&mut a), ids(&mut b));
    assert!(ids_a[0] >= prj.next_id + crate::model::ID_SLOT && ids_a.windows(2).all(|w| w[1] == w[0] + 1));
    assert!(ids_a.iter().all(|id| !ids_b.contains(id)), "{} / {}", ids_a[0], ids_b[0]);
    // The start stays through undo (a clone taken before) and is not stored.
    let mut undone = open(&index).unwrap().0;
    let before = undone.clone();
    let first = undone.new_id();
    undone = before;
    assert_eq!(undone.new_id(), first);
    assert!(!a.to_json().contains("id_floor"));
    // A project file counts on, and so does a subproject: its ids are reserved.
    let mut file = Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(file.new_id(), prj.next_id);
    let mut sub = prj.clone();
    sub.subprojects.parent = Some(Default::default());
    sub.scatter_ids();
    assert_eq!(sub.new_id(), prj.next_id);
}

/// Changes a file as another program would (also its time, whatever the clock does).
fn change(path: &Path, content: &[u8]) {
    std::fs::write(path, content).unwrap();
    let later = SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options().write(true).open(path).unwrap().set_modified(later).unwrap();
}

#[test]
fn a_save_keeps_what_another_program_changed() {
    let dir = temp_dir("folder-tracked");
    let index = dir.join("p.oecp");
    let base = with_document();
    let mut tracked = None;
    assert_eq!(save_tracked(&index, &base, &mut tracked).unwrap(), Saved { changed: split(&base).len(), foreign: 0 });
    // Nothing changed: nothing is written, and nothing is read but the folder.
    assert_eq!(save_tracked(&index, &base, &mut tracked).unwrap(), Saved::default());

    // A colleague's changes arrive (git pull) while the project is open: a page
    // changed, a page added with its place in the index, a device removed.
    let mut theirs = base.clone();
    theirs.pages[2].description = "Theirs".into();
    let id = theirs.next_id + 5000;
    theirs.pages.push(Page::new(id, "90", "Theirs", PageKind::Schematic));
    theirs.next_id = id + 1;
    let gone = theirs.devices.keys().next().unwrap().clone();
    theirs.devices.remove(&gone);
    let files = split(&theirs);
    let page = |p: &Project, i: usize| format!("pages/{}.json", p.pages[i].id);
    for name in [page(&theirs, 2), format!("pages/{id}.json"), "data/next_id.json".to_string()] {
        change(&dir.join(&name), &files[&name]);
    }
    change(&index, &files[INDEX]);
    let device = stored(&dir).unwrap().into_iter().find(|(name, _)| name.starts_with("data/devices/") && !files.contains_key(name)).unwrap();
    std::fs::remove_file(&device.1).unwrap();

    // We changed another page and removed the document page with its file.
    let mut ours = base.clone();
    ours.pages[1].description = "Ours".into();
    let removed = ours.pages.pop().unwrap();
    let saved = save_tracked(&index, &ours, &mut tracked);
    // The index was changed by both (their page, our removed page): nothing is written.
    let err = saved.unwrap_err();
    assert!(err.contains("p.oecp") || err.contains(INDEX), "{err}");
    assert!(dir.join(format!("pages/{}.json", removed.id)).exists());
    assert!(!String::from_utf8(std::fs::read(dir.join(page(&ours, 1))).unwrap()).unwrap().contains("Ours"));

    // Without the change both made to the index, our changes are written next to theirs.
    let mut ours = base.clone();
    ours.pages[1].description = "Ours".into();
    let saved = save_tracked(&index, &ours, &mut tracked).unwrap();
    assert_eq!(saved, Saved { changed: 1, foreign: 5 });
    let (merged, _) = open(&index).unwrap();
    assert_eq!((merged.pages[1].description.as_str(), merged.pages[2].description.as_str()), ("Ours", "Theirs"));
    assert_eq!(merged.pages.len(), base.pages.len() + 1);
    assert!(!merged.devices.contains_key(&gone));
    // Saving again still leaves their files alone, until the project is read again.
    ours.pages[0].description = "More".into();
    assert_eq!(save_tracked(&index, &ours, &mut tracked).unwrap(), Saved { changed: 1, foreign: 5 });
    assert_eq!(open(&index).unwrap().0.pages.len(), base.pages.len() + 1);
    let mut tracked = Some(track(&index).unwrap());
    let (merged, _) = open(&index).unwrap();
    assert_eq!(save_tracked(&index, &merged, &mut tracked).unwrap(), Saved::default());

    // An embedded file we no longer use stays while files of others are unknown: a
    // page of theirs may use it. Alone we remove it.
    let mut alone = merged.clone();
    let doc = alone.pages.iter().position(|p| p.document.is_some()).unwrap();
    alone.pages.remove(doc);
    change(&dir.join(page(&merged, 3)), &{
        let mut p = merged.clone();
        p.pages[3].description = "Again".into();
        split(&p)[&page(&merged, 3)].clone()
    });
    assert_eq!(save_tracked(&index, &alone, &mut tracked).unwrap().foreign, 1);
    assert_eq!(dir.join("files").read_dir().unwrap().count(), 1);
    let mut tracked = Some(track(&index).unwrap());
    let (now, _) = open(&index).unwrap();
    let mut alone = now.clone();
    alone.info.name = "Alone".into();
    assert_eq!(save_tracked(&index, &alone, &mut tracked).unwrap(), Saved { changed: 2, foreign: 0 });
    assert!(!dir.join("files").exists());

    // Another place is written as a whole.
    let other = dir.join("copy").join("p.oecp");
    assert_eq!(save_tracked(&other, &alone, &mut tracked).unwrap().changed, split(&alone).len());
    assert!(open(&other).unwrap().0 == alone);
}

#[test]
fn a_folder_holds_one_project() {
    let dir = temp_dir("folder-one");
    save(&dir.join("a.oecp"), &demo_project()).unwrap();
    let err = save(&dir.join("b.oecp"), &demo_project()).unwrap_err();
    assert!(err.contains("a.oecp"), "{err}");
    assert!(!dir.join("b.oecp").exists());
    save(&dir.join("a.oecp"), &Project::default()).unwrap();
}

#[test]
fn a_project_saved_among_other_files_gets_a_folder_of_its_own() {
    let dir = temp_dir("folder-place");
    // An empty or new folder is taken as it is.
    assert_eq!(place(&dir.join("plant.oecp")), dir.join("plant.oecp"));
    assert_eq!(place(&dir.join("new").join("plant.oecp")), dir.join("new").join("plant.oecp"));
    assert_eq!(place(&dir.join("plant.v2")), dir.join("plant.v2.oecp"));
    // So is the folder of the project itself.
    save(&dir.join("plant.oecp"), &demo_project()).unwrap();
    assert_eq!(place(&dir.join("plant.oecp")), dir.join("plant.oecp"));
    // Next to other files the project goes into a folder named like it.
    assert_eq!(place(&dir.join("other.oecp")), dir.join("other").join("other.oecp"));
    assert_eq!(place(&dir.join("other")), dir.join("other").join("other.oecp"));
}

#[test]
fn missing_and_broken_folders_are_reported() {
    let dir = temp_dir("folder-broken");
    assert!(open(&dir.join("none.oecp")).unwrap_err().contains("none.oecp"));
    let index = dir.join("p.oecp");
    let prj = demo_project();
    save(&index, &prj).unwrap();
    let page = dir.join("pages").join(format!("{}.json", prj.pages[0].id));
    std::fs::remove_file(&page).unwrap();
    assert!(open(&index).unwrap_err().contains(&format!("pages/{}.json", prj.pages[0].id)));
}
