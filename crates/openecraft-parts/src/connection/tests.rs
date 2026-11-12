use super::*;
use crate::{default_catalog, FunctionTemplate};

fn catalog_part(number: &str) -> Part {
    default_catalog().into_iter().find(|p| p.number == number).unwrap()
}

fn pin<'a>(points: &'a [ConnectionPoint], name: &str) -> &'a ConnectionPoint {
    points.iter().find(|c| c.pin == name).unwrap_or_else(|| panic!("no point {name}"))
}

#[test]
fn iec_sides() {
    for top in ["1", "3", "5", "A1", "13", "21", "31", "53", "95", "L", "N", "PE", "L1", "L2", "L3", "1.1", "1.2", "I0.1", "1L1", "3L2", "5L3"] {
        assert!(upper_side(top), "{top} is on top");
    }
    for bottom in ["2", "4", "6", "A2", "14", "22", "32", "34", "96", "L+", "M", "2.1", "2.2", "BN", "", "I0.0", "2T1", "4T2", "6T3", "U", "N'"] {
        assert!(!upper_side(bottom), "{bottom} is at the bottom");
    }
}

#[test]
fn contactor_points_in_two_rows() {
    let part = catalog_part("GEN.CT-9A-24DC");
    let points = default_connection_points(&part);
    assert_eq!(points.len(), 10, "A1 A2, 1-6, 13 14");
    let (a1, a2) = (pin(&points, "A1"), pin(&points, "A2"));
    assert_eq!((a1.pos[1], a1.exit), (0.0, Exit::Up));
    assert_eq!((a2.pos[1], a2.exit), (part.height, Exit::Down));
    // Upper row A1 1 3 5 13 spread over the width, left to right.
    let upper: Vec<&str> = points.iter().filter(|c| c.exit == Exit::Up).map(|c| c.pin.as_str()).collect();
    assert_eq!(upper, ["A1", "1", "3", "5", "13"]);
    let xs: Vec<f32> = points.iter().filter(|c| c.exit == Exit::Up).map(|c| c.pos[0]).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]));
    assert!(xs.iter().all(|x| *x > 0.0 && *x < part.width));
    assert!(points.iter().all(|c| c.pos[2] > 0.0 && c.pos[2] < part.depth));
}

#[test]
fn din_rail_devices_are_entered_from_their_end_faces() {
    // Below the screws on their steps (40 % of the depth); devices on the plate somewhat
    // behind their fronts.
    let mut part = catalog_part("GEN.CT-9A-24DC");
    assert!(part.mounting.starts_with("Mounting rail"), "{}", part.mounting);
    let z = |part: &Part| default_connection_points(part).iter().map(|c| c.pos[2]).collect::<Vec<f32>>();
    assert!(z(&part).iter().all(|&z| (z - 0.4 * part.depth).abs() < 1e-3), "{:?}", z(&part));
    part.mounting = "Mounting plate".into();
    assert!(z(&part).iter().all(|&z| (z - 0.75 * part.depth).abs() < 1e-3), "{:?}", z(&part));
}

#[test]
fn terminals_have_one_point_per_side() {
    let points = default_connection_points(&catalog_part("GEN.TB-2.5"));
    assert_eq!(points.iter().map(|c| (c.pin.as_str(), c.exit)).collect::<Vec<_>>(), [("1", Exit::Up), ("2", Exit::Down)]);
    assert_eq!(points[0].pos[0], 2.6, "centre of the terminal");
}

#[test]
fn door_devices_are_wired_from_the_rear() {
    let part = catalog_part("GEN.PB-GN-1NO");
    let points = default_connection_points(&part);
    assert_eq!(points.len(), 2);
    assert!(points.iter().all(|c| c.exit == Exit::Front && c.pos[2] == part.depth));
    assert!(pin(&points, "13").pos[1] < pin(&points, "14").pos[1]);
}

#[test]
fn plc_modules_are_wired_at_the_bottom() {
    let part = default_catalog().into_iter().find(|p| p.functions.iter().any(|f| f.kind == FnKind::PlcInput)).unwrap();
    let points = default_connection_points(&part);
    assert!(!points.is_empty());
    assert!(points.iter().all(|c| c.exit == Exit::Down));
}

#[test]
fn surge_arresters_have_the_earth_at_the_bottom() {
    let arrester = |groups: &[(FnKind, &str)]| Part {
        width: 72.0,
        height: 90.0,
        depth: 66.0,
        functions: groups.iter().map(|(k, pins)| FunctionTemplate::new(*k, pins)).collect(),
        ..Default::default()
    };
    let side = |points: &[ConnectionPoint], exit| points.iter().filter(|c| c.exit == exit).map(|c| c.pin.clone()).collect::<Vec<_>>();
    // TN-S with remote signalling contact: lines on top, PE and the contact below.
    use FnKind::{Changeover, SurgeProtection as Spd};
    let points = default_connection_points(&arrester(&[(Spd, "L1 PE"), (Spd, "L2 PE"), (Spd, "L3 PE"), (Spd, "N PE"), (Changeover, "11 12 14")]));
    assert_eq!(side(&points, Exit::Up), ["L1", "L2", "L3", "N"]);
    assert_eq!(side(&points, Exit::Down), ["PE", "11", "12", "14"]);
    // TT: N is a line of its own module; DC: L+ and L- on top.
    assert_eq!(side(&default_connection_points(&arrester(&[(Spd, "L1 N"), (Spd, "N PE")])), Exit::Down), ["PE"]);
    assert_eq!(side(&default_connection_points(&arrester(&[(Spd, "L+ PE"), (Spd, "L- PE")])), Exit::Up), ["L+", "L-"]);
    assert_eq!(side(&default_connection_points(&arrester(&[(Spd, "L1 PEN")])), Exit::Down), ["PEN"]);
}

#[test]
fn duplicate_and_missing_data() {
    // The same designation in two functions is one point.
    let part = Part {
        width: 20.0,
        height: 40.0,
        depth: 30.0,
        functions: vec![FunctionTemplate::new(FnKind::Generic, "1 2"), FunctionTemplate::new(FnKind::Generic, "2 3")],
        ..Default::default()
    };
    assert_eq!(default_connection_points(&part).len(), 3);
    // Without a size there is nothing to place (e.g. motors in the field).
    assert!(default_connection_points(&catalog_part("GEN.MOT-0.75KW")).is_empty());
}

#[test]
fn own_points_win_over_generated_ones() {
    let mut part = catalog_part("GEN.MCB-1P-C6");
    let (generated, exact) = part.connection_points_or_default();
    assert!(!exact);
    assert_eq!(generated.len(), 2);
    part.connection_points = vec![ConnectionPoint { pin: "1".into(), pos: [9.0, 10.0, 50.0], exit: Exit::Left, ..Default::default() }];
    assert_eq!(part.connection_points_or_default(), (part.connection_points.clone(), true));
}

#[test]
fn text_form() {
    let points = vec![
        ConnectionPoint { pin: "A1".into(), pos: [5.5, 0.0, 60.0], exit: Exit::Up, ..Default::default() },
        ConnectionPoint { pin: "1.1".into(), pos: [1.0, 2.0, 3.0], exit: Exit::Front, ..Default::default() },
    ];
    let text = points_to_text(&points);
    assert_eq!(text, "A1:5.5/0/60/up|1.1:1/2/3/front");
    assert_eq!(points_from_text(&text), points);
    // Decimal commas, blanks, broken entries.
    let read = points_from_text(" 13 : 2,5 / 0 / 10 / DOWN | broken | X:1/2/up ");
    assert_eq!(read, vec![ConnectionPoint { pin: "13".into(), pos: [2.5, 0.0, 10.0], exit: Exit::Down, ..Default::default() }]);
    assert!(points_from_text("").is_empty());
}

#[test]
fn json_keeps_old_files_valid() {
    let part = catalog_part("GEN.MCB-1P-C6");
    let json = serde_json::to_string(&part).unwrap();
    assert!(!json.contains("connection_points"), "empty list is not written");
    let back: Part = serde_json::from_str(&json).unwrap();
    assert_eq!(back, part);
    let mut own = part.clone();
    own.connection_points = vec![ConnectionPoint { pin: "1".into(), pos: [1.0, 2.0, 3.0], exit: Exit::Right, ..Default::default() }];
    let back: Part = serde_json::from_str(&serde_json::to_string(&own).unwrap()).unwrap();
    assert_eq!(back.connection_points, own.connection_points);
}

#[test]
fn text_form_keeps_wire_limits() {
    let points = vec![
        ConnectionPoint { pin: "1".into(), pos: [5.0, 0.0, 40.0], exit: Exit::Up, min_cross_section: 0.2, max_cross_section: 1.5, max_wires: 1 },
        ConnectionPoint { pin: "2".into(), pos: [5.0, 90.0, 40.0], exit: Exit::Down, ..Default::default() },
    ];
    let text = points_to_text(&points);
    assert_eq!(text, "1:5/0/40/up/0.2/1.5/1|2:5/90/40/down");
    assert_eq!(points_from_text(&text), points);
    assert!(points[0].has_wire_limits() && !points[1].has_wire_limits());
    // Five or six fields are broken.
    assert!(points_from_text("1:5/0/40/up/0.2|2:5/0/40/up/0.2/1.5").is_empty());
}
