use super::*;
use crate::geom::p;
use crate::model::{Page, PageKind, Project, SymbolInst, Wire};
use crate::netlist::{connections, pin_designation, Netlist, Scans};
use crate::ops;
use std::collections::BTreeSet;

fn sym(prj: &mut Project, page: usize, key: &str, pos: P, tag: &str, label: &str) -> u64 {
    let def = crate::symbols::builtin(key).unwrap();
    let id = prj.new_id();
    prj.pages[page].symbols.push(SymbolInst {
        id,
        sym: key.into(),
        pos,
        tag: tag.into(),
        label: label.into(),
        pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
        ..Default::default()
    });
    id
}

fn wire(prj: &mut Project, page: usize, a: P, b: P) -> u64 {
    let id = prj.new_id();
    prj.pages[page].wires.push(Wire { id, a, b, ..Default::default() });
    id
}

fn tee(prj: &mut Project, page: usize, pos: P, dir: TeeDir, variant: u8) -> u64 {
    let id = prj.new_id();
    prj.pages[page].junctions.push(Junction { id, pos, dir, variant, ..Default::default() });
    id
}

/// The connections as (source, target) designations in their order.
fn pairs(prj: &Project) -> Vec<(String, String)> {
    let nl = Netlist::build(prj, true);
    let open = Netlist::build(prj, false);
    connections(prj, &nl, &open).into_iter().map(|c| (pin_designation(prj, c.a), pin_designation(prj, c.b))).collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

/// Three coils around the node (100, 60): -K1 to the left, -K2 to the right, -K3
/// below (the stem points down). The coil's A1 lies 4 mm above its position.
fn three_devices() -> Project {
    let mut prj = Project::default();
    sym(&mut prj, 0, "coil", p(60.0, 70.0), "-K1", "");
    sym(&mut prj, 0, "coil", p(140.0, 70.0), "-K2", "");
    sym(&mut prj, 0, "coil", p(100.0, 90.0), "-K3", "");
    wire(&mut prj, 0, p(100.0, 60.0), p(60.0, 60.0));
    wire(&mut prj, 0, p(60.0, 60.0), p(60.0, 66.0));
    wire(&mut prj, 0, p(100.0, 60.0), p(140.0, 60.0));
    wire(&mut prj, 0, p(140.0, 60.0), p(140.0, 66.0));
    wire(&mut prj, 0, p(100.0, 60.0), p(100.0, 86.0));
    prj
}

#[test]
fn joined_busbar_ordering_keeps_explicit_junction_target_tracing() {
    use crate::panel::{ItemKind, PanelItem};
    let mut prj = three_devices();
    for (name, x, tag) in [("+A", 300., "-K2"), ("+B", 0., "-K3")] {
        let mut enclosure = crate::panel::new_enclosure(&mut prj, "", name);
        enclosure.items.push(PanelItem {
            id: prj.new_id(), kind: ItemKind::Busbar, x, y: 100., w: 250., h: 90., d: 40., ..Default::default()
        });
        enclosure.items.push(PanelItem {
            id: prj.new_id(), kind: ItemKind::Device, tag: tag.into(), x: x + 50., y: 80., w: 50., h: 200., z: 40., ..Default::default()
        });
        prj.layout.enclosures.push(enclosure);
    }
    let id = prj.new_id();
    prj.layout.enclosures[0].items.push(PanelItem {
        id, kind: ItemKind::Device, tag: "-K1".into(), x: 100., y: 300., w: 50., h: 100., ..Default::default()
    });
    crate::panel::bay_right_of(&mut prj, 1, 0);
    assert_eq!(pairs(&prj), [pair("-K1:A1", "-K3:A1"), pair("-K3:A1", "-K2:A1")]);
    tee(&mut prj, 0, p(100., 60.), TeeDir::Down, 3);
    assert_eq!(pairs(&prj), [pair("-K3:A1", "-K1:A1"), pair("-K3:A1", "-K2:A1")]);
}

#[test]
fn square_wire_drag_preserves_directed_junction_tracing() {
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100., 60.), TeeDir::Down, 1);
    let original = prj.clone();
    let expected = pairs(&prj);
    let selected: BTreeSet<_> = prj.pages[0].symbols.iter().filter(|s| s.tag == "-K2").map(|s| s.id).collect();
    let mut moving = crate::routing::Moving::new(&prj, 0, &selected);
    for delta in [p(8., 16.), p(-4., -8.)] {
        moving.step(&mut prj, 0, &selected, delta);
        assert_eq!(pairs(&prj), expected);
        assert_eq!(prj.pages[0].junctions[0].pos, p(100., 60.));
        assert!(fits(&prj, &prj.pages[0], &prj.pages[0].junctions[0]));
        assert!(prj.pages[0].wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y));
    }
    let mut direct = original;
    crate::routing::move_selection(&mut direct, 0, &selected, p(4., 8.));
    assert_eq!(prj.pages[0], direct.pages[0], "a drag and a single move must produce the same routing");
    let all: BTreeSet<_> = ops::all_ids(&prj.pages[0]).into_iter().filter(|id| *id != prj.pages[0].junctions[0].id).collect();
    crate::routing::move_selection(&mut prj, 0, &all, p(8., 4.));
    assert_eq!(prj.pages[0].junctions[0].pos, p(108., 64.));
    assert_eq!(pairs(&prj), expected);
}

#[test]
fn without_a_junction_the_devices_are_chained_in_schematic_order() {
    let prj = three_devices();
    assert_eq!(pairs(&prj), [pair("-K1:A1", "-K3:A1"), pair("-K3:A1", "-K2:A1")]);
    assert_eq!(arms(&prj.pages[0], p(100.0, 60.0)), Some(vec![TeeDir::Left, TeeDir::Right, TeeDir::Down]));
    assert_eq!(stem_of(&[TeeDir::Left, TeeDir::Right, TeeDir::Down]), Some(TeeDir::Down));
    assert_eq!(stem_of(&[TeeDir::Left, TeeDir::Right]), None);
}

#[test]
fn the_variant_names_the_source_and_the_order_of_the_targets() {
    let expected = [
        // 1: from the left arm straight to the right arm, then to the stem.
        (1, [pair("-K1:A1", "-K2:A1"), pair("-K1:A1", "-K3:A1")]),
        (2, [pair("-K2:A1", "-K1:A1"), pair("-K2:A1", "-K3:A1")]),
        (3, [pair("-K3:A1", "-K1:A1"), pair("-K3:A1", "-K2:A1")]),
        (4, [pair("-K3:A1", "-K2:A1"), pair("-K3:A1", "-K1:A1")]),
    ];
    for (variant, want) in expected {
        let mut prj = three_devices();
        tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, variant);
        assert_eq!(pairs(&prj), want, "variant {variant}");
        // The dot representation keeps the routing and the dot.
        prj.pages[0].junctions[0].dot = true;
        assert_eq!(pairs(&prj), want, "variant {variant} as dot");
        assert_eq!(crate::netlist::junctions(&prj, &prj.pages[0]).len(), 1);
    }
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 3);
    assert_eq!(describe(&prj.pages[0].junctions[0]), "Source below, first target on the left, second target on the right");
}

#[test]
fn turning_the_drawing_keeps_the_wires_of_every_direction() {
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 4);
    let want = pairs(&prj);
    let symbols = prj.symbol_table();
    let mut dirs = vec![TeeDir::Down];
    for _ in 0..3 {
        let all: BTreeSet<u64> = ops::all_ids(&prj.pages[0]).into_iter().collect();
        ops::rotate(&symbols, &mut prj.pages[0], &all);
        let j = &prj.pages[0].junctions[0];
        assert!(fits(&prj, &prj.pages[0], j), "{:?}", j.dir);
        dirs.push(j.dir);
        assert_eq!(pairs(&prj), want, "{:?}", j.dir);
    }
    dirs.sort();
    assert_eq!(dirs, TeeDir::ALL, "every direction once");
}

#[test]
fn mirroring_keeps_the_wires() {
    for variant in VARIANTS {
        let mut prj = three_devices();
        tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, variant);
        let want = pairs(&prj);
        // The drawing mirrored at x = 100 by hand, the junction by its method.
        let mut mirrored = three_devices();
        for s in &mut mirrored.pages[0].symbols {
            s.pos.x = 200.0 - s.pos.x;
            s.mirror = true;
        }
        for w in &mut mirrored.pages[0].wires {
            w.a.x = 200.0 - w.a.x;
            w.b.x = 200.0 - w.b.x;
        }
        let mut j = prj.pages[0].junctions[0].clone();
        j.mirror();
        mirrored.pages[0].junctions.push(j);
        assert_eq!(pairs(&mirrored), want, "variant {variant}");
    }
    // Turning: the first arm (left) of a stem pointing down comes to lie above,
    // which is the first arm again; after the second turn it lies to the right.
    let mut j = Junction { dir: TeeDir::Down, variant: 3, ..Default::default() };
    j.turn();
    assert_eq!((j.dir, j.variant), (TeeDir::Left, 3));
    j.turn();
    assert_eq!((j.dir, j.variant), (TeeDir::Up, 4));
    j.turn();
    assert_eq!((j.dir, j.variant), (TeeDir::Right, 4));
    j.turn();
    assert_eq!((j.dir, j.variant), (TeeDir::Down, 3));
    j.mirror();
    assert_eq!((j.dir, j.variant), (TeeDir::Down, 4));
    j.dir = TeeDir::Left;
    j.mirror();
    assert_eq!((j.dir, j.variant), (TeeDir::Right, 4));
}

#[test]
fn a_junction_that_does_not_fit_its_wires_is_ignored_and_reported() {
    let mut prj = three_devices();
    let plain = pairs(&prj);
    // On a plain wire, with the wrong direction, and on a connection point.
    tee(&mut prj, 0, p(80.0, 60.0), TeeDir::Down, 3);
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Up, 3);
    tee(&mut prj, 0, p(60.0, 66.0), TeeDir::Down, 3);
    assert_eq!(pairs(&prj), plain);
    let open = Netlist::build(&prj, false);
    let msgs: Vec<String> = problems(&prj, &open).into_iter().map(|i| i.message()).collect();
    assert_eq!(msgs.len(), 3, "{msgs:?}");
    assert!(msgs.iter().all(|m| m.contains("does not fit")), "{msgs:?}");
    // Two on one point.
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 3);
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 1);
    let open = Netlist::build(&prj, false);
    let msgs: Vec<String> = problems(&prj, &open).into_iter().map(|i| i.message()).collect();
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(msgs[0].contains("same point"));
}

#[test]
fn a_crossing_stays_unconnected() {
    let mut prj = Project::default();
    let h = wire(&mut prj, 0, p(60.0, 60.0), p(140.0, 60.0));
    let v = wire(&mut prj, 0, p(100.0, 20.0), p(100.0, 100.0));
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 3);
    assert_eq!(arms(&prj.pages[0], p(100.0, 60.0)).map(|a| a.len()), Some(4));
    assert!(!fits(&prj, &prj.pages[0], &prj.pages[0].junctions[0]));
    let nl = Netlist::build(&prj, true);
    assert_ne!(nl.wire_net[&h], nl.wire_net[&v]);
}

#[test]
fn moving_a_device_keeps_the_routing_and_the_junction_follows_its_wires() {
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 1);
    let want = pairs(&prj);
    let symbols = prj.symbol_table();
    // -K2 moves down: its wire stretches, the node and the junction stay.
    let k2: BTreeSet<u64> = prj.pages[0].symbols.iter().filter(|s| s.tag == "-K2").map(|s| s.id).collect();
    let attached = ops::attached_wire_ends(&symbols, &prj.pages[0], &k2);
    ops::translate(&mut prj.pages[0], &k2, &attached, p(0.0, 20.0));
    assert_eq!(prj.pages[0].junctions[0].pos, p(100.0, 60.0));
    assert_eq!(pairs(&prj), want);
    // Everything but the junction moves: the node moves, so the junction follows.
    let all: BTreeSet<u64> = ops::all_ids(&prj.pages[0]).into_iter().filter(|id| *id != prj.pages[0].junctions[0].id).collect();
    let attached = ops::attached_wire_ends(&symbols, &prj.pages[0], &all);
    ops::translate(&mut prj.pages[0], &all, &attached, p(8.0, 4.0));
    assert_eq!(prj.pages[0].junctions[0].pos, p(108.0, 64.0));
    assert_eq!(pairs(&prj), want);
    // Only the stem wire moves away: the node stays where the bar is.
    let stem: BTreeSet<u64> = prj.pages[0].wires.iter().filter(|w| w.a.x == w.b.x && w.a.x == 108.0).map(|w| w.id).collect();
    assert_eq!(stem.len(), 1);
    ops::translate(&mut prj.pages[0], &stem, &[], p(4.0, 0.0));
    assert_eq!(prj.pages[0].junctions[0].pos, p(108.0, 64.0));
}

#[test]
fn the_routing_passes_interruption_points() {
    let mut prj = three_devices();
    // The stem leads to an interruption point instead of -K3; -K3 sits on page 2.
    prj.pages[0].symbols.retain(|s| s.tag != "-K3");
    prj.pages[0].wires.retain(|w| !(w.a.x == 100.0 && w.b.x == 100.0));
    sym(&mut prj, 0, "intr", p(100.0, 80.0), "", "X");
    wire(&mut prj, 0, p(100.0, 60.0), p(100.0, 80.0));
    prj.pages.push(Page::new(2, "2", "", PageKind::Schematic));
    sym(&mut prj, 1, "intr", p(100.0, 40.0), "", "X");
    sym(&mut prj, 1, "coil", p(100.0, 60.0), "-K3", "");
    wire(&mut prj, 1, p(100.0, 40.0), p(100.0, 56.0));
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 3);
    assert_eq!(pairs(&prj), [pair("-K3:A1", "-K1:A1"), pair("-K3:A1", "-K2:A1")]);
}

/// A second junction at (140, 60) on the right arm: -K4 above it, -K5 below it, its
/// stem pointing left to the first junction.
fn two_junctions(v1: u8, v2: u8) -> Project {
    let mut prj = three_devices();
    prj.pages[0].symbols.retain(|s| s.tag != "-K2");
    prj.pages[0].wires.retain(|w| !(w.a.x == 140.0 && w.b.x == 140.0));
    sym(&mut prj, 0, "coil", p(140.0, 30.0), "-K4", "");
    sym(&mut prj, 0, "coil", p(140.0, 90.0), "-K5", "");
    wire(&mut prj, 0, p(140.0, 60.0), p(140.0, 34.0));
    wire(&mut prj, 0, p(140.0, 60.0), p(140.0, 86.0));
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, v1);
    tee(&mut prj, 0, p(140.0, 60.0), TeeDir::Left, v2);
    prj
}

#[test]
fn a_junction_hands_its_targets_on_to_the_source_that_reaches_it() {
    // -K3 is the source of the first junction; its second target is the second
    // junction's source, which hands on both its targets.
    let prj = two_junctions(3, 3);
    assert_eq!(pairs(&prj), [pair("-K3:A1", "-K1:A1"), pair("-K3:A1", "-K4:A2"), pair("-K3:A1", "-K5:A1")]);
    let open = Netlist::build(&prj, false);
    assert!(problems(&prj, &open).is_empty());
    // The second junction starts at -K4: -K3 reaches it at a target, so the wire
    // from -K3 ends at -K4, and -K4 goes on to -K5.
    let prj = two_junctions(3, 1);
    let got: BTreeSet<(String, String)> = pairs(&prj).into_iter().collect();
    assert_eq!(got, BTreeSet::from([pair("-K3:A1", "-K1:A1"), pair("-K3:A1", "-K4:A2"), pair("-K4:A2", "-K5:A1")]));
}

#[test]
fn sources_facing_each_other_wire_a_loop_that_is_reported() {
    // The first junction starts at its right arm, the second at its stem: the wire
    // between them has no device to start from, so every device gets wired to every
    // other one across it.
    let prj = two_junctions(2, 3);
    let got: BTreeSet<(String, String)> = pairs(&prj).into_iter().collect();
    assert_eq!(got, BTreeSet::from([pair("-K4:A2", "-K1:A1"), pair("-K5:A1", "-K1:A1"), pair("-K4:A2", "-K3:A1"), pair("-K5:A1", "-K3:A1")]));
    let open = Netlist::build(&prj, false);
    let msgs: Vec<String> = problems(&prj, &open).into_iter().map(|i| i.message()).collect();
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(msgs.iter().all(|m| m.contains("in a loop")), "{msgs:?}");
}

#[test]
fn the_connections_of_a_junction_are_those_of_its_net() {
    let mut prj = two_junctions(3, 3);
    // An unrelated connection elsewhere.
    sym(&mut prj, 0, "coil", p(40.0, 120.0), "-K6", "");
    sym(&mut prj, 0, "coil", p(40.0, 160.0), "-K7", "");
    wire(&mut prj, 0, p(40.0, 124.0), p(40.0, 156.0));
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    let conns = connections(&prj, &nl, &open);
    assert_eq!(conns.len(), 4);
    let of = connections_of(&prj, &open, &conns, 0, &prj.pages[0].junctions[1]);
    assert_eq!(of.len(), 3);
    assert!(of.iter().all(|&i| pin_designation(&prj, conns[i].a) == "-K3:A1"));
}

#[test]
fn files_keep_the_junctions_and_older_files_have_none() {
    let mut prj = three_devices();
    tee(&mut prj, 0, p(100.0, 60.0), TeeDir::Down, 2);
    let text = serde_json::to_string(&prj).unwrap();
    assert!(text.contains("\"junctions\""));
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(back.pages[0].junctions, prj.pages[0].junctions);
    let page: Page = serde_json::from_str(r#"{"id":1,"name":"1"}"#).unwrap();
    assert!(page.junctions.is_empty());
    assert!(!serde_json::to_string(&page).unwrap().contains("junctions"), "empty lists are left out");
    // Copy and paste keep the junction with the wires.
    let all: BTreeSet<u64> = ops::all_ids(&prj.pages[0]).into_iter().collect();
    let clip = ops::copy(&prj.pages[0], &all);
    assert_eq!(clip.junctions.len(), 1);
    prj.pages.push(Page::new(2, "2", "", PageKind::Schematic));
    let ids = ops::paste(&mut prj, 1, &clip, p(60.0, 60.0));
    assert_eq!(prj.pages[1].junctions.len(), 1);
    assert!(ids.contains(&prj.pages[1].junctions[0].id));
    let j = prj.pages[1].junctions[0].clone();
    assert!(fits(&prj, &prj.pages[1], &j), "pasted with its wires at {:?}", j.pos);
    ops::delete(&mut prj.pages[1], &BTreeSet::from([j.id]));
    assert!(prj.pages[1].junctions.is_empty());
}

#[test]
fn a_corner_fits_two_wires_at_a_right_angle_and_changes_no_routing() {
    let mut prj = three_devices();
    // The bend of -K1's wire at (60, 60): arms right and down.
    let want = pairs(&prj);
    let id = prj.new_id();
    prj.pages[0].junctions.push(Junction::corner(p(60.0, 60.0), TeeDir::Down, TeeDir::Right));
    prj.pages[0].junctions[0].id = id;
    let c = prj.pages[0].junctions[0].clone();
    assert_eq!(c.arms(), [TeeDir::Down, TeeDir::Right]);
    assert_eq!(corner_of(&arms(&prj.pages[0], p(60.0, 60.0)).unwrap()), Some((TeeDir::Down, TeeDir::Right)));
    assert!(fits(&prj, &prj.pages[0], &c));
    assert_eq!(pairs(&prj), want);
    assert_eq!(describe(&c), "Arms below and on the right");
    let open = Netlist::build(&prj, false);
    assert!(problems(&prj, &open).is_empty());
    // Wrong arms, or on the T node (three wires): reported.
    prj.pages[0].junctions[0] = Junction::corner(p(60.0, 60.0), TeeDir::Up, TeeDir::Right);
    prj.pages[0].junctions.push(Junction::corner(p(100.0, 60.0), TeeDir::Down, TeeDir::Left));
    let msgs: Vec<String> = problems(&prj, &open).into_iter().map(|i| i.message()).collect();
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(msgs.iter().all(|m| m.contains("Corner on page 1 does not fit")), "{msgs:?}");
    // Turning and mirroring keep the arms with their wires.
    let mut c = Junction::corner(p(0.0, 0.0), TeeDir::Down, TeeDir::Right);
    c.turn();
    assert_eq!(c.arms(), [TeeDir::Up, TeeDir::Left].into_iter().filter(|_| false).chain([TeeDir::Down, TeeDir::Left]).collect::<Vec<_>>(), "down turns to left, right turns to down");
    c.mirror();
    assert_eq!(c.arms(), [TeeDir::Down, TeeDir::Right]);
    // Only one corner is free of wires at a point: both arms are free.
    let mut page = Page::new(7, "7", "", PageKind::Schematic);
    page.junctions.push(Junction::corner(p(40.0, 40.0), TeeDir::Up, TeeDir::Left));
    assert_eq!(free_arms(&page), [(0, p(40.0, 40.0), TeeDir::Up), (0, p(40.0, 40.0), TeeDir::Left)]);
    page.wires.push(Wire { id: 1, a: p(40.0, 40.0), b: p(40.0, 20.0), ..Default::default() });
    assert_eq!(free_arms(&page), [(0, p(40.0, 40.0), TeeDir::Left)]);
}

/// Four coils around (100, 60): -K1 above (A2 at (100, 40)), -K2 below (A1 at
/// (100, 80)), -K3 to the left (A1 at (80, 60), turned), -K4 to the right (A2 at
/// (120, 60), turned); the two wires cross at (100, 60) without ending there.
fn four_devices() -> Project {
    let mut prj = Project::default();
    sym(&mut prj, 0, "coil", p(100.0, 36.0), "-K1", "");
    sym(&mut prj, 0, "coil", p(100.0, 84.0), "-K2", "");
    sym(&mut prj, 0, "coil", p(76.0, 60.0), "-K3", "");
    sym(&mut prj, 0, "coil", p(124.0, 60.0), "-K4", "");
    for s in prj.pages[0].symbols.iter_mut().filter(|s| s.tag == "-K3" || s.tag == "-K4") {
        s.rot = 1;
    }
    wire(&mut prj, 0, p(100.0, 40.0), p(100.0, 80.0));
    wire(&mut prj, 0, p(80.0, 60.0), p(120.0, 60.0));
    prj
}

#[test]
fn a_crossing_connects_the_wires_crossing_there_and_routes_them() {
    let prj = four_devices();
    // Without a symbol the wires cross unconnected.
    let nl = Netlist::build(&prj, true);
    assert_eq!(nl.nets.iter().filter(|n| n.pins.len() == 2).count(), 2, "two separate connections");
    assert!(crate::netlist::junctions(&prj, &prj.pages[0]).is_empty(), "no dot");
    let mut prj = prj;
    let id = prj.new_id();
    prj.pages[0].junctions.push(Junction { id, ..Junction::new(JunctionKind::Crossing, p(100.0, 60.0)) });
    let nl = Netlist::build(&prj, true);
    assert_eq!(nl.nets.iter().filter(|n| n.pins.len() == 4).count(), 1, "one connection of four devices");
    assert!(fits(&prj, &prj.pages[0], &prj.pages[0].junctions[0]));
    assert_eq!(crate::netlist::junctions(&prj, &prj.pages[0]).len(), 1, "the point of the crossing gets the connection dot");
    // Vertical line, variant 2 (EPLAN's example): from above to the bottom first and to
    // the left second; from below to the top first and to the right second.
    assert_eq!(pairs(&prj), [pair("-K1:A2", "-K2:A1"), pair("-K1:A2", "-K3:A1"), pair("-K2:A1", "-K4:A2")]);
    prj.pages[0].junctions[0].variant = 1;
    assert_eq!(pairs(&prj), [pair("-K1:A2", "-K2:A1"), pair("-K1:A2", "-K4:A2"), pair("-K2:A1", "-K3:A1")]);
    prj.pages[0].junctions[0].dir = TeeDir::Left;
    assert_eq!(pairs(&prj), [pair("-K3:A1", "-K4:A2"), pair("-K3:A1", "-K2:A1"), pair("-K4:A2", "-K1:A2")]);
    prj.pages[0].junctions[0].variant = 2;
    assert_eq!(pairs(&prj), [pair("-K3:A1", "-K4:A2"), pair("-K3:A1", "-K1:A2"), pair("-K4:A2", "-K2:A1")]);
    // The dot representation keeps the routing; no loop is reported.
    prj.pages[0].junctions[0].dot = true;
    assert_eq!(pairs(&prj).len(), 3);
    let open = Netlist::build(&prj, false);
    assert!(problems(&prj, &open).is_empty());
    assert_eq!(describe(&prj.pages[0].junctions[0]), "Straight through from on the left to on the right, across from on the left to above and from on the right to below");
    // Turning keeps the links with their wires; mirroring swaps the ends across.
    let mut c = Junction::new(JunctionKind::Crossing, p(0.0, 0.0));
    assert_eq!(c.links(), [(TeeDir::Up, TeeDir::Down), (TeeDir::Up, TeeDir::Left), (TeeDir::Down, TeeDir::Right)]);
    c.turn();
    assert_eq!(c.links(), [(TeeDir::Left, TeeDir::Right), (TeeDir::Left, TeeDir::Down), (TeeDir::Right, TeeDir::Up)]);
    assert_eq!((c.dir, c.variant), (TeeDir::Left, 1));
    // A half turn maps a crossing onto itself; mirroring swaps the ends across.
    c.turn();
    assert_eq!(c.links(), [(TeeDir::Up, TeeDir::Down), (TeeDir::Up, TeeDir::Left), (TeeDir::Down, TeeDir::Right)]);
    assert_eq!((c.dir, c.variant), (TeeDir::Up, 2));
    c.mirror();
    assert_eq!(c.links(), [(TeeDir::Up, TeeDir::Down), (TeeDir::Up, TeeDir::Right), (TeeDir::Down, TeeDir::Left)]);
    assert_eq!((c.dir, c.variant), (TeeDir::Up, 1));
}

#[test]
fn a_bridge_is_a_star_from_its_source_in_a_fixed_order() {
    let mut prj = four_devices();
    // Four wire ends meeting at the point instead of two wires crossing.
    prj.pages[0].wires.clear();
    for end in [p(100.0, 40.0), p(100.0, 80.0), p(80.0, 60.0), p(120.0, 60.0)] {
        wire(&mut prj, 0, p(100.0, 60.0), end);
    }
    let id = prj.new_id();
    prj.pages[0].junctions.push(Junction { id, ..Junction::new(JunctionKind::Bridge, p(100.0, 60.0)) });
    assert!(fits(&prj, &prj.pages[0], &prj.pages[0].junctions[0]));
    // From above: the bottom first, then the left, then the right.
    assert_eq!(pairs(&prj), [pair("-K1:A2", "-K2:A1"), pair("-K1:A2", "-K3:A1"), pair("-K1:A2", "-K4:A2")]);
    prj.pages[0].junctions[0].dir = TeeDir::Right;
    assert_eq!(pairs(&prj), [pair("-K4:A2", "-K3:A1"), pair("-K4:A2", "-K1:A2"), pair("-K4:A2", "-K2:A1")]);
    assert_eq!(describe(&prj.pages[0].junctions[0]), "Source on the right, targets on the left, above and below");
    let mut b = prj.pages[0].junctions[0].clone();
    b.turn();
    assert_eq!(b.dir, TeeDir::Down);
    b.mirror();
    assert_eq!(b.dir, TeeDir::Down);
    b.dir = TeeDir::Left;
    b.mirror();
    assert_eq!(b.dir, TeeDir::Right);
    // A bridge on a T node does not fit and is reported; the connection keeps the
    // schematic order then.
    let mut prj = three_devices();
    prj.pages[0].junctions.push(Junction::new(JunctionKind::Bridge, p(100.0, 60.0)));
    let open = Netlist::build(&prj, false);
    let msgs: Vec<String> = problems(&prj, &open).into_iter().map(|i| i.message()).collect();
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(msgs[0].contains("Bridge on page 1 does not fit"));
    assert_eq!(pairs(&prj), [pair("-K1:A1", "-K3:A1"), pair("-K3:A1", "-K2:A1")]);
}

#[test]
fn a_crossing_placed_or_removed_scans_the_page_again() {
    let mut prj = four_devices();
    let mut scans = Scans::default();
    Netlist::build_with(&prj, true, &mut scans);
    assert_eq!(scans.scanned, 1);
    Netlist::build_with(&prj, true, &mut scans);
    assert_eq!(scans.scanned, 0, "nothing changed");
    prj.pages[0].junctions.push(Junction::new(JunctionKind::Crossing, p(100.0, 60.0)));
    let nl = Netlist::build_with(&prj, true, &mut scans);
    assert_eq!(scans.scanned, 1, "the crossing connects, so the page is scanned again");
    assert_eq!(nl.nets.iter().filter(|n| n.pins.len() == 4).count(), 1);
    prj.pages[0].junctions.push(Junction::corner(p(200.0, 200.0), TeeDir::Down, TeeDir::Right));
    Netlist::build_with(&prj, true, &mut scans);
    assert_eq!(scans.scanned, 0, "a corner changes no connectivity");
}

#[test]
fn next_choice_cycles_through_every_routing_of_a_kind() {
    for kind in [JunctionKind::Tee, JunctionKind::Corner, JunctionKind::Crossing, JunctionKind::Bridge] {
        let start = Junction { id: 7, dot: true, ..Junction::new(kind, P { x: 4.0, y: 8.0 }) };
        let mut j = start.clone();
        let mut seen = Vec::new();
        for _ in 0..4 {
            assert!(!seen.contains(&j.links()), "{kind:?}: {j:?} repeats");
            seen.push(j.links());
            j = next_choice(&j);
            assert_eq!((j.id, j.pos, j.dot, j.kind), (start.id, start.pos, start.dot, kind));
        }
        assert_eq!(j.links(), start.links(), "{kind:?}: back at the start after four");
    }
    // A T-junction keeps its direction; only the routing changes.
    let t = Junction { dir: TeeDir::Left, ..Junction::new(JunctionKind::Tee, P::default()) };
    assert!(choices(&t).iter().all(|c| c.dir == TeeDir::Left));
}
