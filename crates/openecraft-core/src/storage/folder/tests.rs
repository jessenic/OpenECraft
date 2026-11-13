use super::*;
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
