use super::*;
use crate::panel::{new_enclosure, Enclosure, PanelItem};
use crate::routing3d::{Reservation, RoutingPath};

fn project(e: Enclosure) -> Project {
    let mut prj = Project::default();
    prj.layout.enclosures.push(e);
    prj
}

fn single() -> Enclosure {
    let mut prj = Project::default();
    new_enclosure(&mut prj, "", "+A")
}

fn req(ignore: &[u64]) -> Request<'_> {
    Request { kind: RouteKind::Cable, class: Class::Signal, ignore, at: [[0.0; 3]; 2], leave: [[0.0; 3]; 2], out: [0.0; 2], area: 50.0, turn: 12.5, half: 3.0 }
}

fn space(prj: &Project) -> Space {
    let pos = crate::panel::positions(&prj.layout);
    let lu: Vec<usize> = (0..prj.layout.enclosures.len()).collect();
    Space::build(prj, &lu, &pos, &RoutingRules::default())
}

/// A device on the plate from x 125 to 425, y 325 to 425 (enclosure coordinates),
/// 200 mm deep.
fn with_block() -> Enclosure {
    let mut e = single();
    e.items.push(PanelItem { id: 7, kind: ItemKind::Device, tag: "-K1".into(), x: 100.0, y: 300.0, w: 300.0, h: 100.0, d: 200.0, ..Default::default() });
    e
}

fn inside(q: V3, lo: V3, hi: V3) -> bool {
    (0..3).all(|i| q[i] > lo[i] && q[i] < hi[i])
}

#[test]
fn close_bends_leave_out_the_ends_and_lane_changes() {
    let lane_change = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 10.0, 0.0], [300.0, 10.0, 0.0], [300.0, 200.0, 0.0]];
    assert_eq!(close_bends(&lane_change, 50.0), 0);
    let staircase = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 10.0, 0.0], [100.0, 10.0, 200.0], [300.0, 10.0, 200.0]];
    assert_eq!(close_bends(&staircase, 50.0), 1);
    assert_eq!(close_bends(&staircase, 5.0), 0);
    assert_eq!(close_bends(&[[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], 50.0), 0, "the stretches at the ends do not count");
    // Out of a nozzle (50), a short way onto the grid, 50 on, then across and on: the
    // ways onto the grid are no bends, the stretch between two corners of it is.
    let nozzle = [[0.0, 100.0, 0.0], [0.0, 50.0, 0.0], [15.0, 45.0, 0.0], [15.0, -5.0, 0.0], [60.0, -5.0, 0.0], [60.0, -5.0, 300.0], [40.0, 20.0, 310.0]];
    assert_eq!(close_bends(&nozzle, 40.0), 0);
    assert_eq!(close_bends(&nozzle, 50.0), 1);
}

#[test]
fn routes_go_around_devices() {
    let prj = project(with_block());
    let s = space(&prj);
    let (a, b) = ([60.0, 375.0, 60.0], [550.0, 375.0, 60.0]);
    let w = s.route(a, b, &req(&[]), &RoutingRules::default()).expect("a way around");
    assert_eq!((w.points[0], *w.points.last().unwrap()), (a, b));
    for &c in &w.cells {
        assert!(!inside(s.centre(c), [120.0, 320.0, 15.0], [430.0, 430.0, 225.0]), "{:?}", s.centre(c));
    }
    assert!(length(&w.points) > 490.0 + 50.0);
    // Orthogonal between the cells.
    for seg in w.points[1..w.points.len() - 1].windows(2) {
        assert_eq!((0..3).filter(|&i| (seg[0][i] - seg[1][i]).abs() > 1e-3).count(), 1, "{:?}", w.points);
    }
    // Its own device is no obstacle for a route ending at it, but only where the route
    // leaves it: past the end the route goes around it as around any other device.
    let end = [250.0, 375.0, 100.0];
    assert!(s.route(a, end, &Request { at: [a, end], ..req(&[7]) }, &RoutingRules::default()).is_some());
    let w = s.route(a, b, &Request { at: [a, b], ..req(&[7]) }, &RoutingRules::default()).expect("a way around");
    assert!(w.cells.iter().all(|&c| !inside(s.centre(c), [120.0, 320.0, 15.0], [430.0, 430.0, 225.0])), "{:?}", w.points);
}

#[test]
fn ducts_and_paths_are_preferred_and_reservations_kept() {
    let mut e = single();
    // A vertical duct on the left of the plate.
    e.items.push(PanelItem { id: 3, kind: ItemKind::Duct, x: 0.0, y: 0.0, w: 40.0, h: 750.0, d: 60.0, ..Default::default() });
    let prj = project(e.clone());
    let rules = RoutingRules::default();
    let (a, b) = ([60.0, 60.0, 50.0], [60.0, 740.0, 50.0]);
    let s = space(&prj);
    let w = s.route(a, b, &req(&[]), &rules).unwrap();
    assert!(w.free < 100.0, "runs in the duct: {}", w.free);
    // Without the duct it is free air all the way.
    let free = space(&project(single())).route(a, b, &req(&[]), &rules).unwrap();
    assert!(free.free > 600.0);
    // A routing path for tubes only is no channel for cables.
    let mut e2 = single();
    e2.paths.push(RoutingPath { id: 1, kind: PathKind::Tubes, points: vec![[300.0, 60.0, 150.0], [300.0, 740.0, 150.0]], width: 50.0, ..Default::default() });
    let s2 = space(&project(e2));
    let (a2, b2) = ([300.0, 60.0, 150.0], [300.0, 740.0, 150.0]);
    assert!(s2.route(a2, b2, &req(&[]), &rules).unwrap().free > 600.0);
    let tube = Request { kind: RouteKind::Tube, class: Class::Fluid, ..req(&[]) };
    assert!(s2.route(a2, b2, &tube, &rules).unwrap().free < 100.0);
    // A keep-out across the enclosure leaves no way; a reservation for power keeps
    // signals out.
    let mut e3 = single();
    e3.reservations.push(Reservation { id: 1, min: [0.0, 380.0, 0.0], max: [600.0, 420.0, 250.0], ..Default::default() });
    assert!(space(&project(e3.clone())).route(a2, b2, &req(&[]), &rules).is_none());
    e3.reservations[0].mode = ReserveMode::For(Class::Power);
    let s3 = space(&project(e3));
    assert!(s3.route(a2, b2, &req(&[]), &rules).is_none());
    let power = Request { class: Class::Power, ..req(&[]) };
    assert!(s3.route(a2, b2, &power, &rules).is_some());
}

#[test]
fn walls_between_bayed_enclosures_only_at_passages() {
    let mut prj = Project::default();
    for name in ["+A", "+B"] {
        let e = new_enclosure(&mut prj, "", name);
        prj.layout.enclosures.push(e);
    }
    crate::panel::bay_right_of(&mut prj, 1, 0);
    let rules = RoutingRules::default();
    let (a, b) = ([300.0, 400.0, 100.0], [900.0, 400.0, 100.0]);
    assert!(space(&prj).route(a, b, &req(&[]), &rules).is_none(), "closed walls");
    crate::panel::add_passage(&mut prj, 0, 200.0, 80.0, 60.0);
    let w = space(&prj).route(a, b, &req(&[]), &rules).expect("through the passage");
    // Where it crosses x = 600 it is at the passage.
    let cross = w.points.windows(2).find(|s| (s[0][0] - 600.0) * (s[1][0] - 600.0) <= 0.0 && s[0][0] != s[1][0]).unwrap();
    assert!((cross[0][1] - 200.0).abs() <= 45.0 && (cross[0][2] - 80.0).abs() <= 45.0, "{cross:?}");
}

#[test]
fn later_classes_keep_their_distance() {
    let prj = project(single());
    let rules = RoutingRules::default();
    let mut s = space(&prj);
    // Power along the middle.
    let power = s.cells_along(&[[300.0, 60.0, 100.0], [300.0, 740.0, 100.0]]);
    s.occupy(&power, Class::Power, 10.0);
    let (a, b) = ([280.0, 60.0, 100.0], [280.0, 740.0, 100.0]);
    let near = s.route(a, b, &req(&[]), &rules).unwrap();
    s.update_zones(rules.segregation);
    let apart = s.route(a, b, &req(&[]), &rules).unwrap();
    let dist_to_power = |w: &Way| w.cells.iter().map(|&c| (s.centre(c)[0] - 300.0).abs().max((s.centre(c)[2] - 100.0).abs())).sum::<f32>() / w.cells.len() as f32;
    assert!(dist_to_power(&apart) > dist_to_power(&near) + 30.0);
    // Control may run close to power.
    let control = Request { class: Class::Control, ..req(&[]) };
    assert!(length(&s.route(a, b, &control, &rules).unwrap().points) <= length(&near.points) + 1.0);
    assert_eq!(s.classes_at(&power), Class::Power.bit());
}

#[test]
fn path_capacity_and_frozen_ways() {
    let mut e = single();
    e.paths.push(RoutingPath { id: 4, points: vec![[300.0, 60.0, 150.0], [300.0, 740.0, 150.0]], width: 20.0, reserve: 50.0, ..Default::default() });
    let prj = project(e);
    let rules = RoutingRules::default();
    let mut s = space(&prj);
    let (a, b) = ([300.0, 60.0, 150.0], [300.0, 740.0, 150.0]);
    let w = s.route(a, b, &req(&[]), &rules).unwrap();
    assert!(w.free < 100.0);
    // 157 mm² usable: three items of 60 mm² overfill it.
    for _ in 0..3 {
        s.occupy(&w.cells, Class::Signal, 60.0);
    }
    assert_eq!((s.paths[0].items, s.paths[0].used), (3, 180.0));
    assert!(s.paths[0].used > s.paths[0].capacity);
    assert!(s.route(a, b, &req(&[]), &rules).unwrap().free > 600.0, "full: the next goes beside it");
    assert!(s.still_free(&w.points, &req(&[])));
    let behind_plate = [[300.0, 60.0, 150.0], [300.0, 60.0, 5.0], [300.0, 740.0, 5.0], [300.0, 740.0, 150.0]];
    assert!(!s.still_free(&behind_plate, &req(&[])), "behind the mounting plate");
}

#[test]
fn simplify_and_bends() {
    let pts = simplify(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 30.0, 0.0], [20.0, 30.0, 5.0]]);
    assert_eq!(pts, vec![[0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 30.0, 0.0], [20.0, 30.0, 5.0]]);
    assert_eq!(bends(&pts), (2, 30.0));
    assert_eq!(length(&pts), 55.0);
}

#[test]
fn stretches_between_bends_are_at_least_two_bend_radii() {
    let prj = project(with_block());
    let s = space(&prj);
    let rules = RoutingRules::default();
    // Round the device and up to a point just beside its far corner: a cable bends only
    // where it can run on for two bend radii (the arcs of both bends fit between).
    let (a, b) = ([60.0, 375.0, 60.0], [550.0, 375.0, 60.0]);
    for turn in [12.5, 25.0] {
        let r = Request { at: [a, b], leave: [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]], turn, half: 5.0, ..req(&[]) };
        let w = s.route(a, b, &r, &rules).expect("a way");
        let (n, shortest) = bends(&w.points[1..w.points.len() - 1]);
        assert!(n >= 2, "{:?}", w.points);
        assert!(shortest >= turn * 2.0 - 1e-3, "turn {turn}: stretch of {shortest} between bends: {:?}", w.points);
    }
    // With a bend radius the way out of the start has no room for (the device is 65 mm
    // ahead of it), there is no route.
    let r = Request { at: [a, b], leave: [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]], turn: 150.0, half: 5.0, ..req(&[]) };
    assert!(s.route(a, b, &r, &rules).is_none());
}

#[test]
fn routes_leave_their_ends_straight_on() {
    let mut e = with_block();
    e.items[0].d = 80.0;
    let prj = project(e);
    let s = space(&prj);
    let rules = RoutingRules::default();
    // From the front of the device straight out (+z) for the bend radius before the
    // first bend, and into the end point from above (it leaves it upwards, -y).
    let (a, b) = ([250.0, 375.0, 105.0], [550.0, 600.0, 100.0]);
    let r = Request { at: [a, b], leave: [[0.0, 0.0, 1.0], [0.0, -1.0, 0.0]], turn: 100.0, half: 5.0, ..req(&[7]) };
    let w = s.route(a, b, &r, &rules).expect("a way");
    let p = &w.points;
    // The short way to the first cell, then the straight run.
    let (jog, first) = (sub(p[1], p[0]), sub(p[2], p[1]));
    assert!(jog[2] >= 0.0 && jog.iter().all(|v| v.abs() <= 12.5), "{p:?}");
    assert_eq!((first[0], first[1]), (0.0, 0.0), "{p:?}");
    assert!(first[2] >= 100.0 - 12.5 - 1e-3, "{p:?}");
    let n = p.len();
    let (last, jog) = (sub(p[n - 2], p[n - 3]), sub(p[n - 1], p[n - 2]));
    assert!(jog[1] >= 0.0 && jog.iter().all(|v| v.abs() <= 12.5), "{p:?}");
    assert_eq!((last[0], last[2]), (0.0, 0.0), "{p:?}");
    assert!(last[1] >= 100.0 - 12.5 - 1e-3, "{p:?}");
    // Without directions the first stretch is the short way to the nearest cell.
    let any = s.route(a, b, &Request { leave: [[0.0; 3]; 2], ..r }, &rules).expect("a way");
    assert!(length(&any.points) <= length(&w.points) + 1e-3);
}

/// Distance of a point from a box (0 inside).
fn from_box(q: V3, lo: V3, hi: V3) -> f32 {
    let d = [0, 1, 2].map(|i| (lo[i] - q[i]).max(q[i] - hi[i]).max(0.0));
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[test]
fn thick_cables_keep_half_their_thickness_from_devices() {
    let prj = project(with_block());
    let s = space(&prj);
    let (lo, hi) = crate::panel::item_box(&prj.layout.enclosures[0], &prj.layout.enclosures[0].items[0]);
    let (a, b) = ([60.0, 375.0, 60.0], [550.0, 375.0, 60.0]);
    let nearest = |half: f32| {
        let r = Request { at: [a, b], half, ..req(&[]) };
        let w = s.route(a, b, &r, &RoutingRules::default()).expect("a way around");
        w.cells.iter().map(|&c| s.centre(c)).filter(|q| dist(*q, a) > 120.0 && dist(*q, b) > 120.0).map(|q| from_box(q, lo, hi)).fold(f32::INFINITY, f32::min)
    };
    let (thin, thick) = (nearest(3.0), nearest(40.0));
    assert!(thick >= 40.0 && thick > thin, "thin {thin}, thick {thick}");
}

#[test]
fn cables_keep_out_of_ducts_too_small_for_them() {
    let mut e = single();
    e.items.push(PanelItem { id: 3, kind: ItemKind::Duct, x: 0.0, y: 0.0, w: 40.0, h: 750.0, d: 60.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([60.0, 60.0, 50.0], [60.0, 740.0, 50.0]);
    // A thin cable runs in the duct, one thicker than its inside not.
    assert!(s.route(a, b, &Request { at: [a, b], ..req(&[]) }, &rules).unwrap().free < 100.0);
    let thick = s.route(a, b, &Request { at: [a, b], half: 25.0, ..req(&[]) }, &rules).unwrap();
    assert!(thick.free > 500.0, "{}", thick.free);
    let duct = |q: V3| q[0] < 25.0 + 40.0 && q[1] > 25.0 + 100.0 && q[1] < 25.0 + 650.0;
    assert!(!thick.cells.iter().any(|&c| duct(s.centre(c))), "{:?}", thick.points);
}

#[test]
fn cables_keep_out_of_stretches_of_ducts_filled_to_the_limit() {
    let mut e = single();
    e.items.push(PanelItem { id: 3, kind: ItemKind::Duct, x: 0.0, y: 0.0, w: 40.0, h: 750.0, d: 60.0, ..Default::default() });
    let prj = project(e);
    let mut s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([60.0, 60.0, 50.0], [60.0, 740.0, 50.0]);
    let r = Request { at: [a, b], ..req(&[]) };
    let w = s.route(a, b, &r, &rules).unwrap();
    assert!(w.free < 100.0);
    // 70 % of the 1998 mm² inside: 1399 mm². The lower half filled close to it.
    let before = s.loads();
    let lower: Vec<usize> = w.cells.iter().copied().filter(|&c| s.centre(c)[1] > 400.0).collect();
    s.load_ducts(&lower, 1360.0);
    assert!(!s.full_as(&before, r.area), "the next cable no longer fits there");
    assert!(s.full_as(&before, 10.0), "a thin wire still does");
    let full = s.route(a, b, &r, &rules).unwrap();
    let in_duct = |w: &Way, y: (f32, f32)| w.cells.iter().filter(|&&c| s.channel(c, &r) && s.centre(c)[1] > y.0 && s.centre(c)[1] < y.1).count();
    assert_eq!(in_duct(&full, (450.0, 700.0)), 0, "{:?}", full.points);
    assert!(in_duct(&full, (100.0, 350.0)) > 0, "the upper half stays free: {:?}", full.points);
}

#[test]
fn every_corner_keeps_the_full_bend_radius() {
    assert_eq!(corner_radii(&[[0.0; 3], [100.0, 0.0, 0.0], [100.0, 100.0, 0.0], [200.0, 100.0, 0.0]], 80.0), vec![0.0, 80.0, 80.0, 0.0]);
    assert_eq!(corner_radii(&[[0.0; 3], [100.0, 0.0, 0.0]], 80.0), vec![0.0, 0.0]);
}

#[test]
fn thin_obstacles_between_the_cells_block_the_way() {
    // A busbar 10 mm thick lying between two rows of cell centres (its clearance covers
    // none of them): no route crosses it, a thick one keeps half its thickness from it.
    let mut e = single();
    e.items.push(PanelItem { id: 7, kind: ItemKind::Busbar, tag: "-B1".into(), x: 50.0, y: 118.0, w: 500.0, h: 10.0, d: 240.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let (lo, hi) = crate::panel::item_box(&prj.layout.enclosures[0], &prj.layout.enclosures[0].items[0]);
    let around = (lo.map(|v| v - 5.0), hi.map(|v| v + 5.0));
    assert!(s.cells_in(around).is_empty() && !s.cells_in(s.widened(around)).is_empty(), "the bar with its clearance covers no cell centre, widened it does");
    let (a, b) = ([300.0, 60.0, 260.0], [300.0, 740.0, 260.0]);
    for half in [2.0_f32, 6.0, 15.0] {
        let w = s.route(a, b, &Request { at: [a, b], half, ..req(&[]) }, &RoutingRules::default()).expect("a way round");
        let nearest = w.points.windows(2).map(|p| segment_from_box(p[0], p[1], lo, hi)).fold(f32::INFINITY, f32::min);
        assert!(nearest >= half - 0.01, "half {half}: {nearest} from the bar: {:?}", w.points);
    }
}

/// Distance of a segment from a box (sampled), 0 where it enters it.
fn segment_from_box(a: V3, b: V3, lo: V3, hi: V3) -> f32 {
    let n = (dist(a, b) / 2.0).ceil().max(1.0) as usize;
    (0..=n).map(|k| from_box(add(a, scale(sub(b, a), k as f32 / n as f32)), lo, hi)).fold(f32::INFINITY, f32::min)
}

#[test]
fn routes_keep_half_their_thickness_from_the_walls_and_the_plate() {
    // Along the bottom of an empty enclosure (600 wide, 800 high, 250 deep): a thick
    // cable stays its half thickness off the bottom, the sides, the plate and the door.
    let prj = project(single());
    let e = &prj.layout.enclosures[0];
    let s = space(&prj);
    let (a, b) = ([30.0, e.height - 30.0, 40.0], [e.width - 30.0, e.height - 30.0, 40.0]);
    for half in [4.0_f32, 15.0] {
        let w = s.route(a, b, &Request { at: [a, b], half, ..req(&[]) }, &RoutingRules::default()).expect("a way");
        for q in &w.points[1..w.points.len() - 1] {
            assert!(q[1] <= e.height - e.wall() - half + 0.01 && q[1] >= e.wall() + half - 0.01, "half {half}: {q:?}");
            assert!(q[0] >= e.wall() + half - 0.01 && q[0] <= e.width - e.wall() - half + 0.01, "half {half}: {q:?}");
            assert!(q[2] >= PLATE_Z + half - 0.01 && q[2] <= e.depth - e.wall() - half + 0.01, "half {half}: {q:?}");
        }
    }
}

#[test]
fn a_route_without_room_is_not_routed() {
    // Two shelves 50 mm apart across the whole width: a 44 mm cable (half 22) cannot
    // keep 22 from both and has no route; a thinner one gets through between them.
    let mut e = single();
    for (id, y) in [(7, 300.0), (8, 350.0)] {
        e.items.push(PanelItem { id, kind: ItemKind::Device, tag: format!("-K{id}"), x: 0.0, y, w: 550.0, h: 1.0, d: 200.0, ..Default::default() });
    }
    let prj = project(e);
    let s = space(&prj);
    let (a, b) = ([60.0, 100.0, 100.0], [60.0, 700.0, 100.0]);
    assert!(s.route(a, b, &Request { at: [a, b], half: 22.0, ..req(&[]) }, &RoutingRules::default()).is_none());
    s.route(a, b, &Request { at: [a, b], half: 10.0, ..req(&[]) }, &RoutingRules::default()).expect("a thin cable gets through");
}

#[test]
fn cables_keep_apart_in_free_air() {
    let prj = project(single());
    let mut s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([300.0, 60.0, 250.0], [300.0, 740.0, 250.0]);
    let first = s.route(a, b, &req(&[]), &rules).unwrap();
    s.take(&first.points, 3.0, 30.0, &[], &[]);
    // The same way again: beside the first, not inside it, but for the ends.
    let second = s.route(a, b, &req(&[]), &rules).unwrap();
    assert!(!s.still_apart(&first.cells, &req(&[])));
    assert!(s.still_apart(&second.cells, &req(&[])));
    let far = second.points.iter().filter(|q| q[1] > 150.0 && q[1] < 650.0).all(|q| first.points.iter().all(|p| (p[0] - q[0]).hypot(p[2] - q[2]) >= 6.0 - 1e-3 || (p[1] - q[1]).abs() > 1.0));
    assert!(far, "{:?} / {:?}", first.points, second.points);
    // Wires keep apart too (in a duct they lie side by side).
    assert!(!s.still_apart(&first.cells, &Request { kind: RouteKind::Wire, ..req(&[]) }));
    // Near the ends both go to, the second converges with the first.
    let mut s = space(&prj);
    let zone = s.end_zone(&req(&[]));
    s.take(&first.points, 3.0, 30.0, &[(a, zone), (b, zone)], &[]);
    let near_a: Vec<usize> = first.cells.iter().copied().filter(|&i| crate::parts::vec3::dist(s.centre(i), a) <= zone / 2.0).collect();
    assert!(!near_a.is_empty() && s.still_apart(&near_a, &Request { at: [a, b], ..req(&[]) }));
    assert!(!s.still_apart(&first.cells, &Request { at: [a, b], ..req(&[]) }));
}

#[test]
fn ducts_are_entered_through_their_walls_and_kept_off_outside() {
    // A horizontal duct 100 wide and 80 deep across the plate (y 400 .. 500).
    let mut e = single();
    e.items.push(PanelItem { id: 3, kind: ItemKind::Duct, x: 100.0, y: 400.0, w: 400.0, h: 100.0, d: 80.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let rules = RoutingRules::default();
    let (lo, hi) = (add([100.0, 400.0, 0.0], [crate::panel::PLATE_MARGIN, crate::panel::PLATE_MARGIN, PLATE_Z]), add([500.0, 500.0, 80.0], [crate::panel::PLATE_MARGIN, crate::panel::PLATE_MARGIN, PLATE_Z]));
    let in_duct = |c: usize| s.duct_of[c] != 0;
    // A cable 30 mm thick across the duct: through its walls at the height of their
    // slots (above the rim of 20, below the top at 77.5), never through its cover.
    let (a, b) = ([325.0, 325.0, lo[2] + 40.0], [325.0, 625.0, lo[2] + 40.0]);
    let cable = Request { at: [a, b], half: 15.0, turn: 50.0, ..req(&[]) };
    let w = s.route(a, b, &cable, &rules).expect("across the duct");
    let crossings: Vec<&[usize]> = w.cells.windows(2).filter(|c| in_duct(c[0]) != in_duct(c[1])).collect();
    assert_eq!(crossings.len(), 2, "{:?}", w.points);
    for c in crossings {
        let (p, q) = (s.centre(c[0]), s.centre(c[1]));
        assert!(p[2] == q[2] && p[0] == q[0], "through the wall: {p:?} {q:?}");
        let z = p[2] - lo[2];
        assert!(z - 15.0 >= 20.0 - 12.5 && z + 15.0 <= 77.5 + 12.5, "at the slots: {z}");
    }
    // From in front of the duct to behind it: not down through the cover.
    let (a, b) = ([325.0, 475.0, hi[2] + 60.0], [325.0, 625.0, lo[2] + 40.0]);
    let w = s.route(a, b, &Request { at: [a, b], ..cable }, &rules).expect("round the duct");
    assert!(w.cells.windows(2).all(|c| in_duct(c[0]) == in_duct(c[1]) || s.centre(c[0])[2] == s.centre(c[1])[2]), "{:?}", w.points);
    // A cable too thick for the duct running along it keeps half its thickness off it.
    let (a, b) = ([125.0, 545.0, hi[2] + 60.0], [525.0, 545.0, hi[2] + 60.0]);
    let thick = Request { at: [a, b], half: 40.0, ..cable };
    let w = s.route(a, b, &thick, &rules).expect("along the duct");
    for &c in &w.cells {
        assert!(from_box(s.centre(c), lo, hi) >= 40.0 - 0.5, "{:?}", w.points);
    }
}

#[test]
fn the_way_out_of_a_point_inside_its_device_is_open() {
    // A device from x 125 to 325, y 325 to 625, 150 mm deep, with a connection point
    // in its middle that leaves upwards (terminals behind a shroud, a switch on its
    // module): the route runs straight up through the box, nowhere else through it.
    let mut e = single();
    e.items.push(PanelItem { id: 7, kind: ItemKind::Device, tag: "-F1".into(), x: 100.0, y: 300.0, w: 200.0, h: 300.0, d: 150.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([235.0, 485.0, 110.0], [460.0, 160.0, 110.0]);
    let r = Request { at: [a, b], leave: [[0.0, -1.0, 0.0], [0.0; 3]], ..req(&[7]) };
    let w = s.route(a, b, &r, &rules).expect("out through its device");
    let through: Vec<V3> = w.cells.iter().map(|&c| s.centre(c)).filter(|q| inside(*q, [125.0, 325.0, 20.0], [325.0, 625.0, 170.0])).collect();
    assert!(through.len() >= 5, "{:?}", w.points);
    assert!(through.iter().all(|q| (q[0] - a[0]).abs() <= s.cell && (q[2] - a[2]).abs() <= s.cell && q[1] < a[1]), "{through:?}");
    // Without a direction there is no way out of the middle of the device.
    assert!(s.route(a, b, &Request { leave: [[0.0; 3]; 2], ..r }, &rules).is_none());
    // Through another device the way out is not open.
    let other = Request { at: [a, b], leave: [[0.0, -1.0, 0.0], [0.0; 3]], ..req(&[]) };
    assert!(s.route(a, b, &other, &rules).is_none());
}

#[test]
fn the_last_corner_lies_where_the_way_into_the_end_begins() {
    // Two devices one above the other with a single row of cells between them (y 375
    // to 400): a route to a connection point on top of the lower one comes along the
    // row and turns down into the point there, as it may turn right out of its start.
    let mut e = single();
    e.items.push(PanelItem { id: 7, kind: ItemKind::Device, tag: "-K1".into(), x: 100.0, y: 275.0, w: 300.0, h: 70.0, d: 200.0, ..Default::default() });
    e.items.push(PanelItem { id: 8, kind: ItemKind::Device, tag: "-K2".into(), x: 100.0, y: 375.0, w: 300.0, h: 100.0, d: 200.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([510.0, 387.5, 110.0], [310.0, 400.0, 110.0]);
    let r = Request { at: [a, b], leave: [[0.0; 3], [0.0, -1.0, 0.0]], ..req(&[8]) };
    let w = s.route(a, b, &r, &rules).expect("along the row and down into the point");
    assert!(w.cells.iter().all(|&c| (s.centre(c)[1] - 387.5).abs() < 1e-3), "{:?}", w.points);
    // And the other way round.
    let back = Request { at: [b, a], leave: [[0.0, -1.0, 0.0], [0.0; 3]], ..req(&[8]) };
    assert!(s.route(b, a, &back, &rules).is_some());
}

#[test]
fn the_way_out_of_an_end_stays_free_for_its_route() {
    let prj = project(with_block());
    let mut s = space(&prj);
    let rules = RoutingRules::default();
    // A route still to be searched leaves the top of the device upwards.
    let (c, d) = ([285.0, 325.0, 110.0], [285.0, 100.0, 110.0]);
    let up = Request { at: [c, d], leave: [[0.0, -1.0, 0.0], [0.0; 3]], ..req(&[7]) };
    let out = s.cell_of([285.0, 312.5, 110.0]).unwrap();
    // Another one along the top of the device runs across that way.
    let (a, b) = ([60.0, 312.5, 110.0], [510.0, 312.5, 110.0]);
    let across = Request { at: [a, b], ..req(&[]) };
    assert!(s.route(a, b, &across, &rules).unwrap().cells.contains(&out));
    // Kept free, it goes round it; the route it is kept for takes it.
    s.keep_ways_out(c, d, &up);
    let round = s.route(a, b, &across, &rules).expect("round the way out");
    assert!(!round.cells.contains(&out), "{:?}", round.points);
    assert!(s.route(c, d, &up, &rules).unwrap().cells.contains(&out));
    // A route ending right beside it converges with it there.
    let beside = [310.0, 325.0, 110.0];
    let near = Request { at: [beside, a], leave: [[0.0, -1.0, 0.0], [0.0; 3]], ..req(&[7]) };
    assert!(s.route(beside, a, &near, &rules).is_some());
}

#[test]
fn the_way_out_of_an_end_never_runs_through_another_device() {
    // A connection point on the left side of a device that leaves to the left, another
    // device 15 mm beside it: there is no way out through that one to the free cells
    // behind it. With room between the two the route leaves through it.
    let route = |gap: f32| {
        let mut e = single();
        e.items.push(PanelItem { id: 7, kind: ItemKind::Device, tag: "-T1".into(), x: 300.0, y: 300.0, w: 100.0, h: 200.0, d: 150.0, ..Default::default() });
        e.items.push(PanelItem { id: 8, kind: ItemKind::Device, tag: "-X1".into(), x: 260.0 - gap, y: 250.0, w: 40.0, h: 300.0, d: 150.0, ..Default::default() });
        let prj = project(e);
        let s = space(&prj);
        let (a, b) = ([325.0, 437.5, 110.0], [160.0, 110.0, 110.0]);
        let r = Request { at: [a, b], leave: [[-1.0, 0.0, 0.0], [0.0; 3]], ..req(&[7]) };
        s.route(a, b, &r, &RoutingRules::default()).map(|w| w.cells.iter().map(|&c| s.centre(c)).collect::<Vec<V3>>())
    };
    assert!(route(15.0).is_none());
    let way = route(100.0).expect("out between the devices");
    let (lo, hi) = ([185.0, 275.0, 20.0], [225.0, 575.0, 170.0]);
    assert!(way.iter().all(|q| !inside(*q, lo, hi)), "{way:?}");
}

#[test]
fn a_route_comes_into_its_end_the_way_it_leaves_it() {
    // A thin wire to a connection point that it leaves upwards (the way out of it is a
    // single cell), from straight below: it does not run up past the point and back
    // into it, it turns into the way into the point where that begins.
    let prj = project(single());
    let s = space(&prj);
    let (a, b) = ([310.0, 600.0, 110.0], [310.0, 305.0, 110.0]);
    let r = Request { at: [a, b], leave: [[0.0; 3], [0.0, -1.0, 0.0]], ..req(&[]) };
    let w = s.route(a, b, &r, &RoutingRules::default()).expect("round to the way into the point");
    for p in w.points.windows(3) {
        let (u, v) = (sub(p[1], p[0]), sub(p[2], p[1]));
        let along = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]) / (dist(p[0], p[1]) * dist(p[1], p[2]));
        assert!(along > -0.5, "turns back at {:?}: {:?}", p[1], w.points);
    }
    // Its last cell lies above the point, come to from beside it or from above.
    let n = w.cells.len();
    let (last, before) = (s.centre(w.cells[n - 1]), s.centre(w.cells[n - 2]));
    assert!(last[1] < b[1] && before[1] <= last[1] + 1e-3, "{:?}", w.points);
}

#[test]
fn a_way_out_beside_its_end_is_long_enough_to_come_onto_it() {
    // Onto a line beside the end: an S-bend of the radius, two quarter turns where the
    // line lies further aside than they reach.
    assert!((onto_line(10.0, 10.0) - 300f32.sqrt()).abs() < 1e-3);
    assert_eq!(onto_line(25.0, 10.0), 20.0);
    assert_eq!(onto_line(0.0, 10.0), 0.0);
    // A device right above a connection point that leaves upwards bars its way out but
    // for 20 mm; from the cell beside, the route needs the S-bend onto that line and
    // its bend (30 mm more than the cell is away), which fits beside the device.
    let mut e = single();
    e.items.push(PanelItem { id: 8, kind: ItemKind::Device, tag: "-K2".into(), x: 275.0, y: 200.0, w: 25.0, h: 80.0, d: 200.0, ..Default::default() });
    let prj = project(e);
    let s = space(&prj);
    let (a, b) = ([310.0, 320.0, 110.0], [510.0, 100.0, 110.0]);
    let r = Request { at: [a, b], leave: [[0.0, -1.0, 0.0], [0.0; 3]], turn: 10.0, half: 1.5, ..req(&[]) };
    let w = s.route(a, b, &r, &RoutingRules::default()).expect("out beside the device");
    // Straight on from the cell beside for 40 mm and more before it turns.
    let first = s.centre(w.cells[0]);
    assert!((first[0] - a[0]).abs() > 12.5, "{:?}", w.points);
    let straight = w.cells.iter().take_while(|&&c| (s.centre(c)[0] - first[0]).abs() < 1e-3 && (s.centre(c)[2] - first[2]).abs() < 1e-3).count();
    assert!(a[1] - s.centre(w.cells[straight - 1])[1] >= 40.0 - 1.0, "{:?}", w.points);
}

#[test]
fn routes_do_not_cross_each_other_where_they_come_up_to_a_device() {
    // Two terminals 30 mm apart on top of a device, two wires to them from the right,
    // one from above the other: the first, from the terminal nearer the right, runs
    // along y just above the device; the second may not cross it to get to its
    // terminal and comes in above it, nor may it run through the way out of the first.
    let prj = project(with_block());
    let mut s = space(&prj);
    let rules = RoutingRules::default();
    let (a, b) = ([300.0, 320.0, 110.0], [270.0, 320.0, 110.0]);
    let far = [600.0, 310.0, 110.0];
    let first = Request { at: [a, far], leave: [[0.0, -1.0, 0.0], [0.0; 3]], turn: 10.0, half: 1.5, ..req(&[7]) };
    let second = Request { at: [b, far], leave: [[0.0, -1.0, 0.0], [0.0; 3]], turn: 10.0, half: 1.5, ..req(&[7]) };
    s.keep_ways_out(a, far, &first);
    s.keep_ways_out(b, far, &second);
    let w1 = s.route(a, far, &first, &rules).expect("the first");
    let zone = s.end_zone(&first);
    s.take(&w1.points, 1.5, 1.5, &[(a, zone), (far, zone)], &[7]);
    let w2 = s.route(b, far, &second, &rules).expect("the second, round the first");
    // Nowhere do the two run through one cell along different axes.
    let along = |w: &Way| -> Vec<(usize, u8)> { w.cells.windows(2).flat_map(|c| { let (p, q) = (s.centre(c[0]), s.centre(c[1])); let axis = (0..3).find(|&k| (p[k] - q[k]).abs() > 1e-3).unwrap_or(0); [(c[0], axis as u8), (c[1], axis as u8)] }).collect() };
    let (a1, a2) = (along(&w1), along(&w2));
    for &(c, axis) in &a2 {
        assert!(a1.iter().all(|&(d, other)| d != c || other == axis || s.near_point(s.centre(c), &second)), "cross at {:?}: {:?} {:?}", s.centre(c), w1.points, w2.points);
    }
}

#[test]
fn a_way_that_comes_back_to_itself_is_told() {
    let flat = |pts: &[[f32; 2]]| pts.iter().map(|q| [q[0], q[1], 0.0]).collect::<Vec<V3>>();
    // Out, round and back across its first stretch.
    let loop_back = flat(&[[0.0, 0.0], [300.0, 0.0], [300.0, 300.0], [100.0, 300.0], [100.0, -100.0]]);
    let at = meets_itself(&loop_back, 50.0, 20.0, 0).expect("across itself");
    assert!((at[0] - 100.0).abs() < 1e-3 && at[1].abs() < 20.0, "{at:?}");
    // Only the stretches from the given one on are looked at.
    assert!(meets_itself(&loop_back, 50.0, 20.0, 3).is_some());
    let on = flat(&[[0.0, 0.0], [300.0, 0.0], [300.0, 300.0], [100.0, 300.0], [100.0, -100.0], [500.0, -100.0]]);
    assert!(meets_itself(&on, 50.0, 20.0, 0).is_some());
    assert_eq!(meets_itself(&on, 50.0, 20.0, 4), None);
    // Short of itself by its thickness it does not meet itself, nearer it does.
    let short = |gap: f32| flat(&[[0.0, 0.0], [300.0, 0.0], [300.0, 300.0], [100.0, 300.0], [100.0, gap]]);
    assert_eq!(meets_itself(&short(20.0), 50.0, 20.0, 0), None);
    assert!(meets_itself(&short(19.0), 50.0, 20.0, 0).is_some());
    // Back beside itself two bend radii apart, round a corner and along a lane change
    // (the lanes closer than it is thick) it only lies as near to itself as it must.
    assert_eq!(meets_itself(&flat(&[[0.0, 0.0], [300.0, 0.0], [300.0, 100.0], [0.0, 100.0]]), 50.0, 20.0, 0), None);
    assert_eq!(meets_itself(&flat(&[[0.0, 0.0], [100.0, 0.0], [100.0, 10.0], [300.0, 10.0], [300.0, 200.0]]), 50.0, 20.0, 0), None);
    // Through the inside of one of its own bends: 30 mm from both stretches of the
    // corner, but where the bend of 100 mm runs.
    let through = [[-500.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 500.0, 0.0], [0.0, 500.0, 300.0], [-30.0, 500.0, 300.0], [-30.0, 30.0, 300.0], [-30.0, 30.0, -300.0]];
    assert!(meets_itself(&through, 100.0, 25.0, 0).is_some());
    assert_eq!(meets_itself(&through, 0.0, 25.0, 0), None);
}

#[test]
fn a_route_does_not_run_through_itself() {
    // Too stiff to turn into its end at once, a route runs round in a loop: a cable out
    // of a point downwards into one 90 mm below it that it comes into from the right,
    // a wire up out of a terminal into one 130 mm above it and nearer the front that
    // it comes into from above. The shortest loop comes back across the way it went
    // out; it takes one that keeps its thickness from itself, with its bends two
    // radii apart as ever.
    let prj = project(single());
    let s = space(&prj);
    let rules = RoutingRules::default();
    let (up, down, right) = ([0.0, -1.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    for (a, b, leave, turn, half) in [([300.0, 300.0, 110.0], [290.0, 386.0, 105.0], [down, right], 45.0, 5.6), ([300.0, 500.0, 75.0], [345.0, 370.0, 155.0], [up, down], 36.0, 4.5)] {
        let r = Request { at: [a, b], leave, turn, half, ..req(&[]) };
        let w = s.route(a, b, &r, &rules).expect("a way round");
        assert_eq!(meets_itself(&w.points, turn, 2.0 * half + 0.5, 0), None, "{:?}", w.points);
        assert_eq!(close_bends(&w.points, 2.0 * turn), 0, "{:?}", w.points);
    }
    // Out of the nozzle of its device a pipe comes from behind its end point: its way
    // does not come back across the nozzle either.
    let (a, b) = ([300.0, 300.0, 110.0], [290.0, 386.0, 105.0]);
    let r = Request { at: [a, b], leave: [down, right], out: [40.0, 0.0], turn: 45.0, half: 5.6, ..req(&[]) };
    let w = s.route(a, b, &r, &rules).expect("a way round");
    let mut from_nozzle = vec![[300.0, 260.0, 110.0]];
    from_nozzle.extend(&w.points);
    assert_eq!(meets_itself(&from_nozzle, 45.0, 11.7, 0), None, "{from_nozzle:?}");
}
