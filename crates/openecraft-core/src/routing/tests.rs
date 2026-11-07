use super::*;

#[test]
fn automatic_connections_do_not_duplicate_a_wire_running_through_the_ports() {
    use crate::symlib::{UserSymbol,UserPin};
    let mut prj=Project::default();
    prj.pages[0].values.insert("page.explicit_wire_junctions".into(),"true".into());
    prj.symbols.push(UserSymbol {key:"user:right".into(),pins:vec![UserPin::default()],pin_directions:[(0,2)].into(),..Default::default()});
    prj.pages[0].symbols=vec![
        SymbolInst {id:1,sym:"user:right".into(),tag:"-A1".into(),pos:p(80.,80.),..Default::default()},
        SymbolInst {id:2,sym:"user:right".into(),tag:"-A2".into(),pos:p(120.,80.),mirror:true,..Default::default()},
    ];
    prj.pages[0].wires.push(Wire {id:10,a:p(60.,80.),b:p(140.,80.),..Default::default()});
    assert_eq!(apply_autoconnect(&mut prj,0,&BTreeSet::from([1])),0);
}

#[test]
fn explicit_imported_port_directions_drive_new_automatic_wires_after_reopen() {
    use crate::symlib::{UserSymbol,UserPin};
    for crossing in [false,true] {
        let mut prj=Project::default();
        prj.pages[0].values.insert("page.explicit_wire_junctions".into(),"true".into());
        prj.symbols.push(UserSymbol {key:"user:right".into(),pins:vec![UserPin::default()],pin_directions:[(0,2)].into(),..Default::default()});
        prj.pages[0].symbols=vec![
            SymbolInst {id:1,sym:"user:right".into(),tag:"-A1".into(),pos:p(80.,80.),..Default::default()},
            SymbolInst {id:2,sym:"user:right".into(),tag:"-A2".into(),pos:p(120.,80.),mirror:true,..Default::default()},
        ];
        if crossing {prj.pages[0].wires.push(Wire {id:10,a:p(80.,60.),b:p(80.,100.),..Default::default()});}
        prj.next_id=20;
        let mut prj=Project::from_json(&prj.to_json()).unwrap();
        assert_eq!(apply_autoconnect(&mut prj,0,&BTreeSet::from([1])),1);
        let nl=crate::netlist::Netlist::build(&prj,true);
        assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(2,0)]);
        if crossing {assert_ne!(nl.pin_net[&(1,0)],nl.wire_net[&10]);}
    }
}

fn sym(prj: &mut Project, key: &str, pos: P) -> u64 {
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst { id, sym: key.into(), pos, ..Default::default() });
    id
}

fn area() -> R {
    R::from_points(p(0.0, 0.0), p(400.0, 280.0))
}

#[test]
fn imported_plot_frame_does_not_block_connections_or_routes() {
    let mut prj = Project::default();
    prj.symbols.push(crate::symlib::UserSymbol {
        key: "user:frame".into(),
        func: crate::symbols::Func::Graphic,
        prims: vec![crate::symbols::Prim::Poly { pts: vec![p(0., 0.), p(400., 0.), p(400., 280.), p(0., 280.)], closed: true, fill: false }],
        ..Default::default()
    });
    sym(&mut prj, "user:frame", p(0., 0.));
    let top = sym(&mut prj, "no", p(40., 40.));
    sym(&mut prj, "coil", p(40., 80.));
    assert_eq!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([top]), 400.), vec![(p(40., 44.), p(40., 76.))]);
    assert!(search(&prj.symbol_table(), &prj.pages[0], area(), p(20., 160.), p(100., 160.)).is_some());
}

#[test]
fn autoconnect_joins_facing_pins() {
    let mut prj = Project::default();
    let top = sym(&mut prj, "no", p(40.0, 40.0));
    sym(&mut prj, "coil", p(40.0, 80.0));
    // Not aligned: no connection.
    sym(&mut prj, "coil", p(80.0, 80.0));
    let segs = autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([top]), 400.0);
    assert_eq!(segs, vec![(p(40.0, 44.0), p(40.0, 76.0))]);
    assert_eq!(apply_autoconnect(&mut prj, 0, &BTreeSet::from([top])), 1);
    // The pins are connected now.
    assert!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([top]), 400.0).is_empty());
}

#[test]
fn autoconnect_stops_at_symbols_in_between() {
    let mut prj = Project::default();
    let top = sym(&mut prj, "no", p(40.0, 40.0));
    sym(&mut prj, "coil", p(40.0, 120.0));
    // A symbol beside the line but overlapping it.
    sym(&mut prj, "no", p(41.0, 80.0));
    assert!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([top]), 400.0).is_empty());
}

#[test]
fn route_goes_around_symbols() {
    let mut prj = Project::default();
    sym(&mut prj, "coil", p(40.0, 40.0));
    let path = route(&prj.symbol_table(), &prj.pages[0], area(), p(20.0, 40.0), p(60.0, 40.0));
    assert_eq!(path.first(), Some(&p(20.0, 40.0)));
    assert_eq!(path.last(), Some(&p(60.0, 40.0)));
    assert!(path.len() > 2, "{path:?}");
    // Orthogonal and outside the coil.
    for w in path.windows(2) {
        assert!(w[0].x == w[1].x || w[0].y == w[1].y);
    }
    let coil = prj.symbol("coil").unwrap().world_bounds(p(40.0, 40.0), 0, false);
    for q in &path[1..path.len() - 1] {
        assert!(!coil.contains(*q));
    }
}

#[test]
fn route_does_not_run_along_wires() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 99, a: p(0.0, 60.0), b: p(200.0, 60.0), ..Default::default() });
    let path = route(&prj.symbol_table(), &prj.pages[0], area(), p(40.0, 40.0), p(80.0, 80.0));
    for w in path.windows(2) {
        let horizontal_on_wire = w[0].y == 60.0 && w[1].y == 60.0;
        assert!(!horizontal_on_wire, "{path:?}");
    }
    // No corner on the wire.
    assert!(path.iter().all(|q| q.y != 60.0));
}

#[test]
fn straight_route_is_one_segment() {
    let prj = Project::default();
    assert_eq!(route(&prj.symbol_table(), &prj.pages[0], area(), p(20.0, 40.0), p(60.0, 40.0)), vec![p(20.0, 40.0), p(60.0, 40.0)]);
}

#[test]
fn collinear_wires_are_merged() {
    let mut page = Page::default();
    page.wires.push(Wire { id: 1, a: p(0.0, 0.0), b: p(8.0, 0.0), ..Default::default() });
    page.wires.push(Wire { id: 2, a: p(8.0, 0.0), b: p(16.0, 0.0), ..Default::default() });
    page.wires.push(Wire { id: 3, a: p(16.0, 0.0), b: p(16.0, 8.0), ..Default::default() });
    assert_eq!(merge_collinear(&Symbols::default(), &mut page), 1);
    assert_eq!(page.wires.len(), 2);
    assert_eq!((page.wires[0].a, page.wires[0].b), (p(0.0, 0.0), p(16.0, 0.0)));
}

#[test]
fn junctions_are_not_merged() {
    let mut page = Page::default();
    page.wires.push(Wire { id: 1, a: p(0.0, 0.0), b: p(8.0, 0.0), ..Default::default() });
    page.wires.push(Wire { id: 2, a: p(8.0, 0.0), b: p(16.0, 0.0), ..Default::default() });
    page.wires.push(Wire { id: 3, a: p(8.0, 0.0), b: p(8.0, 8.0), ..Default::default() });
    assert_eq!(merge_collinear(&Symbols::default(), &mut page), 0);
}

fn wire(prj: &mut Project, a: P, b: P) -> u64 {
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a, b, ..Default::default() });
    id
}

fn square(prj: &Project) -> bool {
    prj.pages[0].wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y)
}

fn the_wire(prj: &Project, id: u64) -> &Wire {
    prj.pages[0].wires.iter().find(|w| w.id == id).unwrap()
}

/// Which symbols share a net: sorted pin groups, each as sorted (symbol, pin).
fn connections(prj: &Project) -> Vec<Vec<(u64, usize)>> {
    let net = crate::netlist::Netlist::build(prj, false);
    let mut groups: HashMap<usize, Vec<(u64, usize)>> = HashMap::new();
    for (pin, n) in &net.pin_net {
        groups.entry(*n).or_default().push(*pin);
    }
    let mut out: Vec<Vec<(u64, usize)>> = groups.into_values().map(|mut g| { g.sort(); g }).filter(|g| g.len() > 1).collect();
    out.sort();
    out
}

/// Two mains rails with a device stack hanging from them, as in a motor circuit.
struct Stack {
    prj: Project,
    rails: [u64; 2],
    drops: [u64; 2],
    sel: BTreeSet<u64>,
}

fn stack() -> Stack {
    let mut prj = Project::default();
    let rails = [wire(&mut prj, p(20.0, 20.0), p(300.0, 20.0)), wire(&mut prj, p(20.0, 32.0), p(300.0, 32.0))];
    let (l1, l2) = (sym(&mut prj, "no", p(160.0, 80.0)), sym(&mut prj, "no", p(200.0, 80.0)));
    // Feeds of other devices keep the rails connected to something.
    sym(&mut prj, "no", p(260.0, 80.0));
    wire(&mut prj, p(260.0, 20.0), p(260.0, 76.0));
    let drops = [wire(&mut prj, p(160.0, 20.0), p(160.0, 76.0)), wire(&mut prj, p(200.0, 32.0), p(200.0, 76.0))];
    Stack { prj, rails, drops, sel: BTreeSet::from([l1, l2]) }
}

#[test]
fn wires_to_a_rail_stay_vertical_when_their_stack_moves() {
    let Stack { mut prj, rails, drops, sel } = stack();
    let (nets, wires) = (connections(&prj), prj.pages[0].wires.len());
    let (rail_a, rail_b) = (the_wire(&prj, rails[0]).a, the_wire(&prj, rails[0]).b);
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert!(square(&prj), "{:?}", prj.pages[0].wires);
    assert_eq!(prj.pages[0].wires.len(), wires, "no bends needed");
    assert_eq!((the_wire(&prj, drops[0]).a, the_wire(&prj, drops[0]).b), (p(80.0, 20.0), p(80.0, 84.0)));
    assert_eq!((the_wire(&prj, drops[1]).a, the_wire(&prj, drops[1]).b), (p(120.0, 32.0), p(120.0, 84.0)));
    assert_eq!((the_wire(&prj, rails[0]).a, the_wire(&prj, rails[0]).b), (rail_a, rail_b), "the rail stays");
    assert_eq!(connections(&prj), nets);
}

/// The motor feeder of the demo project, as selected with a window around it.
fn demo_feeder(prj: &Project) -> BTreeSet<u64> {
    let symbols = prj.symbol_table();
    let window = R::from_points(p(30.0, 70.0), p(100.0, 200.0));
    let page = &prj.pages[1];
    crate::ops::all_ids(page).into_iter().filter(|id| crate::ops::item_bounds(&symbols, page, *id).is_some_and(|b| window.contains_rect(&b))).collect()
}

#[test]
fn the_motor_feeder_of_the_demo_moves_with_square_wires() {
    for d in [p(-24.0, 12.0), p(80.0, 12.0), p(240.0, -4.0), p(0.0, 40.0)] {
        let mut prj = crate::sample::demo_project();
        let sel = demo_feeder(&prj);
        assert!(sel.len() > 10);
        let nets = connections(&prj);
        let wires = prj.pages[1].wires.len();
        move_selection(&mut prj, 1, &sel, d);
        let diagonal: Vec<&Wire> = prj.pages[1].wires.iter().filter(|w| w.a.x != w.b.x && w.a.y != w.b.y).collect();
        assert!(diagonal.is_empty(), "{d:?}: {diagonal:?}");
        assert_eq!(prj.pages[1].wires.len(), wires, "{d:?}: the junctions slide along the rails");
        assert_eq!(connections(&prj), nets, "{d:?}");
    }
}

#[test]
fn a_junction_slides_along_a_rail_drawn_in_pieces() {
    let mut prj = Project::default();
    let left = wire(&mut prj, p(20.0, 20.0), p(100.0, 20.0));
    wire(&mut prj, p(100.0, 20.0), p(200.0, 20.0));
    wire(&mut prj, p(200.0, 20.0), p(300.0, 20.0));
    let s = sym(&mut prj, "no", p(240.0, 80.0));
    let drop = wire(&mut prj, p(200.0, 20.0), p(200.0, 76.0));
    sym(&mut prj, "no", p(240.0, 120.0));
    let moved = sym(&mut prj, "no", p(200.0, 80.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(-160.0, 0.0));
    assert!(square(&prj));
    assert_eq!((the_wire(&prj, drop).a, the_wire(&prj, drop).b), (p(40.0, 20.0), p(40.0, 76.0)));
    let net = crate::netlist::Netlist::build(&prj, false);
    assert_eq!(net.wire_net[&drop], net.wire_net[&left], "still on the rail");
    assert!(prj.pages[0].symbols.iter().any(|x| x.id == s));
}

#[test]
fn a_move_past_the_end_of_the_rail_keeps_the_connection() {
    let Stack { mut prj, sel, .. } = stack();
    let nets = connections(&prj);
    let wires = prj.pages[0].wires.len();
    move_selection(&mut prj, 0, &sel, p(-152.0, 8.0));
    assert!(square(&prj), "{:?}", prj.pages[0].wires);
    assert!(prj.pages[0].wires.len() > wires, "the drop past the rail end bends");
    assert_eq!(connections(&prj), nets);
}

#[test]
fn a_corner_slides_with_the_moved_item() {
    let mut prj = Project::default();
    let fixed = sym(&mut prj, "no", p(40.0, 80.0));
    let moved = sym(&mut prj, "no", p(120.0, 80.0));
    let across = wire(&mut prj, p(40.0, 40.0), p(120.0, 40.0));
    let down = wire(&mut prj, p(120.0, 40.0), p(120.0, 76.0));
    wire(&mut prj, p(40.0, 40.0), p(40.0, 76.0));
    let nets = connections(&prj);
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(40.0, 16.0));
    assert!(square(&prj));
    assert_eq!((the_wire(&prj, across).a, the_wire(&prj, across).b), (p(40.0, 40.0), p(160.0, 40.0)));
    assert_eq!((the_wire(&prj, down).a, the_wire(&prj, down).b), (p(160.0, 40.0), p(160.0, 92.0)));
    assert_eq!(prj.pages[0].wires.len(), 3);
    assert_eq!(connections(&prj), nets);
    assert!(prj.pages[0].symbols.iter().any(|s| s.id == fixed));
}

#[test]
fn a_bent_wire_between_moved_items_keeps_its_shape() {
    let mut prj = Project::default();
    let a = sym(&mut prj, "no", p(40.0, 80.0));
    let b = sym(&mut prj, "coil", p(80.0, 120.0));
    let pa = p(40.0, 84.0);
    let pb = prj.symbol_table().get("coil").unwrap().pin_world(0, p(80.0, 120.0), 0, false);
    let down = wire(&mut prj, pa, p(40.0, pb.y));
    let across = wire(&mut prj, p(40.0, pb.y), pb);
    let nets = connections(&prj);
    move_selection(&mut prj, 0, &BTreeSet::from([a, b]), p(24.0, 16.0));
    assert!(square(&prj));
    assert_eq!(the_wire(&prj, down).b, the_wire(&prj, across).a);
    assert_eq!(the_wire(&prj, down).a, pa + p(24.0, 16.0));
    assert_eq!(the_wire(&prj, across).b, pb + p(24.0, 16.0));
    assert_eq!(prj.pages[0].wires.len(), 2);
    assert_eq!(connections(&prj), nets);
}

#[test]
fn a_wire_between_connection_points_gets_a_jog_with_its_properties() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    let id = wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    prj.pages[0].wires[0].color = "BU".into();
    prj.pages[0].wires[0].number = "7".into();
    let nets = connections(&prj);
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(40.0, 0.0));
    let wires = &prj.pages[0].wires;
    assert_eq!(wires.len(), 3, "{wires:?}");
    assert!(square(&prj));
    assert!(wires.iter().all(|w| w.color == "BU" && w.number == "7"));
    assert!(wires.iter().any(|w| w.id == id));
    let ids: BTreeSet<u64> = wires.iter().map(|w| w.id).collect();
    assert_eq!(ids.len(), 3, "new ids");
    assert_eq!(connections(&prj), nets);
}

#[test]
fn parallel_jogs_do_not_connect_their_wires() {
    let mut prj = Project::default();
    let mut sel = BTreeSet::new();
    for x in [40.0, 56.0, 72.0] {
        sym(&mut prj, "no", p(x, 40.0));
        sel.insert(sym(&mut prj, "no", p(x, 160.0)));
        wire(&mut prj, p(x, 44.0), p(x, 156.0));
    }
    let nets = connections(&prj);
    assert_eq!(nets.len(), 3);
    for d in [16.0, 32.0, 40.0, 120.0] {
        let mut moved = prj.clone();
        move_selection(&mut moved, 0, &sel, p(d, 0.0));
        assert!(square(&moved), "{d}: {:?}", moved.pages[0].wires);
        assert_eq!(connections(&moved), nets, "{d}: {:?}", moved.pages[0].wires);
    }
}

#[test]
fn a_slide_does_not_connect_foreign_wires() {
    let mut prj = Project::default();
    let rail = wire(&mut prj, p(20.0, 20.0), p(300.0, 20.0));
    let moved = sym(&mut prj, "no", p(160.0, 80.0));
    wire(&mut prj, p(160.0, 20.0), p(160.0, 76.0));
    // A foreign wire ending on the way of the moved drop.
    let foreign = wire(&mut prj, p(60.0, 50.0), p(100.0, 50.0));
    let net = crate::netlist::Netlist::build(&prj, false);
    assert_ne!(net.wire_net[&rail], net.wire_net[&foreign]);
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(-60.0, 0.0));
    let net = crate::netlist::Netlist::build(&prj, false);
    assert_ne!(net.wire_net[&rail], net.wire_net[&foreign], "{:?}", prj.pages[0].wires);
    assert!(square(&prj), "{:?}", prj.pages[0].wires);
}

#[test]
fn diagonal_wires_stretch_as_before() {
    let mut prj = Project::default();
    let moved = sym(&mut prj, "no", p(100.0, 100.0));
    sym(&mut prj, "no", p(40.0, 40.0));
    let id = wire(&mut prj, p(40.0, 44.0), p(100.0, 96.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(8.0, 8.0));
    assert_eq!((the_wire(&prj, id).a, the_wire(&prj, id).b), (p(40.0, 44.0), p(108.0, 104.0)));
    assert_eq!(prj.pages[0].wires.len(), 1);
}

#[test]
fn an_open_wire_end_follows_without_a_bend() {
    let mut prj = Project::default();
    let moved = sym(&mut prj, "no", p(100.0, 100.0));
    let id = wire(&mut prj, p(100.0, 40.0), p(100.0, 96.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(20.0, 0.0));
    assert_eq!((the_wire(&prj, id).a, the_wire(&prj, id).b), (p(120.0, 40.0), p(120.0, 96.0)));
}

#[test]
fn a_drag_depends_only_on_where_it_ends() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    let sel = BTreeSet::from([moved]);
    let (start, next_id) = (prj.pages[0].wires.clone(), prj.next_id);

    let mut direct = prj.clone();
    move_selection(&mut direct, 0, &sel, p(40.0, -40.0));

    let mut moving = Moving::new(&prj, 0, &sel);
    for d in [p(8.0, 0.0), p(32.0, 80.0), p(0.0, -120.0)] {
        moving.step(&mut prj, 0, &sel, d);
    }
    assert_eq!(prj.pages[0].wires, direct.pages[0].wires);
    assert_eq!(prj.pages[0].symbols[1].pos, direct.pages[0].symbols[1].pos);

    moving.step(&mut prj, 0, &sel, p(-40.0, 40.0));
    assert_eq!(prj.pages[0].wires, start, "back at the start: the wire as it was");
    assert_eq!(prj.next_id, next_id);
}

#[test]
fn repeated_moves_extend_the_jog() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    let sel = BTreeSet::from([moved]);
    move_selection(&mut prj, 0, &sel, p(4.0, 0.0));
    move_selection(&mut prj, 0, &sel, p(4.0, 0.0));
    assert_eq!(prj.pages[0].wires.len(), 3, "{:?}", prj.pages[0].wires);
    assert!(square(&prj));
}

#[test]
fn rotating_keeps_attached_wires_square() {
    let Stack { mut prj, sel, .. } = stack();
    let nets = connections(&prj);
    let one = BTreeSet::from([*sel.iter().next().unwrap()]);
    rotate(&mut prj, 0, &one);
    assert!(square(&prj), "{:?}", prj.pages[0].wires);
    assert_eq!(connections(&prj), nets);
}

fn point(prj: &mut Project, pos: P) -> u64 {
    let id = prj.new_id();
    prj.pages[0].conn_defs.push(ConnDef { id, pos, number: "42".into(), ..Default::default() });
    id
}

fn point_pos(prj: &Project, id: u64) -> P {
    prj.pages[0].conn_defs.iter().find(|c| c.id == id).unwrap().pos
}

fn wire_under(prj: &Project, id: u64) -> Option<u64> {
    crate::conndef::wire_at(&prj.pages[0], point_pos(prj, id)).map(|w| w.id)
}

#[test]
fn a_connection_point_stays_on_its_sliding_wire() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = point(&mut prj, p(160.0, 50.0));
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(wire_under(&prj, cd), Some(drops[0]), "{:?}", point_pos(&prj, cd));
}

#[test]
fn a_connection_point_is_not_left_on_the_wire_that_takes_the_place_of_its_own() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = point(&mut prj, p(160.0, 50.0));
    // The second drop lands where the first one was.
    move_selection(&mut prj, 0, &sel, p(-40.0, 0.0));
    assert_eq!(the_wire(&prj, drops[1]).a.x, 160.0);
    assert_eq!(wire_under(&prj, cd), Some(drops[0]), "{:?}", point_pos(&prj, cd));
}

#[test]
fn a_connection_point_stays_on_its_connection_with_a_jog() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    let id = wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    let cd = point(&mut prj, p(40.0, 104.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(40.0, 0.0));
    let on = wire_under(&prj, cd).expect("on a wire");
    let net = crate::netlist::Netlist::build(&prj, false);
    assert_eq!(net.wire_net[&on], net.wire_net[&id]);
}

#[test]
fn a_connection_point_stays_on_a_wire_that_gets_shorter() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    let id = wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    let cd = point(&mut prj, p(40.0, 100.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(0.0, -40.0));
    assert_eq!(wire_under(&prj, cd), Some(id), "{:?}", point_pos(&prj, cd));
}

#[test]
fn a_connection_point_on_a_stretched_wire_stays_where_it_is() {
    let Stack { mut prj, sel, .. } = stack();
    let cd = point(&mut prj, p(160.0, 50.0));
    move_selection(&mut prj, 0, &sel, p(0.0, 40.0));
    assert_eq!(point_pos(&prj, cd), p(160.0, 50.0));
}

#[test]
fn a_connection_point_goes_along_with_a_moved_wire() {
    let mut prj = Project::default();
    let id = wire(&mut prj, p(40.0, 40.0), p(120.0, 40.0));
    let cd = point(&mut prj, p(80.0, 40.0));
    move_selection(&mut prj, 0, &BTreeSet::from([id]), p(0.0, 24.0));
    assert_eq!(point_pos(&prj, cd), p(80.0, 64.0));
}

#[test]
fn a_selected_connection_point_moves_as_told() {
    let Stack { mut prj, sel, .. } = stack();
    let cd = point(&mut prj, p(160.0, 50.0));
    let mut sel = sel;
    sel.insert(cd);
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(point_pos(&prj, cd), p(80.0, 58.0));
    let mut alone = BTreeSet::new();
    alone.insert(cd);
    move_selection(&mut prj, 0, &alone, p(-12.0, 0.0));
    assert_eq!(point_pos(&prj, cd), p(68.0, 58.0), "a point moved on its own may leave the wire");
}

#[test]
fn a_connection_point_returns_with_the_drag() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = point(&mut prj, p(160.0, 50.0));
    let mut moving = Moving::new(&prj, 0, &sel);
    moving.step(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(wire_under(&prj, cd), Some(drops[0]));
    moving.step(&mut prj, 0, &sel, p(40.0, 40.0));
    assert_eq!(wire_under(&prj, cd), Some(drops[0]));
    moving.step(&mut prj, 0, &sel, p(40.0, -48.0));
    assert_eq!(point_pos(&prj, cd), p(160.0, 50.0));
}

#[test]
fn a_connection_point_keeps_off_a_rail_its_wire_crosses() {
    let Stack { mut prj, drops, rails, sel } = stack();
    // The first drop crosses the second rail: halfway it would land on it.
    let cd = point(&mut prj, p(160.0, 48.0));
    move_selection(&mut prj, 0, &sel, p(-80.0, -32.0));
    assert_eq!(the_wire(&prj, rails[1]).a.y, 32.0);
    assert_eq!(wire_under(&prj, cd), Some(drops[0]), "{:?}", point_pos(&prj, cd));
}

fn line(prj: &mut Project, a: P, b: P) -> u64 {
    crate::cabledef::add(prj, 0, a, b, "")
}

fn line_ends(prj: &Project, id: u64) -> (P, P) {
    let c = prj.pages[0].cable_defs.iter().find(|c| c.id == id).unwrap();
    (c.a, c.b)
}

fn crossed(prj: &Project, id: u64) -> Vec<u64> {
    let (a, b) = line_ends(prj, id);
    crate::cabledef::crossed_wires(&prj.pages[0], a, b)
}

fn cores(prj: &Project) -> Vec<(u64, String, String)> {
    let mut v: Vec<_> = prj.pages[0].wires.iter().map(|w| (w.id, w.cable.clone(), w.core.clone())).collect();
    v.sort();
    v
}

#[test]
fn a_cable_line_stays_across_its_sliding_wires() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    let before = cores(&prj);
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(crossed(&prj, cd), drops);
    assert_eq!(line_ends(&prj, cd), (p(70.0, 50.0), p(130.0, 50.0)));
    assert!(crate::cabledef::problems(&prj).is_empty());
    assert_eq!(crate::cabledef::apply(&mut prj, 0, cd), 2);
    assert_eq!(cores(&prj), before, "applying the line again changes nothing");
}

#[test]
fn a_cable_line_on_stretched_wires_stays_where_it_is() {
    let Stack { mut prj, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    move_selection(&mut prj, 0, &sel, p(0.0, 40.0));
    assert_eq!(line_ends(&prj, cd), (p(150.0, 50.0), p(210.0, 50.0)));
}

#[test]
fn a_cable_line_stays_on_wires_that_get_shorter() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    move_selection(&mut prj, 0, &sel, p(0.0, -40.0));
    assert_eq!(crossed(&prj, cd), drops);
    assert_eq!(line_ends(&prj, cd), (p(150.0, 34.0), p(210.0, 34.0)));
}

#[test]
fn a_cable_line_grows_with_a_wire_that_moves_away() {
    let mut prj = stack();
    let cd = line(&mut prj.prj, p(150.0, 50.0), p(210.0, 50.0));
    let one = BTreeSet::from([*prj.sel.iter().nth(1).unwrap()]);
    move_selection(&mut prj.prj, 0, &one, p(20.0, 0.0));
    assert_eq!(crossed(&prj.prj, cd), prj.drops);
    assert_eq!(line_ends(&prj.prj, cd), (p(150.0, 50.0), p(230.0, 50.0)));
}

#[test]
fn a_cable_line_goes_past_a_foreign_wire() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    // The drops land on both sides of the feed at 260, which ends at 76.
    move_selection(&mut prj, 0, &sel, p(80.0, 20.0));
    assert_eq!(crossed(&prj, cd), drops, "{:?}", line_ends(&prj, cd));
}

#[test]
fn a_cable_line_gets_shorter_for_a_wire_landing_beside_it() {
    let mut prj = Project::default();
    let ids = [wire(&mut prj, p(40.0, 20.0), p(40.0, 80.0)), wire(&mut prj, p(60.0, 20.0), p(60.0, 80.0))];
    let foreign = wire(&mut prj, p(100.0, 20.0), p(100.0, 80.0));
    let cd = line(&mut prj, p(30.0, 50.0), p(70.0, 50.0));
    move_selection(&mut prj, 0, &BTreeSet::from([foreign]), p(-36.0, 0.0));
    assert_eq!(crossed(&prj, cd), ids, "{:?}", line_ends(&prj, cd));
}

#[test]
fn a_cable_line_stays_on_its_connection_with_a_jog() {
    let mut prj = Project::default();
    sym(&mut prj, "no", p(40.0, 40.0));
    let moved = sym(&mut prj, "no", p(40.0, 120.0));
    let id = wire(&mut prj, p(40.0, 44.0), p(40.0, 116.0));
    let cd = line(&mut prj, p(30.0, 104.0), p(50.0, 104.0));
    move_selection(&mut prj, 0, &BTreeSet::from([moved]), p(40.0, 0.0));
    assert_eq!(prj.pages[0].wires.len(), 3, "a jog");
    let on = crossed(&prj, cd);
    assert_eq!(on.len(), 1, "{:?}", line_ends(&prj, cd));
    let net = crate::netlist::Netlist::build(&prj, false);
    assert_eq!(net.wire_net[&on[0]], net.wire_net[&id]);
    assert_eq!(line_ends(&prj, cd), (p(70.0, 104.0), p(90.0, 104.0)));
}

#[test]
fn a_cable_line_goes_along_with_moved_wires() {
    let mut prj = Project::default();
    let id = wire(&mut prj, p(40.0, 20.0), p(40.0, 80.0));
    let cd = line(&mut prj, p(30.0, 50.0), p(50.0, 50.0));
    move_selection(&mut prj, 0, &BTreeSet::from([id]), p(24.0, 0.0));
    assert_eq!(line_ends(&prj, cd), (p(54.0, 50.0), p(74.0, 50.0)));
}

#[test]
fn a_selected_cable_line_moves_as_told() {
    let Stack { mut prj, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    let mut sel = sel;
    sel.insert(cd);
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(line_ends(&prj, cd), (p(70.0, 58.0), p(130.0, 58.0)));
    move_selection(&mut prj, 0, &BTreeSet::from([cd]), p(-40.0, 0.0));
    assert_eq!(line_ends(&prj, cd), (p(30.0, 58.0), p(90.0, 58.0)), "a line moved on its own may leave its wires");
}

#[test]
fn a_cable_line_returns_with_the_drag() {
    let Stack { mut prj, drops, sel, .. } = stack();
    let cd = line(&mut prj, p(150.0, 50.0), p(210.0, 50.0));
    let mut moving = Moving::new(&prj, 0, &sel);
    moving.step(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(crossed(&prj, cd), drops);
    moving.step(&mut prj, 0, &sel, p(40.0, 40.0));
    assert_eq!(crossed(&prj, cd), drops);
    moving.step(&mut prj, 0, &sel, p(40.0, -48.0));
    assert_eq!(line_ends(&prj, cd), (p(150.0, 50.0), p(210.0, 50.0)));
}

#[test]
fn a_cable_line_that_crosses_nothing_stays() {
    let Stack { mut prj, sel, .. } = stack();
    let tag = crate::ops::add_cable(&mut prj, "");
    let cd = prj.new_id();
    prj.pages[0].cable_defs.push(CableDef { id: cd, a: p(80.0, 200.0), b: p(120.0, 200.0), cable: tag, shield: false });
    move_selection(&mut prj, 0, &sel, p(-80.0, 8.0));
    assert_eq!(line_ends(&prj, cd), (p(80.0, 200.0), p(120.0, 200.0)));
}

#[test]
fn autoconnect_wires_into_the_free_arms_of_connection_symbols() {
    let mut prj = Project::default();
    // A contact above the corner, its lower pin at (60, 44) facing down; a coil to
    // the right turned by 90°, its A2 at (116, 80) facing left.
    let contact = sym(&mut prj, "no", p(60.0, 40.0));
    let coil = sym(&mut prj, "coil", p(120.0, 80.0));
    prj.pages[0].symbols.iter_mut().find(|s| s.id == coil).unwrap().rot = 1;
    let corner = prj.new_id();
    prj.pages[0].junctions.push(Junction { id: corner, ..Junction::corner(p(60.0, 80.0), TeeDir::Up, TeeDir::Right) });
    // Placing the corner wires both pins in.
    let segs = autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([corner]), 400.0);
    assert_eq!(segs, vec![(p(60.0, 80.0), p(60.0, 44.0)), (p(60.0, 80.0), p(116.0, 80.0))]);
    // A symbol placed facing a free arm connects to it; a used arm takes no second wire.
    let wid = prj.new_id();
    prj.pages[0].wires.push(Wire { id: wid, a: p(60.0, 80.0), b: p(60.0, 44.0), ..Default::default() });
    assert_eq!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([contact]), 400.0), vec![]);
    assert_eq!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([coil]), 400.0), vec![(p(116.0, 80.0), p(60.0, 80.0))]);
    assert_eq!(apply_autoconnect(&mut prj, 0, &BTreeSet::from([corner])), 1);
    assert!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([corner, coil, contact]), 400.0).is_empty());
    assert!(crate::netlist::junctions::fits(&prj, &prj.pages[0], &prj.pages[0].junctions[0]));
    // A T-junction's three arms are wired the same way; its arm away from any pin stays open.
    let mut prj = Project::default();
    sym(&mut prj, "no", p(60.0, 40.0));
    let tee = prj.new_id();
    prj.pages[0].junctions.push(Junction { id: tee, pos: p(60.0, 80.0), dir: TeeDir::Down, variant: 3, ..Default::default() });
    assert_eq!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([tee]), 400.0), vec![]);
    prj.pages[0].junctions[0].dir = TeeDir::Up;
    assert_eq!(autoconnect(&prj.symbol_table(), &prj.pages[0], &BTreeSet::from([tee]), 400.0), vec![(p(60.0, 80.0), p(60.0, 44.0))]);
}
