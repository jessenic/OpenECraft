use super::*;

fn close(a: V3, b: V3) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn vector_basics() {
    assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
    assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
    assert!(close(norm([3.0, 0.0, 4.0]), [0.6, 0.0, 0.8]));
}

#[test]
fn orbit_from_the_front_looks_down_minus_z() {
    let cam = Orbit { yaw: 0.0, pitch: 0.0, dist: 10.0, target: [1.0, 2.0, 3.0] };
    let (eye, right, up, fwd) = cam.basis();
    assert!(close(eye, [1.0, 2.0, 13.0]));
    assert!(close(fwd, [0.0, 0.0, -1.0]));
    assert!(close(right, [1.0, 0.0, 0.0]));
    assert!(close(up, [0.0, 1.0, 0.0]));
}

#[test]
fn segments_and_triangles() {
    let t = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
    assert!(segment_crosses_triangle([2.0, 2.0, -1.0], [2.0, 2.0, 1.0], &t));
    // Ending on the plane, outside, or lying in the plane does not count.
    assert!(!segment_crosses_triangle([2.0, 2.0, 0.0], [2.0, 2.0, 1.0], &t));
    assert!(!segment_crosses_triangle([8.0, 8.0, -1.0], [8.0, 8.0, 1.0], &t));
    assert!(!segment_crosses_triangle([1.0, 1.0, 0.0], [3.0, 1.0, 0.0], &t));
}

#[test]
fn box_surfaces() {
    let a = box_triangles([0.0; 3], [10.0; 3]);
    assert!(point_inside([5.0, 5.0, 5.0], &a));
    assert!(!point_inside([15.0, 5.0, 5.0], &a));
    // Overlapping, touching, apart, contained.
    assert!(surfaces_intersect(&a, &box_triangles([8.0, 2.0, 2.0], [20.0, 8.0, 8.0])));
    assert!(!surfaces_intersect(&a, &box_triangles([10.0, 0.0, 0.0], [20.0, 10.0, 10.0])));
    assert!(!surfaces_intersect(&a, &box_triangles([12.0, 0.0, 0.0], [20.0, 10.0, 10.0])));
    assert!(surfaces_intersect(&a, &box_triangles([2.0; 3], [4.0; 3])));
    assert!(surfaces_intersect(&box_triangles([2.0; 3], [4.0; 3]), &a));
    // Crossing bars (no corner inside the other box).
    let x = box_triangles([-5.0, 4.0, 4.0], [15.0, 6.0, 6.0]);
    assert!(surfaces_intersect(&a, &x));
}

#[test]
fn sums_and_distances() {
    assert_eq!(add([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), [5.0, 7.0, 9.0]);
    assert_eq!(dist([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]), 5.0);
    assert_eq!(manhattan([1.0, 2.0, 3.0], [4.0, 0.0, 3.0]), 5.0);
}

#[test]
fn boxes_are_painted_from_far_to_near() {
    // A long duct along x in front of the plate and a device beside its right end,
    // reaching further out than the duct.
    let duct = ([0.0, 0.0, 0.0], [600.0, 60.0, 60.0]);
    let device = ([620.0, 0.0, 0.0], [680.0, 90.0, 90.0]);
    let pos = |order: Vec<usize>, i: usize| order.iter().position(|&k| k == i).unwrap();
    // Seen from the left the duct is in front of the device, from the right behind it,
    // although the centre of the duct is nearer in both cases.
    let left = paint_order(&[duct, device], [-2000.0, 300.0, 400.0]);
    assert!(pos(left.clone(), 1) < pos(left, 0), "the device first, the duct over it");
    let right = paint_order(&[duct, device], [3000.0, 300.0, 400.0]);
    assert!(pos(right.clone(), 0) < pos(right, 1));
    // From the front, a device on a duct (further out) is painted after it.
    let on = ([100.0, 0.0, 60.0], [160.0, 60.0, 120.0]);
    assert_eq!(paint_order(&[on, duct], [300.0, 300.0, 2000.0]), vec![1, 0]);
}

#[test]
fn overlapping_boxes_and_cycles_still_give_every_box_once() {
    let a = ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    let b = ([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
    let far = ([100.0, 0.0, 0.0], [110.0, 10.0, 10.0]);
    let mut order = paint_order(&[a, b, far], [-50.0, 5.0, 5.0]);
    assert_eq!(order[0], 2, "the farthest one first");
    order.sort();
    assert_eq!(order, vec![0, 1, 2]);
    assert!(paint_order(&[], [0.0; 3]).is_empty());
}

#[test]
fn segments_are_clipped_to_boxes() {
    let duct = ([0.0, 0.0, 0.0], [100.0, 60.0, 60.0]);
    // A wire from a device above the duct down into it and along it.
    assert_eq!(segment_in_box([50.0, 120.0, 30.0], [50.0, 30.0, 30.0], duct), Some((2.0 / 3.0, 1.0)));
    assert_eq!(segment_in_box([10.0, 30.0, 30.0], [90.0, 30.0, 30.0], duct), Some((0.0, 1.0)));
    let (t0, t1) = segment_in_box([-50.0, 30.0, 30.0], [150.0, 30.0, 30.0], duct).unwrap();
    assert!((t0 - 0.25).abs() < 1e-6 && (t1 - 0.75).abs() < 1e-6);
    assert_eq!(segment_in_box([0.0, 100.0, 30.0], [100.0, 100.0, 30.0], duct), None, "above the duct");
    assert_eq!(segment_in_box([-50.0, 30.0, 30.0], [-10.0, 30.0, 30.0], duct), None, "before it");
}

#[test]
fn a_box_hides_another_only_from_its_side() {
    let near = ([0.0, 0.0, 100.0], [10.0, 10.0, 110.0]);
    let far = ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    let front = [5.0, 5.0, 1000.0];
    assert!(may_hide(&near, &far, front) && !may_hide(&far, &near, front));
    let back = [5.0, 5.0, -1000.0];
    assert!(may_hide(&far, &near, back) && !may_hide(&near, &far, back));
}

#[test]
fn a_wire_is_cut_into_pieces_in_and_between_ducts() {
    let left = ([0.0, 0.0, 0.0], [100.0, 60.0, 60.0]);
    let right = ([200.0, 0.0, 0.0], [300.0, 60.0, 60.0]);
    let pieces = split_by_boxes([-100.0, 30.0, 30.0], [300.0, 30.0, 30.0], &[right, left]);
    let round = |v: f32| (v * 100.0).round() / 100.0;
    let got: Vec<(f32, f32, Option<usize>)> = pieces.into_iter().map(|(a, b, i)| (round(a), round(b), i)).collect();
    assert_eq!(got, vec![(0.0, 0.25, None), (0.25, 0.5, Some(1)), (0.5, 0.75, None), (0.75, 1.0, Some(0))]);
    assert_eq!(split_by_boxes([0.0, 100.0, 0.0], [10.0, 100.0, 0.0], &[left]), vec![(0.0, 1.0, None)]);
}

#[test]
fn wires_go_over_the_ducts_and_under_the_devices() {
    // Paint order: duct 0, device 1, duct 2, device 3.
    let rank = [0, 1, 2, 3];
    // In duct 0 at the junction with duct 2, which is painted later: over both.
    assert_eq!(wire_place([(0, true), (2, true)], &rank), Some((2, true)));
    // Device 3 covers it too: still over the ducts, under the device.
    assert_eq!(wire_place([(0, true), (2, true), (3, false)], &rank), Some((2, true)));
    // A device painted before the last duct covering it wins.
    assert_eq!(wire_place([(2, true), (3, false), (1, false)], &rank), Some((1, false)));
    assert_eq!(wire_place([(3, false)], &rank), Some((3, false)));
    assert_eq!(wire_place([], &rank), None);
}

#[test]
fn a_wire_leaving_a_duct_stays_over_it_seen_from_below() {
    // A duct with a device above it; a wire runs from the duct up into the device.
    let duct = ([0.0, 0.0, 0.0], [100.0, 60.0, 60.0]);
    let device = ([30.0, 80.0, 0.0], [70.0, 160.0, 90.0]);
    let eye = [50.0, -500.0, 1000.0];
    let order = paint_order(&[duct, device], eye);
    let mut rank = [0; 2];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    assert!(rank[0] > rank[1], "from below the duct is painted over the device");
    let (a, b) = ([50.0, 30.0, 30.0], [50.0, 120.0, 30.0]);
    let hiders = |piece: &(V3, V3)| [(0, true), (1, false)].into_iter().filter(|(i, _)| may_hide(&[duct, device][*i], piece, eye)).collect::<Vec<_>>();
    // Up to the terminal inside the device the piece is under the device and so under the
    // duct painted later.
    let whole = ([50.0, 60.0, 30.0], [50.0, 120.0, 30.0]);
    assert_eq!(wire_place(hiders(&whole), &rank), Some((1, false)));
    // Cut at the device (the part inside it is not seen) it goes over the duct.
    let pieces = split_by_boxes(a, b, &[duct, device]);
    assert_eq!(pieces.iter().map(|p| p.2).collect::<Vec<_>>(), vec![Some(0), None, Some(1)]);
    let (t0, t1, _) = pieces[1];
    let at = |t: f32| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t);
    let between = (at(t0), at(t1));
    assert_eq!(wire_place(hiders(&between), &rank), Some((0, true)));
}

#[test]
fn corners_are_rounded_with_the_bend_radius() {
    let path = [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 1000.0, 0.0]];
    let round = fillet(&path, 100.0, 4);
    assert!(!round.contains(&path[1]), "the corner is cut off");
    assert_eq!((round[0], round[round.len() - 1]), (path[0], path[2]));
    let arc = &round[1..round.len() - 1];
    assert!(arc.len() >= 3);
    assert!(arc.iter().all(|q| (dist(*q, [900.0, 100.0, 0.0]) - 100.0).abs() < 0.01), "{arc:?}");
    assert!(dist(arc[0], [900.0, 0.0, 0.0]) < 0.01 && dist(arc[arc.len() - 1], [1000.0, 100.0, 0.0]) < 0.01);
    // Two corners too close together for their arcs bend as one curve through the
    // middle between them.
    let zigzag = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 100.0, 0.0], [200.0, 100.0, 0.0]];
    let round = fillet(&zigzag, 500.0, 4);
    assert!(round.iter().all(|q| q[1] >= -1e-3 && q[1] <= 100.0 + 1e-3 && q[0] >= -1e-3 && q[0] <= 200.0 + 1e-3), "{round:?}");
    assert!(round.iter().any(|q| dist(*q, [100.0, 50.0, 0.0]) < 0.5), "through the middle: {round:?}");
    // Straight lines stay as they are.
    assert_eq!(fillet(&[[0.0; 3], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], 50.0, 4).len(), 3);
}

#[test]
fn a_lane_change_is_an_s_curve() {
    // A jog of 50 between two long stretches, with a bend radius of 125: two arcs of
    // 125 turning 36.9 degrees each, 75 along the way on each side; smooth, and
    // nowhere outside the jog across the way.
    let path = [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 50.0, 0.0], [2000.0, 50.0, 0.0]];
    let round = fillet(&path, 125.0, 4);
    assert_eq!((round[0], round[round.len() - 1]), (path[0], path[3]));
    assert!(round.iter().all(|q| q[1] >= -1e-3 && q[1] <= 50.0 + 1e-3), "{round:?}");
    assert!(round.iter().any(|q| dist(*q, [1000.0, 25.0, 0.0]) < 0.01), "through the middle of the jog: {round:?}");
    let first = round.iter().position(|q| q[1] > 1e-3).unwrap();
    assert!((round[first - 1][0] - 925.0).abs() < 1.0, "leaves the way 75 before the jog: {round:?}");
    // The direction never turns by more than a step of the arcs (18.4 degrees).
    for s in round.windows(3) {
        let (u, v) = (norm(sub(s[1], s[0])), norm(sub(s[2], s[1])));
        assert!(dot(u, v) > 0.94, "{s:?}");
    }
    // The stretches before and after it need not run the same way, nor the jog straight
    // across them: an S-curve of the radius all the same, from one stretch into the other.
    let leaning = [[0.0, 0.0, 0.0], [1000.0, 30.0, 4.0], [1010.0, 80.0, 0.0], [2000.0, 60.0, -5.0]];
    let round = fillet(&leaning, 125.0, 8);
    assert!(round.len() > 8, "{round:?}");
    let (u, w) = (norm(sub(leaning[1], leaning[0])), norm(sub(leaning[3], leaning[2])));
    assert!(dot(norm(sub(round[1], round[0])), u) > 0.9999 && dot(norm(sub(round[round.len() - 1], round[round.len() - 2])), w) > 0.9999);
    for s in round.windows(3) {
        // Every point of the curve bends with the radius: the turn at it over the way to
        // its neighbours.
        let (d0, d1) = (sub(s[1], s[0]), sub(s[2], s[1]));
        let (turn, way) = (dot(norm(d0), norm(d1)).clamp(-1.0, 1.0).acos(), length(d0).min(length(d1)));
        assert!(turn < 1e-3 || way / turn > 0.9 * 125.0 || turn < 0.02, "{turn} over {way}: {round:?}");
    }
    // A jog wider than two radii is two ordinary bends with a straight bit between.
    let wide = [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 400.0, 0.0], [2000.0, 400.0, 0.0]];
    let round = fillet(&wide, 125.0, 4);
    assert!(round.iter().any(|q| dist(*q, [1000.0, 125.0, 0.0]) < 0.01) && round.iter().any(|q| dist(*q, [1000.0, 275.0, 0.0]) < 0.01), "{round:?}");
}

#[test]
fn tubes_are_closed_round_and_face_outwards() {
    let enclosed = |tris: &[(Tri, usize)]| tris.iter().map(|([a, b, c], _)| dot(*a, cross(*b, *c)) / 6.0).sum::<f32>();
    let straight = tube(&[[0.0, 0.0, 0.0], [0.0, 0.0, 1000.0]], 10.0, 12);
    assert_eq!(straight.len(), 12 * 2 + 2 * 12);
    // A twelve-sided prism: 3 r² per mm.
    assert!((enclosed(&straight) - 3.0 * 100.0 * 1000.0).abs() < 1.0, "{}", enclosed(&straight));
    // Bent round a corner every ring stays at the radius from the path.
    let path = fillet(&[[0.0, 0.0, 0.0], [500.0, 0.0, 0.0], [500.0, 0.0, 500.0], [500.0, 400.0, 500.0]], 100.0, 4);
    let bent = tube(&path, 10.0, 10);
    assert!(enclosed(&bent) > 0.0);
    for ([a, b, c], _) in &bent {
        for q in [a, b, c] {
            let near = path.iter().map(|p| dist(*p, *q)).fold(f32::INFINITY, f32::min);
            assert!(near <= 10.5, "{q:?} {near}");
        }
    }
    assert!(bent.iter().any(|(_, k)| *k == 9));
}

#[test]
fn close_corners_get_arcs_that_fit_between_them() {
    // A step of 20 between two right-angled corners into another plane: both corners
    // rounded with arcs that share the stretch between them (radius 10 each), the way
    // never beyond the corners.
    let stair = [[0.0, 0.0, 0.0], [300.0, 0.0, 0.0], [300.0, 20.0, 0.0], [300.0, 20.0, 300.0]];
    let round = fillet(&stair, 100.0, 4);
    assert_eq!((round[0], round[round.len() - 1]), (stair[0], stair[3]));
    assert!(!round.contains(&stair[1]) && !round.contains(&stair[2]), "{round:?}");
    assert!(round.iter().all(|q| q[0] <= 300.0 + 1e-3 && q[1] >= -1e-3 && q[1] <= 20.0 + 1e-3), "{round:?}");
    assert!(curvature(&round) * 10.0 <= 1.05, "{}", curvature(&round));
    // A corner 60 from the start of the way: an arc of 60, from the start on.
    let early = [[0.0, 0.0, 0.0], [60.0, 0.0, 0.0], [60.0, 400.0, 0.0]];
    let round = fillet(&early, 100.0, 4);
    assert_eq!(round[0], early[0]);
    assert!(round.iter().all(|q| q[0] <= 60.0 + 1e-3), "{round:?}");
    assert!(curvature(&round) * 60.0 <= 1.03, "{}", curvature(&round));
    // Many corners in a row: each with an arc that fits, within the corners.
    let stairs = [[0.0, 0.0, 0.0], [200.0, 0.0, 0.0], [200.0, 15.0, 0.0], [200.0, 15.0, 15.0], [200.0, 30.0, 15.0], [200.0, 30.0, 300.0]];
    let round = fillet(&stairs, 50.0, 4);
    assert!(round.iter().all(|q| q[0] <= 200.0 + 1e-3 && q[1] <= 30.0 + 1e-3 && q[2] <= 300.0 + 1e-3 && q[2] >= -1e-3), "{round:?}");
    assert_eq!((round[0], round[round.len() - 1]), (stairs[0], stairs[5]));
    // A corner turning by a few degrees only stays sharp.
    let dogleg = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [200.0, 5.0, 0.0]];
    assert_eq!(fillet(&dogleg, 50.0, 4), dogleg.to_vec());
}

#[test]
fn each_corner_gets_its_own_radius() {
    let path = [[0.0, 0.0, 0.0], [500.0, 0.0, 0.0], [500.0, 500.0, 0.0], [0.0, 500.0, 0.0]];
    let round = fillet_each(&path, &[0.0, 50.0, 0.0, 0.0], 4);
    assert!(!round.contains(&path[1]) && round.contains(&path[2]), "{round:?}");
    assert!(round.iter().any(|q| dist(*q, [450.0, 0.0, 0.0]) < 0.01));
    assert_eq!(fillet_each(&path, &[], 4), path.to_vec());
}

#[test]
fn polylines_are_split_along_their_length() {
    let path = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
    assert_eq!(path_length(&path), 20.0);
    let (head, tail) = split_at(&path, 4.0);
    assert_eq!(head, vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    assert_eq!(tail, vec![[4.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]]);
    // Past the corner.
    let (head, tail) = split_at(&path, 15.0);
    assert_eq!(head.last(), Some(&[10.0, 5.0, 0.0]));
    assert_eq!(head.len(), 3);
    assert_eq!(tail, vec![[10.0, 5.0, 0.0], [10.0, 10.0, 0.0]]);
    // Past the end: all of it before.
    let (head, tail) = split_at(&path, 30.0);
    assert_eq!((head, tail), (path.to_vec(), vec![[10.0, 10.0, 0.0]]));
}
