use super::*;
use crate::panel::{validate, Enclosure, Finding, PanelItem};
use crate::parts::Part;

fn item(id: u64, kind: ItemKind, x: f32, y: f32, w: f32, h: f32) -> PanelItem {
    PanelItem { id, kind, x, y, w, h, d: 10.0, ..Default::default() }
}

fn findings(prj: &Project) -> Vec<Finding> {
    validate(prj, &prj.layout.enclosures[0]).into_iter().filter(|f| f.kind == CheckKind::Manufacturing).collect()
}

/// Devices of this part are screwed onto the plate.
const SCREWED: &str = "BOX";

fn project(items: Vec<PanelItem>) -> Project {
    let mut prj = Project::new_default();
    prj.parts.push(Part { number: SCREWED.into(), mounting: "Screw mounting".into(), ..Default::default() });
    prj.layout.enclosures.push(Enclosure { items, ..Default::default() });
    prj
}

#[test]
fn holes_near_the_edge_and_each_other() {
    // Screwed devices: M4 holes 5 mm inside two corners.
    let mut prj = project(vec![PanelItem { part: SCREWED.into(), ..item(1, ItemKind::Device, 0.0, 100.0, 40.0, 40.0) }]);
    assert!(findings(&prj).is_empty(), "Ø3.3 hole 3.35 mm from the edge");
    prj.layout.checks.hole_edge = 5.0;
    let f = findings(&prj);
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].items, vec![1]);
    prj.layout.enclosures[0].items[0].x = 10.0;
    assert!(findings(&prj).is_empty());
    // A second device overlapping its corner: holes 5.7 mm apart.
    prj.layout.enclosures[0].items.push(PanelItem { part: SCREWED.into(), ..item(2, ItemKind::Device, 44.0, 126.0, 40.0, 40.0) });
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items.contains(&1) && f.items.contains(&2)), "{f:#?}");
}

#[test]
fn cutouts_and_holes() {
    let mut cut = item(1, ItemKind::Cutout, 100.0, 100.0, 50.0, 50.0);
    cut.round = true;
    let mut prj = project(vec![cut, item(2, ItemKind::Cutout, 152.0, 100.0, 40.0, 40.0)]);
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items == vec![1, 2]), "2 mm web: {f:#?}");
    prj.layout.enclosures[0].items[1].x = 300.0;
    assert!(findings(&prj).is_empty());
    // A screwed device whose hole lands in the cutout.
    prj.layout.enclosures[0].items.push(PanelItem { part: SCREWED.into(), ..item(3, ItemKind::Device, 140.0, 140.0, 40.0, 40.0) });
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items == vec![3, 1]), "{f:#?}");
}

#[test]
fn profile_lengths_and_rails() {
    let mut prj = project(vec![item(1, ItemKind::Rail, 50.0, 100.0, 30.0, 35.0), item(2, ItemKind::Duct, 0.0, 300.0, 550.0, 40.0)]);
    prj.layout.checks.stock_length = 500.0;
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items == vec![1] && f.severity == Severity::Warning), "too short: {f:#?}");
    assert!(f.iter().any(|f| f.items == vec![2] && f.severity == Severity::Info), "longer than stock: {f:#?}");
    // A device on the rail overhanging its end.
    prj.layout.enclosures[0].items[0].w = 300.0;
    prj.layout.enclosures[0].items.push(PanelItem { z: 7.5, ..item(3, ItemKind::Device, 320.0, 80.0, 45.0, 75.0) });
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items == vec![3, 1]), "{f:#?}");
    // A rail device directly on the plate.
    prj.parts.push(Part { number: "MCB".into(), mounting: "Mounting rail 35 mm".into(), ..Default::default() });
    prj.layout.enclosures[0].items.push(PanelItem { part: "MCB".into(), z: 0.0, ..item(4, ItemKind::Device, 100.0, 400.0, 18.0, 90.0) });
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.items == vec![4]), "{f:#?}");
}

#[test]
fn plate_load() {
    let mut prj = project(vec![PanelItem { tag: "-T1".into(), ..item(1, ItemKind::Device, 100.0, 100.0, 60.0, 100.0) }]);
    prj.parts.push(Part { number: "ENC".into(), tech_data: "Plate load: 10 kg".into(), ..Default::default() });
    prj.parts.push(Part { number: "TR".into(), weight: 12.0, ..Default::default() });
    prj.layout.enclosures[0].part = "ENC".into();
    prj.devices.insert("-T1".into(), crate::model::DeviceData { parts: vec![crate::parts::PartRef { number: "TR".into(), count: 1, total:false }], ..Default::default() });
    let f = findings(&prj);
    assert!(f.iter().any(|f| f.message.contains("12.0 kg")), "{f:#?}");
}
