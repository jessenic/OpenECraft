use super::*;

#[test]
fn explicitly_bound_markers_follow_current_cable_material_without_cached_part_selection() {
    let mut prj=Project::default();
    prj.devices.insert("-Shared".into(),DeviceData {parts:vec![PartRef {number:"MOTOR".into(),count:1,total:false}],..Default::default()});
    prj.cables.push(Cable {tag:"-Shared".into(),part:"CABLE".into(),..Default::default()});
    let mut marker=SymbolInst {tag:"Custom caption".into(),values:BTreeMap::from([
        ("function.cable".into(),"-Shared".into()),("function.part_number".into(),"OLD CACHE".into())]),..Default::default()};
    let device=SymbolInst {tag:"-Shared".into(),..Default::default()};
    assert_eq!(marker.selected_part_number(&prj),Some("CABLE"));assert_eq!(marker.assigned_part_number(&prj,1),Some("CABLE"));
    assert_eq!(marker.assigned_part_number(&prj,2),None);assert_eq!(device.selected_part_number(&prj),Some("MOTOR"));
    prj.cables[0].part="EDITED".into();assert_eq!(marker.selected_part_number(&prj),Some("EDITED"));
    assert_eq!(marker.values["function.part_number"],"OLD CACHE");assert_eq!(device.selected_part_number(&prj),Some("MOTOR"));
    prj.cables[0].part.clear();assert_eq!(marker.selected_part_number(&prj),None);
    marker.values.insert("function.cable".into(),"-Missing".into());marker.tag="-Shared".into();
    assert_eq!(marker.selected_part_number(&prj),None);assert_eq!(marker.assigned_part_number(&prj,1),None);
}

#[test]
fn explicit_function_articles_resolve_shared_device_and_cable_tags_without_bom_duplicates() {
    let mut prj=Project::default();
    prj.devices.insert("-Shared".into(),DeviceData {parts:vec![PartRef {number:"MOTOR".into(),count:1,total:false}],..Default::default()});
    prj.cables.push(Cable {tag:"-Shared".into(),part:"CABLE".into(),..Default::default()});
    let mut symbol=SymbolInst {tag:"-Shared".into(),..Default::default()};
    assert_eq!(symbol.selected_part_number(&prj),Some("MOTOR"));
    symbol.values.insert("function.part_number".into(),"CABLE".into());
    assert_eq!(symbol.selected_part_number(&prj),Some("CABLE"));
    symbol.values.insert("function.part_number".into(),"MOTOR".into());
    assert_eq!(symbol.selected_part_number(&prj),Some("MOTOR"));
    symbol.values.insert("function.part_number".into(),"MISSING".into());
    assert_eq!(symbol.selected_part_number(&prj),None);
    assert_eq!(prj.devices["-Shared"].parts.len(),1);
}

#[test]
fn files_without_structure_cables_and_numbers_still_load() {
    let old = r#"{"pages":[{"id":1,"name":"1","kind":"Schematic","wires":[{"id":2,"a":{"x":0,"y":0},"b":{"x":0,"y":8},"color":"","cross_section":""}]}],"next_id":3}"#;
    let prj: Project = serde_json::from_str(old).unwrap();
    assert!(prj.cables.is_empty());
    assert_eq!(prj.wire_numbering, WireNumbering::default());
    assert!(prj.pages[0].function.is_empty() && prj.pages[0].wires[0].number.is_empty());
    // Empty new wire fields are not written.
    let json = serde_json::to_string(&prj.pages[0].wires[0]).unwrap();
    assert!(!json.contains("number") && !json.contains("cable"), "{json}");
}

#[test]
fn next_tag_counts_per_structure_and_cables() {
    let mut prj = Project::default();
    prj.cables.push(Cable { tag: "=A1-W1".into(), ..Default::default() });
    assert_eq!(prj.next_tag("=A1", "W"), "=A1-W2");
    assert_eq!(prj.next_tag("", "W"), "-W1");
    assert_eq!(prj.cable("=A1-W1").map(|c| c.core_list().len()), Some(0));
}

#[test]
fn project_file_round_trip() {
    let prj = crate::sample::demo_project();
    let back = Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(back, prj);
    assert!(Project::from_json("42").unwrap_err().starts_with("Not a valid project file"));
    // Ids are made unique again for hand edited files.
    let mut low = prj.clone();
    low.next_id = 1;
    assert!(Project::from_json(&low.to_json()).unwrap().next_id > 1);
}

#[test]
fn only_unreleased_schematic_pages_are_editable() {
    let mut pg = Page::new(1, "1", "", PageKind::Schematic);
    assert!(pg.is_editable());
    pg.locked = true;
    assert!(!pg.is_editable());
    assert!(!Page::new(2, "2", "", PageKind::Report(ReportKind::DeviceList.into())).is_editable());
    assert!(!Page::new(3, "3", "", PageKind::Cover).is_editable());
}

#[test]
fn next_tag_skips_unplaced_devices() {
    let mut prj = Project::default();
    prj.devices.insert("-K3".into(), DeviceData::default());
    prj.strips.insert("-X1".into(), Default::default());
    assert_eq!(prj.next_tag("", "K"), "-K4");
    assert_eq!(prj.next_tag("", "X"), "-X2");
}
