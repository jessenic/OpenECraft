use super::*;
use crate::geom::p;

#[test]
fn cable_fields_and_placed_part_properties_follow_an_explicit_reference_with_a_custom_caption() {
    let mut prj=project();
    prj.parts.push(Part {number:"CABLE".into(),manufacturer:"Cable maker".into(),..Default::default()});
    prj.cables.push(Cable {id:80,tag:"-W1".into(),part:"CABLE".into(),cable_type:"Initial cable".into(),length:25.,..Default::default()});
    let mut symbol=prj.pages[0].symbols[0].clone();symbol.id=999;symbol.tag="Custom caption".into();
    symbol.values.insert("function.cable".into(),"-W1".into());
    assert_eq!(text(&prj,0,&symbol,&placed("cable.type")),"Initial cable");
    assert_eq!(symbol_property_text(&prj,0,&symbol,"cable.type"),"Initial cable");
    assert_eq!(text(&prj,0,&symbol,&placed("part.manufacturer")),"Cable maker");
    prj.cables[0].cable_type="Edited cable".into();
    assert_eq!(symbol_property_text(&prj,0,&symbol,"cable.type"),"Edited cable");
    symbol.values.insert("function.cable".into(),"-Missing".into());symbol.tag="-W1".into();
    assert_eq!(symbol_property_text(&prj,0,&symbol,"cable.type"),"");
    assert_eq!(text(&prj,0,&symbol,&placed("part.manufacturer")),"");
    symbol.values.remove("function.cable");
    assert_eq!(symbol_property_text(&prj,0,&symbol,"cable.type"),"Edited cable");
}

#[test]
fn shared_graphics_render_each_cable_context_and_update_after_inventory_edits() {
    use crate::symbols::{Anchor,BlockAlign,BlockRow,PropertyBlock,Prim,Func};
    let mut prj=Project::default();
    let block=PropertyBlock {rows:vec![BlockRow::Property("cable.type".into())],pos:p(10.,10.),step:p(0.,3.5),
        size:2.5,anchor:Anchor::Left,angle:0.,align:BlockAlign::Start,color:None};
    prj.symbols.push(crate::symlib::UserSymbol {key:"user:cable_context".into(),func:Func::Graphic,
        show_tag:false,prims:vec![Prim::PropertyBlock(block)],..Default::default()});
    for (id,tag,value) in [(20,"-W1","First cable"),(30,"-W2","Second cable")] {
        prj.cables.push(Cable {id,tag:tag.into(),cable_type:value.into(),..Default::default()});
        prj.pages[0].symbols.push(SymbolInst {id:id+1,sym:"user:cable_context".into(),pos:p(id as f32,20.),tag:"Caption".into(),
            values:std::collections::BTreeMap::from([("function.cable".into(),tag.into())]),..Default::default()});
    }
    let before=crate::svg::page_svg(&prj,0);assert!(before.contains("First cable") && before.contains("Second cable"));
    prj.cables[0].cable_type="Edited cable".into();
    let saved=Project::from_json(&prj.to_json()).unwrap();let after=crate::svg::page_svg(&saved,0);
    assert!(after.contains("Edited cable") && after.contains("Second cable"));assert!(!after.contains("First cable"));
}

#[test]
fn property_blocks_follow_the_page_and_project_while_retaining_the_selected_function_part() {
    let mut prj=project();
    prj.sections.push(Section {id:100,number:"1".into(),..Default::default()});
    prj.pages[0].section=100;prj.pages[0].name="1.16".into();prj.pages[0].description="CPU".into();
    let mut second=Page::new(101,"1.17","Inputs",PageKind::Schematic);second.section=100;prj.pages.push(second);
    prj.pages.push(Page::new(102,"99","Outside",PageKind::Schematic));
    prj.parts.push(Part {number:"SECOND".into(),..Default::default()});
    prj.devices.get_mut("-K1").unwrap().parts.push(PartRef {number:"SECOND".into(),count:1,total:false});
    prj.pages[0].symbols[0].values.insert("function.part_number".into(),"SECOND".into());
    let symbol=&prj.pages[0].symbols[0];
    for (key,expected) in [("page.local_name","16"),("page.section_pages","2"),("page.description","CPU"),("part.number","SECOND")] {
        assert_eq!(symbol_property_text(&prj,0,symbol,key),expected,"{key}");
    }
    prj.pages[0].name="1.20".into();prj.info.name="Edited plant".into();
    let symbol=&prj.pages[0].symbols[0];
    assert_eq!(symbol_property_text(&prj,0,symbol,"page.local_name"),"20");
    assert_eq!(symbol_property_text(&prj,0,symbol,"project.name"),"Edited plant");
}

fn project() -> Project {
    let mut prj = Project::default();
    prj.parts.push(Part { number: "SIE.3RT2015".into(), type_number: "3RT2015-1BB41".into(), manufacturer: "Siemens".into(), ..Default::default() });
    prj.devices.insert(
        "-K1".into(),
        DeviceData { description: "Main contactor".into(), parts: vec![PartRef { number: "SIE.3RT2015".into(), count: 1, total:false }], ..Default::default() },
    );
    prj.pages[0].symbols.push(SymbolInst { id: 50, sym: "coil".into(), tag: "-K1".into(), pins: vec!["A1".into(), "A2".into()], ..Default::default() });
    prj
}

fn placed(key: &str) -> PlacedProp {
    PlacedProp { key: key.into(), ..Default::default() }
}

#[test]
fn texts_come_from_device_function_and_parts() {
    let prj = project();
    let s = &prj.pages[0].symbols[0];
    assert_eq!(text(&prj, 0, s, &placed("function.pins")), "A1 A2");
    assert_eq!(text(&prj, 0, s, &placed("device.description")), "Main contactor");
    assert_eq!(text(&prj, 0, s, &placed("device.parts")), "SIE.3RT2015");
    assert_eq!(text(&prj, 0, s, &placed("device.type_numbers")), "3RT2015-1BB41");
    assert_eq!(text(&prj, 0, s, &placed("part.manufacturer")), "Siemens");
    assert_eq!(text(&prj, 0, s, &placed("project.name")), "New project");
    assert_eq!(text(&prj, 0, s, &placed("unknown.key")), "");
    let named = PlacedProp { name: true, ..placed("device.description") };
    assert_eq!(text(&prj, 0, s, &named), "Device description: Main contactor");
}

#[test]
fn older_fixed_kinds_are_converted_to_keys() {
    let json = r#"{"sym":"coil","tag":"-K1","props":[{"kind":"PartNumber","offset":{"x":1.0,"y":2.0},"size":2.0},{"kind":"Structure"}]}"#;
    let mut s: SymbolInst = serde_json::from_str(json).unwrap();
    migrate(&mut s);
    assert_eq!(s.props[0].key, "device.parts");
    assert_eq!(s.props[1].key, "device.structure");
    let out = serde_json::to_string(&s).unwrap();
    assert!(!out.contains("\"kind\""), "{out}");
    for k in PropKind::ALL {
        assert!(crate::properties::valid_key(&Project::default(), k.key()).is_some(), "{k:?}");
    }
}

#[test]
fn added_properties_stack_downwards() {
    let symbols = Symbols::default();
    let mut s = SymbolInst { sym: "coil".into(), ..Default::default() };
    add(&symbols, &mut s, "device.description");
    add(&symbols, &mut s, "device.parts");
    assert_eq!(s.props[0].offset, reference(&symbols, &s));
    assert!((s.props[1].offset.y - s.props[0].offset.y - 2.8).abs() < 1e-4);
}

#[test]
fn schemes_are_captured_applied_and_updated() {
    let mut prj = project();
    let symbols = prj.symbol_table();
    let mut s = prj.pages[0].symbols[0].clone();
    add(&symbols, &mut s, "device.description");
    add(&symbols, &mut s, "device.rated_voltage");
    s.props[1].name = true;
    let scheme = capture(&symbols, &s, "Mine");
    assert_eq!(scheme.entries[0].offset, p(0.0, 0.0));
    define_scheme(&mut prj, scheme).unwrap();
    assert!(define_scheme(&mut prj, Scheme { name: "Part data".into(), ..Default::default() }).is_err());
    assert!(define_scheme(&mut prj, Scheme { name: "Bad".into(), entries: vec![placed("nope")], ..Default::default() }).is_err());

    // Another symbol at another place gets the same arrangement relative to it.
    prj.pages[0].symbols.push(SymbolInst { id: 51, sym: "coil".into(), tag: "-K2".into(), pos: p(100.0, 40.0), ..Default::default() });
    assert_eq!(apply_to(&mut prj, "Mine", &[51]).unwrap(), 1);
    let t = &prj.pages[0].symbols[1];
    assert_eq!(t.scheme, "Mine");
    assert_eq!(t.props.len(), 2);
    assert!(t.props[1].name);
    assert_eq!(t.props[0].offset, reference(&symbols, t));

    // Changing the scheme updates every symbol using it.
    let mut changed = scheme_of(&prj, "Mine");
    changed.entries.push(placed("device.parts"));
    define_scheme(&mut prj, changed).unwrap();
    assert_eq!(reapply(&mut prj, "Mine"), 1);
    assert_eq!(prj.pages[0].symbols[1].props.len(), 3);
    remove_scheme(&mut prj, "Mine");
    assert!(prj.pages[0].symbols[1].scheme.is_empty());
    assert!(apply_to(&mut prj, "Mine", &[51]).is_err());
}

fn scheme_of(prj: &Project, name: &str) -> Scheme {
    scheme(prj, name).cloned().unwrap()
}

#[test]
fn builtin_schemes_use_defined_properties() {
    let prj = Project::default();
    for s in builtin_schemes() {
        assert!(!s.title().is_empty());
        for e in &s.entries {
            assert!(crate::properties::valid_key(&prj, &e.key).is_some(), "{}: {}", s.name, e.key);
        }
    }
}
