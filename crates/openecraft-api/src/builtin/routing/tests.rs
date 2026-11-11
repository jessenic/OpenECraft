use super::*;
use crate::builtin::testing::*;

fn demo() -> Session {
    let mut s = demo_session();
    s.project_mut().unwrap().info.name = "Demo".into();
    s
}

#[test]
fn project_cable_parts_are_listed_once_with_local_metadata() {
    let mut s=demo();
    let number="GEN.CBL-4G1.5";
    let prj=s.project_mut().unwrap();
    let part=prj.parts.iter_mut().find(|p|p.number==number).unwrap();
    part.group="Cables".into();part.description="Edited project cable".into();part.tech_data="Outer diameter: 6.3 mm".into();
    prj.parts.push(openecraft_core::parts::Part {number:"PROJECT.CABLE".into(),group:"Cables".into(),..Default::default()});
    let before=serde_json::to_value(s.project().unwrap()).unwrap();
    let choices=run(&mut s,"list_cable_parts",json!({})).unwrap();
    let rows=choices.as_array().unwrap();
    let matches:Vec<_>=rows.iter().filter(|p|p["number"]==number).collect();
    assert_eq!(matches.len(),1);
    assert_eq!(matches[0]["description"],"Edited project cable");
    assert!((matches[0]["diameter_mm"].as_f64().unwrap()-6.3).abs()<0.00001);
    assert!(rows.iter().any(|p|p["number"]=="PROJECT.CABLE"));
    assert_eq!(serde_json::to_value(s.project().unwrap()).unwrap(),before);
}

#[test]
fn catalog_cable_geometry_is_applied_through_the_shared_registry() {
    let mut s=demo();
    s.project_mut().unwrap().parts.push(openecraft_core::parts::Part {number:"DECLARED".into(),group:"Cables".into(),type_number:"Test cable".into(),
        tech_data:"Cores: BN BU\nCross-section: 0.75 mm²\nOuter diameter: 6.3 mm".into(),..Default::default()});
    s.project_mut().unwrap().parts.push(openecraft_core::parts::Part {number:"RELAY".into(),group:"Relays".into(),..Default::default()});
    let old=s.project().unwrap().cable("-W1").unwrap().part.clone();
    s.project_mut().unwrap().pages[0].symbols.push(openecraft_core::model::SymbolInst {id:999_999,sym:"coil".into(),tag:"-W1".into(),
        values:std::collections::BTreeMap::from([("function.part_number".into(),old)]),..Default::default()});
    let result=run(&mut s,"apply_cable_part",json!({"cable":"-W1","part":"DECLARED"})).unwrap();
    assert_eq!((result["part"].as_str(),result["cores"].as_str()),(Some("DECLARED"),Some("BN BU")));
    assert!((result["diameter_mm"].as_f64().unwrap()-6.3).abs()<0.00001);
    assert_eq!(s.project().unwrap().cable("-W1").unwrap().diameter,6.3);
    let selected=s.project().unwrap().pages[0].symbols.last().unwrap().selected_part_number(s.project().unwrap());
    assert_eq!(selected,Some("DECLARED"));
    let before=serde_json::to_value(s.project().unwrap()).unwrap();
    assert!(run(&mut s,"apply_cable_part",json!({"cable":"-W1","part":"MISSING"})).unwrap_err().contains("No part"));
    assert!(run(&mut s,"apply_cable_part",json!({"cable":"-W1","part":"RELAY"})).unwrap_err().contains("not a cable"));
    assert!(run(&mut s,"apply_cable_part",json!({"cable":"-W99","part":"DECLARED"})).unwrap_err().contains("No cable"));
    assert_eq!(serde_json::to_value(s.project().unwrap()).unwrap(),before);
    run(&mut s,"apply_cable_part",json!({"cable":"-W1","part":"GEN.CBL-4G1.5"})).unwrap();
    assert_eq!(s.project().unwrap().cable("-W1").unwrap().diameter,0.);
}

#[test]
fn rules_paths_and_reservations() {
    let mut s = demo();
    let r = run(&mut s, "routing_rules", json!({ "grid": 20, "segregation": 150, "wires_3d": true })).unwrap();
    assert_eq!((r["grid"].as_f64(), r["segregation"].as_f64(), r["wires_3d"].as_bool()), (Some(20.0), Some(150.0), Some(true)));
    let p = run(&mut s, "add_routing_path", json!({ "enclosure": "+CAB1", "points": [[300, 50], [300, 700]], "kind": "cables", "classes": "signal, control", "width": 40 })).unwrap();
    assert_eq!((p["kind"].as_str(), p["points"][0][2].as_f64()), (Some("Cables"), Some(70.0)));
    assert!(run(&mut s, "add_routing_path", json!({ "enclosure": "+CAB1", "points": [[1, 1]] })).unwrap_err().contains("2 points"));
    assert!(run(&mut s, "add_routing_path", json!({ "enclosure": "+CAB1", "points": [[1, 1], [2, 2]], "classes": "mains" })).unwrap_err().contains("Unknown class"));
    let res = run(&mut s, "add_routing_reservation", json!({ "enclosure": "+CAB1", "x": 300, "y": 600, "w": 200, "h": 100, "mode": "power" })).unwrap();
    assert_eq!(res["mode"]["For"], "Power");
    let list = run(&mut s, "list_routing", json!({})).unwrap();
    assert!(list["paths"].as_array().unwrap().iter().any(|x| x["id"] == p["id"]));
    let id = res["id"].as_u64().unwrap();
    run(&mut s, "remove_routing_object", json!({ "enclosure": "+CAB1", "id": id })).unwrap();
    assert!(run(&mut s, "remove_routing_object", json!({ "enclosure": "+CAB1", "id": id })).unwrap_err().contains("No routing path"));
}

#[test]
fn routing_list_freezing_and_shields() {
    let mut s = demo();
    let list = run(&mut s, "list_routing", json!({ "kind": "cable" })).unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0]["key"].as_str(), items[0]["class"].as_str()), (Some("-W1"), Some("power")));
    assert!(items[0]["cut_mm"].as_f64().unwrap() >= items[0]["length_mm"].as_f64().unwrap());
    let all = run(&mut s, "list_routing", json!({})).unwrap();
    assert!(!all["bundles"].as_array().unwrap().is_empty());
    let f = run(&mut s, "freeze_routes", json!({ "keys": "-W1" })).unwrap();
    assert_eq!((f["frozen"].as_u64(), f["total"].as_u64()), (Some(1), Some(1)));
    assert!(run(&mut s, "freeze_routes", json!({ "keys": "-W99" })).unwrap_err().contains("No routed item"));
    let list = run(&mut s, "list_routing", json!({ "kind": "cable" })).unwrap();
    assert_eq!(list["items"][0]["status"], "frozen");
    let r = run(&mut s, "freeze_routes", json!({ "release": true })).unwrap();
    assert_eq!(r["released"], 1);
    let c = run(&mut s, "set_cable_shield", json!({ "cable": "-W1", "shielded": true, "ends": "first", "diameter": 11.5 })).unwrap();
    assert_eq!((c["shielded"].as_bool(), c["ends"].as_str(), c["diameter_mm"].as_f64()), (Some(true), Some("first"), Some(11.5)));
    let list = run(&mut s, "list_routing", json!({})).unwrap();
    let shield = &list["shields"][0];
    assert_eq!(shield["cable"], "-W1");
    assert_eq!(shield["ends"][1]["termination"], "not connected");
    assert!(run(&mut s, "set_cable_shield", json!({ "cable": "-W1", "ends": "middle" })).unwrap_err().contains("Unknown shield ends"));
}

#[test]
fn laying_and_installation_zones() {
    let mut s = demo();
    let r = run(&mut s, "routing_rules", json!({ "laying": "wall", "slot_depth": 40 })).unwrap();
    assert_eq!((r["laying"].as_str(), r["slot_depth"].as_f64(), r["floor_depth"].as_f64()), (Some("Wall"), Some(40.0), Some(60.0)));
    assert!(run(&mut s, "routing_rules", json!({ "laying": "air" })).unwrap_err().contains("air"));
    // One cable in the floor, back to the default.
    let l = run(&mut s, "set_laying", json!({ "item": "-W1", "laying": "floor" })).unwrap();
    assert_eq!((l["default"].as_str(), l["items"]["-W1"].as_str()), (Some("wall"), Some("floor")));
    let l = run(&mut s, "set_laying", json!({ "item": "-W1", "laying": "default" })).unwrap();
    assert_eq!(l["items"], json!({}));
    // The zones of DIN 18015-3; the middle one switched on, a zone of its own, reset.
    let z = run(&mut s, "installation_zones", json!({})).unwrap();
    let names: Vec<&str> = z.as_array().unwrap().iter().map(|z| z["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["ZW-o", "ZW-m", "ZW-u", "ZS-t", "ZS-f", "ZS-e", "ZD-r", "ZF"]);
    let z = run(&mut s, "installation_zones", json!({ "name": "zw-m", "on": true })).unwrap();
    assert_eq!(z[1]["on"], true);
    let z = run(&mut s, "installation_zones", json!({ "name": "ZX", "kind": "above_floor", "from": 600, "to": 800 })).unwrap();
    assert_eq!((z[8]["kind"].as_str(), z[8]["to"].as_f64()), (Some("above_floor"), Some(800.0)));
    assert!(run(&mut s, "installation_zones", json!({ "name": "ZX", "from": 900 })).unwrap_err().contains("ZX"));
    let z = run(&mut s, "installation_zones", json!({ "reset": true })).unwrap();
    assert_eq!((z.as_array().unwrap().len(), z[1]["on"].as_bool()), (8, Some(false)));
}

#[test]
fn failed_search_reuse_is_optional_preserved_and_can_be_disabled() {
    let mut s=demo();assert!(!s.project().unwrap().layout.routing.reuse_failed_searches);
    let on=run(&mut s,"routing_rules",json!({"reuse_failed_searches":true})).unwrap();
    assert_eq!(on["reuse_failed_searches"],true);
    run(&mut s,"routing_rules",json!({"grid":20})).unwrap();
    assert!(s.project().unwrap().layout.routing.reuse_failed_searches);
    run(&mut s,"routing_rules",json!({"reuse_failed_searches":false})).unwrap();
    assert!(!s.project().unwrap().layout.routing.reuse_failed_searches);
}
