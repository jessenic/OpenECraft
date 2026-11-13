use super::*;
use crate::testing::*;
use openecraft_core::model::Project;

fn demo_session() -> Session {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    s
}

fn save_project(dir: &Path, file: &str, name: &str, company: &str) -> String {
    let mut prj = Project::new_default();
    prj.info.name = name.into();
    prj.info.company = company.into();
    let path = dir.join(file);
    std::fs::write(&path, prj.to_json()).unwrap();
    path.display().to_string()
}

#[test]
fn projects_are_indexed_searched_and_changed_in_batch() {
    let dir = temp_dir("mgmt-index");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let a = save_project(&dir, "a.oec", "Line A", "ACME");
    save_project(&dir.join("sub"), "b.oec", "Line B", "Other");
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    let mut s = session();
    let db = dir.join("db.json");
    let out = run(&mut s, "index_projects", json!({ "folder": dir.display().to_string(), "database": db.display().to_string(), "verify": true })).unwrap();
    assert_eq!(out["total"], 2);
    assert_eq!(out["projects"][0]["verified"], json!([0, 0]));

    let found = run(&mut s, "search_projects", json!({ "database": db.display().to_string(), "query": "company=acme" })).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["path"], a);
    let found = run(&mut s, "search_projects", json!({ "folder": dir.display().to_string(), "query": "line" })).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 2);
    // Without database or folder the current folder is searched.
    assert!(run(&mut s, "search_projects", json!({ "query": "nothing-matches-this" })).unwrap().as_array().unwrap().is_empty());

    let dry = run(&mut s, "batch_project_properties", json!({ "folder": dir.display().to_string(), "set": { "creator": "NS", "plant": "North" }, "dry_run": true })).unwrap();
    assert_eq!(dry.as_array().unwrap().len(), 2);
    assert_eq!(dry[0]["written"], false);
    let out = run(&mut s, "batch_project_properties", json!({ "folder": dir.display().to_string(), "query": "company=ACME", "set": { "creator": "NS" }, "replace": "name", "find": "Line", "with": "Conveyor" })).unwrap();
    assert_eq!(out.as_array().unwrap().len(), 1);
    assert_eq!(out[0]["written"], true);
    assert_eq!(out[0]["changes"].as_array().unwrap().len(), 2);
    let changed = Project::from_json(&std::fs::read_to_string(&a).unwrap()).unwrap();
    assert_eq!((changed.info.name.as_str(), changed.info.creator.as_str()), ("Conveyor A", "NS"));
    assert!(run(&mut s, "batch_project_properties", json!({ "folder": dir.display().to_string() })).is_err());
    assert!(run(&mut s, "batch_project_properties", json!({ "set": { "a": "b" } })).is_err());
}

#[test]
fn projects_are_archived_converted_and_verified() {
    let dir = temp_dir("mgmt-archive");
    let mut s = demo_session();
    let v = run(&mut s, "check_project_data", json!({})).unwrap();
    assert_eq!(v["passed"], true);
    s.project_mut().unwrap().devices.insert("-Q99".into(), Default::default());
    let v = run(&mut s, "check_project_data", json!({ "repair": true })).unwrap();
    assert_eq!(v["repaired"].as_array().unwrap().len(), 1);

    let path = dir.join("demo.oecz").display().to_string();
    let out = run(&mut s, "archive_project", json!({ "path": path, "comment": "Handover", "compact": true })).unwrap();
    assert_eq!(out["files"][0], "project.oec");
    let info = run(&mut s, "archive_info", json!({ "path": path })).unwrap();
    assert_eq!((info["name"].as_str(), info["comment"].as_str()), (Some("Demo conveyor"), Some("Handover")));
    let opened = run(&mut s, "open_project", json!({ "path": path, "discard": true })).unwrap();
    assert_eq!(opened["file"], Value::Null);
    assert!(opened.get("conversion").is_none());

    let old = dir.join("old.oec");
    std::fs::write(&old, r#"{ "info": { "name": "Old" }, "pages": [], "next_id": 1 }"#).unwrap();
    let c = run(&mut s, "convert_project", json!({ "path": old.display().to_string() })).unwrap();
    assert_eq!((c["from"].as_u64(), c["to"].as_u64()), (Some(0), Some(u64::from(openecraft_core::model::FORMAT))));
    assert!(std::fs::read_to_string(&old).unwrap().contains(&format!("\"format\": {}", openecraft_core::model::FORMAT)));
    let c = run(&mut s, "convert_project", json!({ "path": path })).unwrap();
    assert!(c["output"].as_str().unwrap().ends_with("demo.oec"));
}

#[test]
fn projects_are_converted_between_file_split_folder_and_archive() {
    let dir = temp_dir("mgmt-split");
    let mut s = session();
    let demo = openecraft_core::sample::demo_project();
    let file = dir.join("demo.oec");
    std::fs::write(&file, demo.to_json()).unwrap();

    // A split project gets a folder of its own next to the file.
    let c = run(&mut s, "convert_project", json!({ "path": file.display().to_string(), "format": "oecp" })).unwrap();
    let index = dir.join("demo").join("demo.oecp");
    assert_eq!(c["output"], index.display().to_string());
    assert!(dir.join("demo/pages").is_dir());
    let opened = run(&mut s, "open_project", json!({ "path": index.display().to_string() })).unwrap();
    assert_eq!(opened["file"], index.display().to_string());
    assert!(s.project().unwrap() == &demo);

    // Back to a project file and into an archive: by format or by the extension.
    let c = run(&mut s, "convert_project", json!({ "path": index.display().to_string(), "format": "oec" })).unwrap();
    let back = dir.join("demo").join("demo.oec");
    assert_eq!(c["output"], back.display().to_string());
    assert!(Project::from_json(&std::fs::read_to_string(&back).unwrap()).unwrap() == demo);
    let archive = dir.join("demo.oecz").display().to_string();
    run(&mut s, "convert_project", json!({ "path": index.display().to_string(), "output": archive })).unwrap();
    assert_eq!(run(&mut s, "archive_info", json!({ "path": archive })).unwrap()["name"], demo.info.name.as_str());
    let other = dir.join("other").join("p.oecp").display().to_string();
    run(&mut s, "convert_project", json!({ "path": archive, "output": other })).unwrap();
    assert!(openecraft_core::storage::folder::open(Path::new(&other)).unwrap().0 == demo);
    assert!(run(&mut s, "convert_project", json!({ "path": file.display().to_string(), "format": "zip" })).unwrap_err().contains("zip"));

    // Split projects are found and changed like project files.
    std::fs::remove_file(&back).unwrap();
    let found = run(&mut s, "search_projects", json!({ "folder": dir.join("demo").display().to_string(), "query": "" })).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!((found[0]["path"].as_str(), found[0]["pages"].as_u64()), (index.to_str(), Some(demo.pages.len() as u64)));
    let out = run(&mut s, "batch_project_properties", json!({ "files": index.display().to_string(), "set": { "creator": "NS" } })).unwrap();
    assert_eq!(out[0]["written"], true);
    assert_eq!(openecraft_core::storage::folder::open(&index).unwrap().0.info.creator, "NS");
    let out = run(&mut s, "batch_project_properties", json!({ "files": index.display().to_string(), "set": { "creator": "NS" } })).unwrap();
    assert_eq!(out[0]["written"], false);

    // A template may be a split project.
    run(&mut s, "new_project", json!({ "template": index.display().to_string(), "discard": true })).unwrap();
    assert_eq!(s.project().unwrap().pages.len(), demo.pages.len());
}

#[test]
fn subprojects_are_extracted_and_inserted() {
    let dir = temp_dir("mgmt-sub");
    let mut s = demo_session();
    let path = dir.join("power.oec").display().to_string();
    let out = run(&mut s, "extract_subproject", json!({ "name": "Power", "path": path, "pages": "2" })).unwrap();
    assert_eq!(out["pages"], json!(["2"]));
    let list = run(&mut s, "list_subprojects", json!({})).unwrap();
    assert_eq!(list["subprojects"][0]["out"], true);
    assert!(s.project().unwrap().pages[1].locked);

    let mut sub = session();
    run(&mut sub, "open_project", json!({ "path": path })).unwrap();
    assert_eq!(run(&mut sub, "list_subprojects", json!({})).unwrap()["parent"]["name"], "Power");
    run(&mut sub, "set_project_info", json!({ "description": "edited" })).unwrap();
    run(&mut sub, "save_project", json!({})).unwrap();

    let r = run(&mut s, "insert_subproject", json!({ "path": path })).unwrap();
    assert_eq!((r["pages_removed"].as_u64(), r["pages_inserted"].as_u64()), (Some(1), Some(1)));
    assert!(!s.project().unwrap().pages[1].locked);
    run(&mut s, "extract_subproject", json!({ "name": "Ctrl", "path": dir.join("c.oec").display().to_string(), "pages": "3" })).unwrap();
    let list = run(&mut s, "discard_subproject", json!({ "name": "Ctrl" })).unwrap();
    assert_eq!(list["subprojects"].as_array().unwrap().len(), 1);
    assert!(run(&mut s, "extract_subproject", json!({ "name": "X", "path": dir.join("x.oec").display().to_string() })).is_err());
}

#[test]
fn model_options_hide_and_show_content() {
    let mut s = demo_session();
    run(&mut s, "define_model_option", json!({ "name": "PLC", "description": "With PLC" })).unwrap();
    let pages = s.project().unwrap().pages.len();
    let out = run(&mut s, "assign_model_option", json!({ "name": "PLC", "pages": "4" })).unwrap();
    assert_eq!(out["options"][0]["pages"], 1);
    let out = run(&mut s, "set_model_option", json!({ "name": "PLC", "active": false })).unwrap();
    assert_eq!(out["moved"], 1);
    assert_eq!(s.project().unwrap().pages.len(), pages - 1);
    run(&mut s, "undo", json!({})).unwrap();
    assert_eq!(s.project().unwrap().pages.len(), pages);
    run(&mut s, "set_model_option", json!({ "name": "PLC", "active": false })).unwrap();
    let out = run(&mut s, "unassign_model_option", json!({ "pages": "4" })).unwrap();
    assert_eq!(out["unassigned"], 1);
    assert_eq!(s.project().unwrap().pages.len(), pages);

    run(&mut s, "define_model_option", json!({ "name": "PLC", "condition": "" })).unwrap();
    assert_eq!(run(&mut s, "list_model_options", json!({})).unwrap()[0]["description"], "With PLC");
    assert!(run(&mut s, "define_model_option", json!({ "name": "PLC", "condition": "NOPE=1" })).is_err());
    assert_eq!(run(&mut s, "apply_model_options", json!({})).unwrap()["switched"], json!([]));
    assert!(run(&mut s, "assign_model_option", json!({ "name": "PLC" })).is_err());
    run(&mut s, "remove_model_option", json!({ "name": "PLC" })).unwrap();
    assert_eq!(run(&mut s, "list_model_options", json!({})).unwrap(), json!([]));
}
