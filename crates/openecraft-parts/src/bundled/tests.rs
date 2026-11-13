use super::*;
use crate::{default_catalog, FnKind};
use crate::portal::catalog_id;
use crate::productgroups::group_id;

#[test]
fn bundled_catalogs_are_consistent() {
    let bundles = all().unwrap();
    assert_eq!(bundles.len(), FILES.len());
    let generic = default_catalog();
    let mut numbers: Vec<String> = generic.iter().map(|p| p.number.clone()).collect();
    for (bundle, id) in bundles.iter().zip(ids()) {
        let c = &bundle.catalog;
        assert_eq!((c.id.as_str(), catalog_id(id)), (id, id.to_string()));
        assert!(!c.manufacturer.is_empty() && !c.version.is_empty() && !bundle.parts.is_empty(), "{id}");
        for p in &bundle.parts {
            assert!(!numbers.contains(&p.number), "duplicate {}", p.number);
            numbers.push(p.number.clone());
            // MANUFACTURER.ORDER, or a number made from the type if the order number is unknown.
            let (prefix, rest) = p.number.split_once('.').unwrap_or_default();
            assert!(!prefix.is_empty() && (rest == p.order_number || p.order_number.is_empty()), "{}: number is not MANUFACTURER.ORDER", p.number);
            assert!(!rest.is_empty() && !rest.contains(['/', ' ']), "{}: number", p.number);
            assert_eq!(p.manufacturer, c.manufacturer, "{}", p.number);
            assert_eq!(p.catalog, c.id, "{}", p.number);
            assert!(group_id(&p.group).is_some(), "{}: unknown group {}", p.number, p.group);
            // Mounting material (ducts) is placed without a device tag.
            let mounting = p.group == "Mounting";
            assert!(!p.description.is_empty() && (mounting || !p.identifier.is_empty()), "{}", p.number);
            // Terminals take their designation from the device tag; accessories have no functions.
            assert!(p.functions.iter().all(|f| f.kind == FnKind::Terminal || !f.pin_list().is_empty()), "{}: function template", p.number);
            assert!(!p.functions.is_empty() || mounting || matches!(p.subgroup.as_str(), "Accessories" | "End bracket"), "{}: no functions", p.number);
            // Accessories pair with parts of their catalog or of the generic one (a
            // module carrying the transformers the distribution system operator brings).
            for a in p.accessories.iter().chain(&p.fits) {
                assert!(bundle.parts.iter().chain(&generic).any(|q| &q.number == a), "{}: accessory {a} missing", p.number);
            }
            assert!(!p.accessory || !crate::accessory::mains_of(bundle.parts.iter().chain(&generic), p).is_empty(), "{}: accessory fits nothing", p.number);
            // Every part with dimensions has a generated model, mounting material too.
            assert_eq!(p.model_3d.is_some(), p.width > 0.0 && p.height > 0.0 && p.depth > 0.0, "{}: generated model", p.number);
            // Units are written alike in every catalog (the facets list each spelling).
            let texts = format!("{} {}", p.description, p.tech_data);
            assert!(!texts.contains(" mm2") && !texts.contains(".0 mm²"), "{}: write cross-sections as 2.5 mm²", p.number);
        }
    }
}

#[test]
fn unknown_catalogs_are_refused() {
    let err = load("nobody").unwrap_err();
    assert!(err.contains("siemens"), "{err}");
    assert_eq!(load("SIEMENS").unwrap().catalog.id, "siemens");
}

#[test]
fn victron_devices_are_wall_mountable() {
    let victron = load("victron").unwrap();
    let wall: Vec<&str> = victron.parts.iter().filter(|p| p.wall_mountable()).map(|p| p.number.as_str()).collect();
    assert!(wall.contains(&"VIC.PMP482305012") && wall.contains(&"VIC.SCC115035210"), "{wall:?}");
    // The BMV-712 display goes into a front panel.
    let bmv = victron.parts.iter().find(|p| p.number == "VIC.BAM030712000").unwrap();
    assert!(!bmv.wall_mountable() && !bmv.mounting_data.door_cutouts.is_empty());
    assert!(victron.parts.iter().all(|p| p.model_3d.is_some()));
}

#[test]
fn hager_signal_contact_fits_several_rccbs() {
    let hager = load("hager").unwrap();
    let cz = hager.parts.iter().find(|p| p.number == "HAG.CZ001").unwrap();
    assert!(cz.accessory && cz.accessory_mount == crate::AccessoryMount::Left);
    let mains: Vec<&str> = crate::accessory::mains_of(&hager.parts, cz).iter().map(|p| p.number.as_str()).collect();
    assert_eq!(mains, ["HAG.CCA216D", "HAG.CDA440D", "HAG.CDA463D"]);
}

#[test]
fn hager_breaker_and_fuse_switch_take_their_accessories() {
    let hager = load("hager").unwrap();
    let get = |n: &str| hager.parts.iter().find(|p| p.number == n).unwrap_or_else(|| panic!("{n}"));
    // The coupling switch of the grid protection: motor operator in front, release
    // and auxiliary switch in its slots.
    let breaker = get("HAG.HNT161GR");
    let mounts: Vec<(&str, crate::AccessoryMount)> =
        crate::accessory::accessories_of(&hager.parts, breaker).iter().map(|p| (p.number.as_str(), p.accessory_mount)).collect();
    use crate::AccessoryMount::*;
    // The 230 V release of the P630 fits the P250 too (it names both).
    assert_eq!(mounts, [("HAG.HXT041H", Front), ("HAG.HXA011H", Inside), ("HAG.HXA021H", Inside), ("HAG.HXA014H", Inside)]);
    // The NH1 fuse switch disconnector stands on its univers N module.
    let module = get("HAG.UE31E1B");
    assert_eq!(module.accessory_mount, Behind);
    assert!(crate::accessory::fits(module, get("HAG.LT153")));
    // The cabinet comes with its plinth.
    assert_eq!(get("HAG.FR25S2").tech_value("Plinth"), Some("200"));
    // The current transformers clip in behind the poles of the NH1 - NH3 strips.
    let ct = get("HAG.LVZW150");
    assert_eq!(ct.accessory_mount, Behind);
    let strips: Vec<&str> = crate::accessory::mains_of(&hager.parts, ct).iter().map(|p| p.number.as_str()).collect();
    assert_eq!(strips, ["HAG.LVSG1CPX", "HAG.LVSG2CPX", "HAG.LVSG3CPX"]);
}

#[test]
fn room_outlets_and_manifolds_go_on_the_wall_with_their_nozzles() {
    let gira = load("gira").unwrap();
    let socket = gira.parts.iter().find(|p| p.number == "GIRA.418803").unwrap();
    assert!(socket.wall_mountable() && socket.model_3d.is_some());
    assert_eq!(socket.tech_value("Mounting height"), Some("300 mm"));
    let frame = gira.parts.iter().find(|p| p.number == "GIRA.021103").unwrap();
    assert!(frame.accessory && crate::accessory::mains_of(&gira.parts, frame).len() >= 4, "the frame fits the inserts");
    let mdt = load("mdt").unwrap();
    assert!(mdt.parts.iter().filter(|p| p.wall_mountable()).count() >= 4, "push buttons and room controller");
    // A manifold of COSMO: flow and return in, each circuit out downwards.
    let cosmo = load("cosmo").unwrap();
    let hkv = cosmo.parts.iter().find(|p| p.number == "COSMO.CVS6").unwrap();
    let nozzles = crate::nozzles(hkv);
    assert_eq!(nozzles.len(), 2 + 2 * 6);
    assert_eq!((nozzles[0].pin.as_str(), nozzles[13].pin.as_str()), ("F", "R6"));
    assert!(nozzles[2..].iter().all(|n| n.dir[1] < -0.9 && n.at[1].abs() < 0.5), "the circuits leave downwards");
    assert!(cosmo.parts.iter().any(|p| p.number == "COSMO.CVB9N") && cosmo.parts.iter().any(|p| p.number == "COSMO.CBRTLTH"));
    // The generic manifolds for 2 to 12 circuits.
    let generic = default_catalog();
    let n = generic.iter().filter(|p| p.subgroup == "Heating circuit manifold").count();
    assert_eq!(n, 11);
}

#[test]
fn pv_modules_of_longi_and_jinko_go_on_the_roof() {
    for (id, n) in [("longi", 3), ("jinko", 3)] {
        let c = load(id).unwrap();
        let modules: Vec<&Part> = c.parts.iter().filter(|p| p.subgroup == "PV module").collect();
        assert_eq!(modules.len(), n, "{id}");
        for p in modules {
            assert_eq!((p.width, p.height, p.depth, p.mounting.as_str()), (1134.0, 1722.0, 30.0, "Roof"), "{}", p.number);
            assert!(p.power.ends_with(" Wp") && p.tech_value("Open-circuit voltage").is_some() && p.functions[0].kind == FnKind::PvGenerator, "{}", p.number);
        }
    }
    let longi = load("longi").unwrap();
    let p = longi.parts.iter().find(|p| p.number == "LONGI.LR5-54HTH-430M").unwrap();
    assert_eq!((p.power.as_str(), p.tech_value("Short-circuit current")), ("430 Wp", Some("14.15 A")));
    // The generic mounting: the rail and the bases carry their hooks, clamps and ballast.
    let generic = default_catalog();
    let rail = generic.iter().find(|p| p.number == "GEN.PV-RAIL-40").unwrap();
    assert_eq!(rail.accessories.len(), 4);
    for a in &rail.accessories {
        let acc = generic.iter().find(|p| &p.number == a).unwrap();
        assert!(acc.accessory && crate::accessory::mains_of(&generic, acc).iter().any(|m| m.number == rail.number), "{a}");
    }
}

#[test]
fn bundled_catalogs_have_logos() {
    for b in all().unwrap() {
        let svg = b.logo.unwrap_or_else(|| panic!("{}: no logo in catalogs/logos", b.catalog.id));
        let id = &b.catalog.id;
        assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"), "{id}");
        assert!(svg.contains(&format!("<title>{}</title>", b.catalog.manufacturer)), "{id}: the logo names the manufacturer");
        assert!(svg.len() < 64 * 1024, "{id}: {} bytes", svg.len());
        assert!(b.catalog.logo.is_none(), "the server sets the logo's file");
        // Nothing that runs or loads anything: no scripts, no event handlers, links only
        // inside the file or to embedded images.
        let lower = svg.to_lowercase();
        assert!(!lower.contains("<script") && !lower.contains("javascript:") && !lower.contains("<foreignobject"), "{id}");
        let handler = |w: &str| w.split_once('=').is_some_and(|(name, _)| name.len() > 2 && name.starts_with("on") && name.chars().all(|c| c.is_ascii_lowercase()));
        assert!(!lower.split_whitespace().any(handler), "{id}: event handler");
        for (i, _) in lower.match_indices("href=\"") {
            let target = &lower[i + 6..];
            assert!(target.starts_with('#') || target.starts_with("data:image/"), "{id}: link {}", &target[..target.len().min(40)]);
        }
    }
    assert_eq!(LOGOS.len(), FILES.len());
}
