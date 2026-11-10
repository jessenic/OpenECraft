use super::*;
use crate::builtin::testing::*;

#[test]
fn open_routing_is_explicit_and_round_trips_through_enclosure_commands() {
    let mut s=demo();
    let e=run(&mut s,"add_enclosure",json!({"name":"Open board","assembly":true})).unwrap();
    assert_eq!(e["open_routing"],false);
    let e=run(&mut s,"set_enclosure",json!({"enclosure":"Open board","open_routing":true})).unwrap();
    assert_eq!(e["open_routing"],true);
    let saved=s.project().unwrap().to_json();
    let restored:Project=serde_json::from_str(&saved).unwrap();
    assert!(restored.layout.enclosures.last().unwrap().open_routing);
    let e=run(&mut s,"set_enclosure",json!({"enclosure":"Open board","open_routing":false})).unwrap();
    assert_eq!(e["open_routing"],false);
}

#[test]
fn panel_rotation_is_queryable_editable_and_validated_atomically_through_registry() {
    let mut s=demo();
    run(&mut s,"add_enclosure",json!({"name":"Rotations"})).unwrap();
    let created=run(&mut s,"place_mounting_plate",json!({"enclosure":"Rotations","width":20,"height":40})).unwrap();
    let id=created["id"].clone();
    let set=run(&mut s,"set_panel_item",json!({"enclosure":"Rotations","id":id,"rotation":-90})).unwrap();
    assert_eq!(set["rotation"],270);assert_eq!(set["w"],20.);assert_eq!(set["h"],40.);
    let before=s.project().unwrap().to_json();
    assert!(run(&mut s,"set_panel_item",json!({"enclosure":"Rotations","id":id,"x":900,"rotation":45})).is_err());
    assert_eq!(s.project().unwrap().to_json(),before);
    assert!(run(&mut s,"set_panel_item",json!({"enclosure":"Rotations","id":id,"rotation":0,"infer_rotation":true})).is_err());
    assert_eq!(s.project().unwrap().to_json(),before);
    let rotated=run(&mut s,"arrange_panel_items",json!({"enclosure":"Rotations","ids":[id],"action":"rotate"})).unwrap();
    assert_eq!(rotated["items"][0]["rotation"],0);
    assert_eq!(rotated["items"][0]["w"],40.);
    let inferred=run(&mut s,"set_panel_item",json!({"enclosure":"Rotations","id":id,"infer_rotation":true})).unwrap();
    assert!(inferred["rotation"].is_null());
}

#[test]
fn material_ownership_and_exclusion_are_editable_without_changing_mounts() {
    let mut s=demo();
    let e=run(&mut s,"add_enclosure",json!({"name":"Owned","owner_device":"-X1","bom_excluded":true})).unwrap();
    assert_eq!(e["owner_device"],"-X1");assert_eq!(e["bom_excluded"],true);
    let item=run(&mut s,"place_mounting_plate",json!({"enclosure":"Owned","width":100,"height":100,"thickness":2})).unwrap();
    let changed=run(&mut s,"set_panel_item",json!({"enclosure":"Owned","id":item["id"],"owner_device":"-X1","bom_excluded":true})).unwrap();
    assert_eq!(changed["owner_device"],"-X1");assert_eq!(changed["bom_excluded"],true);assert_eq!(changed["mounted_on"],0);
    let e=run(&mut s,"set_enclosure",json!({"enclosure":"Owned","owner_device":"","bom_excluded":false})).unwrap();
    assert_eq!(e["owner_device"],"");assert_eq!(e["bom_excluded"],false);
}

#[test]
fn physical_article_changes_update_primary_assignments_and_keep_literal_spares() {
    let mut s=demo();let prj=s.project_mut().unwrap();
    let old=prj.devices["-M1"].parts[0].number.clone();
    let mut replacement=prj.parts.iter().find(|p|p.number==old).unwrap().clone();replacement.number="REPLACEMENT".into();prj.parts.push(replacement.clone());
    replacement.number="NEXT.PART".into();prj.parts.push(replacement);
    prj.devices.get_mut("-M1").unwrap().parts.push(openecraft_core::parts::PartRef {number:old.clone(),count:2,total:true});
    for symbol in prj.pages.iter_mut().flat_map(|p|&mut p.symbols).filter(|s|s.tag=="-M1") {symbol.values.insert("function.part_number".into(),old.clone());}
    prj.layout.enclosures.push(Enclosure {name:"Bound".into(),part:old.clone(),device_tag:"-M1".into(),..Default::default()});
    run(&mut s,"set_enclosure",json!({"enclosure":"Bound","part":"REPLACEMENT"})).unwrap();
    let prj=s.project().unwrap();assert_eq!(prj.devices["-M1"].parts[0].number,"REPLACEMENT");
    assert_eq!(prj.devices["-M1"].parts.last().unwrap().number,old);
    assert!(prj.pages.iter().flat_map(|p|&p.symbols).filter(|s|s.tag=="-M1").all(|s|s.values["function.part_number"]=="REPLACEMENT"));
    let prj=s.project_mut().unwrap();let id=prj.new_id();
    prj.layout.enclosures.last_mut().unwrap().items.push(PanelItem {id,kind:panel::ItemKind::MountingPlate,tag:"-M1".into(),part:"REPLACEMENT".into(),w:100.,h:200.,d:2.,..Default::default()});
    run(&mut s,"set_panel_item",json!({"enclosure":"Bound","id":id,"part":"NEXT.PART"})).unwrap();
    assert_eq!(s.project().unwrap().devices["-M1"].parts[0].number,"NEXT.PART");
    assert_eq!(s.project().unwrap().devices["-M1"].parts.last().unwrap().number,old);
}

#[test]
fn explicit_physical_frames_and_unnumbered_parts_round_trip_through_the_api() {
    let mut s=demo();let part=s.project().unwrap().layout.find_device("-X1").unwrap().1.part.clone();
    let e=run(&mut s,"add_enclosure",json!({"name":"Source body","implicit_plate":false,"mounting_origin":{"x":0,"y":0,"z":0}})).unwrap();
    assert_eq!(e["mounting_frame"][0],json!([0.,0.,0.]));
    assert!(!e["components"].as_array().unwrap().iter().any(|c|c["role"]=="Mounting plate"));
    let item=run(&mut s,"place_panel_device",json!({"enclosure":"Source body","tag":"-X1","part":part,"single_part":true,"width":8,"height":50,"depth":30})).unwrap();
    assert_eq!(item["single_part"],true);assert_eq!(item["terminal"],"");
    let points=run(&mut s,"get_panel_points",json!({"id":item["id"]})).unwrap();
    assert_eq!(points[0]["points"].as_array().unwrap().len(),2);
    assert!(run(&mut s,"set_enclosure",json!({"enclosure":"Source body","mounting_origin":{"x":1}})).is_err());
    let e=run(&mut s,"set_enclosure",json!({"enclosure":"Source body","mounting_origin":{},"implicit_plate":true})).unwrap();
    assert!(e["mounting_origin"].is_null());assert_eq!(e["implicit_plate"],true);
}

#[test]
fn individual_terminals_are_created_queried_and_edited_through_the_registry() {
    let mut s=demo();
    let part=s.project().unwrap().layout.find_device("-X1").unwrap().1.part.clone();
    run(&mut s,"add_enclosure",json!({"name":"Terminals","assembly":true})).unwrap();
    let item=run(&mut s,"place_panel_device",json!({"enclosure":"Terminals","tag":"-X1","terminal":"1","part":part,
        "width":8,"height":50,"depth":30,"x":120,"y":200})).unwrap();
    assert_eq!(item["terminal"],"1");let id=item["id"].as_u64().unwrap();
    let points=run(&mut s,"get_panel_points",json!({"enclosure":"Terminals","id":id})).unwrap();
    assert_eq!(points.as_array().unwrap().len(),1);
    let points=points[0]["points"].as_array().unwrap();assert!(!points.is_empty());
    assert!(points.iter().all(|p|p["terminal"]=="1"));
    let item=run(&mut s,"set_panel_item",json!({"enclosure":"Terminals","id":id,"terminal":"2"})).unwrap();
    assert_eq!(item["terminal"],"2");assert_eq!(item["x"],120.);
}

fn demo() -> Session {
    let mut s = demo_session();
    s.project_mut().unwrap().info.name = "Demo".into();
    s
}

#[test]
fn open_assemblies_expose_explicit_plates_and_move_their_mounted_items() {
    let mut s=with_project(Project::default());
    let e=run(&mut s,"add_enclosure",json!({"name":"Open","assembly":true,"width":800,"height":600,"depth":300})).unwrap();
    assert_eq!(e["kind"],"assembly");assert!(e["components"].as_array().unwrap().is_empty());
    let plate=run(&mut s,"place_mounting_plate",json!({"enclosure":"Open","width":750,"height":550,"thickness":2})).unwrap()["id"].as_u64().unwrap();
    let child=run(&mut s,"place_mounting_plate",json!({"enclosure":"Open","width":100,"height":50,"x":20,"y":30,"z":2})).unwrap()["id"].as_u64().unwrap();
    run(&mut s,"set_panel_item",json!({"enclosure":"Open","id":child,"mounted_on":plate})).unwrap();
    run(&mut s,"set_panel_item",json!({"enclosure":"Open","id":plate,"x":40,"depth":5})).unwrap();
    let project=s.project().unwrap();let item=project.layout.enclosures[0].item(child).unwrap();
    assert_eq!((item.x,item.z,item.mounted_on),(60.,5.,plate));
    assert!(run(&mut s,"set_panel_item",json!({"enclosure":"Open","id":plate,"mounted_on":child})).is_err());
    run(&mut s,"set_panel_item",json!({"enclosure":"Open","id":child,"mounted_on":0})).unwrap();
    run(&mut s,"set_panel_item",json!({"enclosure":"Open","id":plate,"x":80})).unwrap();
    assert_eq!(s.project().unwrap().layout.enclosures[0].item(child).unwrap().x,60.);
    let e=run(&mut s,"set_enclosure",json!({"enclosure":"Open","assembly":false})).unwrap();
    assert_eq!(e["kind"],"cabinet");
    assert!(!e["components"].as_array().unwrap().iter().any(|c|c["role"]=="Mounting plate"));
}

#[test]
fn enclosures_bayed_with_a_cable_passage() {
    let mut s = demo();
    run(&mut s, "add_enclosure", json!({ "name": "+CAB2" })).unwrap();
    assert!(run(&mut s, "add_cable_passage", json!({ "enclosure": "+CAB1" })).unwrap_err().contains("no enclosure bayed"));
    let r = run(&mut s, "bay_enclosure", json!({ "enclosure": "+CAB2", "right_of": "+CAB1" })).unwrap();
    assert_eq!(r["lineup"], json!(["+CAB1", "+CAB2"]));
    assert!(run(&mut s, "bay_enclosure", json!({ "enclosure": "+CAB2", "right_of": "+CAB2" })).unwrap_err().contains("itself"));
    let p = run(&mut s, "add_cable_passage", json!({ "enclosure": "+CAB1", "y": 200, "diameter": 50 })).unwrap();
    assert_eq!((p["cutouts"].as_array().unwrap().len(), p["to"].as_str()), (2, Some("+CAB2")));
    let e = &run(&mut s, "list_enclosures", json!({ "enclosure": "+CAB1" })).unwrap()[0];
    assert_eq!(e["lineup"]["left_of"], "+CAB2");
    let passages = e["lineup"]["passages_right"].as_array().unwrap();
    assert_eq!(passages.len(), 1);
    assert!((passages[0]["y"].as_f64().unwrap() - 200.0).abs() < 0.01);
    let e = &run(&mut s, "list_enclosures", json!({ "enclosure": "+CAB2" })).unwrap()[0];
    assert_eq!((e["lineup"]["right_of"].as_str(), e["lineup"]["position"][0].as_f64()), (Some("+CAB1"), Some(600.0)));
    let r = run(&mut s, "bay_enclosure", json!({ "enclosure": "+CAB2" })).unwrap();
    assert_eq!(r["lineup"], json!(["+CAB2"]));
}

#[test]
fn accessories_items_and_arranging() {
    let mut s = demo();
    let g = run(&mut s, "place_accessory", json!({ "enclosure": "+CAB1", "part": "GEN.GLAND-M25", "face": "bottom", "anchor_x": "center:0", "y": 60 })).unwrap();
    assert_eq!((g["face"].as_str(), g["fastening"].as_str()), (Some("bottom"), Some("clip")));
    assert_eq!(g["x"], (600.0 - 36.0) / 2.0);
    assert!(run(&mut s, "place_accessory", json!({ "enclosure": "+CAB1", "part": "GEN.HANDLE-SWING", "face": "roof" })).unwrap_err().contains("cannot be mounted"));
    assert!(run(&mut s, "place_accessory", json!({ "enclosure": "+CAB1", "part": "GEN.GLAND-M25", "face": "lid" })).unwrap_err().contains("Unknown face"));
    let id = g["id"].as_u64().unwrap();
    let r = run(&mut s, "arrange_panel_items", json!({ "enclosure": "+CAB1", "ids": [id], "action": "pattern", "columns": 4, "pitch_x": 50 })).unwrap();
    assert_eq!(r["created"].as_array().unwrap().len(), 3);
    let m = run(&mut s, "set_panel_item", json!({ "enclosure": "+CAB1", "id": id, "fastening": "rivet_nut", "anchor_y": "end:40" })).unwrap();
    assert_eq!((m["fastening"].as_str(), m["anchor"][1].as_str()), (Some("rivet_nut"), Some("end:40")));
    assert!(run(&mut s, "arrange_panel_items", json!({ "enclosure": "+CAB1", "ids": [id], "action": "spin" })).unwrap_err().contains("Unknown action"));
}

#[test]
fn assemblies_and_macros() {
    let mut s = demo();
    let list = run(&mut s, "list_panel_macros", json!({})).unwrap();
    assert!(list.as_array().unwrap().iter().any(|m| m["name"] == "Filter fan with outlet filter" && m["faces"] == json!(["left", "right"])));
    let r = run(&mut s, "insert_panel_macro", json!({ "enclosure": "+CAB1", "macro": "Cable entry 5 x M25", "x": 100, "y": 60 })).unwrap();
    let ids: Vec<u64> = r["items"].as_array().unwrap().iter().map(|i| i["id"].as_u64().unwrap()).collect();
    assert_eq!(ids.len(), 5);
    let e = &run(&mut s, "list_enclosures", json!({ "enclosure": "+CAB1" })).unwrap()[0];
    assert_eq!(e["assemblies"][0]["items"].as_array().unwrap().len(), 5);
    run(&mut s, "save_panel_macro", json!({ "enclosure": "+CAB1", "ids": &ids[..2], "name": "Two glands" })).unwrap();
    let list = run(&mut s, "list_panel_macros", json!({})).unwrap();
    assert_eq!(list[0]["name"], "Two glands");
    assert_eq!(list[0]["built_in"], false);
    let g = run(&mut s, "group_panel_items", json!({ "enclosure": "+CAB1", "ids": ids })).unwrap();
    assert_eq!(g["group"], 0);
    let e = &run(&mut s, "list_enclosures", json!({ "enclosure": "+CAB1" })).unwrap()[0];
    assert!(e["assemblies"].as_array().unwrap().is_empty());
}
