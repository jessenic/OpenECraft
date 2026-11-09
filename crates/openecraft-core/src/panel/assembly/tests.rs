use super::*;
use crate::parts::{bundled, PartsDb};
use crate::panel::{device_mesh, device_points, device_size, new_enclosure, place_device};

#[test]
fn explicit_half_turn_reverses_side_accessories_and_their_connection_points() {
    let mut prj=project(&[("-F1", &["HAG.CCA216D", "HAG.CZ001"])]);
    let mut it=placed(&mut prj,"-F1");
    it.rotation=Some(crate::panel::PanelRotation::Deg0);
    let initial=device_points(&prj,&it);
    it.rotation=Some(crate::panel::PanelRotation::Deg180);
    let turned=device_points(&prj,&it);
    assert_eq!(initial.len(),turned.len());
    for (a,b) in initial.iter().zip(&turned) {
        assert_eq!(a.pin,b.pin);
        assert!((a.pos[0]+b.pos[0]-2.*it.x-it.w).abs()<0.001);
        assert!((a.pos[1]+b.pos[1]-2.*it.y-it.h).abs()<0.001);
    }
    let assembly=assembly(&prj,"-F1").unwrap();
    let side=assembly.piece_item(&it,&assembly.sides[0]);
    assert!((side.x-(it.x+it.w-9.)).abs()<0.001);
    assert_eq!(side.rotation,it.rotation);
}

/// A project with an enclosure and devices of the given parts.
fn project(devices: &[(&str, &[&str])]) -> Project {
    let mut prj = Project::default();
    let db = PartsDb { parts: bundled::default_parts() };
    for (tag, parts) in devices {
        for n in *parts {
            crate::parts::add_to_device(&mut prj, &db, tag, n, 1);
        }
    }
    let e = new_enclosure(&mut prj, "GEN.ENC-600X800", "+CAB1");
    prj.layout.enclosures.push(e);
    prj
}

fn placed(prj: &mut Project, tag: &str) -> PanelItem {
    let id = place_device(prj, 0, tag, crate::geom::p(100.0, 100.0)).unwrap();
    prj.layout.enclosures[0].item(id).unwrap().clone()
}

#[test]
fn side_contacts_stand_beside_the_main_part() {
    let mut prj = project(&[("-F1", &["HAG.CCA216D", "HAG.CZ001"]), ("-F2", &["GEN.MCB-1P-C6", "GEN.MCB-AUX-1CO"])]);
    let a = assembly(&prj, "-F1").unwrap();
    // The signal contact (9 mm) sits left of the RCCB (35 mm).
    assert_eq!((a.main.part.number.as_str(), a.main.at, a.w, a.d), ("HAG.CCA216D", 9.0, 44.0, 70.0));
    assert_eq!((a.sides[0].part.number.as_str(), a.sides[0].at), ("HAG.CZ001", 0.0));
    let size = device_size(&prj, "-F1").unwrap();
    assert_eq!((size.part.as_str(), size.w, size.h), ("HAG.CCA216D", 44.0, 83.0));
    // The auxiliary switch of the breaker on its right.
    let b = assembly(&prj, "-F2").unwrap();
    assert_eq!((b.main.at, b.sides[0].at, b.w), (0.0, 18.0, 27.0));

    let it = placed(&mut prj, "-F1");
    assert!(mounted(&prj, &it).is_some());
    let points = device_points(&prj, &it);
    let x = |pin: &str| points.iter().find(|p| p.pin == pin).unwrap_or_else(|| panic!("{pin}")).pos[0];
    assert!(x("13") < it.x + 9.0 && x("21") < it.x + 9.0, "contacts on the signal contact");
    assert!(x("1") > it.x + 9.0 && x("N'") < it.x + 44.0, "terminals on the RCCB");
    // The 3D model shows both parts: the signal contact's model fills the left end.
    let tris = device_mesh(&prj, &it).unwrap();
    let left = tris.iter().flatten().map(|v| v[0]).fold(f32::MAX, f32::min);
    let right = tris.iter().flatten().map(|v| v[0]).fold(f32::MIN, f32::max);
    assert!((left - it.x).abs() < 0.01 && (right - (it.x + 44.0)).abs() < 0.01, "{left} {right}");
}

#[test]
fn motor_protection_switches_take_their_auxiliary_switch_on_the_left() {
    // The demo's -Q1: the lateral auxiliary switch (9 mm) left of the switch (45 mm).
    let prj = crate::sample::demo_project();
    let a = assembly(&prj, "-Q1").unwrap();
    assert_eq!((a.main.part.number.as_str(), a.main.at, a.w, a.d), ("GEN.MPS-2.5-4A", 9.0, 54.0, 97.0));
    let (_, it) = crate::panel::locate(&prj, "-Q1").unwrap();
    assert_eq!(it.w, 54.0);
    let points = device_points(&prj, it);
    assert!(points.iter().filter(|p| p.pin == "13" || p.pin == "21").all(|p| p.pos[0] < it.x + 9.0));
}

#[test]
fn front_blocks_are_stacked_in_front() {
    let mut prj = project(&[("-K1", &["GEN.CT-9A-24DC", "GEN.AUX-2NO2NC"])]);
    crate::parts::add_to_device(&mut prj, &PartsDb::default(), "-K1", "GEN.AUX-1NO1NC", 1);
    let a = assembly(&prj, "-K1").unwrap();
    assert_eq!((a.w, a.d, a.front.len()), (45.0, 73.0 + 40.0 + 40.0, 2));
    assert_eq!((a.front[0].z, a.front[1].z, a.front[0].at), (73.0, 113.0, 0.0));
    let it = placed(&mut prj, "-K1");
    let a = assembly(&prj, "-K1").unwrap();
    let sub = a.piece_item(&it, &a.front[1]);
    assert_eq!((sub.y - it.y, sub.h, sub.z - it.z, sub.d), (17.0, 40.0, 113.0, 40.0));
    let deepest = device_mesh(&prj, &it).unwrap().iter().flatten().map(|v| v[2]).fold(f32::MIN, f32::max);
    assert!((deepest - (it.z + it.d)).abs() < 0.01);
}

#[test]
fn plain_devices_and_strips_are_drawn_as_before() {
    let mut prj = project(&[("-F1", &["HAG.CCA216D"]), ("-X1", &["GEN.TB-2.5"])]);
    let it = placed(&mut prj, "-F1");
    assert!(assembly(&prj, "-F1").unwrap().is_plain() && mounted(&prj, &it).is_none());
    assert!(assembly(&prj, "-X1").is_none());
    // A loose accessory (only the accessory on the device) is drawn as the main part.
    let prj = project(&[("-F3", &["HAG.CZ001"])]);
    assert_eq!(assembly(&prj, "-F3").unwrap().main.part.number, "HAG.CZ001");
}

#[test]
fn the_main_part_stands_on_its_base_and_takes_the_accessories_built_into_it() {
    let mut prj = project(&[]);
    let base = Part { number: "OWN.BASE".into(), width: 250.0, height: 450.0, depth: 40.0, accessory: true, accessory_mount: AccessoryMount::Behind, ..Default::default() };
    let switch = Part {
        number: "OWN.Q".into(),
        width: 105.0,
        height: 160.0,
        depth: 90.0,
        functions: vec![crate::parts::FunctionTemplate::new(crate::parts::FnKind::Breaker3, "1 3 5 2 4 6")],
        ..Default::default()
    };
    // An undervoltage release without dimensions in a slot of the switch.
    let release =
        Part { number: "OWN.UVR".into(), accessory: true, accessory_mount: AccessoryMount::Inside, functions: vec![crate::parts::FunctionTemplate::new(crate::parts::FnKind::Coil, "D1 D2")], ..Default::default() };
    prj.parts.extend([base, switch, release]);
    for n in ["OWN.Q", "OWN.BASE", "OWN.UVR"] {
        crate::parts::add_to_device(&mut prj, &PartsDb::default(), "-Q1", n, 1);
    }
    let a = assembly(&prj, "-Q1").unwrap();
    assert_eq!((a.w, a.h, a.d), (250.0, 450.0, 130.0));
    assert_eq!((a.main.at, a.main.z, a.behind[0].at, a.behind[0].z), (72.5, 40.0, 0.0, 0.0));
    assert_eq!(a.inside.len(), 1);
    let size = device_size(&prj, "-Q1").unwrap();
    assert_eq!((size.part.as_str(), size.w, size.h, size.d), ("OWN.Q", 250.0, 450.0, 130.0));

    let it = placed(&mut prj, "-Q1");
    assert!(mounted(&prj, &it).is_some());
    let a = assembly(&prj, "-Q1").unwrap();
    let main = a.piece_item(&it, &a.main);
    assert_eq!((main.x - it.x, main.y - it.y, main.z - it.z, main.w, main.h), (72.5, 145.0, 40.0, 105.0, 160.0));
    // The terminals of the switch and of its release are on the switch, in front of the base.
    let points = device_points(&prj, &it);
    for pin in ["1", "6", "D1", "D2"] {
        let p = points.iter().find(|p| p.pin == pin).unwrap_or_else(|| panic!("{pin}"));
        assert!(p.pos[0] >= main.x && p.pos[0] <= main.x + main.w && p.pos[1] >= main.y && p.pos[1] <= main.y + main.h, "{pin} {:?}", p.pos);
        assert!(p.pos[2] >= main.z, "{pin}");
    }
    // The 3D model shows the base behind the switch.
    let tris = device_mesh(&prj, &it).unwrap();
    let back = tris.iter().flatten().map(|v| v[2]).fold(f32::MAX, f32::min);
    let front = tris.iter().flatten().map(|v| v[2]).fold(f32::MIN, f32::max);
    assert!((back - it.z).abs() < 0.01 && (front - (it.z + 130.0)).abs() < 0.01, "{back} {front}");
}

#[test]
fn transformers_behind_a_vertical_strip_lie_along_it() {
    let mut prj = project(&[]);
    let strip = Part { number: "OWN.STRIP".into(), width: 100.0, height: 740.0, depth: 140.0, ..Default::default() };
    let ct = Part { number: "OWN.CT".into(), width: 90.0, height: 80.0, depth: 60.0, accessory: true, accessory_mount: AccessoryMount::Behind, ..Default::default() };
    prj.parts.extend([strip, ct]);
    crate::parts::add_to_device(&mut prj, &PartsDb::default(), "-Q1", "OWN.STRIP", 1);
    crate::parts::add_to_device(&mut prj, &PartsDb::default(), "-Q1", "OWN.CT", 3);
    let a = assembly(&prj, "-Q1").unwrap();
    // One layer of three transformers along the strip's height, centred under it.
    assert_eq!((a.w, a.h, a.d, a.main.z), (100.0, 740.0, 200.0, 60.0));
    let across: Vec<f32> = a.behind.iter().map(|b| b.across).collect();
    assert_eq!(across, [-80.0, 0.0, 80.0]);
    assert!(a.behind.iter().all(|b| b.at == 5.0 && b.z == 0.0));
    let it = placed(&mut prj, "-Q1");
    let a = assembly(&prj, "-Q1").unwrap();
    let ys: Vec<f32> = a.behind.iter().map(|b| a.piece_item(&it, b).y - it.y).collect();
    assert_eq!(ys, [250.0, 330.0, 410.0]);
}
