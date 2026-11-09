use super::*;

#[test]
fn group_choices_include_project_parts_and_respect_local_overrides() {
    let local=vec![Part {number:"LOCAL".into(),group:"Cables".into(),description:"Project cable".into(),..Default::default()},
        Part {number:"HIDDEN".into(),group:"Other".into(),..Default::default()}];
    let mut db=PartsDb {parts:vec![Part {number:"LOCAL".into(),group:"Cables".into(),description:"Stale catalog".into(),..Default::default()},
        Part {number:"HIDDEN".into(),group:"Cables".into(),..Default::default()},
        Part {number:"CATALOG".into(),group:"Cables".into(),..Default::default()}]};
    let choices=group_choices(&local,Some(&db),"Cables");
    assert_eq!(choices.iter().map(|p|p.number.as_str()).collect::<Vec<_>>(),vec!["CATALOG","LOCAL"]);
    assert_eq!(choices[1].description,"Project cable");
    db.parts.clear();
    assert_eq!(group_choices(&local,Some(&db),"Cables").len(),1,"an explicit empty catalog stays empty");
    let builtin=catalog().iter().find(|p|p.group=="Cables").unwrap();
    let override_part=vec![Part {number:builtin.number.clone(),group:"Other".into(),..Default::default()}];
    assert!(!group_choices(&override_part,None,"Cables").iter().any(|p|p.number==builtin.number));
    assert!(group_choices(&local,None,"Cables").iter().any(|p|p.number=="LOCAL"));
}

#[test]
fn bare_ema_channels_recover_one_physical_card_for_the_panel() {
    use crate::model::SymbolInst;
    use crate::symbols::{Func, PlcChannel};
    use crate::symlib::UserSymbol;
    let mut prj = Project::new_default();
    let mut card = Part { number: "BEC.EL3174".into(), width: 12., height: 100., depth: 68., ..Default::default() };
    for i in 0..4 {
        let key = format!("user:ema:test:1:{i}");
        let channel = PlcChannel {
            family: "test".into(),
            input: true,
            index: i,
            designation: format!("U{}", i + 1),
            address: String::new(),
            function_text: String::new(),
            analog: None,
            digital_terminal: None,
        };
        prj.symbols.push(UserSymbol { key: key.clone(), func: Func::Plc, plc_channel: Some(channel), ..Default::default() });
        card.eplan_variants.push(serde_json::json!({"symbol":{"key":key}}).into());
        prj.pages[0].symbols.push(SymbolInst { id: u64::from(i) + 2, sym: key, tag: "-A3".into(), ..Default::default() });
    }
    let mut db = PartsDb { parts: vec![card.clone()] };
    let mut other = card.clone();
    other.number = "BEC.Other".into();
    db.parts.push(other);
    link_unassigned_plc_parts(&mut prj, &db, None);
    assert!(crate::panel::devices_to_place(&prj).is_empty());
    db.parts.pop();
    link_unassigned_plc_parts(&mut prj, &db, None);
    link_unassigned_plc_parts(&mut prj, &db, Some("-A3"));
    let devices = crate::panel::devices_to_place(&prj);
    assert_eq!(devices.len(), 1);
    assert_eq!((&*devices[0].tag, &*devices[0].part, devices[0].w, devices[0].h), ("-A3", "BEC.EL3174", 12., 100.));
    assert_eq!(prj.devices["-A3"].parts, [PartRef { number: "BEC.EL3174".into(), count: 1, total:false }]);
}

#[test]
fn legacy_single_part_is_migrated() {
    let json = r#"{"devices": {"-K1": {"part": "CT-9A-24DC", "description": ""}},
                  "parts": [{"number": "CT-9A-24DC", "manufacturer": "Generic", "description": "old", "price": 1.0}]}"#;
    let mut prj: Project = serde_json::from_str(json).unwrap();
    prj.normalize();
    assert_eq!(prj.devices["-K1"].parts, vec![PartRef { number: "GEN.CT-9A-24DC".into(), count: 1, total:false }]);
    assert!(prj.parts.iter().any(|p| p.number == "GEN.CT-9A-24DC" && !p.functions.is_empty()));
    assert!(!prj.parts.iter().any(|p| p.number == "CT-9A-24DC"));
}

#[test]
fn every_function_kind_can_be_placed() {
    // Each kind has a symbol, and each template of the catalog fits a symbol of its
    // kind (same number of connection points), so designations can be assigned.
    for k in FnKind::ALL {
        assert!(symbols::library().iter().any(|d| kind_of_symbol(d.key) == Some(k)), "no symbol for {}", k.title());
    }
    for part in catalog() {
        for f in part.functions.iter().filter(|f| f.kind != FnKind::Terminal) {
            let n = f.pin_list().len();
            assert!(
                symbols::library().iter().any(|d| kind_of_symbol(d.key) == Some(f.kind) && d.pins.len() == n),
                "{}: no {} symbol with {n} connection points",
                part.number,
                f.kind.title()
            );
        }
    }
    // Main and auxiliary functions of devices are counted against the templates.
    for d in symbols::library().iter().filter(|d| matches!(d.func, Func::Aux(_))) {
        assert!(kind_of_symbol(d.key).is_some(), "{} has no function kind", d.key);
    }
}

#[test]
fn function_kind_titles_are_translated() {
    let known: std::collections::HashSet<&str> = openecraft_i18n::catalog_entries().into_iter().map(|e| e[0]).collect();
    for k in FnKind::ALL {
        assert!(known.contains(k.title()), "{} missing in the translation catalog", k.title());
    }
}

#[test]
fn designations_from_template_and_device_selection() {
    let mut prj = crate::sample::demo_project();
    // Scramble the contact numbers of -K1, then take them from the parts again.
    for pg in &mut prj.pages {
        for s in pg.symbols.iter_mut().filter(|s| s.tag == "-K1" && s.sym == "no") {
            s.pins = vec!["x".into(), "y".into()];
        }
    }
    assert_eq!(apply_designations(&mut prj, None, "-K1"), 3);
    let mut pins: Vec<String> = placed_functions(&prj, "-K1")
        .into_iter()
        .filter(|f| f.0 == FnKind::No)
        .map(|f| f.1.join("/"))
        .collect();
    pins.sort();
    assert_eq!(pins, vec!["13/14", "53/54", "83/84"]);

    let db = PartsDb::default();
    let placed = placed_functions(&prj, "-K1");
    let main = main_kind(&prj, "-K1");
    assert_eq!(main, Some(FnKind::Coil));
    assert!(fits_device(db.get("GEN.CT-9A-24DC").unwrap(), &placed, main));
    assert!(!fits_device(db.get("GEN.MCB-1P-C6").unwrap(), &placed, main));
}

#[test]
fn parts_are_added_to_devices() {
    let mut prj = Project::default();
    let db = PartsDb::default();
    let number = db.parts[0].number.clone();
    add_to_device(&mut prj, &db, "-K1", &number, 1);
    add_to_device(&mut prj, &db, "-K1", &number, 2);
    assert_eq!(prj.devices["-K1"].parts, vec![PartRef { number: number.clone(), count: 3, total:false }]);
    assert!(prj.parts.iter().any(|p| p.number == number), "copied into the project");
    // Unknown numbers are referenced all the same (typed by hand).
    add_to_device(&mut prj, &db, "-K1", "OWN.1", 1);
    assert_eq!(prj.devices["-K1"].parts.len(), 2);
}

#[test]
fn accessories_are_added_to_the_devices_they_fit() {
    let mut prj = Project::default();
    let db = PartsDb { parts: bundled::default_parts() };
    assign(&mut prj, &db, "-F1", "HAG.CCA216D");
    assign(&mut prj, &db, "-F2", "HAG.MBN106");
    // The signal contact names the RCCB it fits; the breaker has no accessories.
    let choices: Vec<String> = accessory_choices(&prj, Some(&db), "-F1").into_iter().map(|(p, main)| format!("{} {main}", p.number)).collect();
    assert_eq!(choices, ["HAG.CZ001 HAG.CCA216D"]);
    assert!(accessory_choices(&prj, Some(&db), "-F2").is_empty());
    assert_eq!(add_accessory(&mut prj, &db, "-F1", "HAG.CZ001", 1).unwrap(), "HAG.CCA216D");
    assert!(prj.parts.iter().any(|p| p.number == "HAG.CZ001"), "copied into the project");
    assert!(accessory_choices(&prj, Some(&db), "-F1").is_empty(), "already on the device");
    assert!(add_accessory(&mut prj, &db, "-F2", "HAG.CZ001", 1).unwrap_err().contains("fits none"));
    assert!(add_accessory(&mut prj, &db, "-F9", "HAG.CZ001", 1).is_err());
    // Its contacts add to the functions of the device.
    let kinds: Vec<FnKind> = available_functions(&prj, None, "-F1").into_iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FnKind::ResidualCurrent, FnKind::No, FnKind::Nc]);
    assert!(loose_accessories(&prj).is_empty());
    // Taken off its RCCB, the signal contact is loose.
    prj.devices.get_mut("-F1").unwrap().parts.remove(0);
    assert_eq!(loose_accessories(&prj), [("-F1".to_string(), "HAG.CZ001".to_string())]);
}

fn assign(prj: &mut Project, db: &PartsDb, tag: &str, number: &str) {
    add_to_device(prj, db, tag, number, 1);
}

#[test]
fn generic_parts_come_from_the_catalog_when_missing_in_the_database() {
    let mut prj = Project::default();
    ensure_in_project(&mut prj, &PartsDb { parts: Vec::new() }, "GEN.DUCT-60X80");
    assert!(prj.parts.iter().any(|p| p.number == "GEN.DUCT-60X80" && p.height == 60.0 && p.depth == 80.0));
    ensure_in_project(&mut prj, &PartsDb { parts: Vec::new() }, "UNKNOWN");
    assert_eq!(prj.parts.len(), 1);
}

#[test]
fn usage_from_the_placed_kinds_of_all_devices() {
    let prj = crate::sample::demo_project();
    let placed = placed_kinds(&prj);
    for tag in prj.devices.keys() {
        let kinds = placed.get(tag.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        assert_eq!(usage_of(&prj, None, tag, kinds), usage(&prj, None, tag), "{tag}");
    }
}

#[test]
fn assemblies_resolve_nested_components() {
    let mut prj = crate::sample::demo_project();
    let comp = |n: &str, q: f32| crate::parts::Component { number: n.into(), quantity: q };
    prj.parts.push(Part { number: "ASM.TOP".into(), components: vec![comp("ASM.SUB", 2.0), comp("P.A", 1.0)], ..Default::default() });
    prj.parts.push(Part { number: "ASM.SUB".into(), components: vec![comp("P.A", 3.0), comp("ASM.TOP", 1.0)], ..Default::default() });
    let c = assembly_components(&prj, "ASM.TOP", 2.0);
    assert_eq!(c, vec![("ASM.SUB".to_string(), 4.0), ("P.A".to_string(), 14.0)], "nested quantities, the cycle back to the top is ignored");
    assert!(assembly_components(&prj, "P.A", 1.0).is_empty());
}
