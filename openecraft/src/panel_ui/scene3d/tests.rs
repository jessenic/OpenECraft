use super::*;

#[test]
fn duct_fingers_a_tube_passes_through_are_left_out() {
    // Two fingers of a duct; a cable passes through the first, beside the second.
    let duct = ([0.0, 0.0, 0.0], [100.0, 60.0, 60.0]);
    let fingers = vec![(0, 0..6, ([10.0, 0.0, 20.0], [16.0, 2.0, 60.0])), (0, 6..12, ([60.0, 0.0, 20.0], [66.0, 2.0, 60.0]))];
    let through = vec![([13.0, -50.0, 40.0], [13.0, 30.0, 40.0])].into_iter().map(|(a, b)| (vec![a, b], 4.0)).collect::<Vec<_>>();
    assert_eq!(fingers_hit(&fingers, &through, &[duct]), vec![0]);
    // Touching it with its thickness is enough; far away nothing.
    let touching = vec![(vec![[21.0, -50.0, 40.0], [21.0, 30.0, 40.0]], 6.0)];
    assert_eq!(fingers_hit(&fingers, &touching, &[duct]), vec![0]);
    let away = vec![(vec![[40.0, -50.0, 40.0], [40.0, 30.0, 40.0]], 4.0)];
    assert!(fingers_hit(&fingers, &away, &[duct]).is_empty());
}

#[test]
fn wires_along_the_plate_go_over_what_stands_in_their_way() {
    // A run 30 mm in front of the plate crosses a busbar 20 .. 70 mm in front of it.
    let busbar = ([100.0, 0.0, 20.0], [200.0, 50.0, 70.0]);
    let way = over(&[[0.0, 25.0, 30.0], [300.0, 25.0, 30.0]], &[busbar], &[], 1.5, 0.0);
    assert_eq!((way[0], way[way.len() - 1]), ([0.0, 25.0, 30.0], [300.0, 25.0, 30.0]));
    assert!(way.iter().any(|q| q[2] >= 72.5), "in front of it: {way:?}");
    for s in way.windows(2) {
        assert!(vec3::segment_in_box(s[0], s[1], ([98.5, -1.5, 18.5], [201.5, 51.5, 71.5])).is_none(), "{way:?}");
    }
    // Beside it nothing changes.
    let beside = [[0.0, 80.0, 30.0], [300.0, 80.0, 30.0]];
    assert_eq!(over(&beside, &[busbar], &[], 1.5, 0.0), beside.to_vec());
}

#[test]
fn wires_leave_their_own_devices_at_the_height_of_their_connection_points() {
    // A way along the plate (z 5) from a connection point inside the device (box x 0 ..
    // 50, y 0 .. 80) out to x 100: it leaves at the height of the connection point (60).
    let mut way = vec![[25.0, 70.0, 5.0], [25.0, 100.0, 5.0], [100.0, 100.0, 5.0]];
    leave_device(&mut way, ([0.0, 0.0, 0.0], [50.0, 80.0, 70.0]), 60.0);
    assert_eq!(way, vec![[25.0, 100.0, 60.0], [25.0, 100.0, 5.0], [100.0, 100.0, 5.0]]);
    // Nothing inside the device or all of it: as it is.
    let outside = vec![[60.0, 100.0, 5.0], [100.0, 100.0, 5.0]];
    let mut same = outside.clone();
    leave_device(&mut same, ([0.0, 0.0, 0.0], [50.0, 80.0, 70.0]), 60.0);
    assert_eq!(same, outside);
}

#[test]
fn wires_come_down_before_the_ducts_not_through_their_covers() {
    // A run at duct depth (z 30) crosses a device (z up to 70) standing right before a
    // duct (y 100 .. 150, 60 deep): it comes down before the wall of the duct, not into
    // it through its cover.
    let device = ([100.0, 0.0, 0.0], [200.0, 100.0, 70.0]);
    let duct = ([0.0, 100.0, 0.0], [300.0, 150.0, 60.0]);
    let way = over(&[[150.0, -50.0, 30.0], [150.0, 125.0, 30.0]], &[device], &[duct], 1.5, 0.0);
    let down = way.windows(2).find(|s| s[0][2] > 30.0 && s[1][2] == 30.0).expect("comes down");
    assert!(down[1][1] <= 100.0, "{way:?}");
    assert_eq!(way[way.len() - 1], [150.0, 125.0, 30.0]);
    // Without the duct it comes down right behind the device.
    let plain = over(&[[150.0, -50.0, 30.0], [150.0, 125.0, 30.0]], &[device], &[], 1.5, 0.0);
    assert!(plain.windows(2).any(|s| s[0][2] > 30.0 && s[1][2] == 30.0 && s[1][1] > 100.0), "{plain:?}");
}

#[test]
fn wires_come_down_to_the_duct_before_its_wall() {
    // Out of a device (connection point at y 200, z 150) to a duct (y 0 .. 80, 100 deep)
    // whose depth the way runs at: it comes down just before the wall of the duct, not
    // inside the duct through its cover.
    let duct = ([0.0, 0.0, 20.0], [600.0, 80.0, 100.0]);
    let mut along = vec![[300.0, 40.0, 150.0], [300.0, 40.0, 50.0], [100.0, 40.0, 50.0]];
    down_before_duct(&mut along, [300.0, 200.0, 150.0], &[duct], 1.5);
    assert_eq!(along[0][2], 150.0);
    assert!(along[0][1] >= 80.0 + 2.0 && along[0][1] < 90.0, "{along:?}");
    assert_eq!(along[1], [along[0][0], along[0][1], 50.0]);
    assert_eq!(&along[2..], &[[300.0, 40.0, 50.0], [100.0, 40.0, 50.0]]);
    // Outside the ducts it stays as it is.
    let mut plain = vec![[300.0, 140.0, 150.0], [300.0, 140.0, 50.0]];
    down_before_duct(&mut plain, [300.0, 200.0, 150.0], &[duct], 1.5);
    assert_eq!(plain, vec![[300.0, 140.0, 150.0], [300.0, 140.0, 50.0]]);
}

#[test]
fn wire_ends_are_cut_off_the_way() {
    use openecraft_core::production::{EndPiece, EndShape, Termination};
    let way = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 100.0, 0.0]];
    let (red, grey) = (Color32::RED, Color32::GRAY);
    let round = |length: f32, diameter: f32, c: Color32| (EndPiece { length, diameter, color: [0; 3], shape: EndShape::Round }, c);
    // A ferrule (sleeve and collar) at the first end, the bare conductor at the last.
    let ends = [vec![round(8.0, 1.8, grey), round(6.0, 3.8, red)], vec![round(8.0, 1.6, Color32::BROWN)]];
    let up = [0.0, 0.0, 1.0];
    let pieces = with_ends(&way, 3.0, Color32::BLUE, None, &ends, up);
    let looks: Vec<(f32, f32, Color32)> = pieces.iter().map(|p| (vec3::path_length(&p.path), p.diameter, p.color)).collect();
    assert_eq!(looks.len(), 4);
    assert_eq!((looks[0].1, looks[0].2, looks[1].1, looks[1].2), (1.8, grey, 3.8, red));
    assert!((looks[0].0 - 8.0).abs() < 1e-4 && (looks[1].0 - 6.0).abs() < 1e-4, "{looks:?}");
    assert_eq!((looks[2].1, looks[2].2), (3.0, Color32::BLUE));
    assert!((looks[2].0 - 178.0).abs() < 1e-3, "{looks:?}");
    assert_eq!(pieces[0].path[0], way[0]);
    assert_eq!(pieces[3].path.last(), Some(&way[2]));
    assert!((looks[3].0 - 8.0).abs() < 1e-4 && looks[3].2 == Color32::BROWN);
    assert!(pieces.iter().all(|p| p.flat.is_empty()));
    // Too short for its ends: drawn without them.
    let short = [[0.0, 0.0, 0.0], [20.0, 0.0, 0.0]];
    assert_eq!(with_ends(&short, 3.0, Color32::BLUE, None, &ends, up).len(), 1);
    // The look of a wire end: ferrule or bare copper, as the layout sets.
    let mut prj = openecraft_core::sample::demo_project();
    let wire_ends = openecraft_core::production::Ends::default();
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let colors = |look: &EndLook| look.iter().map(|p| p.1).collect::<Vec<_>>();
    assert_eq!(colors(&wire_end(&prj, &wire_ends, "-K1:1", "-X1:1", "1.5 mm²", false)), vec![rgb(openecraft_core::production::SLEEVE_RGB), rgb([35, 35, 38])]);
    prj.layout.ferrules = false;
    assert_eq!(colors(&wire_end(&prj, &wire_ends, "-K1:1", "-X1:1", "1.5 mm²", false)), vec![rgb(openecraft_core::production::COPPER_RGB)]);
    assert!(wire_end(&prj, &wire_ends, "-K1:1", "-X1:1", "", false).is_empty());
    // A ring terminal chosen for the last end: its flat ring lies round the end of the
    // way, across `up`.
    openecraft_core::production::set_termination(&mut prj, "-K1:1", "", Some(Termination::Ring));
    // Two wires sharing a twin ferrule: it is shown at the first of them, the other one
    // shows only its conductor in it.
    let pair = openecraft_core::production::Ends::of([("-X9:1", "-K8:1"), ("-X9:1", "-K9:1")].map(|(a, b)| (a.to_string(), b.to_string(), "1.5 mm²".to_string())));
    prj.layout.ferrules = true;
    openecraft_core::production::choose_termination(&mut prj, &pair, "-X9:1", "-K8:1", Some(Termination::TwinFerrule));
    assert_eq!(colors(&wire_end(&prj, &pair, "-X9:1", "-K8:1", "1.5 mm²", false))[0], rgb(openecraft_core::production::SLEEVE_RGB));
    assert_eq!(colors(&wire_end(&prj, &pair, "-X9:1", "-K9:1", "1.5 mm²", false)), vec![rgb(openecraft_core::production::COPPER_RGB)]);
    let ring = wire_end(&prj, &wire_ends, "-K1:1", "-X1:1", "1.5 mm²", false);
    assert!(matches!(ring[0].0.shape, EndShape::Ring { .. }));
    let pieces = with_ends(&way, 3.0, Color32::BLUE, None, &[Vec::new(), ring], up);
    let tip = pieces.last().unwrap();
    assert!(!tip.flat.is_empty());
    let near = |v: &V3| vec3::length(sub(*v, way[2])) <= 8.0;
    assert!(tip.flat.iter().flatten().all(near), "round the connection point");
    assert!(tip.flat.iter().flatten().all(|v| v[2].abs() <= 0.5), "flat across up");
}
