use super::*;
use crate::model::panel::RoofShape;
use crate::panel::buildings::add_building;

/// A building with a gable roof of 10 × 8 m at 45° (ridge along x at z = 4000, eaves at
/// 3 m) or a flat roof; the building's index.
fn building(shape: RoofShape) -> (Project, usize) {
    let mut prj = Project::default();
    let bi = add_building(&mut prj, "+B1").unwrap();
    let pitch = if shape == RoofShape::Flat { 0.0 } else { 45.0 };
    prj.layout.buildings[bi].roofs.push(Roof { name: "R1".into(), shape, x: 0.0, z: 0.0, width: 10_000.0, depth: 8000.0, eaves: 3000.0, pitch, overhang: 400.0, thickness: 300.0, ..Default::default() });
    (prj, bi)
}

fn array(x: f32, z: f32) -> PvArray {
    PvArray { name: "PV1".into(), x, z, ..Default::default() }
}

#[test]
fn modules_on_rails_lie_parallel_on_their_slope() {
    let (prj, bi) = building(RoofShape::Gable);
    let r = &prj.layout.buildings[bi].roofs[0];
    let a = array(5000.0, 6000.0);
    let g = geometry(r, &a).expect("on the front slope");
    assert_eq!(g.modules.len(), 8);
    let sec = 2f32.sqrt();
    for m in &g.modules {
        let n = normal_of(m);
        assert!((n[1] - 0.5f32.sqrt()).abs() < 1e-3 && n[2] > 0.0, "facing up and to the front: {n:?}");
        for v in m {
            let q = [v[0], v[2]];
            assert!(q[1] > 4000.0, "on the front slope");
            let above = v[1] - r.thickness * sec - g.plane.at(q);
            assert!((above - (STANDOFF + 30.0) * sec).abs() < 0.5, "{above}");
        }
    }
    // Four modules side by side across 4596 mm, two rows up the slope.
    let xs: Vec<f32> = g.modules.iter().flatten().map(|v| v[0]).collect();
    let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    assert!((lo - 2702.0).abs() < 0.5 && (hi - 7298.0).abs() < 0.5, "{lo} {hi}");
    // Portrait: two rails along each row; landscape: two up each column.
    assert_eq!(g.rails.len(), 4);
    assert!(g.rails.iter().all(|(p, q)| (p[2] - q[2]).abs() < 0.01 && (q[0] - p[0] - 4696.0).abs() < 0.5));
    let land = geometry(r, &PvArray { landscape: true, ..a }).unwrap();
    assert_eq!(land.rails.len(), 8);
    assert!(land.rails.iter().all(|(p, q)| (p[0] - q[0]).abs() < 0.01));
}

#[test]
fn rails_hooks_and_clamps_follow_the_modules() {
    let (mut prj, bi) = building(RoofShape::Gable);
    let a = array(5000.0, 6000.0);
    let m: Vec<(Role, f32)> = mounting(&prj, &a);
    // Four rails of 4696 mm: two pieces of 4.2 m each, joined once.
    assert_eq!(m, vec![(Role::Rail, 8.0), (Role::Connector, 4.0), (Role::Hook, 20.0), (Role::EndClamp, 8.0), (Role::MidClamp, 12.0)]);
    let mat = material(&prj, &a);
    assert_eq!(mat[0], ("GEN.PV-430W".to_string(), 8.0));
    assert!(mat.contains(&("GEN.PV-RAIL-40".to_string(), 8.0)) && mat.contains(&("GEN.PV-HOOK".to_string(), 20.0)));
    assert_eq!(peak_power(&prj, &a), Some(3.44));
    // In the bill of materials of the layout, with the array's name.
    prj.layout.buildings[bi].roofs[0].solar.push(a.clone());
    let all = crate::panel::layout_material(&prj);
    let rail = all.iter().find(|(p, ..)| p == "GEN.PV-RAIL-40").expect("the rails");
    assert_eq!((rail.1, rail.2.clone()), (8.0, vec!["+B1 PV1".to_string()]));
    // A device that counts the modules leaves them out.
    let with_device = PvArray { device: "=PV-G1".into(), ..a };
    assert!(material(&prj, &with_device).iter().all(|(p, _)| p != "GEN.PV-430W"));
}

#[test]
fn a_mounting_system_brings_its_own_parts() {
    let (mut prj, _) = building(RoofShape::Gable);
    let part = |number: &str, subgroup: &str, tech: &str| Part { number: number.into(), subgroup: subgroup.into(), tech_data: tech.into(), ..Default::default() };
    let mut rail = part("X.RAIL", "PV mounting rail", "");
    rail.quantity_unit = "m".into();
    rail.accessories = ["X.HOOK", "X.END-30", "X.END-40", "X.MID"].map(String::from).to_vec();
    prj.parts = vec![
        rail,
        part("X.HOOK", "Roof hook", ""),
        part("X.END-30", "PV end clamp", "Module frame height: 30 - 35 mm"),
        part("X.END-40", "PV end clamp", "Module frame height: 38 - 42 mm"),
        part("X.MID", "PV middle clamp", "Module frame height: 30 - 42 mm"),
    ];
    let a = PvArray { system: "X.RAIL".into(), module_height: 40.0, ..array(5000.0, 6000.0) };
    assert_eq!(part_for(&prj, &a, Role::Rail), "X.RAIL");
    assert_eq!(part_for(&prj, &a, Role::EndClamp), "X.END-40", "the clamp for the frame height");
    assert_eq!(part_for(&prj, &a, Role::MidClamp), "X.MID");
    assert_eq!(part_for(&prj, &a, Role::Connector), "GEN.PV-RAIL-CONN", "generic where the system has none");
    // A rail sold by the metre: 4 × 4.696 m, joined every 6 m.
    let m = mounting(&prj, &a);
    assert_eq!(m[0], (Role::Rail, 18.79));
    assert!(m.iter().all(|(r, _)| *r != Role::Connector), "one piece per rail");
}

#[test]
fn flat_roofs_take_tilted_and_east_west_rows() {
    let (mut prj, bi) = building(RoofShape::Flat);
    let id = add_array(&mut prj, bi, 0, None).unwrap();
    let r = &prj.layout.buildings[bi].roofs[0];
    let a = r.solar.iter().find(|a| a.id == id).unwrap().clone();
    assert_eq!((a.mounting, a.landscape, a.columns, a.rows, a.x, a.z), (PvMounting::Tilted, true, 4, 3, 5000.0, 4000.0));
    let g = geometry(r, &a).unwrap();
    assert_eq!(g.modules.len(), 12);
    // Facing the front (+z): the low edge in front, 150 mm + frame above the covering.
    let cover = 3000.0 + 300.0;
    for m in &g.modules {
        let low = m.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
        assert!((low - cover - BASE_HEIGHT - 30.0).abs() < 0.5);
        let front = m.iter().filter(|v| (v[1] - low).abs() < 0.5).map(|v| v[2]).fold(f32::MIN, f32::max);
        assert!(m.iter().all(|v| v[2] <= front + 0.5));
        assert!(normal_of(m)[2] > 0.0, "facing the front");
    }
    // The rows far enough apart not to shade each other at 17° sun.
    let t = 15f32.to_radians();
    assert!((row_pitch(&a) - (1134.0 * t.cos() + 1134.0 * t.sin() / 17f32.to_radians().tan())).abs() < 0.1);
    let m = mounting(&prj, &a);
    assert_eq!(m, vec![(Role::Base, 15.0), (Role::EndClamp, 12.0), (Role::MidClamp, 18.0), (Role::Ballast, 20.0), (Role::Deflector, 12.0)]);
    assert_eq!(part_for(&prj, &a, Role::Base), "GEN.PV-BASE-S15");
    // East-west: pairs facing both ways on one base each.
    let ew = PvArray { mounting: PvMounting::EastWest, azimuth: 90.0, tilt: 10.0, ..a };
    let g = geometry(r, &ew).unwrap();
    assert_eq!(g.modules.len(), 24);
    assert_eq!(g.modules.iter().filter(|m| normal_of(&m[..])[0] > 0.0).count(), 12, "half of them facing +x");
    let m = mounting(&prj, &ew);
    assert_eq!(m, vec![(Role::Base, 15.0), (Role::EndClamp, 24.0), (Role::MidClamp, 36.0), (Role::Ballast, 39.0)]);
    assert_eq!(part_for(&prj, &ew, Role::Base), "GEN.PV-BASE-EW10");
    assert!(problems(&prj).is_empty(), "{:?}", problems(&prj));
}

#[test]
fn arrays_across_the_ridge_or_over_windows_are_reported() {
    let (mut prj, bi) = building(RoofShape::Gable);
    roofs::add_window(&mut prj, bi, 0, Some([5000.0, 6000.0])).unwrap();
    let r = &mut prj.layout.buildings[bi].roofs[0];
    r.solar = vec![
        array(5000.0, 6000.0),
        PvArray { name: "PV2".into(), ..array(5000.0, 4100.0) },
        PvArray { name: "PV3".into(), mounting: PvMounting::Tilted, device: "=PV-G1".into(), ..array(5000.0, 1500.0) },
        PvArray { name: "PV4".into(), ..array(5000.0, -2000.0) },
    ];
    let texts: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    for expected in [
        "+B1: PV array PV1 covers a roof window",
        "+B1: PV array PV2 of roof R1 lies across a ridge, a hip or the edge of the roof",
        "+B1: PV arrays PV1 and PV2 lie over each other",
        "+B1: PV array PV3 is mounted tilted on a pitched roof",
        "+B1: the device =PV-G1 of PV array PV3 does not exist",
        "+B1: PV array PV4 of roof R1 lies outside the roof",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
}

#[test]
fn a_device_counts_the_modules_of_its_array() {
    let (mut prj, bi) = building(RoofShape::Gable);
    let mut dev = crate::model::DeviceData::default();
    dev.parts.push(openecraft_parts::PartRef { number: "GEN.PV-430W".into(), count: 6, total: false });
    prj.devices.insert("=PV-G1".into(), dev);
    prj.layout.buildings[bi].roofs[0].solar.push(PvArray { device: "=PV-G1".into(), ..array(5000.0, 6000.0) });
    let texts: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert_eq!(texts, vec!["+B1: PV array PV1 has 8 modules, its device =PV-G1 has 6".to_string()]);
}

#[test]
fn modules_and_mounting_are_drawn_on_the_roof() {
    let (mut prj, bi) = building(RoofShape::Gable);
    prj.layout.buildings[bi].roofs[0].solar.push(array(5000.0, 6000.0));
    let b = &prj.layout.buildings[bi];
    let f = faces(b, &b.roofs[0]);
    let count = |k: RoofFaceKind| f.iter().filter(|x| x.kind == k).count();
    // Each module: the cells, four frame pieces on top, four sides and the bottom.
    assert_eq!((count(RoofFaceKind::Module), count(RoofFaceKind::ModuleFrame)), (8, 8 * 9));
    assert_eq!(count(RoofFaceKind::Mounting), 4 * 4, "four sides of each rail");
    // A module's part sets its size.
    let mut a = array(0.0, 0.0);
    set_module(&prj, &mut a, "GEN.PV-400W-FB").unwrap();
    assert_eq!((a.module_width, a.module_length, a.module_height), (1134.0, 1722.0, 30.0));
    assert!(set_module(&prj, &mut a, "NOPE").is_err());
}

#[test]
fn a_literal_zero_quantity_counts_no_modules() {
    let (mut prj, bi) = building(RoofShape::Gable);
    let mut dev = crate::model::DeviceData::default();
    dev.parts.push(openecraft_parts::PartRef { number: "GEN.PV-430W".into(), count: 0, total: true });
    prj.devices.insert("=PV-G1".into(), dev);
    prj.layout.buildings[bi].roofs[0].solar.push(PvArray { device: "=PV-G1".into(), ..array(5000.0, 6000.0) });
    let texts: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert_eq!(texts, vec!["+B1: PV array PV1 has 8 modules, its device =PV-G1 has 0".to_string()]);
}
