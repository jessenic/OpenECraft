use super::*;

#[test]
fn duplicate_wire_cleanup_keeps_distinct_label_layouts_with_reversed_ends() {
    use crate::model::wiring::WireLabel;
    let mut prj=Project::default();
    prj.pages[0].wires=vec![
        Wire {id:1,a:p(40.,40.),b:p(80.,40.),core:"BN".into(),labels:vec![WireLabel {along:0.25,..Default::default()}],..Default::default()},
        Wire {id:2,a:p(80.,40.),b:p(40.,40.),core:"BN".into(),labels:vec![WireLabel {along:0.25,..Default::default()}],..Default::default()},
    ];
    cleanup_wires(&mut prj.pages[0]);
    assert_eq!(prj.pages[0].wires.len(),1);
    let wire=&prj.pages[0].wires[0];
    assert_eq!(wire.labels.len(),2);
    assert_eq!(wire.labels.iter().map(|label|label.position(wire)).collect::<Vec<_>>(),[p(50.,39.),p(70.,39.)]);
    let selected=BTreeSet::from([1]);
    rotate(&prj.symbol_table(),&mut prj.pages[0],&selected);
    assert!(prj.pages[0].wires[0].labels.iter().all(|label|label.angle==270.));
}
use crate::geom::p;
use crate::symlib::Symbols;

#[test]
fn invalid_cable_tag_drafts_do_not_change_inventory_or_drawing_references() {
    let mut prj=Project {cables:vec![Cable {id:10,tag:"-W1".into(),..Default::default()},Cable {id:20,tag:"-W2".into(),..Default::default()}],..Default::default()};
    prj.pages[0].wires.push(Wire {id:30,cable:"-W1".into(),core:"BN".into(),..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {id:31,tag:"-W1".into(),values:BTreeMap::from([("function.cable".into(),"-W1".into())]),..Default::default()});
    let baseline=prj.clone();
    for value in [""," ","-W2","W3","-W 3","-W3;4"] {
        assert!(rename_project_cable(&mut prj,10,value).is_err());assert_eq!(prj,baseline,"{value}");
    }
    assert!(rename_project_cable(&mut prj,99,"-W3").is_err());assert_eq!(prj,baseline);
    assert!(!rename_project_cable(&mut prj,10," -W1 ").unwrap());assert_eq!(prj,baseline);
    prj.pages[0].locked=true;let released=prj.clone();
    assert!(rename_project_cable(&mut prj,10,"-W3").is_err());assert_eq!(prj,released);
    prj.pages[0].locked=false;assert!(rename_project_cable(&mut prj,10," -W3 ").unwrap());
    assert_eq!(prj.cables[0].tag,"-W3");assert_eq!(prj.pages[0].wires[0].cable,"-W3");
    assert_eq!(prj.pages[0].symbols[0].values["function.cable"],"-W3");assert_eq!(prj.pages[0].wires[0].core,"BN");
}

#[test]
fn explicit_cable_marker_articles_retarget_even_when_the_old_article_is_shared() {
    let mut prj=Project::default();
    prj.cables.push(Cable {tag:"-Shared".into(),part:"OLD".into(),..Default::default()});
    prj.devices.insert("-Shared".into(),DeviceData {parts:vec![PartRef {number:"OLD".into(),count:1,total:false}],..Default::default()});
    for (id,tag,bound) in [(1,"-Shared",true),(2,"Custom caption",true),(3,"-Shared",false)] {
        let mut symbol=SymbolInst {id,tag:tag.into(),values:BTreeMap::from([("function.part_number".into(),"OLD".into())]),..Default::default()};
        if bound {symbol.values.insert("function.cable".into(),"-Shared".into());}
        prj.pages[0].symbols.push(symbol);
    }
    assert!(apply_project_cable_part(&mut prj,0,&Part {number:"NEW".into(),..Default::default()}));
    for symbol in &prj.pages[0].symbols[..2] {
        assert_eq!(symbol.values["function.part_number"],"NEW");assert_eq!(symbol.selected_part_number(&prj),Some("NEW"));
    }
    assert_eq!(prj.pages[0].symbols[2].values["function.part_number"],"OLD");
    assert_eq!(prj.pages[0].symbols[2].selected_part_number(&prj),Some("OLD"));
}

#[test]
fn cable_renames_follow_explicit_markers_definitions_and_inactive_options() {
    use crate::model::options::{Item,ModelOption,Parked};
    let mut prj=Project::default();
    prj.cables.push(Cable {id:100,tag:"-Shared".into(),cores:"BN BU".into(),..Default::default()});
    prj.pages[0].symbols=vec![SymbolInst {id:1,tag:"-Shared".into(),values:BTreeMap::from([("function.cable".into(),"-Shared".into())]),..Default::default()},
                            SymbolInst {id:2,tag:"-Shared".into(),function_text:"Motor".into(),..Default::default()}];
    prj.devices.insert("-Shared".into(),DeviceData::default());
    prj.field.places.push(crate::field::Place {tag:"-Shared".into(),..Default::default()});
    prj.field.route_mut("-Shared").via.push(9);
    prj.field.accessories.push(crate::field::Accessory {cable:"-Shared".into(),at:"-Shared".into(),..Default::default()});
    prj.pages[0].wires.push(Wire {id:3,cable:"-Shared".into(),core:"BN".into(),..Default::default()});
    prj.pages[0].conn_defs.push(ConnDef {id:4,cable:"-Shared".into(),core:"BU".into(),..Default::default()});
    prj.pages[0].cable_defs.push(CableDef {id:5,cable:"-Shared".into(),shield:true,..Default::default()});
    let mut parked_page=prj.pages[0].clone();parked_page.id=50;
    let items=vec![Item::Page(Box::new(parked_page)),Item::Symbol(Box::new(prj.pages[0].symbols[0].clone())),
                   Item::Wire(Box::new(prj.pages[0].wires[0].clone())),Item::ConnDef(prj.pages[0].conn_defs[0].clone()),
                   Item::CableDef(prj.pages[0].cable_defs[0].clone())];
    prj.model_options.options.push(ModelOption {parked:items.into_iter().map(|item|Parked {page:1,index:0,item,order:vec![]}).collect(),..Default::default()});
    prj.cables[0].tag="-W10".into();rename_cable(&mut prj,"-Shared","-W10");
    assert_eq!(prj.pages[0].symbols[0].tag,"-W10");assert_eq!(prj.pages[0].symbols[0].values["function.cable"],"-W10");
    assert_eq!(prj.pages[0].symbols[1].tag,"-Shared");assert!(prj.devices.contains_key("-Shared"));
    assert_eq!(prj.field.places[0].tag,"-Shared");assert_eq!(prj.field.accessories[0].at,"-Shared");
    assert_eq!(prj.field.routes[0].cable,"-W10");assert_eq!(prj.field.accessories[0].cable,"-W10");
    assert_eq!(prj.pages[0].conn_defs[0].cable,"-W10");assert_eq!(prj.pages[0].cable_defs[0].cable,"-W10");
    let text=serde_json::to_string(&prj).unwrap();assert!(!text.contains("\"cable\":\"-Shared\""));
    let mut reopened:Project=serde_json::from_str(&text).unwrap();remove_cable(&mut reopened,0);
    assert!(reopened.pages[0].symbols[0].tag.is_empty());assert!(!reopened.pages[0].symbols[0].values.contains_key("function.cable"));
    assert!(reopened.pages[0].conn_defs[0].cable.is_empty() && reopened.pages[0].conn_defs[0].core.is_empty());
    assert!(reopened.pages[0].wires[0].core.is_empty());assert!(reopened.pages[0].cable_defs[0].cable.is_empty());
    assert_eq!(reopened.pages[0].symbols[1].tag,"-Shared");
    assert!(reopened.field.routes.is_empty() && reopened.field.accessories.is_empty());
    assert_eq!(reopened.field.places[0].tag,"-Shared");
    let text=serde_json::to_string(&reopened).unwrap();assert!(!text.contains("\"cable\":\"-W10\""));
}

#[test]
fn device_rename_preserves_a_shared_explicit_cable_marker_and_its_route() {
    let mut prj=Project::default();
    prj.cables.push(Cable {tag:"-Shared".into(),..Default::default()});
    prj.pages[0].symbols=vec![SymbolInst {id:1,tag:"-Shared".into(),values:BTreeMap::from([("function.cable".into(),"-Shared".into())]),..Default::default()},
                            SymbolInst {id:2,tag:"-Shared".into(),..Default::default()}];
    prj.field.route_mut("-Shared").via.push(9);
    prj.field.places.push(crate::field::Place {tag:"-Shared".into(),..Default::default()});
    prj.field.accessories.push(crate::field::Accessory {cable:"-Shared".into(),at:"-Shared".into(),..Default::default()});
    rename_device(&mut prj,"-Shared","-M10");
    assert_eq!(prj.pages[0].symbols[0].tag,"-Shared");assert_eq!(prj.pages[0].symbols[0].values["function.cable"],"-Shared");
    assert_eq!(prj.pages[0].symbols[1].tag,"-M10");assert_eq!(prj.cables[0].tag,"-Shared");
    assert_eq!(prj.field.routes[0].cable,"-Shared");assert_eq!(prj.field.places[0].tag,"-M10");
    assert_eq!(prj.field.accessories[0].cable,"-Shared");assert_eq!(prj.field.accessories[0].at,"-M10");
}

#[test]
fn applying_a_cable_article_retargets_its_explicit_functions_and_preserves_shared_motor_selection() {
    let mut prj=Project::default();
    prj.cables.push(Cable {id:10,tag:"-Shared".into(),part:"OLD".into(),length:25.,cores:"BN BU".into(),..Default::default()});
    prj.devices.insert("-Shared".into(),DeviceData {parts:vec![PartRef {number:"MOTOR".into(),count:1,total:false}],..Default::default()});
    for (id,number) in [(1,"OLD"),(2,"MOTOR"),(3,"MISSING")] {
        prj.pages[0].symbols.push(SymbolInst {id,tag:"-Shared".into(),values:BTreeMap::from([("function.part_number".into(),number.into())]),..Default::default()});
    }
    let before=serde_json::to_value(&prj).unwrap();
    let part=Part {number:"NEW".into(),tech_data:"Outer diameter: 6.3 mm".into(),..Default::default()};
    assert!(!apply_project_cable_part(&mut prj,99,&part));assert_eq!(serde_json::to_value(&prj).unwrap(),before);
    assert!(apply_project_cable_part(&mut prj,0,&part));
    let symbols=&prj.pages[0].symbols;
    assert_eq!(symbols[0].selected_part_number(&prj),Some("NEW"));
    assert_eq!(symbols[1].selected_part_number(&prj),Some("MOTOR"));
    assert_eq!(symbols[2].values["function.part_number"],"MISSING");
    assert_eq!(prj.cables[0].length,25.);assert_eq!(prj.cables[0].cores,"BN BU");
    assert_eq!(prj.cables[0].diameter,6.3);assert_eq!(prj.devices["-Shared"].parts[0].number,"MOTOR");
    // The direct text editor uses the same reference operation after changing the value.
    prj.cables[0].part.clear();assert_eq!(retarget_cable_part_references(&mut prj,"-Shared","NEW",""),1);
    assert_eq!(prj.pages[0].symbols[0].values["function.part_number"],"");
    assert!(apply_project_cable_part(&mut prj,0,&part));
    assert_eq!(prj.pages[0].symbols[0].selected_part_number(&prj),Some("NEW"));
}

#[test]
fn ordinary_device_article_references_are_not_retargeted_as_cable_material() {
    let mut prj=Project::default();
    prj.devices.insert("-Shared".into(),DeviceData {parts:vec![PartRef {number:"OLD".into(),count:1,total:false}],..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {tag:"-Shared".into(),values:BTreeMap::from([("function.part_number".into(),"OLD".into())]),..Default::default()});
    assert_eq!(retarget_cable_part_references(&mut prj,"-Shared","OLD","NEW"),0);
    assert_eq!(prj.pages[0].symbols[0].selected_part_number(&prj),Some("OLD"));
}

#[test]
fn portable_cables_keep_core_links_and_reserve_incoming_tags_across_structures() {
    let mut prj=Project::default();prj.pages[0].function="DST".into();prj.pages[0].location="NEW".into();
    prj.cables.push(Cable {id:90,tag:"=DST+NEW-W1".into(),description:"Existing".into(),..Default::default()});
    let mut clip=Clip::new();clip.function="SRC".into();clip.location="OLD".into();
    for (id,tag) in [(1,"=SRC+OLD-W1"),(2,"=SRC+OLD-W2")] {
        clip.cables.push(Cable {id,tag:tag.into(),cores:"BN BU".into(),length:25.,part:"C1".into(),diameter:6.3,..Default::default()});
        clip.symbols.push(SymbolInst {id:id+10,sym:"coil".into(),tag:tag.into(),pos:p(id as f32*10.,0.),
            values:BTreeMap::from([("function.cable".into(),tag.into())]),..Default::default()});
        clip.wires.push(Wire {id:id+20,a:p(0.,id as f32*5.),b:p(40.,id as f32*5.),cable:tag.into(),core:"BN".into(),..Default::default()});
    }
    clip.conn_defs.push(ConnDef {id:40,pos:p(20.,5.),cable:"=SRC+OLD-W1".into(),core:"BU".into(),..Default::default()});
    clip.cable_defs.push(CableDef {id:41,a:p(20.,0.),b:p(20.,20.),cable:"=SRC+OLD-W1".into(),..Default::default()});
    let clip=Clip::from_text(&clip.to_text()).unwrap();
    for _ in 0..2 {paste(&mut prj,0,&clip,p(80.,40.));}
    assert_eq!(prj.cables.iter().map(|c|c.tag.as_str()).collect::<Vec<_>>(),
        ["=DST+NEW-W1","=DST+NEW-W3","=DST+NEW-W2","=DST+NEW-W4","=DST+NEW-W5"]);
    assert_eq!(prj.cables[0].description,"Existing");
    for (i,wire) in prj.pages[0].wires.iter().enumerate() {
        let cable=&prj.cables[i+1];assert_eq!(wire.cable,cable.tag);assert_eq!(wire.core,"BN");
        assert_eq!(prj.pages[0].symbols[i].tag,cable.tag);assert_eq!(cable.part,"C1");assert_eq!(cable.length,25.);
        assert_eq!(prj.pages[0].symbols[i].values["function.cable"],cable.tag);
    }
    assert_eq!(prj.pages[0].conn_defs[0].cable,"=DST+NEW-W3");assert_eq!(prj.pages[0].conn_defs[0].core,"BU");
    assert_eq!(prj.pages[0].cable_defs[1].cable,"=DST+NEW-W4");
    let ids:BTreeSet<_>=prj.cables.iter().skip(1).map(|c|c.id).collect();assert_eq!(ids.len(),4);
}

#[test]
fn copied_cable_references_carry_only_used_inventory_and_catalog_data() {
    let mut prj=Project::default();
    prj.pages[0].wires.push(Wire {id:1,a:p(0.,0.),b:p(10.,0.),cable:"-W1".into(),core:"BN".into(),..Default::default()});
    prj.cables=vec![Cable {tag:"-W1".into(),part:"C1".into(),..Default::default()},Cable {tag:"-W2".into(),..Default::default()}];
    prj.parts.push(Part {number:"C1".into(),..Default::default()});
    let mut clip=copy(&prj.pages[0],&BTreeSet::from([1]));clip.include_project_data(&prj);clip.include_project_data(&prj);
    assert_eq!(clip.cables.len(),1);assert_eq!(clip.cables[0].tag,"-W1");assert_eq!(clip.parts.len(),1);
    clip.cables.clear();paste(&mut prj,0,&clip,p(40.,40.));
    assert!(prj.pages[0].wires[1].cable.is_empty());assert!(prj.pages[0].wires[1].core.is_empty());
}

#[test]
fn cleanup_preserves_overlapping_wires_with_different_contact_or_conductor_properties() {
    let mut prj=Project::default();
    let wire=Wire {id:1,a:p(0.,0.),b:p(40.,0.),..Default::default()};
    prj.pages[0].wires=vec![wire.clone(),Wire {id:2,explicit_junctions:Some(true),..wire.clone()},
        Wire {id:3,..wire.clone()},Wire {id:4,wire_type:"H07V-K".into(),..wire}];
    cleanup_wires(&mut prj.pages[0]);
    assert_eq!(prj.pages[0].wires.iter().map(|w|w.id).collect::<Vec<_>>(),[1,2,4]);
}

#[test]
fn portable_material_counts_and_properties_survive_insertion_and_fresh_device_renaming() {
    let mut prj=Project::default();let mut clip=Clip::new();
    prj.devices.insert("-K1".into(),DeviceData {description:"Existing unplaced".into(),..Default::default()});
    clip.symbols.push(SymbolInst {id:1,sym:"coil".into(),tag:"-K1".into(),..Default::default()});
    let data=DeviceData {parts:vec![PartRef {number:"PRIMARY".into(),count:1,total:false},
        PartRef {number:"PRIMARY".into(),count:0,total:true},PartRef {number:"EXTRA".into(),count:3,total:true}],
        values:BTreeMap::from([("device.rated_power".into(),"7.5".into()),("device.exclude_from_bom".into(),"true".into()),
                              ("device.material_owner".into(),"-Outside".into())]),..Default::default()};
    clip.devices.insert("-K1".into(),data.clone());
    paste(&mut prj,0,&clip,p(80.,40.));
    assert_eq!(prj.devices["-K2"].parts,data.parts);
    assert_eq!(prj.devices["-K2"].values["device.rated_power"],"7.5");
    assert_eq!(prj.devices["-K2"].values["device.exclude_from_bom"],"true");
    assert!(prj.devices["-K2"].values["device.material_owner"].is_empty());
    assert_eq!(prj.devices["-K1"].description,"Existing unplaced");
}

#[test]
fn portable_owner_links_follow_structure_and_renaming_without_consuming_another_incoming_tag() {
    let mut prj=Project::default();prj.pages[0].function="DST".into();prj.pages[0].location="NEW".into();
    prj.pages[0].symbols.push(SymbolInst {id:10,sym:"coil".into(),tag:"=DST+NEW-K1".into(),..Default::default()});
    let mut clip=Clip::new();clip.function="SRC".into();clip.location="OLD".into();
    for (id,tag,label) in [(1,"=SRC+OLD-K1","Owner"),(2,"=SRC+OLD-K2","Child")] {
        clip.symbols.push(SymbolInst {id,sym:"coil".into(),tag:tag.into(),label:label.into(),..Default::default()});
        clip.devices.insert(tag.into(),DeviceData::default());
    }
    clip.devices.get_mut("=SRC+OLD-K2").unwrap().values.insert("device.material_owner".into(),"=SRC+OLD-K1".into());
    clip.symbol_defs.push(crate::symlib::UserSymbol {key:"user:secondary".into(),func:Func::Auxiliary,..Default::default()});
    clip.symbols.push(SymbolInst {id:3,sym:"user:secondary".into(),tag:"=SRC+OLD-K1".into(),label:"Secondary".into(),..Default::default()});
    let inserted=paste(&mut prj,0,&clip,p(80.,40.));
    let symbols:Vec<_>=prj.pages[0].symbols.iter().filter(|s|inserted.contains(&s.id)).collect();
    let owner=symbols.iter().find(|s|s.label=="Owner").unwrap();let child=symbols.iter().find(|s|s.label=="Child").unwrap();
    assert_eq!(owner.tag,"=DST+NEW-K3");assert_eq!(child.tag,"=DST+NEW-K2");
    assert_eq!(prj.devices[&child.tag].values["device.material_owner"],owner.tag);
    assert_eq!(symbols.iter().find(|s|s.label=="Secondary").unwrap().tag,owner.tag);
}

#[test]
fn merging_a_terminal_macro_fills_missing_modes_and_preserves_local_quantities_and_properties() {
    let mut prj=Project::default();let mut clip=Clip::new();
    clip.symbols.push(SymbolInst {id:1,sym:"term".into(),tag:"-X1".into(),label:"1".into(),..Default::default()});
    clip.devices.insert("-X1".into(),DeviceData {parts:vec![PartRef {number:"TB".into(),count:1,total:false},
        PartRef {number:"TB".into(),count:2,total:true}],values:BTreeMap::from([("device.note".into(),"Incoming".into())]),..Default::default()});
    prj.devices.insert("-X1".into(),DeviceData {parts:vec![PartRef {number:"TB".into(),count:1,total:false}],
        values:BTreeMap::from([("device.note".into(),"Local".into())]),..Default::default()});
    paste(&mut prj,0,&clip,p(80.,40.));
    assert_eq!(prj.devices["-X1"].parts.len(),2);assert_eq!(prj.devices["-X1"].parts[1].count,2);
    prj.devices.get_mut("-X1").unwrap().parts[1].count=5;
    paste(&mut prj,0,&clip,p(120.,40.));
    assert_eq!(prj.devices["-X1"].parts.len(),2);assert_eq!(prj.devices["-X1"].parts[1].count,5);
    assert_eq!(prj.devices["-X1"].values["device.note"],"Local");
}

#[test]
fn portable_clip_installs_definitions_without_overwriting_local_edits() {
    let mut project = Project::default();
    let definition = crate::symlib::UserSymbol { key: "user:portable".into(), name: "Incoming".into(), ..Default::default() };
    let mut clip = Clip::new();
    clip.symbol_defs.push(definition.clone());
    clip.symbols.push(SymbolInst { id: 1, sym: definition.key.clone(), pos: p(10., 10.), ..Default::default() });
    project.symbols.push(crate::symlib::UserSymbol { name: "Locally edited".into(), ..definition });
    paste(&mut project, 0, &clip, p(40., 40.));
    assert_eq!(project.symbols.len(), 2);
    assert_eq!(project.symbols[0].name, "Locally edited");
    assert_eq!(project.pages[0].symbols[0].sym, "user:portable_import_1");
    paste(&mut project, 0, &clip, p(80., 40.));
    assert_eq!(project.symbols.len(), 2);
    let restored = Clip::from_text(&clip.to_text()).unwrap();
    assert_eq!(restored.symbol_defs, clip.symbol_defs);
    assert!(clip.symbol(&Project::default(),"user:portable").is_some());
}

#[test]
fn portable_clip_renames_device_assignments_and_keeps_existing_catalog_parts() {
    let mut project=Project::default();
    project.parts.push(Part {number:"P1".into(),description:"Local catalog".into(),..Default::default()});
    project.pages[0].symbols.push(SymbolInst {id:2,sym:"coil".into(),tag:"-K1".into(),..Default::default()});
    let mut clip=Clip::new();
    clip.symbols.push(SymbolInst {id:1,sym:"coil".into(),tag:"-K1".into(),..Default::default()});
    clip.parts.push(Part {number:"P1".into(),description:"Incoming".into(),..Default::default()});
    clip.parts.push(Part {number:"P2".into(),..Default::default()});
    clip.devices.insert("-K1".into(),DeviceData {parts:vec![PartRef {number:"P1".into(),..Default::default()},PartRef {number:"P2".into(),..Default::default()}],..Default::default()});
    paste(&mut project,0,&clip,p(80.,40.));
    assert_eq!(project.parts.len(),2);
    assert_eq!(project.parts[0].description,"Local catalog");
    assert_eq!(project.devices["-K2"].parts.len(),2);
    assert!(!project.devices.contains_key("-K1"));
}

#[test]
fn contact_numbering_and_tag_follow_coil() {
    let mut prj = Project::default();
    let mut ctx = PlaceCtx::default();
    let coil = prj.symbol("coil").unwrap();
    let s = new_symbol(&mut prj, 0, &ctx, &coil, p(40.0, 40.0), 0, false);
    assert_eq!(s.tag, "-K1");
    ctx.remember(&prj, &s);
    prj.pages[0].symbols.push(s);
    let no = prj.symbol("no").unwrap();
    let c1 = new_symbol(&mut prj, 0, &ctx, &no, p(80.0, 40.0), 0, false);
    assert_eq!(c1.tag, "-K1");
    assert_eq!(c1.pins, vec!["13", "14"]);
    prj.pages[0].symbols.push(c1);
    let nc = prj.symbol("nc").unwrap();
    let c2 = new_symbol(&mut prj, 0, &ctx, &nc, p(120.0, 40.0), 0, false);
    assert_eq!(c2.pins, vec!["21", "22"]);
    prj.pages[0].symbols.push(c2);
    // Time-delayed contacts: x7/x8 (NO) and x5/x6 (NC).
    let c3_def = prj.symbol("no_on").unwrap();
    let c3 = new_symbol(&mut prj, 0, &ctx, &c3_def, p(160.0, 40.0), 0, false);
    assert_eq!((c3.tag.as_str(), c3.pins.clone()), ("-K1", vec!["37".to_string(), "38".to_string()]));
    prj.pages[0].symbols.push(c3);
    let c4_def = prj.symbol("nc_off").unwrap();
    let c4 = new_symbol(&mut prj, 0, &ctx, &c4_def, p(200.0, 40.0), 0, false);
    assert_eq!(c4.pins, vec!["45", "46"]);
}

#[test]
fn plc_addresses_increment() {
    let mut prj = Project::default();
    let mut ctx = PlaceCtx::default();
    let di = prj.symbol("plc_in").unwrap();
    for expect in ["I0.0", "I0.1"] {
        let s = new_symbol(&mut prj, 0, &ctx, &di, p(40.0, 40.0), 0, false);
        assert_eq!(s.label, expect);
        ctx.remember(&prj, &s);
        prj.pages[0].symbols.push(s);
    }
}

#[test]
fn paste_renames_existing_devices() {
    let mut prj = Project::default();
    let ctx = PlaceCtx::default();
    let coil = prj.symbol("coil").unwrap();
    let s = new_symbol(&mut prj, 0, &ctx, &coil, p(40.0, 40.0), 0, false);
    let id = s.id;
    prj.pages[0].symbols.push(s);
    let clip = copy(&prj.pages[0], &BTreeSet::from([id]));
    let ids = paste(&mut prj, 0, &clip, p(80.0, 40.0));
    let new = prj.pages[0].symbols.iter().find(|s| ids.contains(&s.id)).unwrap();
    assert_eq!(new.tag, "-K2");
}

#[test]
fn rotate_single_symbol_keeps_origin() {
    let mut prj = Project::default();
    let ctx = PlaceCtx::default();
    let s_def = prj.symbol("no").unwrap();
    let s = new_symbol(&mut prj, 0, &ctx, &s_def, p(40.0, 40.0), 0, false);
    let id = s.id;
    prj.pages[0].symbols.push(s);
    rotate(&Symbols::default(), &mut prj.pages[0], &BTreeSet::from([id]));
    let s = &prj.pages[0].symbols[0];
    assert_eq!((s.pos, s.rot), (p(40.0, 40.0), 1));
}

/// Project with a second page in structure =A1+CAB1.
fn structured() -> Project {
    let mut prj = Project::default();
    let id = prj.new_id();
    let mut pg = Page::new(id, "2", "", PageKind::Schematic);
    pg.function = "A1".into();
    pg.location = "CAB1".into();
    prj.pages.push(pg);
    prj
}

#[test]
fn devices_get_the_page_structure() {
    let mut prj = structured();
    let ctx = PlaceCtx::default();
    let coil = prj.symbol("coil").unwrap();
    let a = new_symbol(&mut prj, 1, &ctx, &coil, p(40.0, 40.0), 0, false);
    assert_eq!(a.tag, "=A1+CAB1-K1");
    prj.pages[1].symbols.push(a);
    // Numbering is per structure.
    assert_eq!(new_symbol(&mut prj, 1, &ctx, &coil, p(80.0, 40.0), 0, false).tag, "=A1+CAB1-K2");
    assert_eq!(new_symbol(&mut prj, 0, &ctx, &coil, p(80.0, 40.0), 0, false).tag, "-K1");
    let term = prj.symbol("term").unwrap();
    assert_eq!(new_symbol(&mut prj, 1, &ctx, &term, p(80.0, 40.0), 0, false).tag, "=A1+CAB1-X1");
}

#[test]
fn paste_moves_devices_into_the_target_structure() {
    let mut prj = structured();
    let ctx = PlaceCtx::default();
    let s_def = prj.symbol("coil").unwrap();
    let s = new_symbol(&mut prj, 0, &ctx, &s_def, p(40.0, 40.0), 0, false);
    let id = s.id;
    prj.pages[0].symbols.push(s);
    let clip = copy(&prj.pages[0], &BTreeSet::from([id]));
    let ids = paste(&mut prj, 1, &clip, p(80.0, 40.0));
    let new = prj.pages[1].symbols.iter().find(|s| ids.contains(&s.id)).unwrap();
    assert_eq!(new.tag, "=A1+CAB1-K1");
}

#[test]
fn page_structure_change_renames_its_devices() {
    let mut prj = structured();
    let ctx = PlaceCtx::default();
    let s_def = prj.symbol("coil").unwrap();
    let s = new_symbol(&mut prj, 1, &ctx, &s_def, p(40.0, 40.0), 0, false);
    prj.pages[1].symbols.push(s);
    // A contact of the coil on the other page follows the device.
    let contact_def = prj.symbol("no").unwrap();
    let mut contact = new_symbol(&mut prj, 0, &ctx, &contact_def, p(80.0, 40.0), 0, false);
    contact.tag = "=A1+CAB1-K1".into();
    prj.pages[0].symbols.push(contact);
    prj.devices.insert("=A1+CAB1-K1".into(), DeviceData { description: "pump".into(), ..Default::default() });
    let dt = crate::structure::Dt::parse_structure;
    set_page_structure(&mut prj, 1, &dt("=A2+CAB1"), true);
    assert_eq!((prj.pages[1].function.as_str(), prj.pages[1].location.as_str()), ("A2", "CAB1"));
    assert_eq!(prj.all_tags(), vec!["=A2+CAB1-K1".to_string()]);
    assert_eq!(prj.devices["=A2+CAB1-K1"].description, "pump");
    // Without moving, the tags stay.
    set_page_structure(&mut prj, 1, &dt("=A3"), false);
    assert_eq!(prj.all_tags(), vec!["=A2+CAB1-K1".to_string()]);
    // Installation site and product move along as well.
    set_page_structure(&mut prj, 1, &dt("=A2++HALL1+CAB1-U1"), false);
    let s_def = prj.symbol("coil").unwrap();
    let mut s = new_symbol(&mut prj, 1, &ctx, &s_def, p(40.0, 80.0), 0, false);
    s.tag = "=A2++HALL1+CAB1-U1-K2".into();
    prj.pages[1].symbols.push(s);
    set_page_structure(&mut prj, 1, &dt("=A2++HALL2+CAB1-U3"), true);
    assert!(prj.all_tags().contains(&"=A2++HALL2+CAB1-U3-K2".to_string()), "{:?}", prj.all_tags());
}

#[test]
fn wires_get_free_cores_in_order() {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.cables.push(Cable { id, tag: "-W1".into(), cores: "BN BK GY GNYE".into(), ..Default::default() });
    for (id, x) in [(10, 60.0), (11, 40.0), (12, 50.0)] {
        prj.pages[0].wires.push(Wire { id, a: p(x, 10.0), b: p(x, 30.0), ..Default::default() });
    }
    // BK is already used by another wire.
    prj.pages[0].wires.push(Wire { id: 13, a: p(0.0, 0.0), b: p(0.0, 9.0), cable: "-W1".into(), core: "BK".into(), ..Default::default() });
    assert_eq!(assign_cores(&mut prj, 0, &BTreeSet::from([10, 11, 12]), "-W1"), 3);
    let core = |id: u64| prj.pages[0].wires.iter().find(|w| w.id == id).unwrap().core.clone();
    assert_eq!((core(11), core(12), core(10)), ("BN".into(), "GY".into(), "GNYE".into()));
    // No free core left, unknown cable.
    prj.pages[0].wires.push(Wire { id: 14, a: p(70.0, 10.0), b: p(70.0, 30.0), ..Default::default() });
    assert_eq!(assign_cores(&mut prj, 0, &BTreeSet::from([14]), "-W1"), 0);
    assert_eq!(assign_cores(&mut prj, 0, &BTreeSet::from([14]), "-W9"), 0);
}

#[test]
fn paste_drops_numbers_and_cores() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(0.0, 0.0), b: p(0.0, 20.0), number: "7".into(), cable: "-W1".into(), core: "BN".into(), ..Default::default() });
    prj.next_id = 6;
    let clip = copy(&prj.pages[0], &BTreeSet::from([5]));
    let ids = paste(&mut prj, 0, &clip, p(40.0, 0.0));
    let w = prj.pages[0].wires.iter().find(|w| ids.contains(&w.id)).unwrap();
    assert!(w.number.is_empty() && w.cable.is_empty() && w.core.is_empty());
}

#[test]
fn rename_device_updates_the_panel_layout() {
    let mut prj = crate::sample::demo_project();
    rename_device(&mut prj, "-K1", "-K10");
    assert!(prj.layout.find_device("-K10").is_some());
    assert!(prj.layout.find_device("-K1").is_none());
}

#[test]
fn add_page_numbers_after_the_highest_page() {
    let mut prj = Project::default();
    prj.pages[0].name = "7".into();
    let at = add_page(&mut prj, PageKind::Report(ReportKind::DeviceList.into()), 0);
    assert_eq!(at, 0);
    assert_eq!(prj.pages[0].name, "8");
    assert_eq!(prj.pages[0].kind, PageKind::Report(ReportKind::DeviceList.into()));
    assert_eq!(add_page(&mut prj, PageKind::Schematic, 99), 2);
}

#[test]
fn released_and_last_pages_are_not_deleted() {
    let mut prj = Project::default();
    assert!(delete_page(&mut prj, 0).is_err(), "last page");
    add_page(&mut prj, PageKind::Schematic, 1);
    prj.pages[1].locked = true;
    assert_eq!(delete_page(&mut prj, 1).unwrap_err(), "Released pages cannot be deleted");
    assert_eq!(delete_page(&mut prj, 0).unwrap().name, "1");
    assert_eq!(prj.pages.len(), 1);
}

#[test]
fn cables_keep_their_wires_consistent() {
    let mut prj = Project::default();
    let tag = add_cable(&mut prj, "");
    assert_eq!(tag, "-W1");
    assert_eq!(add_cable(&mut prj, ""), "-W2");
    prj.pages[0].wires.push(Wire { id: 50, a: p(0.0, 0.0), b: p(0.0, 8.0), cable: "-W1".into(), core: "BN".into(), ..Default::default() });
    prj.cables[0].tag = "-W10".into();
    rename_cable(&mut prj, "-W1", "-W10");
    assert_eq!(prj.pages[0].wires[0].cable, "-W10");
    assert_eq!(remove_cable(&mut prj, 0).tag, "-W10");
    let w = &prj.pages[0].wires[0];
    assert!(w.cable.is_empty() && w.core.is_empty());
    assert_eq!(prj.cables.len(), 1);

    let mut part = crate::parts::Part { number: "GEN.CABLE".into(), type_number: "H05VV-F".into(), ..Default::default() };
    part.tech_data = "Cores: BK BU GNYE
Cross-section: 0.75 mm²".into();
    apply_cable_part(&mut prj.cables[0], &part);
    let c = &prj.cables[0];
    assert_eq!((c.part.as_str(), c.cable_type.as_str()), ("GEN.CABLE", "H05VV-F"));
    assert_eq!((c.cores.as_str(), c.cross_section.as_str()), ("BK BU GNYE", "0.75 mm²"));
}

#[test]
fn cable_part_selection_uses_declared_diameter_and_clears_stale_geometry() {
    let mut cable=Cable {diameter:12.5,..Default::default()};
    let mut part=crate::parts::Part {number:"C".into(),tech_data:"Outer diameter: 6,3 mm".into(),..Default::default()};
    apply_cable_part(&mut cable,&part);
    assert_eq!(cable.diameter,6.3);
    for data in ["", "Outer diameter: unknown", "Outer diameter: -6", "Outer diameter: NaN", "Outer diameter: inf"] {
        cable.diameter=9.;part.tech_data=data.into();apply_cable_part(&mut cable,&part);
        assert_eq!(cable.diameter,0.,"missing or invalid dimensions use the routing estimate");
    }
}

#[test]
fn pages_move_to_another_position() {
    let mut prj = crate::sample::demo_project();
    let names: Vec<String> = prj.pages.iter().map(|p| p.name.clone()).collect();
    assert_eq!(move_page(&mut prj, 0, 2), 2);
    assert_eq!(prj.pages[2].name, names[0]);
    assert_eq!(prj.pages[0].name, names[1]);
    assert_eq!(move_page(&mut prj, 2, 0), 0);
    assert_eq!(prj.pages.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), names);
    assert_eq!(move_page(&mut prj, 99, 0), 99, "out of range: unchanged");
}

#[test]
fn rotating_a_selection_turns_its_texts() {
    let mut prj = crate::model::Project::default();
    let id = prj.new_id();
    prj.pages[0].texts.push(crate::model::TextItem { id, pos: crate::geom::p(100.0, 100.0), text: "T".into(), ..Default::default() });
    let symbols = prj.symbol_table();
    let sel: std::collections::BTreeSet<u64> = [id].into_iter().collect();
    rotate(&symbols, &mut prj.pages[0], &sel);
    assert_eq!(prj.pages[0].texts[0].angle, 270.0, "clockwise like the symbols");
    rotate(&symbols, &mut prj.pages[0], &sel);
    rotate(&symbols, &mut prj.pages[0], &sel);
    rotate(&symbols, &mut prj.pages[0], &sel);
    assert_eq!(prj.pages[0].texts[0].angle, 0.0);
}

#[test]
fn sub_pages_follow_their_page() {
    let mut prj = crate::sample::demo_project();
    let three = prj.pages.iter().position(|p| p.name == "3").unwrap();
    prj.pages[three].function = "A2".into();
    let a = add_subpage(&mut prj, three);
    assert_eq!((a, prj.pages[a].name.as_str()), (three + 1, "3.1"));
    assert_eq!(prj.pages[a].function, "A2", "in the structure of its page");
    let b = add_subpage(&mut prj, three);
    assert_eq!((b, prj.pages[b].name.as_str()), (three + 2, "3.2"));
    let c = add_subpage(&mut prj, a);
    assert_eq!(prj.pages[c].name, "3.3", "a sub-page of a sub-page belongs to the main page");
}

#[test]
fn new_pages_join_the_section_of_the_page_before() {
    let mut prj = Project::default();
    let a = sections::add(&mut prj, 0, "");
    let at = add_page(&mut prj, PageKind::Schematic, 1);
    sections::move_page(&mut prj, at, a).unwrap();
    assert_eq!(prj.pages[1].name, "1.2");
    let at = add_page(&mut prj, PageKind::Schematic, 2);
    assert_eq!((prj.pages[at].section, prj.pages[at].name.as_str()), (a, "1.3"));
    let at = add_subpage(&mut prj, 1);
    assert_eq!((at, prj.pages[at].name.as_str()), (2, "1.2.a"), "lettered once there are sections");
    let at = add_subpage(&mut prj, 2);
    assert_eq!(prj.pages[at].name, "1.2.b");
    let at = add_page(&mut prj, PageKind::Schematic, 0);
    assert_eq!((prj.pages[at].section, prj.pages[at].name.as_str()), (0, "2"));
    assert_eq!(next_page_name(&prj, prj.pages.len()), "1.4");
    assert_eq!(letters(27), "ab");
    assert_eq!(move_page(&mut prj, 2, 1), 2, "pages move within their section only");
    assert_eq!(move_page(&mut prj, 1, 0), 0);
    let b = sections::add(&mut prj, a, "");
    let at = add_page_in(&mut prj, PageKind::Schematic, b);
    assert_eq!((at, prj.pages[at].name.as_str()), (prj.pages.len() - 1, "1.1.1"));
    let at = add_page_in(&mut prj, PageKind::Schematic, a);
    assert_eq!((at, prj.pages[at].name.as_str()), (6, "1.4"));
}

#[test]
fn devices_on_released_pages_are_not_renamed() {
    let mut prj = crate::sample::demo_project();
    assert_eq!(rename_problem(&prj, "-K1", "-K1"), None);
    assert_eq!(rename_problem(&prj, "-K1", "-K9"), None);
    assert!(rename_problem(&prj, "-K1", "-S1").unwrap().contains("already exists"));
    let page = prj.pages.iter().position(|pg| pg.symbols.iter().any(|s| s.tag == "-K1")).unwrap();
    prj.pages[page].locked = true;
    assert!(rename_problem(&prj, "-K1", "-K9").unwrap().contains("released"));
}

#[test]
fn paste_lands_where_its_offset_says() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(0.0, 0.0), b: p(0.0, 20.0), ..Default::default() });
    prj.next_id = 6;
    let clip = copy(&prj.pages[0], &BTreeSet::from([5]));
    // A point off the connection point grid (1 mm grid): the offset keeps the 4 mm grid.
    let at = p(41.0, 9.0);
    let d = paste_offset(&clip, at);
    assert_eq!(d, p(40.0, 8.0));
    let ids = paste(&mut prj, 0, &clip, at);
    let w = prj.pages[0].wires.iter().find(|w| ids.contains(&w.id)).unwrap();
    assert_eq!((w.a, w.b), (p(0.0, 0.0) + d, p(0.0, 20.0) + d));
}

#[test]
fn pasted_devices_keep_the_letters_of_their_tag() {
    let mut prj = Project::default();
    let ctx = PlaceCtx::default();
    let coil = prj.symbol("coil").unwrap();
    let mut s = new_symbol(&mut prj, 0, &ctx, &coil, p(40.0, 40.0), 0, false);
    s.tag = "-Q1".into();
    let id = s.id;
    prj.pages[0].symbols.push(s);
    let clip = copy(&prj.pages[0], &BTreeSet::from([id]));
    let ids = paste(&mut prj, 0, &clip, p(80.0, 40.0));
    let new = prj.pages[0].symbols.iter().find(|s| ids.contains(&s.id)).unwrap();
    assert_eq!(new.tag, "-Q2");
}

#[test]
fn the_wires_of_one_connection_share_a_core() {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.cables.push(Cable { id, tag: "-W1".into(), cores: "BN BK GY".into(), ..Default::default() });
    // An L of two wires, a T of three (one ends on another) and a single wire.
    let wires = [(10, (40.0, 10.0), (40.0, 30.0)), (11, (40.0, 30.0), (60.0, 30.0)), (12, (80.0, 10.0), (80.0, 50.0)), (13, (80.0, 30.0), (100.0, 30.0)), (14, (120.0, 10.0), (120.0, 30.0))];
    for (id, a, b) in wires {
        prj.pages[0].wires.push(Wire { id, a: p(a.0, a.1), b: p(b.0, b.1), ..Default::default() });
    }
    assert_eq!(assign_cores(&mut prj, 0, &BTreeSet::from([10, 11, 12, 13, 14]), "-W1"), 5);
    let core = |prj: &Project, id: u64| prj.pages[0].wires.iter().find(|w| w.id == id).unwrap().core.clone();
    let cores: Vec<String> = [10, 11, 12, 13, 14].iter().map(|&i| core(&prj, i)).collect();
    assert_eq!(cores, ["BN", "BN", "BK", "BK", "GY"]);
    // A new segment of a connection takes the core the connection has.
    prj.pages[0].wires.push(Wire { id: 15, a: p(60.0, 30.0), b: p(60.0, 50.0), ..Default::default() });
    assert_eq!(assign_cores(&mut prj, 0, &BTreeSet::from([15]), "-W1"), 1);
    assert_eq!(core(&prj, 15), "BN");
}

#[test]
fn renaming_reaches_lists_of_tags_and_inactive_options() {
    let mut prj = crate::sample::demo_project();
    let (pi, tag) = prj.pages.iter().enumerate().find_map(|(i, p)| p.symbols.iter().find(|s| s.tag.starts_with("-K")).map(|s| (i, s.tag.clone()))).unwrap();
    prj.pct_loops.push(crate::model::process::PctLoop { devices: vec![tag.clone()], ..Default::default() });
    let subsystem = crate::model::safety::Subsystem { devices: vec![tag.clone()], channel2: vec![tag.clone()], ..Default::default() };
    prj.electrical.safety.push(crate::model::safety::SafetyFunction { subsystems: vec![subsystem], ..Default::default() });
    prj.review.requests.push(crate::model::review::ChangeRequest { id: 1, rev: 1, devices: vec![tag.clone()], ..Default::default() });
    let sym = prj.pages[pi].symbols.iter().find(|s| s.tag == tag).unwrap().id;
    crate::options::define(&mut prj, "Opt", "", "").unwrap();
    crate::options::assign(&mut prj, "Opt", &[sym]).unwrap();
    crate::options::set_active(&mut prj, "Opt", false).unwrap();
    rename_device(&mut prj, &tag, "-K99");
    assert_eq!(prj.pct_loops[0].devices, ["-K99"]);
    let s = &prj.electrical.safety[0].subsystems[0];
    assert_eq!((s.devices.as_slice(), s.channel2.as_slice()), (&["-K99".to_string()][..], &["-K99".to_string()][..]));
    assert_eq!((prj.review.requests[0].devices.as_slice(), prj.review.requests[0].rev), (&["-K99".to_string()][..], 2));
    crate::options::set_active(&mut prj, "Opt", true).unwrap();
    assert_eq!(prj.pages.iter().flat_map(|p| &p.symbols).find(|s| s.id == sym).map(|s| s.tag.as_str()), Some("-K99"));
}

#[test]
fn cleanup_ignores_automatic_numbers_but_keeps_fixed_ones() {
    let mut prj=Project::default();
    let wire=Wire {id:1,a:p(0.,0.),b:p(40.,0.),number:"1".into(),..Default::default()};
    prj.pages[0].wires=vec![wire.clone(),Wire {id:2,number:"2".into(),..wire.clone()},Wire {id:3,number:"3".into(),number_fixed:true,..wire}];
    cleanup_wires(&mut prj.pages[0]);
    assert_eq!(prj.pages[0].wires.iter().map(|w|w.id).collect::<Vec<_>>(),[1,3]);
}
