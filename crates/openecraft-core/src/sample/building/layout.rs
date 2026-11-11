//! Cables, the two rooms with their enclosures, the devices on the walls and floors,
//! the 19-inch rack, the building entries and the wall penetrations between the rooms.

use super::*;
use crate::geom::p;
use crate::model::panel::{Opening, SitePlace};
use crate::panel::{self, Face, ItemKind};

/// Cables: tag, part, length in m, what it connects.
const CABLES: &[(&str, &str, f32, &str)] = &[
    ("=GRID-W1", "GEN.CBL-NAYY-4X150", 35.0, "Utility supply"),
    ("=GRID-W2", "GEN.CBL-NYY-4X150", 6.0, "Main cable"),
    ("=MD-W11", "GEN.CBL-NYY-5X25", 14.0, "Automation cabinet"),
    ("=MD-W12", "GEN.CBL-NYY-5X35", 16.0, "Battery storage"),
    ("=MD-W13", "GEN.CBL-NYY-5X35", 14.0, "Pump cabinet"),
    ("=WAT-W31", "GEN.CBL-7G1.0", 20.0, "Valve actuator"),
    ("=WAT-W32", "GEN.CBL-SENS-3X0.34", 10.0, "Level transmitter"),
    ("=WAT-W33", "GEN.CBL-SENS-3X0.34", 10.0, "Pressure transmitter"),
    ("=WAT-W34", "GEN.CBL-SENS-3X0.34", 10.0, "Dry-run protection"),
    ("=WAT-W41", "GEN.CBL-4G1.5", 8.0, "Pump motor 1"),
    ("=WAT-W42", "GEN.CBL-4G1.5", 8.0, "Pump motor 2"),
    ("=PV-W51", "GEN.CBL-PV-2X6", 30.0, "PV string 1"),
    ("=PV-W52", "GEN.CBL-PV-2X6", 30.0, "PV string 2"),
    ("=PV-W53", "GEN.CBL-PV-2X6", 5.0, "PV input Inverter RS 1"),
    ("=PV-W54", "GEN.CBL-PV-2X6", 5.0, "PV input Inverter RS 2"),
    ("=PV-W55", "GEN.CBL-NYM-3X2.5", 6.0, "AC output Inverter RS 1"),
    ("=PV-W56", "GEN.CBL-NYM-3X2.5", 6.0, "AC output Inverter RS 2"),
    ("=ESS-W22", "GEN.CBL-NYM-3X2.5", 5.0, "PDU 19-inch rack"),
    ("=ESS-W23", "GEN.CBL-NYM-3X2.5", 8.0, "Control voltage automation cabinet"),
    ("=ESS-W71", "GEN.CBL-BAT-2X35", 4.0, "Battery 1"),
    ("=ESS-W72", "GEN.CBL-BAT-2X35", 4.0, "Battery 2"),
    ("=ESS-W73", "GEN.CBL-BAT-2X35", 4.0, "Battery 3"),
    ("=ESS-W74", "GEN.CBL-BAT-2X35", 4.0, "Battery 4"),
    ("=ESS-W81", "GEN.CBL-BAT-2X70", 3.0, "MultiPlus-II 1 DC"),
    ("=ESS-W82", "GEN.CBL-BAT-2X70", 3.0, "MultiPlus-II 2 DC"),
    ("=ESS-W83", "GEN.CBL-BAT-2X70", 3.0, "MultiPlus-II 3 DC"),
    ("=ESS-W84", "GEN.CBL-BAT-2X35", 3.0, "Inverter RS 1 DC"),
    ("=ESS-W85", "GEN.CBL-BAT-2X35", 3.0, "Inverter RS 2 DC"),
    ("=AIR-W1", "GEN.CBL-4G1.5", 12.0, "Compressor motor"),
    ("=HEAT-W1", "GEN.CBL-5G2.5", 10.0, "Heat pump"),
    ("=HEAT-W2", "GEN.CBL-NYM-3X2.5", 8.0, "Circulating pump heat pump"),
    ("=HEAT-W3", "GEN.CBL-NYM-3X2.5", 10.0, "Circulating pump hot water"),
    ("=GRID-W3", "GEN.CBL-H07VK-1X25-GNYE", 8.0, "Main bonding conductor"),
];

pub(super) fn cables(prj: &mut Project, db: &PartsDb) {
    let patch: Vec<(String, f32)> = (91..=102).map(|n| (format!("=ESS-W{n}"), 3.0)).collect();
    let all = CABLES.iter().map(|&(t, p, l, d)| (t.to_string(), p, l, d)).chain(patch.iter().map(|(t, l)| (t.clone(), "GEN.CBL-RJ45-CAT6", *l, "Patch cable")));
    for (tag, number, length, desc) in all {
        let part = db.get(number).unwrap_or_else(|| panic!("cable part {number}")).clone();
        crate::parts::ensure_in_project(prj, db, number);
        let id = prj.new_id();
        prj.cables.push(Cable {
            id,
            tag,
            cable_type: part.type_number.clone(),
            cores: part.tech_value("Cores").unwrap_or_default().into(),
            cross_section: part.tech_value("Cross-section").unwrap_or_default().into(),
            length,
            part: part.number.clone(),
            description: desc.into(),
            ..Default::default()
        });
    }
    // Cores of the control cables on their two-point connections.
    for (page, a, b, cable, core) in [
        ("3.4", (48.0, 164.0), (48.0, 196.0), "=WAT-W31", "1"),
        ("3.4", (56.0, 180.0), (56.0, 196.0), "=WAT-W31", "2"),
        ("3.3", (40.0, 64.0), (40.0, 68.0), "=WAT-W31", "3"),
        ("3.3", (80.0, 64.0), (80.0, 68.0), "=WAT-W31", "4"),
        ("3.3", (120.0, 64.0), (120.0, 80.0), "=WAT-W34", "BK"),
        ("3.5", (100.0, 68.0), (100.0, 120.0), "=WAT-W32", "BK"),
        ("3.5", (200.0, 68.0), (200.0, 120.0), "=WAT-W33", "BK"),
    ] {
        let pi = prj.pages.iter().position(|pg| pg.name == page).unwrap_or_else(|| panic!("page {page}"));
        let w = prj.pages[pi].wires.iter_mut().find(|w| (w.a.x, w.a.y) == a && (w.b.x, w.b.y) == b).unwrap_or_else(|| panic!("wire on {page}"));
        w.cable = cable.into();
        w.core = core.into();
    }
}

fn profile(prj: &mut Project, enc: usize, kind: ItemKind, part: &str, a: (f32, f32), b: (f32, f32)) {
    let item = panel::new_profile(prj, kind, part, p(a.0, a.1), p(b.0, b.1));
    prj.layout.enclosures[enc].items.push(item);
}

/// Devices side by side from `x` on the rail at height `rail_y`.
fn on_rail(prj: &mut Project, enc: usize, rail_y: f32, x: f32, tags: &[&str]) {
    let mut x = x;
    for tag in tags {
        let (w, h) = panel::device_size(prj, tag).map(|d| (d.w, d.h)).unwrap_or((40.0, 40.0));
        panel::place_device(prj, enc, tag, p(x, rail_y - h / 2.0));
        x += panel::snap(w + 10.0);
    }
}

/// A floor-standing enclosure with a frame of ducts `d`, horizontal ducts and rails.
fn cabinet(prj: &mut Project, part: &str, name: &str, d: &str, rails: &[f32], ducts: &[f32]) -> usize {
    let mut e = panel::new_enclosure(prj, part, name);
    e.build.gland_plate = true;
    prj.layout.enclosures.push(e);
    let ei = prj.layout.enclosures.len() - 1;
    let (w, h) = prj.layout.enclosures[ei].plate_size();
    // The frame of ducts along the edges of the plate, the rails between them.
    let f = panel::profile(prj, ItemKind::Duct, d).0 / 2.0;
    profile(prj, ei, ItemKind::Duct, d, (0.0, f), (w, f));
    profile(prj, ei, ItemKind::Duct, d, (0.0, h - f), (w, h - f));
    profile(prj, ei, ItemKind::Duct, d, (f, 2.0 * f), (f, h - 2.0 * f));
    profile(prj, ei, ItemKind::Duct, d, (w - f, 2.0 * f), (w - f, h - 2.0 * f));
    for &y in ducts {
        profile(prj, ei, ItemKind::Duct, d, (2.0 * f, y), (w - 2.0 * f, y));
    }
    for &y in rails {
        profile(prj, ei, ItemKind::Rail, panel::RAIL_PART, (2.0 * f + 20.0, y), (w - 2.0 * f - 20.0, y));
    }
    ei
}

fn gland(prj: &mut Project, enc: usize, part: &str, face: Face, x: f32, y: f32) {
    panel::place_accessory(prj, enc, part, face, p(x, y));
}

/// A cable gland in the roof right above a device: its cable goes straight up.
fn gland_over(prj: &mut Project, enc: usize, part: &str, tag: &str) {
    gland_near(prj, enc, part, tag, 0.0, 0.0);
}

/// A cable gland in the roof above a device, `dx` beside and `dz` behind the point
/// right above its middle: a gland for each further cable to the device.
fn gland_near(prj: &mut Project, enc: usize, part: &str, tag: &str, dx: f32, dz: f32) {
    let e = &prj.layout.enclosures[enc];
    let Some(it) = e.items.iter().find(|i| i.tag == tag) else { return };
    let c = panel::face_point(e, it.face, [it.x + it.w / 2.0, it.y + it.h / 2.0, it.z + it.d]);
    gland_at(prj, enc, part, c[0] + dx, c[2] + dz);
}

/// A cable gland in the roof with its middle in the routing cell of `x` and depth `z`
/// (enclosure coordinates).
fn gland_at(prj: &mut Project, enc: usize, part: &str, x: f32, z: f32) {
    let w = prj.parts.iter().find(|q| q.number == part).map_or(30.0, |q| q.width);
    let cell = prj.layout.routing.grid.max(5.0);
    let mid = |v: f32| ((v / cell).floor() + 0.5) * cell - w / 2.0;
    gland(prj, enc, part, Face::Roof, mid(x), mid(z));
}

/// Pipe fittings on a wall of a room, their middles on one line `y` below the ceiling.
fn fittings(prj: &mut Project, room: usize, face: Face, y: f32, at: &[(&str, f32)]) {
    for (tag, x) in at {
        let h = panel::device_size(prj, tag).map_or(100.0, |d| d.h);
        panel::mount_on_wall(prj, room, tag, face, p(*x, y - h / 2.0)).unwrap_or_else(|e| panic!("{tag}: {e}"));
    }
}

/// Where the coupling of a compressed air outlet hangs: its middle this far beside the
/// outlet port of the unit before it, its top this much lower than a unit would hang
/// below that one.
const COUPLING_ASIDE: f32 = 150.0;
const COUPLING_BELOW: f32 = 80.0;

pub(super) fn rooms(prj: &mut Project, db: &PartsDb, section: u64) {
    let [md, cc1, rack, acd, gak, hak] = enclosures(prj, db);
    house_connection_room(prj, md, hak);
    technical_room(prj, [cc1, rack, acd, gak]);

    // Mounting layout pages of the enclosures and rooms.
    let ids: Vec<(u64, String)> = prj.layout.enclosures.iter().map(|e| (e.id, e.name.clone())).collect();
    for (n, (id, name)) in ids.iter().enumerate() {
        sec_page(prj, section, &(n + 1).to_string(), &format!("Mounting layout {name}"), PageKind::Mounting(*id), "", "");
    }
}

/// The enclosures with their devices: main distribution, automation and pump cabinets,
/// AC distribution, generator junction box, 19-inch rack, house connection box.
fn enclosures(prj: &mut Project, db: &PartsDb) -> [usize; 6] {
    for part in [
        "GEN.HAK-410X720X185",
        "GEN.TS-1000X2000X600",
        "GEN.TS-800X2000X500",
        "GEN.CE-760X760X300",
        "GEN.PC-400X600X200",
        "GEN.RK-600X2000X1000",
        panel::RAIL_PART,
        panel::DUCT_PART,
        "GEN.BB-3X30X10",
        "GEN.GLAND-M16",
        "GEN.GLAND-M20",
        "GEN.GLAND-M32",
        "GEN.GLAND-M40",
        "GEN.GLAND-M63",
        "GEN.DUCT-100X80",
        "GEN.DUCT-100X120",
        "GEN.DUCT-80X80",
        "GEN.GLAND-M25",
        "GEN.PE-BAR-10",
        "GEN.HANDLE-SWING",
        "GEN.RK-PATCH-24P",
        "GEN.RK-BLANK-1U",
        "GEN.RK-CABLE-1U",
    ] {
        crate::parts::ensure_in_project(prj, db, part);
    }
    // Main distribution: busbar system, NH fuse switch disconnectors on it, the
    // meters and terminals on rails; every cable goes up through a gland right above
    // the device it lands on.
    let md = cabinet(prj, "GEN.TS-1000X2000X600", "+MD", "GEN.DUCT-100X80", &[900.0, 1250.0, 1550.0], &[180.0, 760.0, 1100.0, 1400.0, 1700.0]);
    // The ducts above and below the fuse switch disconnectors hold the 185 mm² feeders,
    // the one below the meters their cables, the one along the right edge takes them
    // up to their glands: deeper, so that they lie side by side in them, cross one
    // over the other and bend in them with their bend radius.
    let deep = panel::profile(prj, ItemKind::Duct, "GEN.DUCT-100X120").1;
    for it in prj.layout.enclosures[md].items.iter_mut().filter(|i| i.kind == ItemKind::Duct && ([0.0, 130.0, 710.0, 1050.0].contains(&i.y) || (i.h > i.w && i.x > 400.0))) {
        it.part = "GEN.DUCT-100X120".into();
        it.d = deep;
    }
    // The incoming terminals feed the bars they are mounted on (the main cable comes
    // straight down to them from its gland), the fuse switch disconnectors sit on them.
    profile(prj, md, ItemKind::Busbar, "GEN.BB-3X30X10", (120.0, 300.0), (840.0, 300.0));
    for (tag, x) in [("=MD+MD-X0", 140.0), ("=MD+MD-Q1", 260.0), ("=MD+MD-Q2", 380.0), ("=MD+MD-Q3", 560.0)] {
        panel::place_device(prj, md, tag, p(x, 255.0));
    }
    on_rail(prj, md, 900.0, 240.0, &["=MD+MD-P1", "=MD+MD-P2", "=MD+MD-P3", "=MD+MD-F11", "=MD+MD-F10"]);
    on_rail(prj, md, 900.0, 730.0, &["=MD+MD-X2"]);
    on_rail(prj, md, 1250.0, 560.0, &["=MD+MD-T11"]);
    panel::place_accessory(prj, md, "GEN.PE-BAR-10", Face::Plate, p(120.0, 1760.0));
    panel::place_accessory(prj, md, "GEN.HANDLE-SWING", Face::Door, p(890.0, 900.0));
    // The main cable comes in above its terminals, the feeders leave through glands
    // of their own above the top duct, up to the duct on the right.
    gland_over(prj, md, "GEN.GLAND-M63", "=MD+MD-X0");
    // The main bonding conductor from the PE terminals down to the main earthing busbar:
    // its gland above them, behind the row of the feeder glands.
    let e = &prj.layout.enclosures[md];
    if let Some(it) = e.items.iter().find(|i| i.tag == "=MD+MD-X2") {
        let x = panel::face_point(e, it.face, [it.x + it.w / 2.0, it.y, 0.0])[0];
        gland_at(prj, md, "GEN.GLAND-M25", x, 200.0);
    }
    let duct_x = panel::PLATE_MARGIN + prj.layout.enclosures[md].plate_size().0 - panel::profile(prj, ItemKind::Duct, "GEN.DUCT-100X80").0 / 2.0;
    for k in 0..3 {
        gland_at(prj, md, "GEN.GLAND-M40", duct_x - 40.0 - 75.0 * k as f32, 87.5);
    }

    // Automation cabinet +CC1 and pump cabinet +CC2, bayed: power at the top, the PLC
    // and the EtherCAT terminals at the bottom; the sensor cables leave through the
    // gland plate.
    let duct = "GEN.DUCT-80X80";
    let cc1 = cabinet(prj, "GEN.TS-800X2000X500", "+CC1", duct, &[380.0, 720.0, 1050.0, 1400.0], &[230.0, 560.0, 900.0, 1230.0, 1580.0]);
    on_rail(prj, cc1, 380.0, 100.0, &["=CTL+CC1-X0", "=CTL+CC1-Q1", "=CTL+CC1-F1", "=CTL+CC1-F2", "=CTL+CC1-X2"]);
    on_rail(prj, cc1, 720.0, 300.0, &["=CTL+CC1-X1", "=CTL+CC1-T1", "=CTL+CC1-B2", "=CTL+CC1-E2"]);
    on_rail(prj, cc1, 1050.0, 480.0, &["=CTL+CC1-X3"]);
    on_rail(prj, cc1, 1400.0, 480.0, &["=CTL+CC1-A1", "=CTL+CC1-A2", "=CTL+CC1-A3", "=CTL+CC1-A4"]);
    for (tag, x) in [("=CTL+CC1-P1", 300.0), ("=CTL+CC1-P2", 400.0)] {
        panel::place_device_on(prj, cc1, tag, p(x, 900.0), Face::Door);
    }
    panel::place_accessory(prj, cc1, "GEN.HANDLE-SWING", Face::Door, p(700.0, 900.0));
    let cc2 = cabinet(prj, "GEN.TS-800X2000X500", "+CC2", duct, &[380.0, 720.0, 1050.0, 1400.0], &[230.0, 560.0, 900.0, 1230.0, 1580.0]);
    on_rail(prj, cc2, 380.0, 100.0, &["=WAT+CC2-X0", "=WAT+CC2-Q1", "=WAT+CC2-Q11", "=WAT+CC2-Q12"]);
    on_rail(prj, cc2, 720.0, 100.0, &["=WAT+CC2-K11", "=WAT+CC2-K12"]);
    on_rail(prj, cc2, 720.0, 480.0, &["=WAT+CC2-X11"]);
    on_rail(prj, cc2, 720.0, 600.0, &["=WAT+CC2-X12"]);
    on_rail(prj, cc2, 1050.0, 100.0, &["=AIR+CC2-Q1", "=AIR+CC2-K1", "=HEAT+CC2-Q1", "=HEAT+CC2-F2", "=HEAT+CC2-K1", "=HEAT+CC2-K2", "=HEAT+CC2-K3"]);
    on_rail(prj, cc2, 1050.0, 410.0, &["=AIR+CC2-X1"]);
    on_rail(prj, cc2, 1050.0, 520.0, &["=HEAT+CC2-X1"]);
    on_rail(prj, cc2, 1400.0, 100.0, &["=CTL+CC2-A10", "=CTL+CC2-A11", "=CTL+CC2-A12", "=CTL+CC2-A13", "=CTL+CC2-A14"]);
    panel::place_accessory(prj, cc2, "GEN.HANDLE-SWING", Face::Door, p(700.0, 900.0));
    panel::bay_right_of(prj, cc2, cc1);
    panel::add_passage(prj, cc1, 300.0, 120.0, panel::PASSAGE_DIAMETER);
    // A cable gland in the roof for every cable, sized to it, above the device it goes
    // to: the feeders (22 and 25 mm) M40, the control cables theirs, the patch cable
    // and the three sensor cables to the PLC M16 in a row behind it; the heat pump's
    // three cables one behind the other above their strip.
    for (enc, part, tag, dx, dz) in [
        (cc1, "GEN.GLAND-M40", "=CTL+CC1-X0", 0.0, 0.0),
        (cc1, "GEN.GLAND-M25", "=CTL+CC1-X1", 0.0, 0.0),
        (cc1, "GEN.GLAND-M20", "=CTL+CC1-X3", 0.0, 0.0),
        (cc1, "GEN.GLAND-M16", "=CTL+CC1-A1", -75.0, 100.0),
        (cc1, "GEN.GLAND-M16", "=CTL+CC1-A1", 0.0, 100.0),
        (cc1, "GEN.GLAND-M16", "=CTL+CC1-A1", 75.0, 100.0),
        (cc1, "GEN.GLAND-M16", "=CTL+CC1-A1", 150.0, 100.0),
        (cc2, "GEN.GLAND-M40", "=WAT+CC2-X0", 0.0, 0.0),
        (cc2, "GEN.GLAND-M20", "=WAT+CC2-X11", 0.0, 0.0),
        (cc2, "GEN.GLAND-M20", "=WAT+CC2-X12", 0.0, 0.0),
        (cc2, "GEN.GLAND-M20", "=AIR+CC2-X1", 0.0, 0.0),
        (cc2, "GEN.GLAND-M20", "=HEAT+CC2-X1", 0.0, 0.0),
        (cc2, "GEN.GLAND-M20", "=HEAT+CC2-X1", 0.0, 100.0),
        (cc2, "GEN.GLAND-M20", "=HEAT+CC2-X1", 0.0, 200.0),
    ] {
        gland_near(prj, enc, part, tag, dx, dz);
    }

    // AC distribution of the battery storage and the generator junction box, both
    // hanging on the wall.
    let mut acd = panel::new_enclosure(prj, "GEN.CE-760X760X300", "+ACD");
    acd.elevation = Some(1000.0);
    prj.layout.enclosures.push(acd);
    let acd = prj.layout.enclosures.len() - 1;
    profile(prj, acd, ItemKind::Rail, panel::RAIL_PART, (40.0, 200.0), (680.0, 200.0));
    profile(prj, acd, ItemKind::Rail, panel::RAIL_PART, (40.0, 470.0), (680.0, 470.0));
    on_rail(prj, acd, 200.0, 40.0, &["=ESS+ACD-Q1", "=ESS+ACD-F1", "=ESS+ACD-F2", "=ESS+ACD-F3", "=ESS+ACD-F10", "=ESS+ACD-F11", "=ESS+ACD-F12", "=ESS+ACD-F13"]);
    on_rail(prj, acd, 470.0, 40.0, &["=ESS+ACD-F21", "=ESS+ACD-F22", "=ESS+ACD-P10", "=ESS+ACD-X11", "=ESS+ACD-X21", "=ESS+ACD-X22"]);
    on_rail(prj, acd, 470.0, 360.0, &["=ESS+ACD-X0"]);
    on_rail(prj, acd, 470.0, 460.0, &["=ESS+ACD-X1"]);
    on_rail(prj, acd, 470.0, 560.0, &["=ESS+ACD-X2"]);
    // The feeder (25 mm) through an M40 gland, the two cables to each of the other
    // strips through M20 glands one behind the other.
    gland_over(prj, acd, "GEN.GLAND-M40", "=ESS+ACD-X0");
    for (tag, dz) in [("=ESS+ACD-X1", 0.0), ("=ESS+ACD-X1", 100.0), ("=ESS+ACD-X2", 0.0), ("=ESS+ACD-X2", 100.0)] {
        gland_near(prj, acd, "GEN.GLAND-M20", tag, 0.0, dz);
    }
    let mut gak = panel::new_enclosure(prj, "GEN.PC-400X600X200", "+GAK");
    gak.elevation = Some(1100.0);
    prj.layout.enclosures.push(gak);
    let gak = prj.layout.enclosures.len() - 1;
    // The string fuses under the entry of the strings, their terminals and the surge
    // arresters on a rail below them: the conductors run down from one to the next.
    profile(prj, gak, ItemKind::Rail, panel::RAIL_PART, (20.0, 150.0), (340.0, 150.0));
    on_rail(prj, gak, 150.0, 20.0, &["=PV+GAK-F1", "=PV+GAK-F2"]);
    profile(prj, gak, ItemKind::Rail, panel::RAIL_PART, (20.0, 360.0), (340.0, 360.0));
    on_rail(prj, gak, 360.0, 20.0, &["=PV+GAK-X1", "=PV+GAK-F3", "=PV+GAK-F4"]);
    // The two strings come in through the roof above their fuses, the two cables to
    // the inverters on the wall above the box leave through the roof too.
    for (tag, dx) in [("=PV+GAK-F1", 0.0), ("=PV+GAK-F2", 0.0), ("=PV+GAK-X1", 150.0), ("=PV+GAK-X1", 225.0)] {
        gland_near(prj, gak, "GEN.GLAND-M20", tag, dx, 0.0);
    }

    // The 19-inch rack: batteries at the bottom, network at the top.
    let rack = panel::new_enclosure(prj, "GEN.RK-600X2000X1000", "+RACK");
    prj.layout.enclosures.push(rack);
    let rack = prj.layout.enclosures.len() - 1;
    for (i, tag) in ["=ESS+RACK-G1", "=ESS+RACK-G2", "=ESS+RACK-G3", "=ESS+RACK-G4"].into_iter().enumerate() {
        panel::mount_in_rack(prj, rack, tag, Some(1 + 4 * i as u32)).expect("battery in the rack");
    }
    for (what, unit) in [("GEN.RK-BLANK-1U", 17), ("=ESS+RACK-X3", 30), ("GEN.RK-CABLE-1U", 39), ("=ESS+RACK-K2", 40), ("GEN.RK-PATCH-24P", 41)] {
        panel::mount_in_rack(prj, rack, what, Some(unit)).expect("19-inch part");
    }
    // Each cable its own gland in the roof: the battery cables, the supply of the PDU
    // and the patch cables.
    for (i, part) in ["GEN.GLAND-M32", "GEN.GLAND-M32", "GEN.GLAND-M32", "GEN.GLAND-M32", "GEN.GLAND-M20"].into_iter().enumerate() {
        gland(prj, rack, part, Face::Roof, 100.0 + 80.0 * i as f32, 400.0);
    }
    for i in 0..6 {
        gland(prj, rack, "GEN.GLAND-M16", Face::Roof, 100.0 + 60.0 * i as f32, 520.0);
    }
    // The house connection box: the NH fuse base of the utility, fed from below: the
    // supply cable comes straight up from its gland in the bottom, the main cable
    // leaves straight up through the top; the PEN terminal block beside the supply
    // cable.
    let mut hak = panel::new_enclosure(prj, "GEN.HAK-410X720X185", "+HAK");
    hak.elevation = Some(1100.0);
    prj.layout.enclosures.push(hak);
    let hak = prj.layout.enclosures.len() - 1;
    panel::place_device(prj, hak, "=GRID+HAK-F1", p(10.0, 150.0));
    panel::place_device(prj, hak, "=GRID+HAK-X0", p(215.0, 530.0));
    // Both glands in line with the middle terminals of the fuse base.
    let (x, z) = (panel::PLATE_MARGIN + 10.0 + 135.0 - 40.0, panel::PLATE_Z + 60.0 - 40.0);
    for face in [Face::Roof, Face::Bottom] {
        gland(prj, hak, "GEN.GLAND-M63", face, x, z);
    }
    [md, cc1, rack, acd, gak, hak]
}

/// House connection room +HR: 4 x 3 m, building entries for the supply cable and the
/// water main in its left wall, the house connection box above the cable entry with the
/// main earthing busbar and the lug of the foundation earth electrode beside it, the
/// main distribution at the rear wall.
fn house_connection_room(prj: &mut Project, md: usize, hak: usize) {
    let mut hr = panel::new_room(prj, "+HR", 4000.0, 3000.0, 2800.0);
    hr.site = Some(SitePlace { x: 0.0, z: 0.0, angle: 0.0 });
    prj.layout.enclosures.push(hr);
    let hr = prj.layout.enclosures.len() - 1;
    // Walls: 0 rear, 1 right (to the technical room), 2 front, 3 left.
    // The cable entry low in the wall: the supply cable, 45 mm thick, bends up out of
    // it and over into the house connection box with its bend radius of 180 mm.
    panel::add_opening(prj, hr, Opening::Entry, Face::Wall(3), Some(600.0), None, Some(225.0)).expect("cable entry");
    panel::add_opening(prj, hr, Opening::Entry, Face::Wall(3), Some(2700.0), Some((200.0, 200.0)), Some(1140.0)).expect("water entry");
    panel::add_opening(prj, hr, Opening::Door, Face::Wall(2), Some(400.0), None, None).expect("door");
    panel::add_opening(prj, hr, Opening::Penetration, Face::Wall(1), Some(1000.0), None, None).expect("cable penetration");
    panel::add_opening(prj, hr, Opening::Penetration, Face::Wall(1), Some(2400.0), Some((200.0, 200.0)), Some(1140.0)).expect("pipe penetration");
    panel::set_room(prj, md, Some(hr), Some(SitePlace { x: 1700.0, z: 100.0, angle: 0.0 })).expect("main distribution");
    // The left wall runs from the front (z 3000) to the rear seen from inside: the box
    // hangs 500 - 910 mm along it, its front facing into the room.
    panel::set_room(prj, hak, Some(hr), Some(SitePlace { x: 0.0, z: 2500.0, angle: 90.0 })).expect("house connection box");
    panel::mount_on_wall(prj, hr, "=GRID+HR-X1", Face::Wall(3), p(950.0, 2220.0)).expect("main earthing busbar");
    panel::mount_on_wall(prj, hr, "=GRID+HR-E1", Face::Wall(3), p(960.0, 2320.0)).expect("foundation earth electrode");
    // The water main along a pipe run on the left wall to the motor valve beside the
    // wall penetration; its wall sleeve sits right before the building entry, far enough
    // from the ball valve for the pipe out of the wall to bend round and along it.
    fittings(prj, hr, Face::Wall(3), 1560.0, &[("=WAT+HR-UM1", 2510.0), ("=WAT+HR-QM1", 2000.0), ("=WAT+HR-BF1", 1600.0), ("=WAT+HR-RM1", 1250.0)]);
    fittings(prj, hr, Face::Wall(1), 1560.0, &[("=WAT+HR-QM2", 1900.0)]);
    panel::add_pipe_run(prj, hr, Face::Wall(3), &[p(1100.0, 1560.0), p(2700.0, 1560.0)], None, None, Some("PR1")).expect("pipe run");
    panel::add_wall_profile(prj, hr, ItemKind::Duct, "GEN.DUCT-100X80", Face::Roof, p(400.0, 1150.0), p(3900.0, 1150.0)).expect("cable tray");
}

/// Technical room +TR behind the 240 mm wall: the line-up and the rack at the rear
/// wall, the inverters on the right wall, tanks, pumps, compressor and heat pump on the
/// floor, the outlets of the compressed air on the common wall.
fn technical_room(prj: &mut Project, [cc1, rack, acd, gak]: [usize; 4]) {
    let mut tr = panel::new_room(prj, "+TR", 7000.0, 5000.0, 2800.0);
    tr.site = Some(SitePlace { x: 4240.0, z: 0.0, angle: 0.0 });
    prj.layout.enclosures.push(tr);
    let tr = prj.layout.enclosures.len() - 1;
    panel::add_opening(prj, tr, Opening::Door, Face::Wall(2), Some(6000.0), None, None).expect("door");
    panel::add_opening(prj, tr, Opening::Window, Face::Wall(1), Some(3600.0), None, None).expect("window");
    panel::set_room(prj, cc1, Some(tr), Some(SitePlace { x: 400.0, z: 100.0, angle: 0.0 })).expect("line-up");
    panel::set_room(prj, rack, Some(tr), Some(SitePlace { x: 2400.0, z: 100.0, angle: 0.0 })).expect("rack");
    panel::set_room(prj, acd, Some(tr), Some(SitePlace { x: 3400.0, z: 0.0, angle: 0.0 })).expect("AC distribution");
    panel::set_room(prj, gak, Some(tr), Some(SitePlace { x: 4500.0, z: 0.0, angle: 0.0 })).expect("generator junction box");
    for (tag, x, y) in [
        ("=ESS+TR-T1", 500.0, 800.0),
        ("=ESS+TR-T2", 1000.0, 800.0),
        ("=ESS+TR-T3", 1500.0, 800.0),
        ("=PV+TR-T1", 2100.0, 800.0),
        ("=PV+TR-T2", 2700.0, 800.0),
        ("=ESS+TR-K1", 3300.0, 1000.0),
        ("=ESS+TR-F1", 500.0, 1700.0),
        ("=ESS+TR-P1", 900.0, 1800.0),
        ("=ESS+TR-F2", 1300.0, 1700.0),
        ("=ESS+TR-F3", 1700.0, 1700.0),
    ] {
        panel::mount_on_wall(prj, tr, tag, Face::Wall(1), p(x, y)).unwrap_or_else(|e| panic!("{tag}: {e}"));
    }
    // Floor: the storage tank, the booster pumps with their motors in line in front of
    // them, the pressure vessel; front wall: the valves along a pipe run.
    for (tag, x, z) in [
        ("=WAT+TR-CM1", 3600.0, 3300.0),
        ("=WAT+TR-GP1", 5100.0, 3000.0),
        ("=WAT+TR-M1", 5135.0, 3690.0),
        ("=WAT+TR-GP2", 5600.0, 3000.0),
        ("=WAT+TR-M2", 5635.0, 3690.0),
        ("=WAT+TR-CM2", 6300.0, 3600.0),
        ("=AIR+TR-GQ1", 300.0, 1400.0),
        ("=AIR+TR-M1", 1500.0, 1500.0),
        ("=AIR+TR-CM1", 300.0, 2200.0),
        ("=AIR+TR-HQ1", 1100.0, 2300.0),
        ("=HEAT+TR-CM1", 5000.0, 1400.0),
        ("=HEAT+TR-EQ1", 6100.0, 1400.0),
    ] {
        panel::mount_on_wall(prj, tr, tag, Face::Bottom, p(x, z)).unwrap_or_else(|e| panic!("{tag}: {e}"));
    }
    // The pumps are coupled to their motors, raised on risers to the pumps' shaft
    // height; the compressor beside its motor is belt-driven.
    for (machine, motor, riser) in [("=WAT+TR-GP1", "=WAT+TR-M1", 27.0), ("=WAT+TR-GP2", "=WAT+TR-M2", 27.0), ("=AIR+TR-GQ1", "=AIR+TR-M1", 0.0)] {
        crate::sample::drive(prj, tr, machine, motor, riser);
    }
    fittings(prj, tr, Face::Wall(2), 1560.0, &[("=WAT+TR-QM3", 2800.0), ("=WAT+TR-RM2", 2200.0), ("=WAT+TR-RM3", 1800.0), ("=WAT+TR-QM4", 1000.0), ("=WAT+TR-UM2", 300.0)]);
    // Hot water beside them, in the order of the pipes: the valve and the distribution
    // to the floors, the circulation return with its pump and check valve, the cold
    // water inlet with its valve and check valve.
    fittings(prj, tr, Face::Wall(2), 1560.0, &[("=HEAT+TR-QM1", 5720.0), ("=HEAT+TR-UM1", 5050.0), ("=HEAT+TR-UM2", 4350.0), ("=HEAT+TR-GP2", 4050.0), ("=HEAT+TR-RM2", 3800.0), ("=HEAT+TR-QM2", 3500.0), ("=HEAT+TR-RM1", 3250.0)]);
    panel::add_pipe_run(prj, tr, Face::Wall(2), &[p(200.0, 1560.0), p(5800.0, 1560.0)], None, Some(250.0), Some("PR2")).expect("pipe run");
    fittings(prj, tr, Face::Wall(1), 2000.0, &[("=HEAT+TR-GP1", 2300.0)]);
    // Compressed air: the distribution line along the common wall, each outlet hanging
    // from it with its treatment. The units have their ports on their sides: the
    // coupling hangs beside the last one and lower, where the pipe out of its outlet
    // port bends down into it.
    for ((devices, _), x) in super::air::OUTLETS.iter().zip([1200.0, 1600.0, 2000.0, 2900.0]) {
        let (mut y, mut right) = (1000.0, x);
        for (i, (_, tag)) in devices.iter().enumerate() {
            let tag = format!("=AIR+TR{tag}");
            let (w, h) = panel::device_size(prj, &tag).map_or((80.0, 80.0), |d| (d.w, d.h));
            let coupling = i + 1 == devices.len();
            let at = if coupling { p(right + COUPLING_ASIDE - w / 2.0, y + COUPLING_BELOW) } else { p(x - w / 2.0, y) };
            panel::mount_on_wall(prj, tr, &tag, Face::Wall(3), at).unwrap_or_else(|e| panic!("{tag}: {e}"));
            (y, right) = (y + h + 60.0, x + w / 2.0);
        }
    }
    panel::add_pipe_run(prj, tr, Face::Wall(3), &[p(1000.0, 900.0), p(3100.0, 900.0)], None, None, Some("PR3")).expect("pipe run");
    panel::add_wall_profile(prj, tr, ItemKind::Duct, "GEN.DUCT-100X80", Face::Roof, p(100.0, 1150.0), p(6900.0, 1150.0)).expect("cable tray");
}
