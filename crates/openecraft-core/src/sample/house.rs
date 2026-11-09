//! Level 16: the plant of level 15 with an office building +B3: the office +OF1 on its
//! ground level and the attic +AT1 under a gable roof whose eaves lie a knee wall above
//! the attic floor, so the attic's ceiling slopes down to 1.2 m at its long walls; a
//! stair turned once at a quarter landing leads up, two roof windows sit in the front
//! slope and a PV array of ten LONGi modules on rails and roof hooks in the rear one,
//! one string on a 10 kW inverter.
//! The rooms get Gira socket outlets, a network outlet and a switch, an MDT KNX push
//! button and room temperature controller, a TV outlet, a COSMO heating circuit manifold
//! for six circuits and a single-room control box.

use super::*;
use crate::geom::p;
use crate::model::panel::{Opening, SitePlace};
use crate::panel::{self, buildings as bld, roofs, solar, stairs, Face, RoofShape, StairKind};

/// Height of the knee wall of the attic: the eaves above its floor, mm.
const KNEE_WALL: f32 = 1000.0;

pub(super) fn project() -> Project {
    let mut prj = super::site::project();
    prj.info.name = "Plant with office building".into();
    prj.info.description = "An office building with attic, stair and roof\nbeside the pump station".into();
    prj.structures.define(crate::structure::Aspect::Location, "OF1", "Office");
    prj.structures.define(crate::structure::Aspect::Location, "AT1", "Attic");
    prj.structures.define(crate::structure::Aspect::Function, "PV", "Photovoltaics");
    let db = PartsDb { parts: crate::parts::bundled::default_parts() };

    // The office building beside the plant: two levels, a room of 8 × 6 m on each.
    let b3 = bld::add_building(&mut prj, "+B3").expect("house");
    prj.layout.buildings[b3].description = "Office building".into();
    let l0 = prj.layout.buildings[b3].levels[0].id;
    let l1 = bld::add_level(&mut prj, b3, None, None, None).expect("house");
    let room = |prj: &mut Project, name: &str, level: u64| {
        let ri = bld::add_room(prj, name, Some(level)).expect("house");
        let e = &mut prj.layout.enclosures[ri];
        (e.width, e.depth) = (8000.0, 6000.0);
        e.site = Some(SitePlace::default());
        ri
    };
    let office = room(&mut prj, "+OF1", l0);
    let attic = room(&mut prj, "+AT1", l1);
    panel::add_opening(&mut prj, office, Opening::Door, Face::Wall(2), Some(5000.0), None, None).expect("house");

    // The stair in the right half of the office: up towards the rear, turned left at
    // the landing.
    let st = stairs::add_stair(&mut prj, b3, StairKind::QuarterLanding, 6900.0, 5800.0, l0, Some(l1)).expect("house");
    if let Some(s) = prj.layout.buildings[b3].stairs.iter_mut().find(|s| s.id == st) {
        s.angle = 90.0;
    }

    // The gable roof with its eaves a knee wall above the attic floor, two roof windows
    // in the front slope.
    roofs::add_roof(&mut prj, b3, RoofShape::Gable).expect("house");
    let attic_floor = prj.layout.buildings[b3].levels[1].elevation;
    let r = &mut prj.layout.buildings[b3].roofs[0];
    (r.eaves, r.pitch) = (attic_floor + KNEE_WALL, 40.0);
    for x in [2500.0, 5500.0] {
        roofs::add_window(&mut prj, b3, 0, Some([x, 4700.0])).expect("house");
    }
    // Two rows of five LONGi modules in the rear slope, on the generic rails and hooks.
    crate::parts::ensure_in_project(&mut prj, &db, "LONGI.LR5-54HTH-430M");
    let pv = solar::add_array(&mut prj, b3, 0, Some([4000.0, 1500.0])).expect("house");
    let mut a = prj.layout.buildings[b3].roofs[0].solar.iter().find(|a| a.id == pv).cloned().expect("house");
    solar::set_module(&prj, &mut a, "LONGI.LR5-54HTH-430M").expect("house");
    a.columns = 5;
    if let Some(s) = prj.layout.buildings[b3].roofs[0].solar.iter_mut().find(|s| s.id == pv) {
        *s = a;
    }
    // All ten in series on the first MPP tracker of a 10 kW inverter.
    crate::parts::ensure_in_project(&mut prj, &db, "GEN.INV-PV-10KW");
    let mut inverter = crate::model::DeviceData::default();
    inverter.parts.push(crate::parts::PartRef { number: "GEN.INV-PV-10KW".into(), count: 1, total: false });
    prj.devices.insert("=PV-T1".into(), inverter);
    solar::strings::add_string(&mut prj, b3, 0, 0).expect("house");

    // The installation: outlets and switches at their mounting heights, the manifold
    // and the control box of the underfloor heating.
    for n in ["GIRA.418803", "GIRA.245200", "GIRA.310600", "MDT.BE-TA55P4.01", "MDT.SCN-RTR55S.01", "GEN.TV-SAT-UP", "COSMO.CVS6", "COSMO.CBRTLTH"] {
        crate::parts::ensure_in_project(&mut prj, &db, n);
    }
    let place = |prj: &mut Project, ri: usize, part: &str, face: Face, x: f32, y: f32| {
        panel::place_accessory(prj, ri, part, face, p(x, y)).expect("house");
    };
    for x in [1500.0, 4500.0] {
        place(&mut prj, office, "GIRA.418803", Face::Wall(0), x, 0.0);
    }
    place(&mut prj, office, "GIRA.245200", Face::Wall(0), 4650.0, 0.0);
    place(&mut prj, office, "GEN.TV-SAT-UP", Face::Wall(1), 2500.0, 0.0);
    place(&mut prj, office, "GIRA.310600", Face::Wall(2), 4800.0, 0.0);
    place(&mut prj, office, "MDT.BE-TA55P4.01", Face::Wall(2), 4900.0, 0.0);
    // The manifold low on the left wall.
    let h = prj.layout.enclosures[office].height;
    place(&mut prj, office, "COSMO.CVS6", Face::Wall(3), 3000.0, h - 300.0 - 420.0);
    // In the attic: on the knee wall under the slope, the controller on the gable wall.
    place(&mut prj, attic, "GIRA.418803", Face::Wall(0), 2000.0, 0.0);
    place(&mut prj, attic, "COSMO.CBRTLTH", Face::Wall(0), 4000.0, 0.0);
    place(&mut prj, attic, "MDT.SCN-RTR55S.01", Face::Wall(1), 3000.0, 0.0);
    prj
}
