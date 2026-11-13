use super::*;
use crate::{default_catalog, ConnectionPoint, Exit};

#[test]
fn csv_roundtrip() {
    let parts = default_catalog();
    let csv = to_csv(&parts);
    let back = from_csv(&csv).unwrap();
    assert_eq!(parts.len(), back.len());
    for (a, b) in parts.iter().zip(&back) {
        assert_eq!(a.number, b.number);
        assert_eq!(a.functions, b.functions);
        assert_eq!(a.accessories, b.accessories);
        assert_eq!((a.accessory, a.accessory_mount, &a.fits), (b.accessory, b.accessory_mount, &b.fits), "{}", a.number);
        assert_eq!(a.description, b.description);
    }
}

#[test]
fn accessories_in_csv() {
    let part = Part { number: "OWN.AUX".into(), accessory: true, accessory_mount: crate::AccessoryMount::Left, fits: vec!["OWN.K1".into(), "OWN.K2".into()], ..Default::default() };
    let back = from_csv(&to_csv(std::slice::from_ref(&part))).unwrap();
    assert_eq!((back[0].accessory, back[0].accessory_mount, back[0].fits.clone()), (true, crate::AccessoryMount::Left, part.fits));
    // Files without the columns read plain parts.
    let plain = from_csv("Part number;Accessories
OWN.K1;OWN.AUX
").unwrap();
    assert_eq!((plain[0].accessory, plain[0].accessory_mount, plain[0].accessories.clone()), (false, crate::AccessoryMount::Auto, vec!["OWN.AUX".to_string()]));
}

#[test]
fn json_import() {
    let db = PartsDb::default();
    let json = serde_json::to_string(&db).unwrap();
    assert_eq!(import(&json).unwrap().len(), db.parts.len());
}

#[test]
fn connection_points_in_csv() {
    let mut part = Part { number: "OWN.K1".into(), ..Default::default() };
    part.connection_points = vec![
        ConnectionPoint { pin: "A1".into(), pos: [5.5, 0.0, 60.0], exit: Exit::Up, ..Default::default() },
        ConnectionPoint { pin: "14".into(), pos: [40.0, 74.0, 55.25], exit: Exit::Down, ..Default::default() },
    ];
    let csv = to_csv(std::slice::from_ref(&part));
    let header = csv.lines().next().unwrap();
    assert!(header.contains(";Connection points;Catalog;Lifecycle;Successor;Clearance top;") && header.ends_with(";Terminal type;Wall mounting;Floor mounting;Components;Accessory;Accessory mounting;Fits"));
    assert!(csv.contains("A1:5.5/0/60/up|14:40/74/55.25/down"));
    assert_eq!(from_csv(&csv).unwrap()[0].connection_points, part.connection_points);
    // Files without the column (older exports) have no points.
    let old = "Part number;Description
OWN.K2;Relay
";
    assert!(from_csv(old).unwrap()[0].connection_points.is_empty());
}


#[test]
fn typed_mounting_data_in_csv() {
    let mut part = Part { number: "X.1".into(), ..Default::default() };
    part.mounting_data.power_loss = 7.5;
    part.mounting_data.set("Mounting holes", "M5: 9,12; 69,72");
    part.mounting_data.set("Door cutout", "Ø30.5");
    let csv = to_csv(std::slice::from_ref(&part));
    let back = from_csv(&csv).unwrap();
    assert_eq!(back[0].mounting_data, part.mounting_data);
    // Values that cannot be read stay in the technical data.
    let text = "Part number;Power loss;Door cutout\nY.1;a lot;Ø22.5\n";
    let y = &from_csv(text).unwrap()[0];
    assert_eq!(y.mounting_data.door_cutouts, [crate::PlacedCutout::centered(crate::Cutout::Round { d: 22.5 })]);
    assert_eq!(y.tech_data, "Power loss: a lot");
}

#[test]
fn import_moves_technical_data_into_typed_fields() {
    let text = "Part number;Technical data\nZ.1;\"Power loss: 4 W\nCoil voltage: 24 V\"\n";
    let z = &crate::import(text).unwrap()[0];
    assert_eq!(z.mounting_data.power_loss, 4.0);
    assert_eq!(z.tech_data, "Coil voltage: 24 V");
}

#[test]
fn assembly_components_in_csv() {
    let mut part = Part { number: "ASM.1".into(), ..Default::default() };
    part.components = vec![crate::Component { number: "A".into(), quantity: 2.0 }, crate::Component { number: "B".into(), quantity: 1.0 }];
    let csv = to_csv(std::slice::from_ref(&part));
    assert!(csv.contains("A:2|B"));
    assert_eq!(from_csv(&csv).unwrap()[0].components, part.components);
    assert_eq!(crate::components_from_text("X:1,5 | Y"), vec![crate::Component { number: "X".into(), quantity: 1.5 }, crate::Component { number: "Y".into(), quantity: 1.0 }]);
}

#[test]
fn comment_lines_before_the_header_are_skipped() {
    let parts = [Part { number: "A-1".into(), description: "Relay".into(), ..Default::default() }];
    let text = format!("# format=openecraft-parts;server_version=0.1.2\n#second\n{}", to_csv(&parts));
    let read = import(&text).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!((read[0].number.as_str(), read[0].description.as_str()), ("A-1", "Relay"));
}
