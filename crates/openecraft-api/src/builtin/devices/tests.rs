use super::*;
use crate::builtin::testing::*;
use openecraft_core::model::{Cable,SymbolInst};

#[test]
fn renaming_a_selected_cable_marker_does_not_rename_a_device_sharing_its_tag() {
    let mut s=new_session();let prj=s.project_mut().unwrap();
    prj.cables.push(Cable {id:10,tag:"-W1".into(),..Default::default()});
    prj.devices.entry("-W1".into()).or_default();
    prj.pages[0].symbols.push(SymbolInst {id:11,sym:"coil".into(),tag:"-W1".into(),values:[("function.cable".into(),"-W1".into())].into(),..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {id:12,sym:"coil".into(),tag:"-W1".into(),..Default::default()});
    let renamed=run(&mut s,"rename_symbol_device",json!({"id":11,"new_tag":"-W2"})).unwrap();
    assert_eq!(renamed["changed"],true);let prj=s.project().unwrap();
    assert_eq!(prj.cables[0].tag,"-W2");assert_eq!(prj.pages[0].symbols[0].tag,"-W2");
    assert_eq!(prj.pages[0].symbols[1].tag,"-W1");assert!(prj.devices.contains_key("-W1"));
    let before=prj.to_json();assert!(run(&mut s,"rename_symbol_device",json!({"id":11,"new_tag":"bad tag"})).is_err());
    assert_eq!(s.project().unwrap().to_json(),before);
    run(&mut s,"rename_device",json!({"tag":"-W2","new_tag":"-W3"})).unwrap();
    assert_eq!(s.project().unwrap().cables[0].tag,"-W3");
}

#[test]
fn physical_devices_without_a_schematic_or_extras_remain_listed() {
    let mut s=demo_session();let prj=s.project_mut().unwrap();
    prj.devices.entry("-PhysicalOnly".into()).or_default().parts.push(parts::PartRef {number:"PRIMARY".into(),count:1,total:false});
    prj.layout.enclosures[0].items.push(openecraft_core::panel::PanelItem {id:999,tag:"-PhysicalOnly".into(),..Default::default()});
    let devices=run(&mut s,"list_devices",json!({})).unwrap();
    assert_eq!(devices.as_array().unwrap().iter().filter(|d|d["tag"]=="-PhysicalOnly").count(),1);
}

#[test]
fn material_owners_follow_rename_and_suppressed_unplaced_devices_remain_accessible() {
    let mut s=demo_session();
    let prj=s.project_mut().unwrap();
    let child=prj.devices.entry("-Owned".into()).or_default();
    child.values.insert("device.material_owner".into(),"-X1".into());
    child.values.insert("device.exclude_from_bom".into(),"true".into());
    prj.layout.enclosures[0].owner_device="-X1".into();prj.layout.enclosures[0].items[0].owner_device="-X1".into();
    run(&mut s,"rename_device",json!({"tag":"-X1","new_tag":"-X9"})).unwrap();
    let devices=run(&mut s,"list_devices",json!({})).unwrap();
    let child=devices.as_array().unwrap().iter().find(|d|d["tag"]=="-Owned").unwrap();
    assert_eq!(child["owner_device"],"-X9");assert_eq!(child["bom_excluded"],true);
    assert_eq!(s.project().unwrap().layout.enclosures[0].owner_device,"-X9");
    assert_eq!(s.project().unwrap().layout.enclosures[0].items[0].owner_device,"-X9");
    run(&mut s,"set_properties",json!({"object":"device:-Owned","values":{"device.material_owner":"-K1","device.exclude_from_bom":false}})).unwrap();
    let prj=s.project().unwrap();let reopened=Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(reopened.devices["-Owned"].values["device.material_owner"],"-K1");
    assert_eq!(reopened.devices["-Owned"].values["device.exclude_from_bom"],"false");
}

#[test]
fn material_only_devices_and_body_bindings_survive_renaming_and_reserve_their_tags() {
    let mut s=demo_session();let number=s.project().unwrap().devices["-X1"].parts[0].number.clone();
    run(&mut s,"add_device_part",json!({"tag":"-U1","part":number,"count":3,"total":true})).unwrap();
    s.project_mut().unwrap().layout.enclosures.push(openecraft_core::panel::Enclosure {name:"Body".into(),device_tag:"-U1".into(),..Default::default()});
    run(&mut s,"rename_device",json!({"tag":"-U1","new_tag":"-U2"})).unwrap();
    assert_eq!(s.project().unwrap().layout.enclosures.last().unwrap().device_tag,"-U2");
    assert!(s.project().unwrap().devices["-U2"].parts[0].total);
    assert!(run(&mut s,"rename_device",json!({"tag":"-K1","new_tag":"-U2"})).is_err());
    assert_eq!(s.project().unwrap().next_tag("","U"),"-U3");
}

#[test]
fn literal_part_assignments_are_editable_and_keep_distinct_primary_and_extra_counts() {
    let mut s=demo_session();
    let number=s.project().unwrap().devices["-X1"].parts[0].number.clone();
    let added=run(&mut s,"add_device_part",json!({"tag":"-X1","part":number,"count":2,"total":true})).unwrap();
    let index=added["index"].as_u64().unwrap();assert_eq!(added["part"]["total"],true);
    assert!(index>1);assert!(!s.project().unwrap().devices["-X1"].parts[0].total);
    run(&mut s,"set_device_part_quantity",json!({"tag":"-X1","index":index,"count":0})).unwrap();
    assert_eq!(s.project().unwrap().devices["-X1"].parts[index as usize-1].quantity(),0);
    assert!(run(&mut s,"set_device_part_quantity",json!({"tag":"-X1","index":index,"total":false})).is_err());
    assert!(s.project().unwrap().devices["-X1"].parts[index as usize-1].total);
    let changed=run(&mut s,"set_device_part_quantity",json!({"tag":"-X1","index":index,"count":4,"total":false})).unwrap();
    assert_eq!(changed["part"]["count"],4);assert!(!s.project().unwrap().devices["-X1"].parts[index as usize-1].total);
    let added=run(&mut s,"add_device_part",json!({"tag":"-PhysicalOnly","part":number,"count":3,"total":true})).unwrap();
    assert_eq!(added["index"],1);
    let devices=run(&mut s,"list_devices",json!({})).unwrap();
    assert!(devices.as_array().unwrap().iter().any(|d|d["tag"]=="-PhysicalOnly" && d["parts"][0]["count"]==3));
}

#[test]
fn invalid_literal_counts_and_assignment_indices_do_not_change_the_selected_slot() {
    let mut s=demo_session();let number=s.project().unwrap().devices["-X1"].parts[0].number.clone();
    assert!(run(&mut s,"add_device_part",json!({"tag":"-X1","part":number,"count":0})).is_err());
    assert!(run(&mut s,"add_device_part",json!({"tag":"","part":number,"count":1})).is_err());
    assert!(run(&mut s,"set_device_part_quantity",json!({"tag":"-X1","index":100,"count":2})).is_err());
    run(&mut s,"add_device_part",json!({"tag":"-Maximum","part":number,"count":u32::MAX,"total":true})).unwrap();
    assert!(run(&mut s,"add_device_part",json!({"tag":"-Maximum","part":number,"count":1,"total":true})).is_err());
    assert_eq!(s.project().unwrap().devices["-Maximum"].parts[0].count,u32::MAX);
}
