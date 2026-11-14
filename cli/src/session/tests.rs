use super::*;
use crate::testing::{session, temp_dir};

#[test]
fn tools_need_an_open_project() {
    let mut s = session();
    assert!(s.project().is_err());
    assert!(s.save(None).is_err());
    assert!(s.undo().is_err());
    assert!(!s.unsaved());
}

#[test]
fn new_project_is_unsaved_until_written() {
    let dir = temp_dir("session-save");
    let mut s = session();
    s.replace(Project::default(), None);
    assert!(s.unsaved());
    assert!(s.save(None).is_err(), "no file yet");
    let file = dir.join("a.oec");
    s.save(Some(file.to_str().unwrap())).unwrap();
    assert!(!s.unsaved());
    assert_eq!(s.path.as_deref(), Some(file.as_path()));

    s.project_mut().unwrap().info.name = "Changed".into();
    assert!(s.unsaved());
    s.save(None).unwrap();

    let mut other = session();
    other.open(file.to_str().unwrap()).unwrap();
    assert_eq!(other.project().unwrap().info.name, "Changed");
    assert!(!other.unsaved());
}

#[test]
fn split_projects_are_saved_in_place() {
    let dir = temp_dir("session-split");
    let mut s = session();
    s.replace(openecraft_core::sample::demo_project(), None);
    let index = dir.join("demo.oecp");
    s.save(Some(index.to_str().unwrap())).unwrap();
    assert!(!s.unsaved());
    let page = dir.join("pages").join(format!("{}.json", s.project().unwrap().pages[0].id));
    assert!(page.is_file());

    let mut other = session();
    other.open(index.to_str().unwrap()).unwrap();
    assert!(other.project().unwrap() == s.project().unwrap());
    assert_eq!((other.path.as_deref(), other.archive.as_deref()), (Some(index.as_path()), None));
    // The command line writes changes back: only the files that changed.
    let before = std::fs::read(&page).unwrap();
    other.project_mut().unwrap().info.name = "Changed".into();
    other.write_back().unwrap();
    assert_eq!(std::fs::read(&page).unwrap(), before);
    assert!(std::fs::read_to_string(&index).unwrap().contains("Changed"));
    assert!(!other.unsaved());
}

#[test]
fn a_save_keeps_the_changes_another_program_made_to_a_split_project() {
    let dir = temp_dir("session-foreign");
    let index = dir.join("demo.oecp");
    let mut mine = session();
    mine.replace(openecraft_core::sample::demo_project(), None);
    mine.save(Some(index.to_str().unwrap())).unwrap();
    // A colleague's change arrives while the project is open (git pull).
    let mut theirs = session();
    theirs.open(index.to_str().unwrap()).unwrap();
    theirs.project_mut().unwrap().pages[1].description = "Theirs".into();
    theirs.save(None).unwrap();

    mine.project_mut().unwrap().pages[2].description = "Mine".into();
    mine.commit();
    mine.save(None).unwrap();
    // Both changes are in the folder and in the project.
    let pages = &mine.project().unwrap().pages;
    assert_eq!((pages[1].description.as_str(), pages[2].description.as_str()), ("Theirs", "Mine"));
    assert!(!mine.unsaved());
    // Undo takes back our change, not theirs.
    assert!(mine.can_undo());
    mine.undo().unwrap();
    let pages = &mine.project().unwrap().pages;
    assert_eq!((pages[1].description.as_str(), pages[2].description == "Mine"), ("Theirs", false));
    mine.redo().unwrap();
    assert!(!mine.unsaved());
    let mut check = session();
    check.open(index.to_str().unwrap()).unwrap();
    assert!(check.project().unwrap() == mine.project().unwrap());

    // The same page changed by both: nothing is overwritten.
    theirs.open(index.to_str().unwrap()).unwrap();
    theirs.project_mut().unwrap().pages[1].description = "Theirs again".into();
    theirs.save(None).unwrap();
    mine.project_mut().unwrap().pages[1].description = "Mine too".into();
    let err = mine.save(None).unwrap_err();
    assert!(err.contains("pages/"), "{err}");
    check.open(index.to_str().unwrap()).unwrap();
    assert_eq!(check.project().unwrap().pages[1].description, "Theirs again");
    // Under another name the project can still be saved.
    mine.save(Some(dir.join("mine").join("demo.oecp").to_str().unwrap())).unwrap();

    // New objects of two copies get different ids.
    let (a, b) = (mine.project_mut().unwrap().new_id(), theirs.project_mut().unwrap().new_id());
    assert_ne!(a, b);
}

#[test]
fn open_reports_unreadable_and_invalid_files() {
    let dir = temp_dir("session-open");
    let mut s = session();
    assert!(s.open(dir.join("missing.oec").to_str().unwrap()).unwrap_err().starts_with("Cannot read"));
    let bad = dir.join("bad.oec");
    std::fs::write(&bad, "not json").unwrap();
    assert!(s.open(bad.to_str().unwrap()).unwrap_err().starts_with("Not a valid project file"));
    assert!(s.open(dir.join("missing.oecp").to_str().unwrap()).unwrap_err().starts_with("Cannot read"));
}

#[test]
fn every_commit_is_one_undo_step() {
    let mut s = session();
    s.replace(Project::default(), None);
    assert!(!s.can_undo());
    s.project_mut().unwrap().info.name = "A".into();
    s.commit();
    s.project_mut().unwrap().info.name = "B".into();
    s.commit();
    assert!(s.undo().unwrap());
    assert_eq!(s.project().unwrap().info.name, "A");
    assert!(s.undo().unwrap());
    assert!(!s.undo().unwrap(), "nothing left to undo");
    assert!(s.redo().unwrap());
    assert_eq!(s.project().unwrap().info.name, "A");
}
