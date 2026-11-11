use super::*;
use crate::testing::*;
use openecraft_core::macros::from_page;
use openecraft_core::sample::demo_project;

#[test]
fn options_rules_and_generation() {
    let dir = temp_dir("config");
    let file = dir.join("macros.json");
    let mut lib = MacroLibrary::default();
    let demo = demo_project();
    let mut m = from_page(&demo.pages[1], "Motor");
    m.page_description = "Motor {LINE}".into();
    lib.upsert_variant(m, "");
    std::fs::write(&file, serde_json::to_string(&lib).unwrap()).unwrap();

    let mut s = new_session();
    run(&mut s, "set_option", json!({ "name": "LINE", "choices": "L1, L2", "value": "L2" })).unwrap();
    assert!(run(&mut s, "set_option", json!({ "name": "LINE", "value": "L9" })).unwrap_err().contains("not one of its choices"));
    let cfg = run(&mut s, "add_rule", json!({ "macro": "Motor", "condition": "LINE=L2" })).unwrap();
    assert_eq!(cfg["rules"][0]["number"], 1);
    let g = run(&mut s, "generate_pages", json!({ "macros": file.to_str().unwrap() })).unwrap();
    assert_eq!(g["pages"].as_array().unwrap().len(), 1);
    assert_eq!(s.project().unwrap().pages[1].description, "Motor L2");
    run(&mut s, "set_option", json!({ "name": "LINE", "value": "L1" })).unwrap();
    let g = run(&mut s, "generate_pages", json!({ "macros": file.to_str().unwrap() })).unwrap();
    assert_eq!((g["removed"].as_u64(), g["pages"].as_array().unwrap().len()), (Some(1), 0));
    run(&mut s, "delete_rule", json!({ "number": 1 })).unwrap();
    assert!(run(&mut s, "delete_rule", json!({ "number": 1 })).is_err());
    run(&mut s, "delete_option", json!({ "name": "LINE" })).unwrap();
    assert_eq!(run(&mut s, "get_configuration", json!({})).unwrap(), json!({ "options": [], "rules": [] }));
}

#[test]
fn preplanning_is_imported_from_csv_and_pages_generated() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let dir = temp_dir("preplanning");
    let csv = dir.join("plan.csv");
    std::fs::write(&csv, "Segment;Tag;Kind;Description;Power (kW);Voltage;Part;Macro\nPlant/Pumps;-M60;Motor;Pump 1;7,5;400 V;;Page one\nPlant/Pumps;-M61;Motor;Pump 2;5;400 V;;\n").unwrap();
    let r = run(&mut s, "import_preplanning", json!({ "path": csv.to_str().unwrap() })).unwrap();
    assert_eq!((r["segments_created"].as_u64(), r["devices_added"].as_u64()), (Some(2), Some(2)));
    let prj = s.project().unwrap();
    let mut m = openecraft_core::macros::from_page(&prj.pages[1], "Page one");
    m.kind = openecraft_core::macros::MacroKind::Page;
    let lib = dir.join("macros.json");
    std::fs::write(&lib, serde_json::to_string(&openecraft_core::macros::MacroLibrary { macros: vec![m] }).unwrap()).unwrap();
    let g = run(&mut s, "generate_preplanning_pages", json!({ "macros": lib.to_str().unwrap(), "segment": "Plant/Pumps" })).unwrap();
    assert_eq!(g["pages"].as_array().unwrap().len(), 1);
    assert!(run(&mut s, "generate_preplanning_pages", json!({ "macros": lib.to_str().unwrap(), "segment": "Nope" })).is_err());
}

#[test]
fn macro_boxes_are_updated_from_a_library_file() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let dir = temp_dir("macro-boxes");
    let prj = s.project_mut().unwrap();
    let sel: std::collections::BTreeSet<u64> = [prj.pages[1].symbols[0].id].into();
    let m = openecraft_core::macros::from_selection(&prj.pages[1], &sel, "One");
    openecraft_core::macro_boxes::place(prj, 1, &m, &Default::default(), openecraft_core::geom::p(200.0, 150.0));
    let mut changed = m.clone();
    changed.clip.texts.push(openecraft_core::model::TextItem { id: 1, text: "NEW".into(), size: 3.0, ..Default::default() });
    let lib = dir.join("macros.json");
    std::fs::write(&lib, serde_json::to_string(&MacroLibrary { macros: vec![changed] }).unwrap()).unwrap();
    let r = run(&mut s, "update_macro_boxes", json!({ "macros": lib.to_str().unwrap() })).unwrap();
    assert_eq!(r["updated"], 1);
    assert!(s.project().unwrap().pages[1].texts.iter().any(|t| t.text == "NEW"));
}

#[test]
fn check_schemes_are_written_and_read() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    s.project_mut().unwrap().check_settings.disabled.push("cables".into());
    let file = temp_dir("check-scheme").join("scheme.json");
    run(&mut s, "export_check_scheme", json!({ "path": file.to_str().unwrap() })).unwrap();
    s.project_mut().unwrap().check_settings.disabled.clear();
    let r = run(&mut s, "import_check_scheme", json!({ "path": file.to_str().unwrap() })).unwrap();
    assert_eq!(r["disabled"], json!(["cables"]));
}

#[test]
fn settings_schemes_are_written_and_read() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let file = temp_dir("settings-scheme").join("settings.json");
    run(&mut s, "export_settings", json!({ "path": file.to_str().unwrap() })).unwrap();
    run(&mut s, "new_project", json!({ "discard": true })).unwrap();
    let r = run(&mut s, "import_settings", json!({ "path": file.to_str().unwrap() })).unwrap();
    assert_eq!(r["taken"].as_array().unwrap().len(), 10);
}
