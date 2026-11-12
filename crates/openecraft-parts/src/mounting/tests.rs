use super::*;

#[test]
fn dimensions_read_typed_and_legacy_values_without_accepting_invalid_units() {
    let mut part=Part {tech_data:"Outer diameter: 6,3 MM\nOffset: -2 mm\nBad: NaN\nWrong unit: 1 cm".into(),..Default::default()};
    assert_eq!(part.dimension_mm("Outer diameter"),Some(6.3));
    assert_eq!(part.dimension_mm("Offset"),Some(-2.));
    assert_eq!(part.dimension_mm("Bad"),None);
    assert_eq!(part.dimension_mm("Wrong unit"),None);
    assert_eq!(part.dimension_mm("Missing"),None);
    assert!(part.mounting_data.set("Clearance top","12 mm"));
    assert_eq!(part.dimension_mm("Clearance top"),Some(12.));
}

#[test]
fn values_round_trip_through_text() {
    let mut m = MountingData::default();
    assert!(m.set("Power loss", "7.5 W"));
    assert!(m.set("clearance top", "50 mm"));
    assert!(m.set("Mounting holes", "M4: 6,8; 42,8"));
    assert!(m.set("Door cutout", "Ø22.5"));
    assert!(m.set("Cutout", "92 x 92"));
    assert!(m.set("Faces", "Left, Right"));
    assert_eq!(m.get("Power loss").as_deref(), Some("7.5"));
    assert_eq!(m.get("Clearance top").as_deref(), Some("50"));
    assert_eq!(m.get("Mounting holes").as_deref(), Some("M4: 6,8; 42,8"));
    assert_eq!(m.get("Door cutout").as_deref(), Some("Ø22.5"));
    assert_eq!(m.get("Cutout").as_deref(), Some("92 x 92"));
    assert_eq!(m.get("Faces").as_deref(), Some("Left, Right"));
    assert_eq!(m.get("Clearance left"), None);
    assert!(m.set("Power loss", ""));
    assert_eq!(m.get("Power loss"), None);
}

#[test]
fn bad_values_and_unknown_keys_are_refused() {
    let mut m = MountingData::default();
    assert!(!m.set("Power loss", "a lot"));
    assert!(!m.set("Mounting holes", "M4 6,8"));
    assert!(!m.set("Door cutout", "round"));
    assert!(!m.set("Colour", "grey"));
    assert!(m.is_empty());
    assert!(m.set("Clearance", "100"));
    assert_eq!((m.clearance_top, m.clearance_bottom), (100.0, 100.0));
}

#[test]
fn cutouts_parse_round_and_rectangular() {
    assert_eq!(Cutout::parse("Ø30.5"), Some(Cutout::Round { d: 30.5 }));
    assert_eq!(Cutout::parse("22,5 mm"), Some(Cutout::Round { d: 22.5 }));
    assert_eq!(Cutout::parse("177 × 177"), Some(Cutout::Rect { w: 177.0, h: 177.0 }));
    assert_eq!(Cutout::parse("0"), None);
}

#[test]
fn tech_data_lines_move_into_typed_fields() {
    let mut p = Part {
        tech_data: "Power loss: 3 W\nClearance: 100 mm (above and below)\nMounting holes: M4: 6,8; 42,8\nCoil voltage: 24 V DC\nDoor cutout: oval".into(),
        ..Default::default()
    };
    assert_eq!(p.data_value("Power loss").as_deref(), Some("3 W"), "free text before");
    assert_eq!(p.migrate_tech_data(), 3);
    assert_eq!(p.tech_data, "Coil voltage: 24 V DC\nDoor cutout: oval", "unknown keys and unreadable values stay");
    assert_eq!(p.mounting_data.power_loss, 3.0);
    assert_eq!(p.mounting_data.clearance_bottom, 100.0);
    assert_eq!(p.mounting_data.holes, vec![[6.0, 8.0], [42.0, 8.0]]);
    assert_eq!(p.data_value("Power loss").as_deref(), Some("3"));
    assert_eq!(p.data_value("Door cutout").as_deref(), Some("oval"), "falls back to the technical data");
    assert_eq!(p.migrate_tech_data(), 0);
}

#[test]
fn typed_values_win_over_the_technical_data() {
    let mut p = Part { tech_data: "Power loss: 3 W".into(), ..Default::default() };
    p.mounting_data.power_loss = 5.0;
    assert_eq!(p.data_value("Power loss").as_deref(), Some("5"));
    assert_eq!(p.migrate_tech_data(), 0, "a typed value is not overwritten");
    assert_eq!(p.tech_data, "Power loss: 3 W");
}

#[test]
fn empty_data_is_not_serialized() {
    let p = Part::default();
    assert!(!serde_json::to_string(&p).unwrap().contains("mounting_data"));
    let mut q = Part::default();
    q.mounting_data.power_loss = 2.0;
    let json = serde_json::to_string(&q).unwrap();
    assert!(json.contains("\"mounting_data\":{\"power_loss\":2.0}"), "{json}");
    let back: Part = serde_json::from_str(&json).unwrap();
    assert_eq!(back.mounting_data.power_loss, 2.0);
}

#[test]
fn wall_mounting_is_a_typed_flag() {
    let mut m = MountingData::default();
    assert_eq!(m.get("Wall mounting"), None);
    assert!(m.set("Wall mounting", "yes"));
    assert!(m.wall_mount);
    assert_eq!(m.get("Wall mounting").as_deref(), Some("yes"));
    assert!(!m.set("Wall mounting", "maybe"));
    assert!(m.set("Wall mounting", ""));
    assert!(m.is_empty());
    let mut p = Part { tech_data: "Wall mounting: yes".into(), ..Default::default() };
    assert!(p.wall_mountable(), "from the technical data");
    assert_eq!(p.migrate_tech_data(), 1);
    assert!(p.mounting_data.wall_mount && p.wall_mountable());
    assert!(Part { mounting: "Wall mounting".into(), ..Default::default() }.wall_mountable());
    assert!(!Part::default().wall_mountable());
}

#[test]
fn floor_mounting_is_a_typed_flag() {
    let mut m = MountingData::default();
    assert!(m.set("Floor mounting", "yes"));
    assert!(m.floor_mount && !m.wall_mount);
    assert_eq!(m.get("Floor mounting").as_deref(), Some("yes"));
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(json, "{\"floor_mount\":true}");
    assert_eq!(serde_json::from_str::<MountingData>(&json).unwrap(), m);
    assert!(m.set("Floor mounting", "no"));
    assert!(m.is_empty());
    let mut p = Part { tech_data: "Floor mounting: yes".into(), ..Default::default() };
    assert!(p.floor_standing(), "from the technical data");
    assert_eq!(p.migrate_tech_data(), 1);
    assert!(p.mounting_data.floor_mount && p.tech_data.is_empty());
    assert!(Part { mounting: "Floor".into(), ..Default::default() }.floor_standing());
    assert!(!Part::default().floor_standing());
}

#[test]
fn placed_cutouts_in_their_text_form() {
    let c = PlacedCutout::parse("197 x 141 @ 0/-6,5 R3 15°").unwrap();
    assert_eq!(c, PlacedCutout { shape: Cutout::Rect { w: 197.0, h: 141.0 }, offset: [0.0, -6.5], angle: 15.0, radius: 3.0 });
    assert_eq!(c.text(), "197 x 141 @ 0/-6.5 R3 15°");
    assert_eq!(PlacedCutout::parse("92 x 92 R2").unwrap().radius, 2.0, "radius without a place");
    assert_eq!(PlacedCutout::parse("22,5 mm").unwrap(), PlacedCutout::centered(Cutout::Round { d: 22.5 }));
    assert!(PlacedCutout::parse("92 x 92 @ 10").is_none(), "x and y");
    let list = parse_cutouts("197 x 141 @ 0/-6.5; Ø16 @ 80/60").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(cutouts_text(&list), "197 x 141 @ 0/-6.5; Ø16 @ 80/60");
    let mut m = MountingData::default();
    assert!(m.set("Cutout", "197 x 141 @ 0/-6.5; Ø16 @ 80/60"));
    assert_eq!(m.cutouts, list);
    assert!(m.set("Cutout", ""));
    assert!(m.cutouts.is_empty());
    assert!(!m.set("Cutout", "92 x 92; oval"), "all or nothing");
}

#[test]
fn single_cutouts_of_older_files_are_read_as_a_centred_one() {
    let old: MountingData = serde_json::from_str(r#"{"cutout": {"rect": {"w": 177.0, "h": 177.0}}, "door_cutout": {"round": {"d": 22.5}}}"#).unwrap();
    assert_eq!(old.cutouts, [PlacedCutout::centered(Cutout::Rect { w: 177.0, h: 177.0 })]);
    assert_eq!(old.door_cutouts, [PlacedCutout::centered(Cutout::Round { d: 22.5 })]);
    let mut new = old.clone();
    new.cutouts[0].offset = [0.0, 5.0];
    let text = serde_json::to_string(&new).unwrap();
    assert!(text.contains(r#""cutouts":[{"shape":{"rect""#), "{text}");
    assert_eq!(serde_json::from_str::<MountingData>(&text).unwrap(), new);
    assert!(serde_json::from_str::<MountingData>(r#"{"cutout": null}"#).unwrap().cutouts.is_empty());
}

#[test]
fn cutouts_of_a_part_and_their_outline() {
    let mut p = Part { width: 100.0, height: 50.0, tech_data: "Cutout: 40 x 20 @ 10/0".into(), ..Default::default() };
    assert_eq!(p.cutouts(false)[0].offset, [10.0, 0.0], "from the technical data of older parts");
    assert_eq!(p.cutouts(true), p.cutouts(false), "the door takes the cutouts without door cutouts");
    p.mounting_data.door_cutouts = vec![PlacedCutout::centered(Cutout::Round { d: 22.5 })];
    assert_eq!(p.cutouts(true).len(), 1);
    assert!(matches!(p.cutouts(true)[0].shape, Cutout::Round { .. }));
    let outline = p.cutouts(false)[0].outline(p.width, p.height);
    assert_eq!(outline, [[80.0, 35.0], [40.0, 35.0], [40.0, 15.0], [80.0, 15.0]]);
    let turned = PlacedCutout { angle: 90.0, ..p.cutouts(false)[0] }.outline(p.width, p.height);
    assert!(turned.iter().all(|[x, y]| (x - 60.0).abs() <= 10.0 + 1e-4 && (y - 25.0).abs() <= 20.0 + 1e-4), "{turned:?}");
}

#[test]
fn cutouts_survive_the_app_storage() {
    let m = MountingData {
        cutouts: vec![PlacedCutout { offset: [0.0, -6.5], angle: 15.0, ..PlacedCutout::centered(Cutout::Rect { w: 197.0, h: 141.0 }) }],
        door_cutouts: vec![PlacedCutout::centered(Cutout::Round { d: 22.5 })],
        ..Default::default()
    };
    let text = ron::to_string(&m).unwrap();
    assert_eq!(ron::from_str::<MountingData>(&text).unwrap(), m);
    // Stored by older versions: one optional cutout.
    let old: MountingData = ron::from_str("(cutout: Some(rect(w: 177.0, h: 177.0)), door_cutout: None)").unwrap();
    assert_eq!(old.cutouts, [PlacedCutout::centered(Cutout::Rect { w: 177.0, h: 177.0 })]);
    assert!(old.door_cutouts.is_empty());
}
