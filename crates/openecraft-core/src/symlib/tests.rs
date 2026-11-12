use super::*;

#[test]
fn symbol_snapshots_follow_definition_edits_without_comparing_drawings() {
    let mut prj = crate::model::Project::default();
    let builtins = prj.symbol_table();
    assert!(builtins.same_snapshot(&prj.symbol_table()));
    prj.symbols.push(UserSymbol { key: "user:snapshot".into(), ..Default::default() });
    let before = prj.symbol_table();
    assert!(!before.same_snapshot(&builtins));
    assert!(before.same_snapshot(&before.clone()));
    assert!(before.same_snapshot(&prj.clone().symbol_table()));
    prj.symbols[0].name = "Edited definition".into();
    assert!(!before.same_snapshot(&prj.symbol_table()));
    assert!(prj.symbol_table().same_snapshot(&prj.symbol_table()));
}

#[test]
fn imported_secondary_function_belongs_to_its_master_without_a_contact_kind() {
    let mut prj = Project::default();
    for (key, func) in [("user:master", Func::Main { mirror: false }), ("user:slave", Func::Auxiliary)] {
        prj.symbols.push(UserSymbol { key: key.into(), func, ..Default::default() });
        let id = prj.pages[0].symbols.len() as u64 + 1;
        prj.pages[0].symbols.push(crate::model::SymbolInst { id, sym: key.into(), tag: "-A1".into(), ..Default::default() });
    }
    let index = crate::index::Index::build(&prj);
    assert_eq!(index.devices["-A1"].mains.len(), 1);
    assert_eq!(index.devices["-A1"].auxs.len(), 1);
    assert!(prj.symbol("user:slave").unwrap().contact_kind().is_none());
}

#[test]
fn imported_symbol_domain_keeps_fluid_ports_out_of_electrical_nets() {
    use crate::model::{SymbolInst, Wire};
    let mut prj = Project::default();
    for (key, domain, x) in [("user:fluid", SymbolDomain::Fluid, 20.), ("user:electrical", SymbolDomain::Electrical, 30.)] {
        prj.symbols.push(UserSymbol {
            key: key.into(), domain,
            pins: vec![UserPin { pos: p(0., 0.), label: "1".into(), internal: false }],
            ..Default::default()
        });
        prj.pages[0].symbols.push(SymbolInst { id: x as u64, sym: key.into(), pos: p(x, 20.), ..Default::default() });
    }
    prj.pages[0].wires.push(Wire { id: 50, a: p(20., 20.), b: p(30., 20.), ..Default::default() });
    assert!(crate::netlist::Netlist::build(&prj, true).nets.iter().any(|net| net.mixed));
    let before = fingerprint(&prj.symbols);
    prj.symbols[0].domain = SymbolDomain::Electrical;
    assert_ne!(before, fingerprint(&prj.symbols));
    assert!(!prj.symbol("user:fluid").unwrap().is_fluid());
    let old: UserSymbol = serde_json::from_str(r#"{"key":"user:old"}"#).unwrap();
    assert_eq!(old.domain, SymbolDomain::Electrical);
}

#[test]
fn embedded_bitmap_roundtrips_moves_and_invalidates_symbol_cache() {
    let mut symbol = UserSymbol { key: "user:bitmap".into(), ..Default::default() };
    symbol.prims.push(Prim::Image {
        name: "sample.png".into(), data: String::new().into(),
        corners: [p(0., 0.), p(4., 0.), p(4., 6.), p(0., 6.)],
    });
    let first = fingerprint(&[symbol.clone()]);
    let json = serde_json::to_string(&symbol).unwrap();
    assert_eq!(serde_json::from_str::<UserSymbol>(&json).unwrap(), symbol);
    assert_eq!(prim_distance(&symbol.prims[0], p(2., 2.)), 0.);
    translate(&mut symbol.prims[0], p(10., 20.));
    assert_ne!(first, fingerprint(&[symbol.clone()]));
    let bounds = to_def(&symbol).bounds;
    assert!(bounds.contains(p(10., 20.)));
    assert!(bounds.contains(p(14., 26.)));
}
use crate::index::Index;
use crate::model::{Page, PageKind, SymbolInst};
use crate::netlist::Netlist;
use crate::analysis::Analysis;

fn box_symbol(key: &str) -> UserSymbol {
    UserSymbol {
        key: key.into(),
        name: "Test box".into(),
        letter: "A".into(),
        prims: vec![Prim::Poly { pts: vec![p(-4.0, -4.0), p(4.0, -4.0), p(4.0, 4.0), p(-4.0, 4.0)], closed: true, fill: false }],
        pins: vec![UserPin { pos: p(0.0, -8.0), label: "1".into(), internal: false }, UserPin { pos: p(0.0, 8.0), label: "2".into(), internal: false }],
        ..Default::default()
    }
}

#[test]
fn user_symbols_are_looked_up_per_project() {
    let mut prj = crate::model::Project::default();
    let u = box_symbol("user:test_table");
    prj.symbols.push(u.clone());
    let def = prj.symbol("user:test_table").expect("found");
    assert_eq!(def.category, Category::User);
    assert_eq!((def.pins.len(), def.library), (2, "User"));
    // Another project with the same key has a definition of its own.
    let mut other = crate::model::Project::default();
    let mut v = u.clone();
    v.pins.push(UserPin { pos: p(8.0, 8.0), label: "3".into(), internal: false });
    other.symbols.push(v);
    assert_eq!(other.symbol("user:test_table").unwrap().pins.len(), 3);
    assert_eq!(prj.symbol("user:test_table").unwrap().pins.len(), 2);
    // Edits are seen at once, removed symbols are gone, built-in ones stay.
    prj.symbols[0].pins.push(UserPin { pos: p(8.0, 8.0), label: "3".into(), internal: false });
    assert_eq!(prj.symbol("user:test_table").unwrap().pins.len(), 3);
    prj.symbols.clear();
    assert!(prj.symbol("user:test_table").is_none());
    assert!(prj.symbol("coil").is_some());
    // Keys without the prefix never reach the user table.
    assert!(prj.symbol_table().get("test_table").is_none());
}

#[test]
fn drawing_snapshots_reuse_definitions_until_an_edit() {
    let mut prj = Project::default();
    prj.symbols.push(box_symbol("user:drive"));
    let before = prj.clone();
    let table = before.symbol_table().user.unwrap();
    assert!(before.symbols.same_snapshot(&prj.symbols));
    assert!(Arc::ptr_eq(&table, &prj.symbol_table().user.unwrap()));

    prj.symbols[0].pins[0].label = "X137.13".into();
    assert!(!before.symbols.same_snapshot(&prj.symbols));
    assert_eq!(prj.symbol("user:drive").unwrap().pins[0].label, "X137.13");
    assert_eq!(before.symbol("user:drive").unwrap().pins[0].label, "1");
    assert_eq!(table["user:drive"].pins[0].label, "1");
}

#[test]
fn keys_are_unique_slugs() {
    let mut list = vec![];
    let k = new_key(&list, "Motor starter (3~)");
    assert_eq!(k, "user:motor_starter_3");
    list.push(UserSymbol { key: k.clone(), ..Default::default() });
    assert_eq!(new_key(&list, "Motor starter (3~)"), "user:motor_starter_3_2");
    assert_eq!(new_key(&list, "äöü"), "user:symbol");
}

#[test]
fn copy_of_builtin_keeps_function_and_pins() {
    let coil = symbols::builtin("coil").unwrap();
    let u = from_def(coil, &[]);
    assert!(u.key.starts_with(PREFIX));
    assert_eq!(u.func, coil.func);
    assert_eq!(u.pins.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), vec!["A1", "A2"]);
    assert_eq!(u.prims, coil.prims);
    assert!(problems(&u).is_empty());
}

#[test]
fn problems_are_reported() {
    let mut u = box_symbol("user:p");
    assert!(problems(&u).is_empty());
    u.pins[1].pos = p(1.0, 8.0);
    u.pins.push(UserPin { pos: p(0.0, -8.0), label: "3".into(), internal: false });
    let msgs = problems(&u);
    assert!(msgs.iter().any(|m| m.contains("not on the")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("lies on another")), "{msgs:?}");
    u.pins.clear();
    assert!(problems(&u).iter().any(|m| m.contains("no connection points")));
}

#[test]
fn hit_test_prefers_pins_and_moves_elements() {
    let mut u = box_symbol("user:h");
    u.prims.push(Prim::Line { a: p(0.0, -8.0), b: p(0.0, -4.0), dashed: false });
    // The pin lies on the line: the pin wins.
    assert_eq!(hit(&u, p(0.1, -8.0), 0.5), Some(Hit::Pin(0)));
    assert_eq!(hit(&u, p(0.1, -6.0), 0.5), Some(Hit::Prim(1)));
    assert_eq!(hit(&u, p(4.2, 0.0), 0.5), Some(Hit::Prim(0)));
    assert_eq!(hit(&u, p(20.0, 20.0), 0.5), None);
    translate(&mut u.prims[1], p(2.0, 0.0));
    assert_eq!(u.prims[1], Prim::Line { a: p(2.0, -8.0), b: p(2.0, -4.0), dashed: false });
    let text = Prim::Text { pos: p(0.0, 0.0), text: "ABC".into(), size: 2.0, anchor: Anchor::Left, angle: None, color:None };
    assert_eq!(prim_distance(&text, p(1.0, 0.0)), 0.0);
    assert!(prim_distance(&text, p(-3.0, 0.0)) > 2.9);
}

#[test]
fn removing_a_pin_fixes_bridges() {
    let mut u = box_symbol("user:b");
    u.pins.push(UserPin { pos: p(8.0, 8.0), label: "3".into(), internal: false });
    u.bridges = vec![(0, 1), (1, 2)];
    u.fluid_ports.insert(0, crate::fluid::PortKind::Pressure);
    u.fluid_ports.insert(2, crate::fluid::PortKind::Passage);
    remove_pin(&mut u, 0);
    assert_eq!(u.pins.len(), 2);
    assert_eq!(u.bridges, vec![(0, 1)]);
    assert_eq!(u.fluid_ports, [(1, crate::fluid::PortKind::Passage)].into_iter().collect());
    assert_eq!(next_pin_label(&u), "4");
}

#[test]
fn merge_adds_and_replaces() {
    let mut list = vec![box_symbol("user:a")];
    let mut changed = box_symbol("user:a");
    changed.name = "Changed".into();
    let foreign = UserSymbol { key: "coil".into(), name: "Foreign".into(), ..Default::default() };
    let (added, replaced) = merge(&mut list, SymbolLibrary { symbols: vec![changed, foreign] });
    assert_eq!((added, replaced), (1, 1));
    assert_eq!(list[0].name, "Changed");
    // Keys of other programs are made user keys (no clash with built-in symbols).
    assert_eq!(list[1].key, "user:foreign");
}

#[test]
fn placed_user_symbols_work_like_built_in_ones() {
    let mut prj = Project { pages: vec![Page::new(1, "1", "", PageKind::Schematic)], next_id: 10, ..Default::default() };
    prj.symbols.push(box_symbol("user:test_placed"));
    prj.pages[0].symbols.push(SymbolInst {
        id: 2,
        sym: "user:test_placed".into(),
        pos: p(40.0, 40.0),
        tag: "-A1".into(),
        ..Default::default()
    });
    // Saved and loaded: the symbol travels with the project.
    let json = serde_json::to_string(&prj).unwrap();
    let mut back: Project = serde_json::from_str(&json).unwrap();
    back.normalize();
    assert_eq!(back.pages[0].symbols[0].pins, vec!["1", "2"]);
    let idx = Index::build(&back);
    assert_eq!(idx.devices["-A1"].mains.len(), 1);
    let nl = Netlist::build(&back, true);
    assert_eq!(nl.unconnected.len(), 2);
    let an = Analysis::checked(&back);
    assert!(!an.issues.iter().any(|i| i.message().contains("Unknown symbol")), "{:?}", an.issues.iter().map(|i| i.message()).collect::<Vec<_>>());
}

#[test]
fn library_files_hold_one_or_many_symbols() {
    let u = UserSymbol { key: "u_lamp".into(), name: "Lamp".into(), ..Default::default() };
    let lib = SymbolLibrary { symbols: vec![u.clone()] };
    assert_eq!(SymbolLibrary::from_json(&serde_json::to_string(&lib).unwrap()).unwrap().symbols, lib.symbols);
    assert_eq!(SymbolLibrary::from_json(&serde_json::to_string(&u).unwrap()).unwrap().symbols, lib.symbols);
    assert!(SymbolLibrary::from_json("42").is_err());
}

#[test]
fn scaled_keys_carry_the_factor_of_library_symbols_only() {
    assert_eq!(scaled_key("user:ema:x", 0.5), "user:ema:x@0.5");
    assert_eq!(scaled_key("user:ema:x@0.5", 2.0), "user:ema:x@2", "the factor is replaced");
    assert_eq!(scaled_key("user:ema:x@0.5", 1.0), "user:ema:x");
    assert_eq!(scaled_key("user:x", 0.333333), "user:x@0.333");
    assert_eq!(scaled_key("user:x", 100.0), "user:x@10", "limited to the range");
    assert_eq!(scaled_key("coil", 2.0), "coil", "built-in symbols keep their size");
    assert_eq!(scale_of("user:x@0.5"), ("user:x", 0.5));
    assert_eq!(scale_of("user:x"), ("user:x", 1.0));
    assert_eq!(scale_of("user:x@abc"), ("user:x@abc", 1.0));
    assert_eq!(scale_of("coil@2"), ("coil@2", 1.0));
}

/// A project with a library symbol (connection points 16 mm apart, a text of 2.5 mm)
/// placed at (100, 100) with a wire at each connection point.
fn scaled_project() -> Project {
    let mut prj = Project::default();
    prj.symbols.push(UserSymbol {
        key: "user:imported".into(),
        func: Func::Main { mirror: false },
        prims: vec![
            Prim::Line { a: p(0.0, -8.0), b: p(0.0, 8.0), dashed: false },
            Prim::Circle { c: p(0.0, 0.0), r: 4.0, fill: false, color: None },
            Prim::Text { pos: p(6.0, 0.0), text: "M".into(), size: 2.5, anchor: Anchor::Left, angle: None, color: None },
        ],
        pins: vec![UserPin { pos: p(0.0, -8.0), label: "1".into(), internal: false }, UserPin { pos: p(0.0, 8.0), label: "2".into(), internal: false }],
        ..Default::default()
    });
    let pg = &mut prj.pages[0];
    pg.symbols.push(crate::model::SymbolInst { id: 1, sym: "user:imported".into(), tag: "-M1".into(), pos: p(100.0, 100.0), pins: vec!["1".into(), "2".into()], ..Default::default() });
    pg.wires.push(crate::model::Wire { id: 2, a: p(100.0, 60.0), b: p(100.0, 92.0), ..Default::default() });
    pg.wires.push(crate::model::Wire { id: 3, a: p(100.0, 108.0), b: p(100.0, 140.0), ..Default::default() });
    prj.next_id = 10;
    prj
}

#[test]
fn a_scaled_symbol_keeps_the_size_of_its_texts() {
    let prj = scaled_project();
    let half = prj.symbol("user:imported@0.5").unwrap();
    assert_eq!(half.key, "user:imported@0.5");
    assert_eq!(half.pins.iter().map(|x| x.pos).collect::<Vec<_>>(), [p(0.0, -4.0), p(0.0, 4.0)]);
    assert!(matches!(half.prims[1], Prim::Circle { r, .. } if r == 2.0));
    assert!(matches!(&half.prims[2], Prim::Text { pos, size, .. } if *pos == p(3.0, 0.0) && *size == 2.5), "the text moves but keeps its size");
    assert!(half.bounds.height() < prj.symbol("user:imported").unwrap().bounds.height());
    assert!(prj.symbol("user:missing@0.5").is_none());
    assert!(prj.symbol("coil@0.5").is_none());
}

#[test]
fn wires_follow_the_connection_points_of_a_scaled_symbol() {
    let mut prj = scaled_project();
    crate::ops::set_symbol_scale(&mut prj, 0, 1, 2.0).unwrap();
    let pg = &prj.pages[0];
    assert_eq!(pg.symbols[0].sym, "user:imported@2");
    assert_eq!((pg.wires[0].a, pg.wires[0].b), (p(100.0, 60.0), p(100.0, 84.0)));
    assert_eq!((pg.wires[1].a, pg.wires[1].b), (p(100.0, 116.0), p(100.0, 140.0)));
    // Still one placed instance of the library symbol; back to its own size.
    assert_eq!(usage(&prj, "user:imported"), 1);
    crate::ops::set_symbol_scale(&mut prj, 0, 1, 1.0).unwrap();
    assert_eq!(prj.pages[0].symbols[0].sym, "user:imported");
    assert_eq!(prj.pages[0].wires[0].b, p(100.0, 92.0));
    // The netlist still connects both wires to the motor.
    crate::ops::set_symbol_scale(&mut prj, 0, 1, 0.5).unwrap();
    let nl = crate::netlist::Netlist::build(&prj, true);
    assert_eq!(nl.nets.iter().filter(|n| n.pins.iter().any(|x| x.sym == 0)).count(), 2);
    // Built-in symbols are not scaled.
    prj.pages[0].symbols.push(crate::model::SymbolInst { id: 4, sym: "coil".into(), ..Default::default() });
    assert!(crate::ops::set_symbol_scale(&mut prj, 0, 4, 2.0).is_err());
}

#[test]
fn dragging_a_factor_keeps_few_scaled_symbols() {
    let prj = scaled_project();
    let table = prj.symbol_table();
    for i in 0..200 {
        assert!(table.get(&scaled_key("user:imported", 1.0 + i as f32 / 100.0)).is_some());
    }
    assert!(table.scaled.lock().unwrap().len() <= MAX_SCALED);
}
