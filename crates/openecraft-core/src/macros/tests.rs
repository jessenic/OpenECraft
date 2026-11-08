use super::*;
use crate::geom::p;
use crate::sample::demo_project;

#[test]
fn portable_device_properties_and_owner_placeholders_resolve_with_tags() {
    let m:Macro=serde_json::from_value(serde_json::json!({"clip":{"devices":{
        "-K{UNIT}":{"values":{"device.rated_power":"{POWER}","device.material_owner":"-X{OWNER}"}}
    }}})).unwrap();
    assert_eq!(m.placeholders(),["OWNER","POWER","UNIT"]);
    let clip=m.resolve(&BTreeMap::from([("UNIT".into(),"3".into()),("OWNER".into(),"9".into()),("POWER".into(),"7.5".into())]));
    assert_eq!(clip.devices["-K3"].values["device.material_owner"],"-X9");
    assert_eq!(clip.devices["-K3"].values["device.rated_power"],"7.5");
}

#[test]
fn function_part_references_resolve_and_survive_merging_terminal_parts() {
    let m: Macro = serde_json::from_value(serde_json::json!({"clip":{
        "symbols":[{"sym":"term","tag":"-X1","label":"2","values":{"function.part_number":"{ARTICLE}","function.manufacturer":"{MAKER}"}}],
        "devices":{"-X1":{"parts":[{"number":"{ARTICLE}","count":1}]}},
        "parts":[{"number":"{ARTICLE}"}]
    }})).unwrap();
    assert_eq!(m.placeholders(), ["ARTICLE", "MAKER"]);
    let clip = m.resolve(&BTreeMap::from([("ARTICLE".into(), "A123".into()), ("MAKER".into(), "Vendor".into())]));
    let mut prj = Project::default();
    prj.devices.entry("-X1".into()).or_default().parts.push(crate::model::PartRef { number: "EXISTING".into(), count: 1, total:false });
    let ids = ops::paste(&mut prj, 0, &clip, p(40., 40.));
    let id = *ids.iter().find(|id| prj.pages[0].symbols.iter().any(|s| s.id == **id)).unwrap();
    let function = crate::properties::Object::Function(id);
    assert_eq!(crate::properties::get(&prj, &function, "part.number"), "A123");
    assert_eq!(crate::properties::get(&prj, &function, "function.part_index"), "2");
    assert_eq!(crate::properties::get(&prj, &function, "function.manufacturer"), "Vendor");
}

#[test]
fn portable_device_and_catalog_placeholders_resolve_together() {
    let m: Macro = serde_json::from_value(serde_json::json!({
        "clip": {
            "devices": {"-K{UNIT}": {"description": "{LOAD}", "parts": [{"number": "{ARTICLE}", "quantity": 1}]}},
            "parts": [{"number": "{ARTICLE}", "description": "{LOAD}"}]
        }
    })).unwrap();
    assert_eq!(m.placeholders(), ["ARTICLE", "LOAD", "UNIT"]);
    let clip = m.resolve(&BTreeMap::from([
        ("UNIT".into(), "7".into()), ("LOAD".into(), "Pump".into()), ("ARTICLE".into(), "A123".into()),
    ]));
    assert_eq!(clip.devices["-K7"].parts[0].number, "A123");
    assert_eq!(clip.devices["-K7"].description, "Pump");
    assert_eq!((clip.parts[0].number.as_str(), clip.parts[0].description.as_str()), ("A123", "Pump"));
}

/// Window macro of the motor feeder on page 2 with placeholders.
fn motor_macro(prj: &Project) -> Macro {
    let page = &prj.pages[1];
    let sel: BTreeSet<u64> = page.symbols.iter().filter(|s| ["-Q1", "-K1", "-M1"].contains(&s.tag.as_str())).map(|s| s.id).collect();
    let mut m = from_selection(page, &sel, "DOL starter");
    for s in &mut m.clip.symbols {
        if s.tag == "-M1" {
            s.function_text = "{MOTOR}\n{POWER}".into();
        }
    }
    m.value_sets.push(ValueSet { name: "Pump".into(), values: BTreeMap::from([("MOTOR".into(), "Pump".into()), ("POWER".into(), "1.5 kW".into())]) });
    m
}

#[test]
fn placeholders_are_found_and_filled() {
    let prj = demo_project();
    let m = motor_macro(&prj);
    assert_eq!(m.placeholders(), vec!["MOTOR".to_string(), "POWER".to_string()]);
    let clip = m.resolve(&m.value_sets[0].values);
    let motor = clip.symbols.iter().find(|s| s.tag == "-M1").unwrap();
    assert_eq!(motor.function_text, "Pump\n1.5 kW");
    // Unknown placeholders stay; braces without a name are not placeholders.
    let mut set = BTreeSet::new();
    scan("{A} {} {not valid!} {B_2}", &mut set);
    assert_eq!(set.into_iter().collect::<Vec<_>>(), ["A", "B_2"]);
    assert_eq!(fill("{A}-{C}", &BTreeMap::from([("A".into(), "x".into())])), "x-{C}");
}

#[test]
fn inserted_window_macros_get_new_tags() {
    let mut prj = demo_project();
    let m = motor_macro(&prj);
    let clip = m.resolve(&m.value_sets[0].values);
    let ids = ops::paste(&mut prj, 1, &clip, p(240.0, 150.0));
    let tags: BTreeSet<String> = prj.pages[1].symbols.iter().filter(|s| ids.contains(&s.id)).map(|s| s.tag.clone()).collect();
    assert_eq!(tags, BTreeSet::from(["-K2".to_string(), "-M2".to_string(), "-Q2".to_string()]));
}

#[test]
fn page_macros_create_a_page() {
    let mut prj = demo_project();
    prj.pages[2].function = "CONV".into();
    let mut m = from_page(&prj.pages[2], "Control circuit");
    m.page_description = "Control {NAME}".into();
    assert_eq!(m.kind, MacroKind::Page);
    let pages = prj.pages.len();
    let at = insert_page(&mut prj, 3, &m, &BTreeMap::from([("NAME".into(), "pump".into())]));
    assert_eq!((at, prj.pages.len()), (4, pages + 1));
    let new = &prj.pages[at];
    assert_eq!((new.description.as_str(), new.function.as_str(), new.name.as_str()), ("Control pump", "CONV", "9"));
    assert_eq!(new.symbols.len(), prj.pages[2].symbols.len());
    // Same positions as on the source page.
    let pos = |pg: &Page| pg.symbols.iter().map(|s| (s.pos.x as i32, s.pos.y as i32)).collect::<Vec<_>>();
    assert_eq!(pos(new), pos(&prj.pages[2]));
}

#[test]
fn library_upsert_and_merge() {
    let prj = demo_project();
    let mut lib = MacroLibrary::default();
    lib.upsert(motor_macro(&prj));
    let mut changed = motor_macro(&prj);
    changed.description = "new".into();
    lib.upsert(changed);
    assert_eq!(lib.macros.len(), 1);
    assert_eq!(lib.macros[0].description, "new");
    let other = MacroLibrary { macros: vec![from_page(&prj.pages[3], "PLC page")] };
    assert_eq!(lib.merge(other), 1);
    let json = serde_json::to_string(&lib).unwrap();
    assert_eq!(serde_json::from_str::<MacroLibrary>(&json).unwrap(), lib);
}

#[test]
fn library_files_hold_one_or_many_macros() {
    let m = Macro { name: "Motor".into(), ..Default::default() };
    let lib = MacroLibrary { macros: vec![m.clone()] };
    assert_eq!(MacroLibrary::from_json(&serde_json::to_string(&lib).unwrap()).unwrap(), lib);
    assert_eq!(MacroLibrary::from_json(&serde_json::to_string(&m).unwrap()).unwrap(), lib);
    assert!(MacroLibrary::from_json("42").is_err());
}

#[test]
fn variants_are_saved_and_chosen() {
    let prj = demo_project();
    let mut lib = MacroLibrary::default();
    let dol = motor_macro(&prj);
    assert!(!lib.upsert_variant(dol.clone(), ""));
    let mut sd = from_selection(&prj.pages[1], &BTreeSet::new(), "DOL starter");
    sd.description = "Star-delta".into();
    assert!(!lib.upsert_variant(sd, "B"));
    let m = lib.get("DOL starter").unwrap();
    assert_eq!(m.variant_names(), ["A", "B"]);
    assert_eq!(m.with_variant("A").unwrap().clip, dol.clip);
    let b = m.with_variant("B").unwrap();
    assert!(b.clip.symbols.is_empty());
    assert_eq!((b.description.as_str(), b.variant.as_str()), ("Star-delta", "B"));
    assert!(b.variants.is_empty());
    assert!(m.with_variant("C").is_none());
    // Saving again replaces the variant; the macro's own variant can be renamed.
    assert!(lib.upsert_variant(dol.clone(), "B"));
    assert_eq!(lib.get("DOL starter").unwrap().with_variant("B").unwrap().clip, dol.clip);
    let m = lib.macros.iter_mut().find(|m| m.name == "DOL starter").unwrap();
    assert!(m.remove_variant("B"));
    assert!(!m.remove_variant("A"));
    assert_eq!(m.variant_names(), ["A"]);
}

#[test]
fn files_without_variants_still_load() {
    let lib = MacroLibrary::from_json(r#"{"macros":[{"name":"X","kind":"Page"}]}"#).unwrap();
    assert_eq!(lib.macros[0].variant_names(), ["A"]);
    let json = serde_json::to_string(&lib).unwrap();
    assert!(!json.contains("variants"));
}
