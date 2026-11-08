use super::*;

#[test]
fn imported_junction_connects_paths_without_becoming_a_device_endpoint() {
    use crate::symlib::{UserPin, UserSymbol};
    let mut prj = Project::default();
    prj.symbols.push(UserSymbol {
        key: "user:junction".into(), func: Func::Junction,
        pins: vec![UserPin { pos: p(0., 0.), label: "a".into(), internal: false }, UserPin { pos: p(5., 0.), label: "a".into(), internal: false }],
        bridges: vec![(0, 1)], ..Default::default()
    });
    prj.symbols.push(UserSymbol {
        key: "user:end".into(), pins: vec![UserPin { pos: p(0., 0.), label: "1".into(), internal: false }], ..Default::default()
    });
    prj.pages[0].symbols = vec![
        SymbolInst { id: 1, sym: "user:end".into(), tag: "-A1".into(), pos: p(10., 20.), ..Default::default() },
        SymbolInst { id: 2, sym: "user:junction".into(), pos: p(20., 20.), ..Default::default() },
        SymbolInst { id: 3, sym: "user:end".into(), tag: "-A2".into(), pos: p(40., 20.), ..Default::default() },
    ];
    prj.pages[0].wires = vec![
        crate::model::Wire { id: 4, a: p(10., 20.), b: p(20., 20.), ..Default::default() },
        crate::model::Wire { id: 5, a: p(25., 20.), b: p(40., 20.), ..Default::default() },
    ];
    let nets = Netlist::build(&prj, true);
    assert_eq!(nets.pin_net.get(&(1, 0)), nets.pin_net.get(&(3, 0)));
    assert!(!nets.pin_net.contains_key(&(2, 0)));
    assert!(nets.nets.iter().any(|net| net.pins.len() == 2));
}
use crate::geom::p;
use crate::model::{SymbolInst, Wire};

fn sym(id: u64, key: &str, pos: P, tag: &str, label: &str) -> SymbolInst {
    let def = crate::symbols::builtin(key).unwrap();
    SymbolInst {
        id,
        sym: key.into(),
        pos,
        tag: tag.into(),
        label: label.into(),
        pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn wire_connects_pins_and_potential_names_net() {
    let mut prj = Project::default();
    let page = &mut prj.pages[0];
    page.symbols.push(sym(10, "pot_24", p(100.0, 40.0), "", "+24V"));
    page.symbols.push(sym(11, "coil", p(100.0, 100.0), "-K1", ""));
    // potential pin at (100,44), coil A1 at (100,96)
    page.wires.push(Wire { id: 20, a: p(100.0, 44.0), b: p(100.0, 96.0), ..Default::default() });
    let nl = Netlist::build(&prj, true);
    let n1 = nl.pin_net[&(11, 0)];
    assert_eq!(nl.nets[n1].name, "+24V");
    assert_eq!(nl.wire_net[&20], n1);
    // A2 is unconnected
    assert!(nl.unconnected.iter().any(|&(_, id, pin)| id == 11 && pin == 1));
}

#[test]
fn t_junction_connects() {
    let mut prj = Project::default();
    let page = &mut prj.pages[0];
    page.wires.push(Wire { id: 1, a: p(0.0, 0.0), b: p(40.0, 0.0), ..Default::default() });
    page.wires.push(Wire { id: 2, a: p(20.0, 0.0), b: p(20.0, 20.0), ..Default::default() });
    let nl = Netlist::build(&prj, true);
    assert_eq!(nl.wire_net[&1], nl.wire_net[&2]);
    assert_eq!(junctions(&prj, &prj.pages[0]).len(), 1);
}

#[test]
fn devices_in_the_cabinet_are_chained_before_field_devices() {
    let prj = crate::sample::demo_project();
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    let pairs: Vec<(String, String)> = connections(&prj, &nl, &open).into_iter().map(|c| (pin_designation(&prj, c.a), pin_designation(&prj, c.b))).collect();
    // 0 V: the two door lamps are bridged in the cabinet, one wire leaves for the valve.
    assert!(pairs.contains(&("-P1:X2".into(), "-P2:X2".into())), "{pairs:?}");
    assert!(pairs.contains(&("-P2:X2".into(), "-Y1:A2".into())));
    assert!(!pairs.iter().any(|(a, b)| a == "-Y1:A2" || (b == "-Y1:A2" && a == "-P1:X2")));
}

/// Pseudo-random wires (straight and diagonal, sharing end points): the row / column
/// shortcut of `junctions` finds the same dots as testing every end against every wire.
#[test]
fn junctions_match_the_exhaustive_count() {
    let mut page = Page::new(1, "1", "", PageKind::Schematic);
    let mut seed = 12345u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) % 13) as f32 * 4.0
    };
    for id in 1..=120u64 {
        let a = p(next(), next());
        let b = match id % 3 {
            0 => p(a.x, next()),
            1 => p(next(), a.y),
            _ => p(next(), next()),
        };
        page.wires.push(Wire { id, a, b, ..Default::default() });
    }
    let mut expected: Vec<(i64, i64)> = wire_ends(&page)
        .values()
        .filter(|(q, n)| n + 2 * page.wires.iter().filter(|w| on_segment_interior(*q, w.a, w.b)).count() >= 3)
        .map(|(q, _)| q.key())
        .collect();
    let mut got: Vec<(i64, i64)> = junctions(&Project::default(), &page).iter().map(|q| q.key()).collect();
    expected.sort();
    got.sort();
    assert!(expected.len() > 5, "{expected:?}");
    assert_eq!(got, expected);
}

#[test]
fn wire_ends_touching_nothing_are_open() {
    let mut prj = Project::default();
    let page = &mut prj.pages[0];
    page.symbols.push(sym(11, "coil", p(100.0, 100.0), "-K1", ""));
    // From A1 (100,96) up into empty space, a branch ending on that wire.
    page.wires.push(Wire { id: 1, a: p(100.0, 96.0), b: p(100.0, 60.0), ..Default::default() });
    page.wires.push(Wire { id: 2, a: p(60.0, 80.0), b: p(100.0, 80.0), ..Default::default() });
    page.wires.push(Wire { id: 3, a: p(60.0, 80.0), b: p(60.0, 40.0), ..Default::default() });
    let open = open_wire_ends(&prj, &prj.pages[0]);
    assert_eq!(open, vec![(1, p(100.0, 60.0)), (3, p(60.0, 40.0))]);
}

#[test]
fn a_tap_with_its_own_cross_section_keeps_it() {
    // The bars of a busbar (300 mm²) from the supply to -K1, a tap off them (35 mm²) to
    // -K2: the connection to -K2 has the tap's cross-section, not the net's.
    let mut prj = Project::default();
    let page = &mut prj.pages[0];
    page.symbols.push(sym(10, "pot_24", p(100.0, 40.0), "", "+24V"));
    page.symbols.push(sym(11, "coil", p(100.0, 100.0), "-K1", ""));
    page.symbols.push(sym(12, "coil", p(200.0, 100.0), "-K2", ""));
    page.wires.push(Wire { id: 20, a: p(100.0, 44.0), b: p(100.0, 96.0), cross_section: "300 mm²".into(), ..Default::default() });
    page.wires.push(Wire { id: 21, a: p(100.0, 70.0), b: p(200.0, 70.0), cross_section: "35 mm²".into(), ..Default::default() });
    page.wires.push(Wire { id: 22, a: p(200.0, 70.0), b: p(200.0, 96.0), cross_section: "35 mm²".into(), ..Default::default() });
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    let conns = connections(&prj, &nl, &open);
    assert_eq!(conns.len(), 1);
    assert_eq!((pin_designation(&prj, conns[0].a), pin_designation(&prj, conns[0].b)), ("-K1:A1".to_string(), "-K2:A1".to_string()));
    assert_eq!(conns[0].cross_section, "35 mm²");
    assert_eq!(crate::potentials::props_of(&prj, &nl, &open.nets[0]).cross_section, "300 mm²", "the net's own");
}

#[test]
fn potentials_and_interruptions_without_pins_name_nothing() {
    // User symbols may have the function of a potential without a connection point.
    let mut prj = Project::default();
    for (key, func) in [("user:pot", Func::Potential), ("user:int", Func::Interruption)] {
        prj.symbols.push(crate::model::symlib::UserSymbol { key: key.into(), func, ..Default::default() });
    }
    let page = &mut prj.pages[0];
    page.symbols.push(SymbolInst { id: 10, sym: "user:pot".into(), pos: p(40.0, 40.0), label: "L1".into(), ..Default::default() });
    page.symbols.push(SymbolInst { id: 11, sym: "user:int".into(), pos: p(80.0, 40.0), label: "A".into(), ..Default::default() });
    let nl = Netlist::build(&prj, true);
    assert!(nl.nets.iter().all(|n| !n.named));
    let _ = crate::analysis::Analysis::build(&prj);
}
#[test]
fn internal_bridge_attachments_connect_without_becoming_device_endpoints() {
    let mut project = Project::default();
    let definition = crate::symlib::UserSymbol {
        key: "user:bridge".into(),
        pins: vec![
            crate::symlib::UserPin { pos: p(0.,0.), label: "1".into(), internal:false },
            crate::symlib::UserPin { pos: p(4.,0.), label: String::new(), internal:true },
            crate::symlib::UserPin { pos: p(8.,0.), label: "2".into(), internal:false },
        ],
        bridges:vec![(0,1),(1,2)], ..Default::default()
    };
    project.pages[0].symbols.push(SymbolInst { id:2,sym:definition.key.clone(),..Default::default() });
    project.symbols.push(definition);
    let nets=Netlist::build(&project,true);
    assert_eq!(nets.pin_net[&(2,0)],nets.pin_net[&(2,2)]);
    assert!(!nets.pin_net.contains_key(&(2,1)));
}
