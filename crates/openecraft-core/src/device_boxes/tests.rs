use super::*;
use crate::analysis::Analysis;
use crate::model::{PinLabel, Wire};
use crate::netlist::pin_designation;
use crate::symbols::Anchor;

/// A box -A1 from (40, 40) to (80, 64) with the points 1 and 2 on its top edge and
/// L on its bottom edge.
fn setup() -> (Project, u64, [u64; 3]) {
    let mut prj = Project::default();
    let b = add_box(&mut prj, 0, p(40.0, 40.0), p(80.0, 64.0), "", "Power supply").unwrap();
    let p1 = place_point(&mut prj, 0, p(48.0, 40.0), 0, "", "").unwrap();
    let p2 = place_point(&mut prj, 0, p(56.0, 40.0), 0, "", "").unwrap();
    let l = place_point(&mut prj, 0, p(48.0, 64.0), 2, "", "L").unwrap();
    (prj, b, [p1, p2, l])
}

fn sym(prj: &Project, id: u64) -> &SymbolInst {
    prj.pages.iter().flat_map(|pg| pg.symbols.iter()).find(|s| s.id == id).unwrap()
}

#[test]
fn points_placed_on_a_box_are_numbered_connections_of_its_device() {
    let (prj, b, [p1, p2, l]) = setup();
    let bx = sym(&prj, b);
    assert_eq!(bx.tag, "-A1");
    assert_eq!(bx.size, Some(p(40.0, 24.0)));
    assert_eq!(rect(&prj, bx), Some(R::from_points(p(40.0, 40.0), p(80.0, 64.0))));
    assert_eq!([&sym(&prj, p1).pins[0], &sym(&prj, p2).pins[0], &sym(&prj, l).pins[0]], ["1", "2", "L"]);
    assert!([p1, p2, l].iter().all(|id| sym(&prj, *id).tag == "-A1"));
    assert_eq!(points_of(&prj, &prj.pages[0], b), vec![p1, p2, l]);
    assert_eq!(box_of(&prj, &prj.pages[0], sym(&prj, l)), Some(b));
    // The connection leads away from the box: up on the top edge, down on the bottom.
    let def = prj.symbol(DEVICE_POINT).unwrap();
    assert_eq!(def.pin_world(0, p(48.0, 40.0), 0, false), p(48.0, 36.0));
    assert_eq!(def.pin_world(0, p(48.0, 64.0), 2, false), p(48.0, 68.0));
    let idx = crate::index::Index::build(&prj);
    let dev = &idx.devices["-A1"];
    assert_eq!((dev.mains.len(), dev.others.len(), dev.auxs.len()), (1, 3, 0));
    // Off any box a point needs a tag.
    let mut prj = prj;
    assert!(place_point(&mut prj, 0, p(200.0, 200.0), 0, "", "").is_err());
    let id = place_point(&mut prj, 0, p(200.0, 200.0), 0, "-A1", "").unwrap();
    assert_eq!(sym(&prj, id).pins[0], "3");
}

#[test]
fn designations_count_on() {
    assert_eq!(increment("13").as_deref(), Some("14"));
    assert_eq!(increment("X9").as_deref(), Some("X10"));
    assert_eq!(increment("1.9").as_deref(), Some("1.10"));
    assert_eq!(increment("+24V"), None);
    assert_eq!(increment("PE"), None);
    let (mut prj, _, _) = setup();
    assert_eq!(next_designation(&prj, "-A1"), "3");
    assert_eq!(next_designation(&prj, "-A9"), "1");
    place_point(&mut prj, 0, p(64.0, 40.0), 0, "", "X13").unwrap();
    assert_eq!(next_designation(&prj, "-A1"), "X14");
}

#[test]
fn the_points_of_a_box_are_separate_connections() {
    let (mut prj, _, [p1, p2, _]) = setup();
    // A second box above, its points facing down onto the points 1 and 2.
    let b2 = add_box(&mut prj, 0, p(40.0, 0.0), p(80.0, 16.0), "", "").unwrap();
    assert_eq!(sym(&prj, b2).tag, "-A2");
    place_point(&mut prj, 0, p(48.0, 16.0), 2, "", "").unwrap();
    place_point(&mut prj, 0, p(56.0, 16.0), 2, "", "").unwrap();
    for x in [48.0, 56.0] {
        let id = prj.new_id();
        prj.pages[0].wires.push(Wire { id, a: p(x, 20.0), b: p(x, 36.0), ..Default::default() });
    }
    let an = Analysis::build(&prj);
    let mut pairs: Vec<(String, String)> = an
        .connections
        .iter()
        .map(|c| {
            let (a, b) = (pin_designation(&prj, c.a), pin_designation(&prj, c.b));
            if a < b { (a, b) } else { (b, a) }
        })
        .collect();
    pairs.sort();
    assert_eq!(pairs, vec![("-A1:1".to_string(), "-A2:1".to_string()), ("-A1:2".into(), "-A2:2".into())]);
    let net = |id: u64| an.netlist.pin_net.get(&(id, 0)).copied();
    assert!(net(p1).is_some() && net(p1) != net(p2), "different points are not connected");
}

#[test]
fn resizing_moves_the_points_on_moving_edges_with_their_wires() {
    let (mut prj, b, [p1, _, l]) = setup();
    let right = place_point(&mut prj, 0, p(80.0, 48.0), 1, "", "+24V").unwrap();
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(84.0, 48.0), b: p(120.0, 48.0), ..Default::default() });
    resize(&mut prj, 0, b, R::from_points(p(40.0, 40.0), p(100.0, 72.0))).unwrap();
    assert_eq!(sym(&prj, b).size, Some(p(60.0, 32.0)));
    assert_eq!(sym(&prj, right).pos, p(100.0, 48.0), "right edge moved");
    assert_eq!(sym(&prj, p1).pos, p(48.0, 40.0), "top edge stayed");
    assert_eq!(sym(&prj, l).pos, p(48.0, 72.0), "bottom edge moved");
    let w = prj.pages[0].wires.iter().find(|w| w.id == id).unwrap();
    assert_eq!((w.a, w.b), (p(104.0, 48.0), p(120.0, 48.0)), "the wire follows");
    assert!(resize(&mut prj, 0, b, R::from_points(p(40.0, 40.0), p(41.0, 80.0))).is_err());
}

#[test]
fn retagging_a_box_takes_its_points_and_points_can_be_assigned() {
    let (mut prj, b, [p1, p2, l]) = setup();
    let outside = place_point(&mut prj, 0, p(200.0, 40.0), 0, "-B7", "1").unwrap();
    set_tag(&mut prj, 0, b, "-G1");
    assert!([b, p1, p2, l].iter().all(|id| sym(&prj, *id).tag == "-G1"));
    assert_eq!(sym(&prj, outside).tag, "-B7");
    assign(&mut prj, 0, outside, b).unwrap();
    assert_eq!(sym(&prj, outside).tag, "-G1");
    assert!(assign(&mut prj, 0, b, b).is_err(), "a box is no connection point");
    // Its tag is shown while it is not on its box.
    let def = prj.symbol(DEVICE_POINT).unwrap();
    assert!(crate::render::hides_tag(&prj, 0, &def, sym(&prj, p1)));
    assert!(!crate::render::hides_tag(&prj, 0, &def, sym(&prj, outside)));
}

#[test]
fn copies_of_a_box_keep_their_points_on_the_copied_device() {
    let (mut prj, b, points) = setup();
    let sel = with_points(&prj, 0, &BTreeSet::from([b]));
    assert_eq!(sel.len(), 4);
    let clip = ops::copy(&prj.pages[0], &sel);
    let ids = ops::paste(&mut prj, 0, &clip, p(120.0, 40.0));
    let copies: Vec<&SymbolInst> = prj.pages[0].symbols.iter().filter(|s| ids.contains(&s.id)).collect();
    assert_eq!(copies.len(), 4);
    assert!(copies.iter().all(|s| s.tag == "-A2"), "{:?}", copies.iter().map(|s| &s.tag).collect::<Vec<_>>());
    let new_box = copies.iter().find(|s| is_box(s)).unwrap().id;
    assert_eq!(points_of(&prj, &prj.pages[0], new_box).len(), 3);
    assert!(points.iter().all(|id| sym(&prj, *id).tag == "-A1"), "the original stays");
    // A macro of the box does the same.
    let m = crate::macros::from_selection(&prj.pages[0], &sel, "PSU");
    let ids = ops::paste(&mut prj, 0, &m.clip, p(200.0, 40.0));
    assert!(prj.pages[0].symbols.iter().filter(|s| ids.contains(&s.id)).all(|s| s.tag == "-A3"));
}

#[test]
fn boxes_and_points_survive_saving_and_export() {
    let (mut prj, b, [_, _, l]) = setup();
    let s = prj.pages[0].symbols.iter_mut().find(|s| s.id == l).unwrap();
    s.pin_label = Some(PinLabel { offset: p(2.0, -2.0), anchor: Anchor::Left });
    let back = crate::archive::unpack(&crate::archive::pack(&prj)).unwrap();
    assert_eq!(back.pages[0].symbols, prj.pages[0].symbols);
    assert_eq!(sym(&back, b).size, Some(p(40.0, 24.0)));
    let svg = crate::svg::page_svg(&back, 0);
    assert!(svg.contains(">L</text>") && svg.contains(">Power supply</text>"), "designations and function text are drawn");
    assert_eq!(svg.matches(">-A1</text>").count(), 1, "the tag is written once, at the box");
    let def = back.symbol(DEVICE_POINT).unwrap();
    let (q, anchor, _) = crate::render::pin_label_at(&def, sym(&back, l), 0);
    assert_eq!((q, anchor), (p(50.0, 62.0), Anchor::Left));
}

#[test]
fn repeated_designations_of_a_device_are_reported() {
    let (mut prj, _, _) = setup();
    let issues = |prj: &Project| {
        let an = Analysis::build(prj);
        crate::checks::check(prj, &an.index, &an.netlist).iter().map(|i| i.message()).collect::<Vec<_>>()
    };
    assert!(!issues(&prj).iter().any(|m| m.contains("more than once")), "{:?}", issues(&prj));
    place_point(&mut prj, 0, p(64.0, 64.0), 2, "", "L").unwrap();
    assert!(issues(&prj).iter().any(|m| m == "-A1: designation 'L' used more than once"), "{:?}", issues(&prj));
}

#[test]
fn only_the_outline_of_a_box_is_hit() {
    let r = R::from_points(p(0.0, 0.0), p(40.0, 20.0));
    assert!(outline_hit(r, p(20.0, 0.3), 0.5));
    assert!(outline_hit(r, p(40.2, 10.0), 0.5));
    assert!(!outline_hit(r, p(20.0, 10.0), 0.5));
}
/// The device of `setup` and an empty second page.
fn with_second_page() -> Project {
    let (mut prj, _, _) = setup();
    let id = prj.new_id();
    prj.pages.push(Page { id, ..Default::default() });
    prj
}

fn messages(prj: &Project) -> Vec<String> {
    crate::checks::check(prj, &crate::index::Index::build(prj), &crate::netlist::Netlist::build(prj, true)).iter().map(|i| i.message().to_string()).collect()
}

fn continuing(prj: &mut Project, role: DeviceBoxRole) -> u64 {
    let b = add_box(prj, 1, p(40.0, 40.0), p(80.0, 64.0), "-A1", "").unwrap();
    set_role(prj, 1, b, role).unwrap();
    b
}

#[test]
fn an_extension_adds_connection_points_without_a_second_device() {
    let mut prj = with_second_page();
    continuing(&mut prj, DeviceBoxRole::Extension);
    let x = place_point(&mut prj, 1, p(48.0, 40.0), 0, "", "").unwrap();
    assert_eq!((sym(&prj, x).tag.as_str(), sym(&prj, x).pins[0].as_str()), ("-A1", "3"));
    assert_eq!(device_pins(&prj, "-A1"), ["1", "2", "L", "3"]);
    let found = messages(&prj);
    assert!(!found.iter().any(|m| m.contains("main function placed") || m.contains("more than once")), "{found:?}");
    place_point(&mut prj, 1, p(56.0, 40.0), 0, "", "L").unwrap();
    assert!(messages(&prj).iter().any(|m| m == "-A1: designation 'L' used more than once"));
}

#[test]
fn a_view_connects_the_points_it_shows_and_lists_them_once() {
    for explicit in [false, true] {
        let mut prj = with_second_page();
        if explicit {
            prj.pages[1].values.insert("page.explicit_wire_junctions".into(), "true".into());
        }
        let v = continuing(&mut prj, DeviceBoxRole::View);
        show_pins(&mut prj, 1, v, &["L".to_string()]).unwrap();
        let point = points_of(&prj, &prj.pages[1], v)[0];
        let far = sym(&prj, point).pos + p(40.0, -20.0);
        let other = place_point(&mut prj, 1, far, 2, "-A9", "1").unwrap();
        let def = prj.symbol(DEVICE_POINT).unwrap();
        let end = |s: &SymbolInst| def.pin_world(0, s.pos, s.rot, s.mirror);
        let (a, b) = (end(sym(&prj, point)), end(sym(&prj, other)));
        let id = prj.new_id();
        prj.pages[1].wires.push(Wire { id, a, b, ..Default::default() });
        let (bridged, open) = (crate::netlist::Netlist::build(&prj, true), crate::netlist::Netlist::build(&prj, false));
        let net = &bridged.nets[bridged.pin_net[&(point, 0)]];
        let mut pins: Vec<String> = net.pins.iter().map(|&q| pin_designation(&prj, q)).collect();
        pins.sort();
        assert_eq!(pins, ["-A1:L", "-A9:1"]);
        assert_eq!(net.view_pins.len(), 1);
        let source = prj.pages[0].symbols.iter().find(|s| s.pins.first().is_some_and(|d| d == "L")).unwrap().id;
        assert!(!bridged.unconnected.iter().any(|u| u.1 == source), "wired through the view");
        let connections = crate::netlist::connections(&prj, &bridged, &open);
        let pairs: Vec<[String; 2]> = connections.iter().map(|c| [pin_designation(&prj, c.a), pin_designation(&prj, c.b)]).collect();
        assert_eq!(pairs, [["-A1:L", "-A9:1"]], "explicit: {explicit}");
        assert_eq!(device_pins(&prj, "-A1"), ["1", "2", "L"], "a view adds nothing");
        assert!(!messages(&prj).iter().any(|m| m.contains("-A1:") && (m.contains("main function placed") || m.contains("more than once"))));
    }
}

#[test]
fn views_only_show_points_the_device_has() {
    let mut prj = with_second_page();
    let v = continuing(&mut prj, DeviceBoxRole::View);
    assert!(show_pins(&mut prj, 1, v, &["X9".to_string()]).is_err());
    let unknown = place_point(&mut prj, 1, p(48.0, 40.0), 0, "", "X9").unwrap();
    assert!(messages(&prj).iter().any(|m| m == "-A1: device box shows connection point 'X9' that the device does not have"));
    ops::delete(&mut prj.pages[1], &BTreeSet::from([unknown]));
    // Without a designation the next point not shown yet is taken, on the top edge.
    let first = place_point(&mut prj, 1, p(48.0, 40.0), 0, "", "").unwrap();
    assert_eq!(sym(&prj, first).pins[0], "1");
    let second = place_point(&mut prj, 1, p(56.0, 40.0), 0, "", "").unwrap();
    assert_eq!(sym(&prj, second).pins[0], "2");
    show_pins(&mut prj, 1, v, &["L".to_string(), "2".to_string()]).unwrap();
    let shown = shown_pins(&prj, &prj.pages[1], v);
    assert!(shown.contains(&"L".to_string()) && shown.contains(&"2".to_string()) && !shown.contains(&"1".to_string()), "{shown:?}");
}

#[test]
fn a_continuing_box_without_its_device_is_reported() {
    let mut prj = Project::default();
    let b = add_box(&mut prj, 0, p(40.0, 40.0), p(80.0, 64.0), "-A5", "").unwrap();
    set_role(&mut prj, 0, b, DeviceBoxRole::Extension).unwrap();
    assert!(messages(&prj).iter().any(|m| m == "-A5: device box extends or shows a device that is not placed"));
}

#[test]
fn copies_of_a_continuing_box_stay_on_the_device() {
    let mut prj = with_second_page();
    let v = continuing(&mut prj, DeviceBoxRole::View);
    show_pins(&mut prj, 1, v, &["L".to_string()]).unwrap();
    let sel = with_points(&prj, 1, &BTreeSet::from([v]));
    let clip = ops::copy(&prj.pages[1], &sel);
    let ids = ops::paste(&mut prj, 1, &clip, p(120.0, 40.0));
    assert_eq!(ids.len(), 2);
    assert!(prj.pages[1].symbols.iter().filter(|s| ids.contains(&s.id)).all(|s| s.tag == "-A1"));
}

#[test]
fn the_role_survives_saving_and_the_default_is_not_written() {
    let mut prj = with_second_page();
    let v = continuing(&mut prj, DeviceBoxRole::View);
    let back = crate::archive::unpack(&crate::archive::pack(&prj)).unwrap();
    assert_eq!(sym(&back, v).box_role, DeviceBoxRole::View);
    let plain = serde_json::to_string(&prj.pages[0].symbols[0]).unwrap();
    assert!(!plain.contains("box_role"), "{plain}");
}

#[test]
fn boxes_refer_to_their_device_and_the_device_to_its_boxes() {
    let mut prj = with_second_page();
    let e = continuing(&mut prj, DeviceBoxRole::Extension);
    let idx = crate::index::Index::build(&prj);
    let at = |prj: &Project, page: usize, id: u64| {
        let s = sym(prj, id);
        crate::references::of_symbol(prj, &idx, page, &prj.symbol(&s.sym).unwrap(), s).iter().map(|r| r.page).collect::<Vec<_>>()
    };
    let main = prj.pages[0].symbols.iter().find(|s| is_box(s)).unwrap().id;
    assert_eq!(at(&prj, 0, main), [Some(1)]);
    assert_eq!(at(&prj, 1, e), [Some(0)]);
}
