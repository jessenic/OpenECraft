use super::*;

fn project() -> Project {
    let mut prj = Project::default();
    prj.parts.push(Part { number: "GEN.MOTOR".into(), voltage: "400".into(), weight: 12.5, datasheet: "motor.pdf".into(), ..Default::default() });
    prj.devices.insert("-M1".into(), DeviceData { parts: vec![PartRef { number: "GEN.MOTOR".into(), count: 2 }], ..Default::default() });
    prj.pages[0].symbols.push(SymbolInst { id: 10, sym: "motor3".into(), tag: "-M1".into(), ..Default::default() });
    prj.pages[0].symbols.push(SymbolInst { id: 11, sym: "term".into(), tag: "-X1".into(), label: "3".into(), ..Default::default() });
    prj.pages[0].symbols.push(SymbolInst { id: 12, sym: "plc_in".into(), tag: "-A1".into(), label: "I0.0".into(), ..Default::default() });
    prj.cables.push(Cable { tag: "-W1".into(), cores: "1 2 3 GNYE".into(), ..Default::default() });
    prj
}

#[test]
fn function_part_selection_is_local_live_and_does_not_change_device_aggregates() {
    let mut prj = project();
    prj.parts.push(Part { number: "SECOND".into(), manufacturer: "Maker two".into(), ..Default::default() });
    prj.devices.get_mut("-M1").unwrap().parts.push(PartRef { number: "SECOND".into(), count: 1 });
    let function = Object::Function(10);
    assert_eq!(get(&prj, &function, "part.number"), "GEN.MOTOR");
    assert_eq!(get(&prj, &function, "function.part_index"), "1");
    set(&mut prj, &function, "function.part_index", "2").unwrap();
    assert_eq!(get(&prj, &function, "part.number"), "SECOND");
    assert_eq!(get(&prj, &function, "function.manufacturer"), "Maker two");
    assert_eq!(get(&prj, &Object::Device("-M1".into()), "device.parts"), "GEN.MOTOR, SECOND");
    assert_eq!(Object::Function(10).classes(&prj).last(), Some(&ObjectClass::Part));
    prj.parts.iter_mut().find(|p| p.number == "SECOND").unwrap().manufacturer = "Updated maker".into();
    assert_eq!(get(&prj, &function, "function.manufacturer"), "Updated maker");
    set(&mut prj, &function, "function.manufacturer", "Function override").unwrap();
    assert_eq!(get(&prj, &function, "function.manufacturer"), "Function override");
    set(&mut prj, &function, "function.manufacturer", "").unwrap();
    assert_eq!(get(&prj, &function, "function.manufacturer"), "Updated maker");
    for index in ["0", "-1", "3"] {
        let before = prj.to_json();
        assert!(set(&mut prj, &function, "function.part_index", index).is_err());
        assert_eq!(prj.to_json(), before);
    }
    prj.devices.get_mut("-M1").unwrap().parts[1].number = "MISSING.CATALOG".into();
    set(&mut prj, &function, "function.part_index", "2").unwrap();
    assert_eq!(get(&prj, &function, "part.number"), "MISSING.CATALOG");
    assert_eq!(get(&prj, &function, "function.manufacturer"), "");
    let prj = Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(get(&prj, &function, "part.number"), "MISSING.CATALOG");
}

#[test]
fn removing_a_selected_part_cannot_silently_bind_to_another_part() {
    let mut prj = project();
    let function = Object::Function(10);
    prj.devices.get_mut("-M1").unwrap().parts.extend([PartRef { number: "SECOND".into(), count: 1 }, PartRef { number: "THIRD".into(), count: 1 }]);
    set(&mut prj, &function, "function.part_index", "2").unwrap();
    prj.devices.get_mut("-M1").unwrap().parts.remove(0);
    assert_eq!(get(&prj, &function, "part.number"), "SECOND");
    assert_eq!(get(&prj, &function, "function.part_index"), "1");
    prj.devices.get_mut("-M1").unwrap().parts.remove(0);
    assert_eq!(get(&prj, &function, "part.number"), "");
    assert_eq!(get(&prj, &function, "function.part_index"), "0");
    set(&mut prj, &function, "function.part_index", "").unwrap();
    assert_eq!(get(&prj, &function, "part.number"), "THIRD");
}

#[test]
fn builtin_definitions_are_consistent() {
    let mut keys = std::collections::HashSet::new();
    let mut addressable = 0;
    for d in builtin() {
        assert!(keys.insert(d.key.as_str()), "duplicate key {}", d.key);
        assert!(d.key.starts_with(ObjectClass::ALL.iter().find(|c| c.key() == d.key.split('.').next().unwrap()).map(|c| c.key()).unwrap_or("?")), "{}", d.key);
        assert_eq!(d.class.key(), d.key.split('.').next().unwrap(), "class of {}", d.key);
        assert!(!d.title().is_empty());
        assert_eq!(d.choices.is_empty(), d.ty != DataType::Choice, "choices of {}", d.key);
        if !d.fallback.is_empty() {
            assert!(builtin_def(&d.fallback).is_some(), "fallback of {}", d.key);
        }
        addressable += d.value_keys().len();
    }
    // Hundreds of definitions, several hundred more indexed supplementary fields.
    assert!(builtin().len() > 300, "{}", builtin().len());
    assert!(addressable > 750, "{addressable}");
    for c in ObjectClass::ALL {
        assert!(builtin().iter().any(|d| d.class == c), "{c:?}");
        assert_eq!(ObjectClass::from_key(c.key()), Some(c));
    }
    for t in DataType::ALL {
        assert_eq!(DataType::from_key(t.key()), Some(t));
    }
}

#[test]
fn keys_with_indices() {
    let prj = Project::default();
    assert_eq!(split_key("device.supplementary[12]"), ("device.supplementary", Some(12)));
    assert_eq!(split_key("device.note"), ("device.note", None));
    assert!(valid_key(&prj, "device.supplementary[100]").is_some());
    assert!(valid_key(&prj, "device.supplementary[101]").is_none());
    assert!(valid_key(&prj, "device.supplementary").is_none());
    assert!(valid_key(&prj, "device.note[1]").is_none());
    assert_eq!(def(&prj, "device.supplementary[3]").unwrap().title_of("device.supplementary[3]"), "Supplementary field [3]");
}

#[test]
fn values_are_checked_and_normalised() {
    let d = |key: &str| builtin_def(key).unwrap();
    assert_eq!(d("device.poles").check(" 3 ").unwrap(), "3");
    assert!(d("device.poles").check("three").is_err());
    assert_eq!(d("device.rated_current").check("2,5").unwrap(), "2.5");
    assert_eq!(d("device.safety_relevant").check("yes").unwrap(), "true");
    assert!(d("device.safety_relevant").check("maybe").is_err());
    assert_eq!(d("device.ip").check("ip54").unwrap(), "IP54");
    assert!(d("device.ip").check("IP99").is_err());
    assert_eq!(d("device.installed").check("2026-10-24").unwrap(), "2026-10-24");
    assert!(d("device.installed").check("24.10.2026").is_err());
    assert_eq!(d("device.rated_voltage").format("24"), "24 V");
    assert_eq!(d("device.rated_voltage").format("24 V DC"), "24 V DC");
    assert_eq!(d("device.safety_relevant").format("false"), "No");
}

#[test]
fn values_are_stored_per_object() {
    let mut prj = project();
    let dev = Object::Device("-M1".into());
    set(&mut prj, &dev, "device.speed", "1450").unwrap();
    set(&mut prj, &dev, "device.supplementary[7]", "spare").unwrap();
    assert_eq!(get(&prj, &dev, "device.speed"), "1450");
    assert_eq!(display(&prj, &dev, "device.speed"), "1450 rpm");
    assert_eq!(prj.devices["-M1"].values["device.supplementary[7]"], "spare");
    assert_eq!(origin(&prj, &dev, "device.speed"), Origin::Stored);
    set(&mut prj, &dev, "device.speed", "").unwrap();
    assert_eq!(origin(&prj, &dev, "device.speed"), Origin::None);

    // Properties of other classes are refused, read-only ones too.
    assert!(set(&mut prj, &dev, "cable.route", "x").is_err());
    assert!(set(&mut prj, &dev, "device.tag", "-M2").is_err());
    assert!(set(&mut prj, &dev, "nope", "x").is_err());

    // A device that only exists through its values is created and removed again.
    let k5 = Object::Device("-K5".into());
    set(&mut prj, &k5, "device.note", "later").unwrap();
    assert!(prj.devices.contains_key("-K5"));
    set(&mut prj, &k5, "device.note", "").unwrap();
    assert!(!prj.devices.contains_key("-K5"));

    let page = Object::Page(prj.pages[0].id);
    set(&mut prj, &page, "page.drawing_number", "D-100").unwrap();
    assert_eq!(prj.pages[0].values["page.drawing_number"], "D-100");
    let cable = Object::Cable("-W1".into());
    set(&mut prj, &cable, "cable.drag_chain", "1").unwrap();
    assert_eq!(display(&prj, &cable, "cable.drag_chain"), "Yes");
    assert_eq!(get(&prj, &cable, "cable.core_count"), "4");
    set(&mut prj, &Object::Project, "project.customer", "ACME").unwrap();
    assert_eq!(prj.info.properties["project.customer"], "ACME");
}

#[test]
fn model_fields_are_read_and_written() {
    let mut prj = project();
    let dev = Object::Device("-M1".into());
    set(&mut prj, &dev, "device.description", "Pump motor").unwrap();
    assert_eq!(prj.devices["-M1"].description, "Pump motor");
    set(&mut prj, &dev, "device.function_text", "Pump").unwrap();
    assert_eq!(prj.pages[0].symbols[0].function_text, "Pump");
    assert_eq!(get(&prj, &dev, "device.letter"), "M");
    assert_eq!(get(&prj, &dev, "device.functions"), "1");
    assert_eq!(get(&prj, &dev, "device.definition"), "Motor 3-phase");

    let term = Object::Function(11);
    assert_eq!(get(&prj, &term, "terminal.strip"), "-X1");
    set(&mut prj, &term, "terminal.number", "4").unwrap();
    assert_eq!(prj.pages[0].symbols[1].label, "4");
    set(&mut prj, &term, "terminal.level", "2").unwrap();
    assert!(set(&mut prj, &term, "plc.symbolic", "x").is_err());
    let plc = Object::Function(12);
    set(&mut prj, &plc, "plc.symbolic", "Start_Pump").unwrap();
    assert_eq!(get(&prj, &plc, "plc.address"), "I0.0");

    let cable = Object::Cable("-W1".into());
    set(&mut prj, &cable, "cable.length", "12.5").unwrap();
    assert_eq!(prj.cables[0].length, 12.5);
    set(&mut prj, &Object::Project, "project.name", "Plant 7").unwrap();
    assert_eq!(prj.info.name, "Plant 7");
    let page = Object::Page(prj.pages[0].id);
    assert!(set(&mut prj, &page, "page.name", "").is_err());
    prj.pages[0].locked = true;
    assert!(set(&mut prj, &page, "page.description", "x").is_err());
}

#[test]
fn values_fall_back_to_function_definition_and_part_data() {
    let mut prj = project();
    let dev = Object::Device("-M1".into());
    assert_eq!(get(&prj, &dev, "device.rated_voltage"), "400");
    assert_eq!(origin(&prj, &dev, "device.rated_voltage"), Origin::Default);
    assert_eq!(get(&prj, &dev, "device.weight"), "12.5");
    assert_eq!(get(&prj, &dev, "device.total_weight"), "25");
    assert_eq!(get(&prj, &dev, "device.datasheet"), "motor.pdf");
    assert_eq!(get(&prj, &dev, "part.weight"), "12.5");
    assert_eq!(get(&Project::default(), &Object::Part("GEN.MOTOR".into()), "part.weight"), "");

    let mut f = functions::derive(&prj, functions::builtin_def("fn.motor3").unwrap());
    f.defaults.insert("device.rated_voltage".into(), "230".into());
    f.defaults.insert("device.efficiency_class".into(), "IE3".into());
    let key = f.key.clone();
    functions::define(&mut prj, f).unwrap();
    functions::assign(&mut prj, 10, &key).unwrap();
    assert_eq!(get(&prj, &dev, "device.rated_voltage"), "230");
    assert_eq!(origin(&prj, &dev, "device.efficiency_class"), Origin::FunctionDefinition);
    set(&mut prj, &dev, "device.rated_voltage", "690").unwrap();
    assert_eq!(get(&prj, &dev, "device.rated_voltage"), "690");
}

#[test]
fn keys_follow_classes_and_function_definitions() {
    let prj = project();
    let motor = keys_of(&prj, &Object::Device("-M1".into()), true, false);
    assert!(motor.iter().any(|(k, _)| k == "device.speed"));
    assert!(!motor.iter().any(|(k, _)| k == "device.coil_voltage"));
    assert!(motor.iter().any(|(k, _)| k == "part.weight"));
    assert!(keys_of(&prj, &Object::Device("-M1".into()), false, false).iter().any(|(k, _)| k == "device.coil_voltage"));
    let filled: Vec<String> = keys_of(&prj, &Object::Device("-M1".into()), true, true).into_iter().map(|(k, _)| k).collect();
    assert!(filled.contains(&"device.rated_voltage".to_string()));
    assert!(!filled.contains(&"device.speed".to_string()));
    let term = keys_of(&prj, &Object::Function(11), true, false);
    assert!(term.iter().any(|(k, _)| k == "terminal.level"));
    assert!(!term.iter().any(|(k, _)| k.starts_with("plc.")));
    assert_eq!(keys_of(&prj, &Object::Project, false, false).iter().filter(|(k, _)| k.starts_with("project.supplementary")).count(), 100);
}

#[test]
fn user_properties_are_defined_and_removed() {
    let mut prj = project();
    let d = PropDef { key: "user.colour_code".into(), name: "Colour code".into(), class: ObjectClass::Device, group: Group::User, ty: DataType::Choice, choices: vec!["A".into(), "B".into()], ..Default::default() };
    define(&mut prj, d.clone()).unwrap();
    assert!(define(&mut prj, PropDef { key: "device.note".into(), ..d.clone() }).is_err());
    assert!(define(&mut prj, PropDef { key: "bad key".into(), ..d.clone() }).is_err());
    assert!(define(&mut prj, PropDef { name: " ".into(), ..d.clone() }).is_err());
    assert!(define(&mut prj, PropDef { class: ObjectClass::Part, ..d.clone() }).is_err());
    let dev = Object::Device("-M1".into());
    set(&mut prj, &dev, "user.colour_code", "b").unwrap();
    assert_eq!(get(&prj, &dev, "user.colour_code"), "B");
    assert!(set(&mut prj, &dev, "user.colour_code", "C").is_err());
    prj.pages[0].symbols[0].props.push(PlacedProp { key: "user.colour_code".into(), ..Default::default() });
    assert_eq!(statistics(&prj)[&ObjectClass::Device], 1);
    assert_eq!(undefine(&mut prj, "user.colour_code").unwrap(), 1);
    assert!(prj.pages[0].symbols[0].props.is_empty());
    assert!(undefine(&mut prj, "user.colour_code").is_err());
}

#[test]
fn libraries_round_trip_and_merge() {
    let mut prj = project();
    define(&mut prj, PropDef { key: "user.x".into(), name: "X".into(), ..Default::default() }).unwrap();
    functions::define(&mut prj, FunctionDef { key: "user.f".into(), name: "F".into(), symbol: "coil".into(), ..Default::default() }).unwrap();
    let json = prj.property_db.to_json();
    let lib = Library::from_json(&json).unwrap();
    assert_eq!(lib, prj.property_db);
    assert!(Library::from_json("[1]").is_err());

    let mut other = Library::default();
    let mut foreign = lib.clone();
    foreign.properties.push(PropDef { key: "device.note".into(), name: "Shadow".into(), ..Default::default() });
    assert_eq!(other.merge(foreign), 2);
    assert_eq!(other.properties.len(), 1);
    assert_eq!(other.merge(lib), 2);
    assert_eq!(other.functions.len(), 1);
}

#[test]
fn objects_are_parsed_and_copied() {
    let mut prj = project();
    assert_eq!(Object::parse(&prj, "device:-M1").unwrap(), Object::Device("-M1".into()));
    assert_eq!(Object::parse(&prj, "page:1").unwrap(), Object::Page(prj.pages[0].id));
    assert_eq!(Object::parse(&prj, "function:11").unwrap(), Object::Function(11));
    assert_eq!(Object::parse(&prj, "project").unwrap(), Object::Project);
    assert!(Object::parse(&prj, "device:-Q9").is_err());
    assert!(Object::parse(&prj, "thing:1").is_err());
    let m1 = Object::Device("-M1".into());
    set(&mut prj, &m1, "device.speed", "1450").unwrap();
    prj.pages[0].symbols.push(SymbolInst { id: 13, sym: "motor3".into(), tag: "-M2".into(), ..Default::default() });
    assert_eq!(copy_values(&mut prj, &m1, &[Object::Device("-M2".into()), m1.clone()]), 1);
    assert_eq!(get(&prj, &Object::Device("-M2".into()), "device.speed"), "1450");
    prj.devices.get_mut("-M2").unwrap().values.insert("gone.key".into(), "1".into());
    assert_eq!(undefined_values(&prj, &Object::Device("-M2".into())), vec![("gone.key".to_string(), "1".to_string())]);
}
