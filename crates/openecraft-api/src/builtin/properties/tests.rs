use super::*;

#[test]
fn live_function_references_are_read_only_and_queryable_through_the_registry() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({"page":"1","symbol":"coil","tag":"-K1","x":40,"y":40})).unwrap();
    s.project_mut().unwrap().pages.push(openecraft_core::model::Page {name:"2".into(),..Default::default()});
    let aux = run(&mut s, "place_symbol", json!({"page":"2","symbol":"no","tag":"-K1","x":80,"y":40})).unwrap();
    let id = aux["id"].as_u64().unwrap();
    let before = run(&mut s, "list_cross_references", json!({"id":id})).unwrap();
    assert_eq!(before[0]["references"][0]["page"], "1");
    run(&mut s, "set_page", json!({"page":"1","name":"37a"})).unwrap();
    let after = run(&mut s, "list_cross_references", json!({"id":id,"page":"2"})).unwrap();
    assert_eq!(after[0]["references"][0]["page"], "37a");
    assert_ne!(before[0]["text"],after[0]["text"]);
    let properties = run(&mut s,"get_properties",json!({"object":format!("function:{id}"),"all":true})).unwrap();
    let row = properties["properties"].as_array().unwrap().iter().find(|r|r["key"]=="function.cross_references").unwrap();
    assert_eq!(row["display"],after[0]["text"]);
    assert!(run(&mut s,"set_properties",json!({"object":format!("function:{id}"),"values":{"function.cross_references":"old"}})).is_err());
}

#[test]
fn live_sheet_frame_fields_are_available_through_the_shared_property_registry() {
    let mut s=new_session();
    run(&mut s,"set_page",json!({"page":"1","name":"16","function":"Site","location":"CC1"})).unwrap();
    let output=run(&mut s,"get_properties",json!({"object":"page:16","all":true})).unwrap();
    let rows=output["properties"].as_array().unwrap();
    for (key,value) in [("page.local_name","16"),("page.section_last_number","16"),
        ("page.function_designation","=Site"),("page.location_designation","+CC1")] {
        assert_eq!(rows.iter().find(|r|r["key"]==key).unwrap()["display"],value,"{key}");
    }
    assert!(run(&mut s,"set_properties",json!({"object":"page:16","values":{"page.section_last_number":"99"}})).is_err());
}
use crate::builtin::testing::*;

fn motor_session() -> (Session, u64) {
    let mut s = new_session();
    let m = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "motor3", "x": 100, "y": 100 })).unwrap();
    (s, m["id"].as_u64().unwrap())
}

#[test]
fn placing_another_part_on_a_device_binds_the_new_function_to_that_part() {
    use openecraft_core::parts::Part;
    let mut s = new_session();
    s.project_mut().unwrap().parts.extend([
        Part { number: "FIRST".into(), symbol: "coil".into(), ..Default::default() },
        Part { number: "SECOND".into(), symbol: "coil".into(), manufacturer: "Second maker".into(), ..Default::default() },
    ]);
    run(&mut s, "place_part", json!({"page":"1","part":"FIRST","tag":"-K1","x":40,"y":40})).unwrap();
    let second = run(&mut s, "place_part", json!({"page":"1","part":"SECOND","tag":"-K1","x":80,"y":40})).unwrap();
    let functions = run(&mut s, "list_function_parts", json!({})).unwrap();
    let function = functions.as_array().unwrap().iter().find(|f| f["symbol"] == second["id"]).unwrap();
    assert_eq!(function["part_number"], "SECOND");
    assert_eq!(function["manufacturer"], "Second maker");
}

#[test]
fn selected_function_parts_are_editable_and_batch_read_through_the_plugin_registry() {
    use openecraft_core::model::{PartRef, DeviceData};
    use openecraft_core::parts::Part;
    let (mut s, id) = motor_session();
    let prj = s.project_mut().unwrap();
    prj.parts.extend([Part { number: "FIRST".into(), ..Default::default() }, Part { number: "SECOND".into(), manufacturer: "Maker".into(), ..Default::default() }]);
    prj.devices.insert("-M1".into(), DeviceData { parts: vec![PartRef { number: "FIRST".into(), count: 1, total:false }, PartRef { number: "SECOND".into(), count: 1, total:false }], ..Default::default() });
    run(&mut s, "set_properties", json!({"object":format!("function:{id}"),"values":{"function.part_index":2,"function.selection_text":"Source variant*","function.supplementary[3]":"Additional note"}})).unwrap();
    let list = run(&mut s, "list_function_parts", json!({})).unwrap();
    let function = list.as_array().unwrap().iter().find(|v| v["symbol"] == id).unwrap();
    assert_eq!(function["part_number"], "SECOND");
    assert_eq!(function["manufacturer"], "Maker");
    assert_eq!(function["selection_text"], "Source variant*");
    assert_eq!(function["supplementary"][2], "Additional note");
    assert!(run(&mut s, "set_properties", json!({"object":format!("function:{id}"),"values":{"function.part_index":3}})).is_err());
}

#[test]
fn definitions_are_listed_and_defined() {
    let (mut s, _) = motor_session();
    let all = run(&mut s, "list_property_definitions", json!({})).unwrap();
    assert!(all.as_array().unwrap().len() > 300);
    let motor = run(&mut s, "list_property_definitions", json!({ "class": "device", "filter": "speed" })).unwrap();
    assert_eq!(motor[0]["key"], "device.speed");
    assert_eq!(motor[0]["unit"], "rpm");
    assert!(run(&mut s, "list_property_definitions", json!({ "class": "planet" })).is_err());

    let d = run(&mut s, "define_property", json!({ "key": "user.zone", "name": "Zone", "class": "device", "type": "choice", "choices": "A, B" })).unwrap();
    assert_eq!(d["choices"], json!(["A", "B"]));
    assert!(run(&mut s, "define_property", json!({ "key": "device.note", "name": "x", "class": "device" })).is_err());
    assert!(run(&mut s, "define_property", json!({ "key": "user.x", "name": "x", "class": "device", "type": "blob" })).is_err());
    run(&mut s, "set_properties", json!({ "object": "device:-M1", "values": { "user.zone": "b" } })).unwrap();
    let r = run(&mut s, "remove_property_definition", json!({ "key": "user.zone" })).unwrap();
    assert_eq!(r["values_removed"], 1);
}

#[test]
fn values_are_set_and_read() {
    let (mut s, id) = motor_session();
    let out = run(&mut s, "set_properties", json!({ "object": "device:-M1", "values": { "device.rated_power": 5.5, "device.description": "Pump", "device.sto": true } })).unwrap();
    assert_eq!(out["values"]["device.rated_power"], "5.5");
    assert_eq!(out["values"]["device.sto"], "true");
    assert!(run(&mut s, "set_properties", json!({ "object": "device:-M1", "values": { "device.poles": "many" } })).is_err());
    assert!(run(&mut s, "set_properties", json!({ "object": "device:-M9", "values": {} })).is_err());

    let v = run(&mut s, "get_properties", json!({ "object": "device:-M1" })).unwrap();
    assert_eq!(v["function_definition"], "fn.motor3");
    let props = v["properties"].as_array().unwrap();
    let power = props.iter().find(|p| p["key"] == "device.rated_power").unwrap();
    assert_eq!((power["display"].as_str(), power["origin"].as_str()), (Some("5.5 kW"), Some("set")));
    assert!(props.iter().any(|p| p["key"] == "device.description" && p["origin"] == "model"));
    assert!(!props.iter().any(|p| p["key"] == "device.speed"));
    let all = run(&mut s, "get_properties", json!({ "object": "device:-M1", "all": true, "filter": "rated speed" })).unwrap();
    assert_eq!(all["properties"].as_array().unwrap().len(), 1);

    run(&mut s, "set_properties", json!({ "object": format!("function:{id}"), "values": { "function.note": "check" } })).unwrap();
    run(&mut s, "set_properties", json!({ "object": "page:1", "values": { "page.drawing_number": "D-1" } })).unwrap();
    run(&mut s, "set_properties", json!({ "object": "project", "values": { "project.customer": "ACME" } })).unwrap();
    assert_eq!(s.project().unwrap().info.properties["project.customer"], "ACME");
}

#[test]
fn function_definitions_are_defined_and_assigned() {
    let (mut s, id) = motor_session();
    let list = run(&mut s, "list_function_definitions", json!({ "filter": "motor" })).unwrap();
    let m3 = list.as_array().unwrap().iter().find(|d| d["key"] == "fn.motor3").unwrap();
    assert_eq!(m3["used"], 1);
    let d = run(
        &mut s,
        "define_function",
        json!({ "key": "user.pump_motor", "from": "fn.motor3", "name": "Pump motor", "scheme": "Motor data", "defaults": { "device.rated_power": "7,5", "device.efficiency_class": "IE3" } }),
    )
    .unwrap();
    assert_eq!(d["defaults"]["device.rated_power"], "7.5");
    assert_eq!(d["letter"], "M");
    assert!(run(&mut s, "define_function", json!({ "key": "user.x", "scheme": "Nope" })).is_err());
    assert!(run(&mut s, "define_function", json!({ "key": "user.x", "defaults": { "device.ip": "IP99" } })).is_err());

    let a = run(&mut s, "assign_function_definition", json!({ "id": id, "function": "user.pump_motor" })).unwrap();
    assert_eq!((a["function"].as_str(), a["scheme"].as_str()), (Some("user.pump_motor"), Some("Motor data")));
    let v = run(&mut s, "get_properties", json!({ "object": "device:-M1", "filter": "efficiency class" })).unwrap();
    assert_eq!(v["properties"][0]["origin"], "function_definition");
    let r = run(&mut s, "remove_function_definition", json!({ "key": "user.pump_motor" })).unwrap();
    assert_eq!(r["symbols_reset"], 1);
    assert!(run(&mut s, "remove_function_definition", json!({ "key": "fn.motor3" })).is_err());
}

#[test]
fn properties_are_placed_and_schemes_reused() {
    let (mut s, id) = motor_session();
    run(&mut s, "set_properties", json!({ "object": "device:-M1", "values": { "device.rated_power": "4", "device.speed": "1450" } })).unwrap();
    let p = run(&mut s, "place_property", json!({ "id": id, "key": "device.rated_power", "name": true })).unwrap();
    assert_eq!(p["text"], "Rated power: 4 kW");
    run(&mut s, "place_property", json!({ "id": id, "key": "device.supplementary[3]" })).unwrap();
    assert!(run(&mut s, "place_property", json!({ "id": id, "key": "device.supplementary[300]" })).is_err());
    let sc = run(&mut s, "save_property_scheme", json!({ "name": "Motor plate", "id": id })).unwrap();
    assert_eq!(sc["entries"].as_array().unwrap().len(), 2);

    let m2 = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "motor3", "x": 200, "y": 100 })).unwrap()["id"].as_u64().unwrap();
    let r = run(&mut s, "apply_property_scheme", json!({ "name": "Motor plate", "ids": [m2] })).unwrap();
    assert_eq!(r["symbols"], 1);
    let r = run(&mut s, "apply_property_scheme", json!({ "name": "Motor plate" })).unwrap();
    assert_eq!(r["symbols"], 2);
    let list = run(&mut s, "list_property_schemes", json!({})).unwrap();
    let mine = list.as_array().unwrap().iter().find(|x| x["name"] == "Motor plate").unwrap();
    assert_eq!(mine["used"], 2);
    assert!(run(&mut s, "apply_property_scheme", json!({ "name": "Nope" })).is_err());
}
