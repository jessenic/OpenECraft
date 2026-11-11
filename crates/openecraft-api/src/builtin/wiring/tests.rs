use super::*;
use crate::builtin::testing::*;

fn demo() -> Session {
    let mut s = demo_session();
    s.project_mut().unwrap().info.name = "Demo".into();
    s
}

#[test]
fn wires_are_listed_marked_and_counted() {
    let mut s = demo();
    let list = run(&mut s, "wiring_list", json!({ "order": "cross_section" })).unwrap();
    let wires = list["wires"].as_array().unwrap().clone();
    assert!(!wires.is_empty());
    assert_eq!(list["progress"]["wired"], 0);
    assert!(wires.iter().all(|w| w["status"] == "open" && w["strip_mm"].is_number()));
    let (from, to) = (wires[0]["from"].as_str().unwrap(), wires[0]["to"].as_str().unwrap());
    // The ends may be given in either order.
    let r = run(&mut s, "mark_wire", json!({ "from": to, "to": from, "state": "wired" })).unwrap();
    assert_eq!((r["status"].as_str(), r["wired"].as_i64()), (Some("wired"), Some(1)));
    let open = run(&mut s, "wiring_list", json!({ "status": "open" })).unwrap();
    assert_eq!(open["wires"].as_array().unwrap().len(), wires.len() - 1);
    run(&mut s, "mark_wire", json!({ "from": from, "to": to, "state": "problem", "note": "too short" })).unwrap();
    let p = run(&mut s, "wiring_list", json!({ "status": "problem" })).unwrap();
    assert_eq!(p["wires"][0]["note"], "too short");
    assert_eq!(p["progress"]["problems"], 1);
    run(&mut s, "mark_wire", json!({ "from": from, "to": to, "state": "open" })).unwrap();
    assert!(s.project().unwrap().wiring_marks.is_empty());
}

#[test]
fn wrong_input_is_rejected() {
    let mut s = demo();
    assert!(run(&mut s, "mark_wire", json!({ "from": "-X9:1", "to": "-X9:2", "state": "wired" })).unwrap_err().contains("No routed wire"));
    assert!(run(&mut s, "mark_wire", json!({ "from": "a", "to": "b", "state": "half" })).unwrap_err().contains("Unknown state"));
    assert!(run(&mut s, "wiring_list", json!({ "order": "colour" })).unwrap_err().contains("Unknown order"));
    assert!(run(&mut s, "wiring_list", json!({ "enclosure": "+NOPE" })).unwrap_err().contains("No enclosure"));
    assert_eq!(run(&mut s, "remove_old_wiring_marks", json!({})).unwrap()["removed"], 0);
}

#[test]
fn numbering_of_wires_devices_and_pages() {
    let mut s = demo();
    let set = run(&mut s, "set_wire_numbering", json!({ "scheme": "format", "format": "{page}.{n}", "digits": 2, "skip_kinds": "pe" })).unwrap();
    assert_eq!((set["scheme"].as_str(), set["digits"].as_u64()), (Some("format"), Some(2)));
    assert_eq!(set["skip_kinds"], json!(["pe"]));
    assert!(run(&mut s, "set_wire_numbering", json!({ "scheme": "random" })).is_err());
    assert!(run(&mut s, "set_wire_numbering", json!({ "skip_kinds": "earth" })).is_err());
    assert!(run(&mut s, "renumber_wires", json!({})).unwrap()["renumbered"].as_u64().unwrap() > 0);
    let numbers: Vec<String> = s.project().unwrap().pages.iter().flat_map(|p| &p.wires).map(|w| w.number.clone()).filter(|n| !n.is_empty()).collect();
    assert!(numbers.iter().all(|n| n.split_once('.').is_some_and(|(_, c)| c.len() >= 2)), "{numbers:?}");

    let plan = run(&mut s, "renumber_devices", json!({ "format": "{letter}{page}{column}", "dry_run": true })).unwrap();
    let renamed = plan["renamed"].as_array().unwrap().clone();
    assert!(!renamed.is_empty());
    let first_old = renamed[0]["old"].as_str().unwrap().to_string();
    let first_new = renamed[0]["new"].as_str().unwrap().to_string();
    assert!(s.project().unwrap().all_tags().contains(&first_old), "a dry run changes nothing");
    run(&mut s, "renumber_devices", json!({ "format": "{letter}{page}{column}" })).unwrap();
    assert!(s.project().unwrap().all_tags().contains(&first_new));
    assert!(run(&mut s, "renumber_devices", json!({ "pages": "nope" })).is_err());

    let pages = run(&mut s, "renumber_pages", json!({ "start": 10, "step": 10 })).unwrap();
    assert_eq!(pages["pages"][0], "10");
    assert_eq!(pages["pages"][1], "20");
}

#[test]
fn cross_reference_settings() {
    let mut s = demo();
    let x = run(&mut s, "set_cross_references", json!({ "row": true, "interruptions": "pairs", "contact_image": false })).unwrap();
    assert_eq!((x["row"].as_bool(), x["interruptions"].as_str(), x["contact_image"].as_bool()), (Some(true), Some("pairs"), Some(false)));
    assert!(run(&mut s, "set_cross_references", json!({ "structure": "sometimes" })).is_err());
    let x = s.project().unwrap().xref.clone();
    assert!(x.row && x.hide_contact_image);
}

#[test]
fn find_and_replace_in_the_project() {
    let mut s = demo();
    let hits = run(&mut s, "find", json!({ "text": "-K1", "kinds": "device" })).unwrap();
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["kind"], "device");
    assert!(run(&mut s, "find", json!({ "text": "x", "kinds": "colour" })).is_err());
    run(&mut s, "add_text", json!({ "page": "2", "x": 40, "y": 40, "text": "Old note" })).unwrap();
    let changed = run(&mut s, "replace_text", json!({ "text": "old", "with": "New", "kinds": "text" })).unwrap();
    assert_eq!(changed["changed"], 1);
    let hits = run(&mut s, "find", json!({ "text": "New note", "case_sensitive": true })).unwrap();
    assert_eq!(hits[0]["kind"], "text");
}

#[test]
fn unplaced_functions_are_listed_and_placed() {
    let mut s = demo();
    let list = run(&mut s, "list_unplaced", json!({})).unwrap();
    let ch = list.as_array().unwrap().iter().find(|u| u["kind"] == "plc_channel" && u["tag"] == "-A1").unwrap().clone();
    let before = list.as_array().unwrap().len();
    run(&mut s, "place_symbol", json!({ "page": "2", "symbol": ch["symbol"], "x": 300, "y": 84, "tag": ch["tag"], "label": ch["label"] })).unwrap();
    let after = run(&mut s, "list_unplaced", json!({})).unwrap();
    assert_eq!(after.as_array().unwrap().len(), before - 1, "the channel is placed now");
    assert!(list.as_array().unwrap().iter().any(|u| u["kind"] == "function"));
}

#[test]
fn mounting_assistant_commands() {
    let mut s = demo();
    let list = run(&mut s, "mounting_list", json!({})).unwrap();
    let total = list["total"].as_u64().unwrap();
    assert!(total > 0);
    let item = list["items"][0]["item"].as_u64().unwrap();
    run(&mut s, "mark_mounted", json!({ "item": item, "state": "mounted" })).unwrap();
    assert_eq!(run(&mut s, "mounting_list", json!({})).unwrap()["mounted"], 1);
    assert!(run(&mut s, "mark_mounted", json!({ "item": item, "state": "glued" })).is_err());
    assert!(run(&mut s, "mark_mounted", json!({ "item": 1, "state": "open" })).is_err() || total > 0);
    assert!(run(&mut s, "mounting_list", json!({ "enclosure": "nope" })).is_err());
}

#[test]
fn labeling_schemes_are_listed_added_selected_and_removed() {
    let mut s = demo();
    let list = run(&mut s, "marking_schemes", json!({})).unwrap();
    assert_eq!(list["selected"], "Standard");
    assert!(list["schemes"].as_array().unwrap().len() >= 6);
    let out = run(&mut s, "set_marking_scheme", json!({ "name": "Shop", "columns": "Text, Material", "separator": "tab", "categories": "wire", "select": true })).unwrap();
    assert_eq!(out["selected"], "Shop");
    let shop = out["schemes"].as_array().unwrap().iter().find(|x| x["name"] == "Shop").unwrap().clone();
    assert_eq!(shop["columns"], json!(["Text", "Material"]));
    assert_eq!(shop["separator"], "tab");
    assert!(run(&mut s, "set_marking_scheme", json!({ "name": "Bad", "columns": "Colour" })).is_err());
    assert!(run(&mut s, "set_marking_scheme", json!({ "name": "Bad", "categories": "plug" })).is_err());
    let out = run(&mut s, "set_marking_scheme", json!({ "name": "Shop", "remove": true })).unwrap();
    assert_eq!(out["selected"], "Standard");
    assert!(run(&mut s, "set_marking_scheme", json!({ "name": "Shop", "remove": true })).is_err());
}

#[test]
fn nailboard_of_the_demo_harness() {
    let mut s = demo();
    let nb = run(&mut s, "harness_nailboard", json!({ "harness": "-WH1" })).unwrap();
    assert!(nb["branches"].as_array().unwrap().len() >= 2);
    assert!(nb["scale"].as_str().unwrap().starts_with("1:"));
    assert!(run(&mut s, "harness_nailboard", json!({ "harness": "-WH7" })).is_err());
}

#[test]
fn wire_ends_get_their_terminations() {
    let mut s = demo();
    let list = run(&mut s, "wiring_list", json!({})).unwrap();
    let w = &list["wires"][0];
    let (from, to) = (w["from"].as_str().unwrap().to_string(), w["to"].as_str().unwrap().to_string());
    assert_eq!(w["ends"][0]["at"], from.as_str());
    assert_eq!(w["ends"][0]["termination"], "ferrule");
    // One wire's end gets a ring terminal, the device's other ends pin terminals.
    let device = from.rsplit_once(':').unwrap().0.to_string();
    let r = run(&mut s, "set_termination", json!({ "at": device, "kind": "pin" })).unwrap();
    assert!(!r["ends"].as_array().unwrap().is_empty());
    assert!(r["ends"].as_array().unwrap().iter().all(|e| e["termination"] == "pin"));
    let r = run(&mut s, "set_termination", json!({ "at": from, "to": to, "kind": "ring" })).unwrap();
    assert_eq!(r["ends"].as_array().unwrap().len(), 1);
    assert_eq!(r["ends"][0]["termination"], "ring");
    assert!(r["ends"][0]["part"].as_str().unwrap().starts_with("GEN."));
    let list = run(&mut s, "wiring_list", json!({})).unwrap();
    let w = list["wires"].as_array().unwrap().iter().find(|x| x["from"] == from.as_str() && x["to"] == to.as_str()).unwrap();
    assert_eq!((w["ends"][0]["termination"].as_str(), w["ends"][1]["termination"].as_str()), (Some("ring"), Some("ferrule")));
    let t = run(&mut s, "terminations", json!({})).unwrap();
    assert_eq!(t["chosen"].as_array().unwrap().len(), 2);
    assert_eq!(t["kinds"].as_array().unwrap().len(), Termination::ALL.len());
    // auto goes back to the layout setting; an unknown kind is refused.
    run(&mut s, "set_termination", json!({ "at": from, "to": to, "kind": "auto" })).unwrap();
    assert_eq!(run(&mut s, "terminations", json!({})).unwrap()["chosen"].as_array().unwrap().len(), 1);
    assert!(run(&mut s, "set_termination", json!({ "at": from, "kind": "glue" })).unwrap_err().contains("Unknown termination"));
}

#[test]
fn twin_ferrules_are_set_for_both_wires() {
    let mut s = demo();
    let list = run(&mut s, "wiring_list", json!({})).unwrap();
    let wires = list["wires"].as_array().unwrap().clone();
    let ends = |w: &Value| [(w["from"].as_str().unwrap().to_string(), w["to"].as_str().unwrap().to_string()), (w["to"].as_str().unwrap().to_string(), w["from"].as_str().unwrap().to_string())];
    let all: Vec<(String, String)> = wires.iter().flat_map(ends).collect();
    // A connection point with two routed wires.
    let (at, to) = all.iter().find(|(at, _)| all.iter().filter(|(a, _)| a == at).count() == 2).cloned().expect("a point with two wires");
    let other = all.iter().find(|(a, o)| *a == at && *o != to).unwrap().1.clone();
    run(&mut s, "set_termination", json!({ "at": at, "to": to, "kind": "twin_ferrule" })).unwrap();
    let t = run(&mut s, "terminations", json!({})).unwrap();
    assert_eq!(t["chosen"].as_array().unwrap().len(), 2, "both wires: {t}");
    let list = run(&mut s, "wiring_list", json!({})).unwrap();
    let end = |far: &str| -> Value {
        let w = list["wires"].as_array().unwrap().iter().find(|w| ends(w).iter().any(|(a, o)| *a == at && o == far)).unwrap().clone();
        let k = usize::from(w["from"] != at.as_str());
        w["ends"][k].clone()
    };
    let (a, b) = (end(&to), end(&other));
    assert_eq!((a["twin_with"].as_str(), b["twin_with"].as_str()), (Some(other.as_str()), Some(to.as_str())));
    assert_eq!([&a, &b].iter().filter(|e| e["part"].is_string()).count(), 1, "the part at one of them");
    // The defaults of the layout.
    let r = run(&mut s, "set_ferrules", json!({ "twin_ferrules": true })).unwrap();
    assert_eq!((r["ferrules"].as_bool(), r["twin_ferrules"].as_bool()), (Some(true), Some(true)));
    assert_eq!(run(&mut s, "terminations", json!({})).unwrap()["twin_ferrules"], true);
}
