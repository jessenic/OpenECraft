use super::*;

#[test]
fn catalog_numbers_unique_and_accessories_exist() {
    let parts = default_catalog();
    for (i, p) in parts.iter().enumerate() {
        assert!(parts[i + 1..].iter().all(|q| q.number != p.number), "duplicate {}", p.number);
        for a in p.accessories.iter().chain(&p.fits) {
            assert!(parts.iter().any(|q| &q.number == a), "{} accessory {a} missing", p.number);
        }
        // Accessories are flagged and mounted on their main parts.
        if p.accessory {
            assert!(!crate::accessory::mains_of(&parts, p).is_empty(), "{} fits nothing", p.number);
        }
        for a in crate::accessory::accessories_of(&parts, p).iter().filter(|a| a.subgroup.starts_with("Auxiliary")) {
            assert!(a.accessory, "{} is no accessory", a.number);
        }
    }
}

#[test]
fn generated_models_match_mounting_data() {
    let parts = default_catalog();
    let mut with_models = 0;
    for p in &parts {
        match (&p.model_2d, &p.model_3d) {
            (Some(m2), Some(m3)) => {
                with_models += 1;
                let s = m3.size();
                for (got, want) in [(s[0], p.width), (s[1], p.height), (s[2], p.depth)] {
                    assert!((got - want).abs() < 0.01, "{}: model {s:?} vs {} x {} x {}", p.number, p.width, p.height, p.depth);
                }
                assert_eq!((m2.width, m2.height), (p.width, p.height), "{}", p.number);
                assert!(m3.triangles.iter().flatten().all(|&i| (i as usize) < m3.vertices.len()));
            }
            (None, None) => {
                // Only parts without dimensions have no model.
                assert!(p.width == 0.0 || p.height == 0.0 || p.depth == 0.0, "{} has no model", p.number);
            }
            _ => panic!("{}: only one of the models", p.number),
        }
    }
    assert!(with_models >= 25, "{with_models}");
}

#[test]
fn generated_models_keep_existing_models() {
    let mut p = Part { number: "X.1".into(), group: "Relays, contactors".into(), width: 45.0, height: 57.5, depth: 73.0, ..Default::default() };
    add_generated_models(&mut p);
    assert_eq!(p.model_3d.as_ref().unwrap().size(), [45.0, 57.5, 73.0]);
    let own = Model2d { width: 1.0, height: 1.0, prims: Vec::new() };
    let mut q = Part { model_2d: Some(own.clone().into()), ..p.clone() };
    q.model_3d = None;
    add_generated_models(&mut q);
    assert_eq!((q.model_2d, q.model_3d), (Some(own.into()), None));
    let mut none = Part { number: "X.2".into(), ..Default::default() };
    add_generated_models(&mut none);
    assert!(none.model_2d.is_none());
}

#[test]
fn mounting_material_has_models_in_proportion() {
    let parts = default_catalog();
    let mounting: Vec<&Part> = parts.iter().filter(|p| p.group == "Mounting" && p.width > 0.0 && p.height > 0.0 && p.depth > 0.0).collect();
    assert!(mounting.len() > 60, "{}", mounting.len());
    for p in mounting {
        let (m2, m3) = (p.model_2d.as_ref().unwrap(), p.model_3d.as_ref().unwrap());
        // Small enough to be sent with every part and drawn at once.
        assert!(m3.triangles.len() <= 600 && m2.prims.len() <= 400, "{}: {} triangles, {} lines", p.number, m3.triangles.len(), m2.prims.len());
        let inside = |q: [f32; 2]| (-0.01..=p.width + 0.01).contains(&q[0]) && (-0.01..=p.height + 0.01).contains(&q[1]);
        for prim in &m2.prims {
            let ok = match prim {
                Prim2d::Line { a, b } => inside(*a) && inside(*b),
                Prim2d::Rect { min, max, .. } => inside(*min) && inside(*max) && min[0] <= max[0] && min[1] <= max[1],
                Prim2d::Circle { c, r, .. } => inside([c[0] - r, c[1] - r]) && inside([c[0] + r, c[1] + r]),
                Prim2d::Polyline { pts, .. } => pts.iter().all(|q| inside(*q)),
                Prim2d::Text { .. } => true,
            };
            assert!(ok, "{}: {prim:?} outside {} x {}", p.number, p.width, p.height);
        }
    }
    // The shapes differ: a duct is no rail.
    let tris = |n: &str| parts.iter().find(|p| p.number == n).and_then(|p| p.model_3d.as_ref()).map(|m| m.triangles.len());
    assert_ne!(tris("GEN.RAIL-TS35-1M"), tris("GEN.DUCT-40X60"));
}

#[test]
fn the_catalog_has_parts_for_19_inch_racks() {
    use crate::mechanics::{rack_rail_number, rack_units, system, RACK_SYSTEM, RACK_UNIT, RACK_WIDTH};
    let parts = default_catalog();
    let racks: Vec<&Part> = parts.iter().filter(|p| p.tech_value("Enclosure system").is_some_and(|s| s.trim() == RACK_SYSTEM)).collect();
    assert_eq!(racks.len(), system(RACK_SYSTEM).unwrap().sizes.len());
    for r in racks {
        assert!(parts.iter().any(|p| p.number == rack_rail_number(rack_units(r.height))), "rails of {}", r.number);
    }
    let rack_mounted: Vec<&Part> = parts.iter().filter(|p| p.tech_value("Rack units").is_some()).collect();
    assert!(rack_mounted.len() >= 8);
    for p in rack_mounted {
        let u: f32 = p.tech_value("Rack units").unwrap().trim().parse().unwrap();
        assert!((p.width - RACK_WIDTH).abs() < 0.01 && (p.height - u * RACK_UNIT).abs() < 0.01, "{}", p.number);
    }
}

#[test]
fn the_catalog_has_a_part_for_wall_mounting() {
    let parts = default_catalog();
    let light = parts.iter().find(|p| p.number == "GEN.LIGHT-EM-3W").expect("emergency light");
    assert!(light.wall_mountable() && light.width > 0.0 && light.height > 0.0);
}

#[test]
fn cable_glands_come_in_both_materials_and_from_a_manufacturer() {
    let parts = default_catalog();
    let part = |n: &str| parts.iter().find(|p| p.number == n).unwrap_or_else(|| panic!("{n}"));
    for size in ["M16", "M20", "M25", "M32", "M40", "M63"] {
        let (plastic, metal) = (part(&format!("GEN.GLAND-{size}")), part(&format!("GEN.GLAND-{size}-MS")));
        assert_eq!((plastic.subgroup.as_str(), metal.subgroup.as_str()), ("Cable gland", "Cable gland"));
        assert_eq!((plastic.tech_value("Clamping range min"), plastic.tech_value("Clamping range max")), (metal.tech_value("Clamping range min"), metal.tech_value("Clamping range max")));
        assert!(plastic.tech_value("Material").is_some_and(|m| m.contains("Polyamide")) && metal.tech_value("Material").is_some_and(|m| m.contains("nickel")), "{size}");
        // Drawn in their colours: the same shape, a different look.
        let (m1, m2) = (plastic.model_3d.as_ref().unwrap(), metal.model_3d.as_ref().unwrap());
        assert_eq!(m1.triangles.len(), m2.triangles.len());
        assert!(m1.color(0).is_some() && m1.color(m1.triangles.len() - 1) != m2.color(m2.triangles.len() - 1), "{size}");
    }
    assert!(part("GEN.GLAND-M20-EMC").description.contains("EMC") && part("GEN.GLAND-M20-EMC").tech_value("Shielding").is_some());
    let lapp = part("LAPP.53111020");
    assert_eq!((lapp.manufacturer.as_str(), lapp.subgroup.as_str(), lapp.tech_value("Clamping range max")), ("LAPP", "Cable gland", Some("13")));
    assert!(lapp.description.contains("SKINTOP") && lapp.model_3d.is_some());
    assert_eq!(part("LAPP.53112020").tech_value("Material"), Some("Brass, nickel-plated"));
}

/// Two triangles of a model in one plane, facing the same way, one over the other:
/// drawn, they flicker (the depth test cannot tell which is in front).
fn overlapping_faces(m: &crate::Mesh3d) -> Vec<(usize, usize)> {
    use crate::vec3::{cross, dot, norm, sub};
    let tri = |i: usize| m.triangles[i].map(|k| m.vertices[k as usize]);
    let normal = |t: [[f32; 3]; 3]| norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    let centroid = |t: [[f32; 3]; 3]| [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
    let inside = |q: [f32; 3], t: [[f32; 3]; 3], n: [f32; 3]| (0..3).all(|i| dot(cross(sub(t[(i + 1) % 3], t[i]), sub(q, t[i])), n) > 1e-4);
    let mut out = Vec::new();
    for i in 0..m.triangles.len() {
        let (a, na) = (tri(i), normal(tri(i)));
        for j in i + 1..m.triangles.len() {
            let (b, nb) = (tri(j), normal(tri(j)));
            if dot(na, nb) < 0.999 || dot(na, sub(b[0], a[0])).abs() > 0.01 {
                continue;
            }
            if inside(centroid(a), b, nb) || inside(centroid(b), a, na) {
                out.push((i, j));
            }
        }
    }
    out
}

#[test]
fn cable_gland_models_have_no_faces_over_each_other() {
    // The seal insert stood flush with the mouth of the dome: on the outermost face of
    // every gland two caps lay over each other and flickered in the 3D view.
    for p in default_catalog().iter().filter(|p| p.subgroup == "Cable gland") {
        let m = p.model_3d.as_ref().unwrap();
        assert!(overlapping_faces(m).is_empty(), "{}: {:?}", p.number, overlapping_faces(m));
    }
}

#[test]
fn the_catalog_has_fluid_parts_for_rooms() {
    let parts = default_catalog();
    let part = |n: &str| parts.iter().find(|p| p.number == n).unwrap_or_else(|| panic!("{n}"));
    for n in ["GEN.PUMP-CENT-DN25", "GEN.TANK-200L"] {
        let p = part(n);
        assert!(p.floor_standing() && !p.wall_mountable() && p.width > 0.0 && p.depth > 0.0, "{n}");
    }
    for n in ["GEN.VLV-BALL-DN25", "GEN.GAUGE-0-10BAR"] {
        let p = part(n);
        assert!(p.wall_mountable() && !p.floor_standing() && p.width > 0.0, "{n}");
    }
}

#[test]
fn terminals_show_their_openings_and_marker() {
    let mut parts = default_catalog();
    parts.extend(crate::bundled::default_parts());
    let part = |n: &str| parts.iter().find(|p| p.number == n).unwrap_or_else(|| panic!("{n}"));
    // Boxes of a colour in the model: (min, max) of their triangles.
    let painted = |n: &str, c: [u8; 3]| -> Vec<([f32; 3], [f32; 3])> {
        let m = part(n).model_3d.as_ref().unwrap();
        (m.triangles.iter().zip(&m.colors))
            .filter(|(_, col)| **col == c)
            .map(|(t, _)| {
                let v = t.map(|i| m.vertices[i as usize]);
                ([0, 1, 2].map(|k| v.iter().map(|a| a[k]).fold(f32::MAX, f32::min)), [0, 1, 2].map(|k| v.iter().map(|a| a[k]).fold(f32::MIN, f32::max)))
            })
            .collect()
    };
    // Push-in terminals (the generic ones, Phoenix Contact PT) with a pusher in each outer
    // quarter, screw terminals (Phoenix Contact UT) with a screw there; the wire entries
    // at the ends, the white marker field in the middle.
    for (n, clamp) in [("GEN.TB-2.5", PUSHER), ("GEN.TB-35", PUSHER), ("GEN.TB-N-16", PUSHER), ("PXC.3209510", PUSHER), ("PXC.3044076", models::DARK)] {
        let h = part(n).height;
        let clamps = painted(n, clamp);
        assert!(clamps.iter().any(|(_, hi)| hi[1] < 0.25 * h) && clamps.iter().any(|(lo, _)| lo[1] > 0.75 * h), "{n}");
        assert!(painted(n, if clamp == PUSHER { models::DARK } else { PUSHER }).is_empty(), "{n}: one kind of clamp");
        let inside = painted(n, DARK_INSIDE);
        assert!(inside.iter().any(|(lo, _)| lo[1] < 0.1 * h) && inside.iter().any(|(_, hi)| hi[1] > 0.9 * h), "{n}");
        assert!(painted(n, models::WHITE).iter().all(|(lo, hi)| lo[1] > 0.3 * h && hi[1] < 0.7 * h), "{n}");
        assert!(!painted(n, models::WHITE).is_empty(), "{n}");
    }
    assert!(!painted("GEN.TB-PE-2.5", [232, 204, 48]).is_empty(), "the yellow marker field of PE");
    assert!(painted("GEN.TB-END", models::DARK).is_empty() && painted("GEN.TB-END", DARK_INSIDE).is_empty());
    assert!(!painted("GEN.EB-35", models::STEEL).is_empty(), "the screw of the end bracket");
    // Sizes after Phoenix Contact PT 2,5, D-ST 2,5 and CLIPFIX 35: width, length and the
    // height above the rail.
    for (n, size) in [("GEN.TB-2.5", [5.2, 48.6, 29.3]), ("GEN.TB-END", [2.2, 48.6, 29.1]), ("GEN.EB-35", [9.5, 54.6, 26.0]), ("PXC.3209510", [5.2, 48.6, 29.3])] {
        assert_eq!(part(n).size(), Some(size), "{n}");
    }
}

#[test]
fn connectors_have_a_pin_layout_for_every_pin() {
    use crate::connector::PinLayout;
    let parts = default_catalog();
    let connectors: Vec<&Part> = parts.iter().filter(|p| p.group == "Connectors").collect();
    assert!(connectors.len() > 400, "{}", connectors.len());
    for p in &connectors {
        let layout = PinLayout::of_part(p).unwrap_or_else(|| panic!("{} has no pin layout", p.number));
        let pins = p.functions.iter().filter(|f| f.kind == FnKind::PlugPin).count();
        assert!(pins <= layout.slots() && pins > 0, "{}: {pins} pins, {} positions", p.number, layout.slots());
        assert!(matches!(p.tech_value("Gender"), Some("male" | "female" | "hermaphroditic")), "{}", p.number);
        assert!(p.functions.iter().map(|f| &f.pins).enumerate().all(|(i, a)| p.functions[i + 1..].iter().all(|f| &f.pins != a)), "{}: pins named twice", p.number);
    }
    let series = [
        "GX12", "GX16", "GX20", "M5", "M8", "M12", "M16", "M23", "M23 power", "M40 power", "7/8\"", "DIN EN 175301-803", "D-sub", "IDC 2.54", "RJ11", "RJ12", "RJ45",
        "Heavy duty", "DT", "Sealed 1.5", "DIN 41612", "PLC front connector", "Single row 2.5", "Dual row 3.0", "Dual row 4.2", "Pluggable 3.5", "Pluggable 5.08",
        "Pluggable 7.62", "IEC 60309", "IEC 60320", "Mains plug", "Installation plug connector", "PV", "Battery connector", "Banana 4 mm", "Thermocouple", "BNC",
    ];
    for series in series {
        assert!(connectors.iter().any(|p| p.tech_value("Series") == Some(series)), "{series}");
    }
    // A ribbon cable for every IDC socket.
    for p in connectors.iter().filter(|p| p.tech_value("Termination") == Some("IDC")) {
        let n = p.functions.len();
        let cable = parts.iter().find(|c| c.number == format!("GEN.CBL-FLAT-{n}")).unwrap_or_else(|| panic!("ribbon cable for {}", p.number));
        assert_eq!(cable.tech_value("Cable form"), Some("Flat"));
        assert_eq!(cable.tech_value("Cores").map(|c| c.split(' ').count()), Some(n));
    }
}
