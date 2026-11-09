use super::*;
use crate::geom::p;
use crate::panel::{collisions, PanelItem};
use crate::analysis::Analysis;
use crate::sample::demo_project;

#[test]
fn explicit_rotations_turn_asymmetric_drilling_without_scaling_hole_spacing() {
    let part=Part {number:"ROTATED".into(),width:20.,height:40.,tech_data:"Mounting holes: M5: 3,7".into(),..Default::default()};
    for (angle,expected) in [(0,p(103.,207.)),(90,p(133.,203.)),(180,p(117.,233.)),(270,p(107.,217.))] {
        let mut dev=item(1,ItemKind::Device,100.,200.,80.,80.);
        dev.part=part.number.clone();dev.rotation=crate::panel::PanelRotation::from_degrees(angle);
        let holes=item_holes(&dev,std::slice::from_ref(&part));
        assert_eq!(holes.len(),1);assert_eq!(holes[0].pos,expected);assert_eq!(holes[0].thread,"M5");
    }
}

fn item(id: u64, kind: ItemKind, x: f32, y: f32, w: f32, h: f32) -> PanelItem {
    PanelItem { id, kind, x, y, w, h, d: 40.0, ..Default::default() }
}

#[test]
fn drill_diameters() {
    assert_eq!(drill_diameter("M5"), Some(4.2));
    assert_eq!(drill_diameter("m4"), Some(3.3));
    assert_eq!(drill_diameter("Ø4.5"), Some(4.5));
    assert_eq!(drill_diameter("6,6"), Some(6.6));
    assert_eq!(drill_diameter("M7"), None);
}

#[test]
fn holes_along_profiles() {
    let v = line_positions(1000.0, 12.5, 200.0);
    assert_eq!(v.len(), 6);
    assert_eq!((v[0], v[5]), (12.5, 987.5));
    assert!(v.windows(2).all(|w| w[1] - w[0] <= 200.0 + 1e-3));
    assert_eq!(line_positions(20.0, 12.5, 200.0), vec![10.0]);
    // A horizontal rail: M5 holes on its centre line.
    let rail = item(1, ItemKind::Rail, 10.0, 100.0, 400.0, 35.0);
    let hs = item_holes(&rail, &[]);
    assert!(hs.iter().all(|h| h.pos.y == 117.5 && h.thread == "M5" && h.diameter == 4.2));
    assert_eq!(hs.first().unwrap().pos.x, 22.5);
    // Busbar systems: a pair of holder holes per position.
    let bb = item(2, ItemKind::Busbar, 0.0, 300.0, 500.0, 75.0);
    let hs = item_holes(&bb, &[]);
    assert_eq!(hs.len(), 2 * line_positions(500.0, 25.0, 250.0).len());
    assert!(hs.iter().all(|h| h.thread == "M6"));
    let e = Enclosure { items: vec![bb], ..Default::default() };
    assert_eq!(busbar_holders(&e), 3);
}

#[test]
fn device_holes_from_part_data() {
    let mut part = Part { number: "T".into(), width: 78.0, height: 84.0, ..Default::default() };
    // Devices on rails need no holes, devices on the plate two by default.
    let mut dev = PanelItem { id: 1, kind: ItemKind::Device, part: "T".into(), x: 100.0, y: 50.0, w: 78.0, h: 84.0, z: 7.5, ..Default::default() };
    assert!(item_holes(&dev, std::slice::from_ref(&part)).is_empty());
    dev.z = 0.0;
    let hs = item_holes(&dev, std::slice::from_ref(&part));
    assert_eq!(hs.iter().map(|h| h.pos).collect::<Vec<_>>(), vec![p(105.0, 55.0), p(173.0, 129.0)]);
    part.tech_data = "Mounting holes: M5: 9,12; 69,72".into();
    let hs = item_holes(&dev, std::slice::from_ref(&part));
    assert_eq!(hs.iter().map(|h| h.pos).collect::<Vec<_>>(), vec![p(109.0, 62.0), p(169.0, 122.0)]);
    assert!(hs.iter().all(|h| h.thread == "M5"));
    // Turned by 90°: the pattern turns with the device.
    dev.w = 84.0;
    dev.h = 78.0;
    let hs = item_holes(&dev, std::slice::from_ref(&part));
    assert_eq!(hs.iter().map(|h| h.pos).collect::<Vec<_>>(), vec![p(100.0 + 84.0 - 12.0, 59.0), p(100.0 + 84.0 - 72.0, 119.0)]);
    for h in &hs {
        assert!(dev.rect().contains(h.pos));
    }
}

#[test]
fn nothing_is_mounted_over_a_cutout() {
    let mut cut = item(1, ItemKind::Cutout, 100.0, 100.0, 50.0, 50.0);
    cut.z = -3.0;
    cut.d = 3.0;
    let on_plate = PanelItem { z: 0.0, ..item(2, ItemKind::Duct, 120.0, 0.0, 40.0, 300.0) };
    let above = PanelItem { z: 7.5, ..item(3, ItemKind::Device, 100.0, 100.0, 45.0, 70.0) };
    let e = Enclosure { items: vec![cut, on_plate, above], ..Default::default() };
    assert_eq!(collisions(&e, &[]), vec![(1, 2), (2, 3)]);
}

#[test]
fn nc_export() {
    let mut prj = demo_project();
    let e = &mut prj.layout.enclosures[0];
    let id = 9999;
    e.items.push(PanelItem { id, kind: ItemKind::Cutout, x: 400.0, y: 600.0, w: 22.5, h: 22.5, round: true, ..Default::default() });
    let (_, ph) = e.plate_size();
    let csv = nc_csv(&prj, 0, Face::Plate);
    let lines: Vec<&str> = csv.lines().collect();
    let n_holes = holes(&prj.layout.enclosures[0], &prj.parts).len();
    assert!(n_holes > 0);
    assert_eq!(lines.len(), 1 + n_holes + 1);
    // Coordinates from the bottom left corner of the plate.
    let cut = lines.last().unwrap();
    assert!(cut.contains("Cutout round"), "{cut}");
    assert!(cut.contains(&format!(";{:.1};", ph - 611.25)), "{cut}");
    let dxf = nc_dxf(&prj, 0, Face::Plate);
    assert!(dxf.contains("\nHOLE_D4.2\n") || dxf.contains("\nHOLE_D3.3\n"));
    assert!(dxf.contains("\nCUTOUT\n"));
    assert!(dxf.contains("\nPLATE\n"));
    assert!(dxf.ends_with("EOF\n"));
    assert_eq!(dxf.matches("0\nCIRCLE\n").count(), n_holes + holes(&prj.layout.enclosures[0], &prj.parts).iter().filter(|h| !h.thread.is_empty()).count() + 1);
}

#[test]
fn wire_end_treatment() {
    assert_eq!(cross_section_mm2("1.5 mm²"), Some(1.5));
    assert_eq!(cross_section_mm2("2,5mm2"), Some(2.5));
    assert_eq!(cross_section_mm2("AWG 14"), None);
    let t = end_treatment("1.5 mm²", true).unwrap();
    assert_eq!((t.strip, t.ferrule.unwrap().color, t.ferrule.unwrap().part), (8.0, "black", "GEN.FER-1.5"));
    assert_eq!(end_treatment("6 mm²", false).unwrap(), EndTreatment { strip: 10.0, ferrule: None });
    // Unusual cross-sections get a stripping length but no ferrule.
    assert_eq!(end_treatment("1.2", true).unwrap().ferrule, None);
    assert!(end_treatment("", true).is_none());
    // Every ferrule is in the catalog.
    let catalog = crate::parts::catalog();
    for f in FERRULES {
        assert!(catalog.iter().any(|p| p.number == f.part), "{}", f.part);
    }
}

#[test]
fn ferrules_in_wire_list_and_bom() {
    let mut prj = demo_project();
    for w in prj.pages.iter_mut().flat_map(|pg| pg.wires.iter_mut()).filter(|w| w.cable.is_empty()) {
        w.cross_section = "0.75 mm²".into();
    }
    let an = Analysis::checked(&prj);
    assert!(!an.wiring.is_empty());
    let list = crate::reports::production_wire_list(&prj, &an);
    let col = list.headers.iter().position(|h| *h == "Ferrule part").unwrap();
    assert!(list.rows.iter().any(|r| r[col] == "GEN.FER-0.75"));
    // Routed (panel) wires come first.
    let len = list.headers.iter().position(|h| *h == "Length (mm)").unwrap();
    let first_unrouted = list.rows.iter().position(|r| r[len].is_empty()).unwrap_or(list.rows.len());
    assert!(list.rows[first_unrouted..].iter().all(|r| r[len].is_empty()));
    let bom = &an.report(&prj, crate::model::ReportKind::BillOfMaterials).unwrap();
    let row = bom.rows.iter().find(|r| r[3] == "GEN.FER-0.75").expect("ferrules in the BOM");
    assert_eq!(row[1], (2 * an.wiring.iter().filter(|w| w.cross_section == "0.75 mm²").count()).to_string());
    assert!(!row[6].contains("not in project"), "described from the catalog");
    prj.layout.ferrules = false;
    let an = Analysis::checked(&prj);
    assert!(!an.report(&prj, crate::model::ReportKind::BillOfMaterials).unwrap().rows.iter().any(|r| r[3] == "GEN.FER-0.75"));
}

#[test]
fn door_cutouts_and_nc_data() {
    let prj = demo_project();
    let e = &prj.layout.enclosures[0];
    let door = openings(e, &prj.parts, Face::Door);
    // One cutout per door device: Ø22.5 for command and signal devices.
    assert_eq!(door.len(), 6);
    assert!(door.iter().all(|o| o.round && o.w == 22.5));
    // The door has no drilling pattern of the plate and vice versa.
    assert!(openings(e, &prj.parts, Face::Plate).is_empty());
    let csv = nc_csv(&prj, 0, Face::Door);
    assert_eq!(csv.lines().count(), 1 + 6);
    assert!(csv.lines().skip(1).all(|l| l.contains(";Door;") && l.contains("Cutout round")));
    let dxf = nc_dxf(&prj, 0, Face::Door);
    assert!(dxf.contains("\nDOOR\n") && !dxf.contains("HOLE_D"));
    assert_eq!(dxf.matches("0\nCIRCLE\n").count(), 6);
    // Part data overrides the default: rectangular cutout.
    let part = Part { number: "D".into(), tech_data: "Door cutout: 92 x 68 mm".into(), ..Default::default() };
    let it = item(1, ItemKind::Device, 0.0, 0.0, 96.0, 72.0);
    assert_eq!(door_cutout(Some(&part), &it), (false, 92.0, 68.0));
    let part = Part { number: "D".into(), tech_data: "Door cutout: Ø30.5".into(), ..Default::default() };
    assert_eq!(door_cutout(Some(&part), &it), (true, 30.5, 30.5));
    assert_eq!(door_cutout(None, &it), (false, 92.0, 68.0));
}

#[test]
fn bars_sit_at_their_centre_distance() {
    let parts = crate::parts::catalog();
    // 185 mm apart in the middle of the 400 mm wide system.
    assert_eq!(bar_centres(parts, "GEN.BB-3X30X10-185", 400.0), vec![15.0, 200.0, 385.0]);
    assert_eq!(bar_centres(parts, "GEN.BB-3X30X10-60", 150.0), vec![15.0, 75.0, 135.0]);
    // Without a centre distance, or one the system is too narrow for: evenly spaced.
    assert_eq!(bar_centres(parts, "GEN.BB-3X30X10", 90.0), vec![15.0, 45.0, 75.0]);
    assert_eq!(bar_centres(parts, "GEN.BB-3X30X10-185", 300.0), vec![50.0, 150.0, 250.0]);
}
