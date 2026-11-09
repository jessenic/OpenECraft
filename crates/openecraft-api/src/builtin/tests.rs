use super::*;
use crate::{Args, Origin, Plugins};
use openecraft_core::sample::demo_project;
use serde_json::Map;

#[test]
fn registered_cable_rename_keeps_explicit_marker_and_shared_device_separate() {
    use openecraft_core::model::{Cable,ConnDef,SymbolInst};
    let p=Plugins::new(Vec::<String>::new());let mut prj=Project::default();
    prj.cables.push(Cable {id:10,tag:"-Shared".into(),..Default::default()});
    prj.pages[0].symbols=vec![SymbolInst {id:11,sym:"coil".into(),tag:"-Shared".into(),..Default::default()},
                            SymbolInst {id:12,sym:"coil".into(),tag:"-Shared".into(),..Default::default()}];
    prj.pages[0].conn_defs.push(ConnDef {id:13,cable:"-Shared".into(),core:"BN".into(),..Default::default()});
    for (command,args) in [("set_properties",serde_json::json!({"object":"function:11","values":{"function.cable":"-Shared"}})),
        ("table_set",serde_json::json!({"table":"cables","key":"10","column":"Cable","value":"-W20"}))] {
        p.run(p.command(command).unwrap(),&mut prj,args.as_object().unwrap()).unwrap();
    }
    assert_eq!(prj.pages[0].symbols[0].tag,"-W20");assert_eq!(prj.pages[0].symbols[0].values["function.cable"],"-W20");
    assert_eq!(prj.pages[0].symbols[1].tag,"-Shared");assert_eq!(prj.pages[0].conn_defs[0].cable,"-W20");
    let reopened:Project=serde_json::from_str(&serde_json::to_string(&prj).unwrap()).unwrap();
    assert_eq!(reopened.pages[0].symbols[0].values["function.cable"],"-W20");
    prj.cables.push(Cable {id:20,tag:"-W2".into(),..Default::default()});let before=prj.clone();
    for value in ["","-W2","without-prefix"] {
        let args=serde_json::json!({"table":"cables","key":"10","column":"Cable","value":value});
        assert!(p.run(p.command("table_set").unwrap(),&mut prj,args.as_object().unwrap()).is_err());assert_eq!(prj,before);
    }
}

#[test]
fn statistics_counts_the_demo_project() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let out = p.run(p.command("project_statistics").unwrap(), &mut prj, &Map::new()).unwrap();
    assert_eq!(out["pages"], prj.pages.len());
    assert_eq!(out["errors"], 0);
    let table = (p.report("project_statistics").unwrap().run)(&prj, &Analysis::checked(&prj)).unwrap();
    assert_eq!(table.rows.len(), 8);
    let tags = (p.exporter("device_tags").unwrap().run)(&prj, &Analysis::build(&prj), &Args(&Map::new())).unwrap().text().unwrap();
    assert_eq!(tags.lines().count(), openecraft_core::structure::device_tags(&prj).len());
    assert!(p.exporter("pdf").is_some() && p.exporter("dwg").is_some_and(|e| e.binary), "built-in exports are offered");
    assert!(p.list().iter().any(|l| l.info.id == "statistics" && l.origin == Origin::BuiltIn));
}

#[test]
fn project_checks_are_a_built_in_plugin() {
    let p = Plugins::new(Vec::<String>::new());
    let prj = demo_project();
    let an = Analysis::build(&prj);
    assert!(an.issues.is_empty(), "build leaves the issues to the checks");
    let names: Vec<&str> = p.checks().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"devices") && names.contains(&"panel_layout"), "{names:?}");
    let issues = p.check(&prj, &an);
    assert!(issues.len() >= Analysis::checked(&prj).issues.len());
    assert!(p.list().iter().any(|l| l.info.id == "project_checks"));
}

#[test]
fn macro_boxes_are_listed() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    assert_eq!(p.run(p.command("list_macro_boxes").unwrap(), &mut prj, &Map::new()).unwrap(), serde_json::json!([]));
    let sel: std::collections::BTreeSet<u64> = [prj.pages[1].symbols[0].id].into();
    let m = openecraft_core::macros::from_selection(&prj.pages[1], &sel, "One");
    let values = [("NAME".to_string(), "X".to_string())].into();
    openecraft_core::macro_boxes::place(&mut prj, 1, &m, &values, openecraft_core::geom::p(200.0, 150.0));
    let list = p.run(p.command("list_macro_boxes").unwrap(), &mut prj, &Map::new()).unwrap();
    assert_eq!(list[0]["macro"], "One");
    assert_eq!(list[0]["items"], 1);
    assert_eq!(list[0]["values"]["NAME"], "X");
}

#[test]
fn a_failed_command_changes_nothing_and_old_parameter_names_work() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let before = prj.clone();
    // set_potential checks the cross-section after creating the definition.
    let args = serde_json::json!({ "name": "+99V", "cross_section": "thick" });
    assert!(p.run(p.command("set_potential").unwrap(), &mut prj, args.as_object().unwrap()).is_err());
    assert_eq!(prj, before);
    // rename_device: tag / new_tag, also old / new.
    let args = serde_json::json!({ "old": "-S4", "new": "-S9" });
    assert_eq!(p.run(p.command("rename_device").unwrap(), &mut prj, args.as_object().unwrap()).unwrap()["new"], "-S9");
    let args = serde_json::json!({ "tag": "-S9", "new_tag": "-S4" });
    assert_eq!(p.run(p.command("rename_device").unwrap(), &mut prj, args.as_object().unwrap()).unwrap()["new"], "-S4");
    assert_eq!(renamed_param("replace_text", "with"), Some("replace"));
    assert_eq!(renamed_param("replace_text", "kinds"), None);
}

#[test]
fn accessories_are_listed_and_added() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let mut run = |name: &str, args: serde_json::Value| p.run(p.command(name).unwrap(), &mut prj, args.as_object().unwrap());
    // The Hager signal contact fits two RCCBs of the bundled catalog.
    let cz = run("list_accessories", serde_json::json!({ "part": "HAG.CZ001" })).unwrap();
    assert_eq!((cz["accessory"].as_bool(), cz["mount"].as_str()), (Some(true), Some("left")));
    assert!(cz["fits"].as_array().unwrap().iter().any(|f| f["number"] == "HAG.CCA216D"));
    let cda = run("list_accessories", serde_json::json!({ "part": "HAG.CDA440D" })).unwrap();
    assert_eq!(cda["accessories"][0]["number"], "HAG.CZ001");
    // The contactor -K1 of the demo takes the auxiliary contact block it does not have yet.
    let k1 = run("list_accessories", serde_json::json!({ "tag": "-K1" })).unwrap();
    assert!(k1["accessories"].as_array().unwrap().iter().any(|a| a["accessory"]["number"] == "GEN.AUX-1NO1NC" && a["fits"] == "GEN.CT-9A-24DC"), "{k1}");
    let added = run("add_accessory", serde_json::json!({ "tag": "-K1", "part": "GEN.AUX-1NO1NC" })).unwrap();
    assert_eq!(added["fits"], "GEN.CT-9A-24DC");
    assert!(run("add_accessory", serde_json::json!({ "tag": "-K1", "part": "HAG.CZ001" })).unwrap_err().contains("fits none"));
    assert!(run("list_accessories", serde_json::json!({})).is_err());
    let devices = run("list_devices", serde_json::json!({})).unwrap();
    let k1 = devices.as_array().unwrap().iter().find(|d| d["tag"] == "-K1").unwrap();
    assert!(k1["parts"].as_array().unwrap().iter().any(|r| r["number"] == "GEN.AUX-1NO1NC" && r["accessory_of"] == "GEN.CT-9A-24DC"), "{k1}");
}

#[test]
fn values_out_of_range_are_refused_not_corrected() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let mut run = |name: &str, args: serde_json::Value| p.run(p.command(name).unwrap(), &mut prj, args.as_object().unwrap());
    assert_eq!(run("set_frame", serde_json::json!({ "name": "Tiny", "columns": 0 })).unwrap_err(), "columns must be between 1 and 60");
    assert_eq!(run("set_wire_numbering", serde_json::json!({ "start": -5 })).unwrap_err(), "start must not be negative");
    assert_eq!(run("set_wire_numbering", serde_json::json!({ "step": 0 })).unwrap_err(), "step must be at least 1");
    assert!(run("set_wire_numbering", serde_json::json!({ "format": "{bogus}" })).unwrap_err().starts_with("Unknown placeholder {bogus}"));
    assert!(run("set_wire_numbering", serde_json::json!({ "format": "{page}.{n}" })).is_ok());
    assert!(run("renumber_devices", serde_json::json!({ "format": "{letter}{x}" })).is_err());
    assert!(run("renumber_pages", serde_json::json!({ "start": -5 })).is_err());
    assert!(run("renumber_pages", serde_json::json!({ "step": 0 })).is_err());
    assert!(run("set_terminal", serde_json::json!({ "strip": "-X1", "terminal": "1", "level": 99 })).is_err());
    assert!(run("add_enclosure", serde_json::json!({ "name": "+CAB9", "width": -5 })).is_err());
    assert!(run("add_room", serde_json::json!({ "name": "+R1", "width": 0 })).is_err());
    let room = run("add_room", serde_json::json!({ "name": "+R1", "width": 5000, "depth": 4000 })).unwrap();
    assert_eq!(room["name"], "+R1");
    assert_eq!(run("add_room", serde_json::json!({ "name": "+R1" })).unwrap_err(), "Room +R1 exists already");
    // A cabinet is placed in its room or not at all.
    let err = run("place_in_room", serde_json::json!({ "enclosure": "+CAB1", "room": "+R1", "x": 100000, "z": 0 })).unwrap_err();
    assert_eq!(err, "+CAB1 would stand outside the room +R1");
    assert!(run("place_in_room", serde_json::json!({ "enclosure": "+CAB1", "room": "+R1", "x": 100, "z": 100 })).is_ok());
}

#[test]
fn names_codes_and_identifiers_are_checked() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = openecraft_core::sample::example("pump_station").unwrap();
    let mut run = |name: &str, args: serde_json::Value| p.run(p.command(name).unwrap(), &mut prj, args.as_object().unwrap());
    // Structure identifiers keep no blanks; renaming needs an identifier that exists.
    assert!(run("set_page", serde_json::json!({ "page": "2.1", "location": "has space" })).unwrap_err().contains("must not contain blanks"));
    assert!(run("define_structure", serde_json::json!({ "aspect": "location", "id": "BAD ID" })).is_err());
    assert_eq!(run("rename_structure", serde_json::json!({ "aspect": "location", "id": "NOSUCH", "new_id": "X" })).unwrap_err(), "No identifier +NOSUCH in the project");
    assert!(run("rename_structure", serde_json::json!({ "aspect": "location", "id": "CAB1", "new_id": "FLD" })).unwrap_err().contains("pass merge"));
    assert!(run("rename_structure", serde_json::json!({ "aspect": "location", "id": "CAB1", "new_id": "FLD", "merge": true })).is_ok());
    // Page names lose their blanks at the ends.
    assert_eq!(run("set_page", serde_json::json!({ "page": "2.1", "name": " 20 " })).unwrap()["name"], "20");
    // Check settings take what `check` and `list_checks` show.
    assert!(run("set_check", serde_json::json!({ "name": "devices", "enabled": false })).unwrap().get("note").is_none());
    assert!(run("set_check", serde_json::json!({ "name": "my_plugin_check", "enabled": false })).unwrap()["note"].is_string());
    assert!(run("set_check", serde_json::json!({ "name": "Not A Check", "enabled": false })).is_err());
    assert!(run("set_message_level", serde_json::json!({ "code": "bogus code", "level": "error" })).is_err());
    assert!(run("set_message_level", serde_json::json!({ "code": "devices.d5601b", "level": "error" })).is_ok());
    assert!(run("accept_message", serde_json::json!({ "fingerprint": "nosuch" })).is_err());
    assert!(run("accept_message", serde_json::json!({ "fingerprint": "devices.d5601b:45b990c0:2" })).is_ok());
    // Sections are numbered with letters and digits.
    for number in ["-1", "0", "a b"] {
        assert!(run("add_section", serde_json::json!({ "number": number })).is_err(), "{number}");
    }
    assert_eq!(run("add_section", serde_json::json!({ "number": "9" })).unwrap()["number"], "9");
    // Languages of ISO 639-1, the targets separated by commas or blanks.
    assert!(run("set_project_languages", serde_json::json!({ "source": "xx" })).is_err());
    let langs = run("set_project_languages", serde_json::json!({ "source": "en", "targets": "de, fr" })).unwrap();
    assert_eq!(langs["targets"].as_array().unwrap().len(), 2);
    // Potentials have known colours; conditions of rules are checked.
    assert!(run("set_potential", serde_json::json!({ "name": "L1", "color": "notacolor" })).is_err());
    assert_eq!(run("set_potential", serde_json::json!({ "name": "L1", "new_name": "L9" })).unwrap()["name"], "L9");
    run("set_option", serde_json::json!({ "name": "Opt", "choices": "a,b", "value": "a" })).unwrap();
    assert!(run("add_rule", serde_json::json!({ "macro": "M", "condition": "Opt == a" })).unwrap_err().contains("needs OPTION=value"));
    assert!(run("add_rule", serde_json::json!({ "macro": "M", "condition": "Nope=a" })).is_err());
    assert!(run("add_rule", serde_json::json!({ "macro": "M", "condition": "Opt=a" })).is_ok());
}
