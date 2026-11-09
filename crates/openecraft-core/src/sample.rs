//! Demo project: direct-on-line motor starter with control circuit and PLC I/O,
//! and the example projects, from a first circuit to a whole building.

use crate::geom::{p, P};
use crate::model::*;
use crate::parts::PartsDb;
use openecraft_i18n::tr_const;

mod building;
mod cabinets;
mod connectors;
mod lamp;
mod metering;
mod house;
mod nshv;
mod plant;
mod press;
mod pump_room;
mod relay;
mod room;
mod site;
mod star_delta;
mod workflow;

/// An example project of the gallery.
pub struct Example {
    /// Stable name (command line `--example`).
    pub key: &'static str,
    /// 1 = first steps … higher levels build on the features of the lower ones.
    pub level: u8,
    pub title: &'static str,
    /// The features it shows.
    pub description: &'static str,
    pub build: fn() -> Project,
}

/// The example projects, simple ones first.
pub const EXAMPLES: [Example; 16] = [
    Example {
        key: "lamp_circuit",
        level: 1,
        title: tr_const!("1 · First circuit: push buttons and lamps"),
        description: tr_const!("One schematic page with +24V / 0V potentials, push buttons, a selector switch, lamps and a fuse; parts and the bill of materials."),
        build: lamp::project,
    },
    Example {
        key: "self_holding_relay",
        level: 2,
        title: tr_const!("2 · Relay with self-holding contact"),
        description: tr_const!("Start / stop circuit with a relay: coil and contacts form one device with cross references and contact image, terminal strip, wire numbers, terminal diagram and connection list."),
        build: relay::project,
    },
    Example {
        key: "star_delta_starter",
        level: 3,
        title: tr_const!("3 · Star-delta motor starter"),
        description: tr_const!("Power and control circuit on several pages: main switch, motor protection, three contactors with interlocks, on-delay timer, interruption points, potentials with wire colours, motor cable and cable reports."),
        build: star_delta::project,
    },
    Example {
        key: "conveyor",
        level: 4,
        title: tr_const!("4 · Conveyor with PLC and control cabinet"),
        description: tr_const!("The demo project: PLC inputs and outputs, panel layout of the enclosure with rails, ducts and door devices, door harness and motor cable."),
        build: demo_project,
    },
    Example {
        key: "pneumatic_press",
        level: 5,
        title: tr_const!("5 · Electro-pneumatic press"),
        description: tr_const!("Fluid power and electrics in one project: pneumatic circuit with service unit, valve terminal, 5/2 valves, flow control valves and cylinders; solenoid coils and position switches are linked to the valves and cylinders."),
        build: press::project,
    },
    Example {
        key: "pump_station",
        level: 6,
        title: tr_const!("6 · Pump station with structure identifiers"),
        description: tr_const!("Plant structure after IEC 81346 (functions and locations), page sections, single-line diagram, a pump direct on line and one with frequency converter, PLC cards with digital and analog channels, a sensor plug and the PLC assignment."),
        build: plant::project,
    },
    Example {
        key: "engineering_workflow",
        level: 7,
        title: tr_const!("7 · Revisions, review and change requests"),
        description: tr_const!("The conveyor after its first release: a released and locked revision, changes marked since, review comments with redlining, a change request, revision B in the approval workflow and a baseline to compare with."),
        build: workflow::project,
    },
    Example {
        key: "cabinet_lineup",
        level: 8,
        title: tr_const!("8 · Control cabinets in a line-up"),
        description: tr_const!("The pump station built into two bayed floor-standing enclosures: busbar system, rails and ducts, door devices and handles, filter fan, light, PE bar, cable glands, a cable passage, climate calculation, duct fill and 3D routing."),
        build: cabinets::project,
    },
    Example {
        key: "electrical_room",
        level: 9,
        title: tr_const!("9 · Electrical room"),
        description: tr_const!("The cabinet line-up in its room: L-shaped floor plan, inner wall, door and window, the line-up at its place, a mounting rail with a socket outlet on a wall, a wall-mounted emergency light and a cable tray under the ceiling."),
        build: room::project,
    },
    Example {
        key: "pump_room",
        level: 10,
        title: tr_const!("10 · Pump room"),
        description: tr_const!("The room of the line-up with its pumps: pump motors fed through cable glands in the roofs of the cabinets, a pump and a vessel standing on the floor, ball valves and a gauge on the wall, and the DN25 pipes of the water circuit routed in 3D along a pipe run on the wall."),
        build: pump_room::project,
    },
    Example {
        key: "building_services",
        level: 11,
        title: tr_const!("11 · Building services with water supply and PV storage"),
        description: tr_const!("A whole building in two rooms: the supply cable and the water main come in through building entries to the house connection box and the remote controlled valve; main distribution with busbars, Hager NH fuse switch disconnectors of three sizes and energy meters; cables and the water pipe pass a wall penetration into the technical room with PLC and pump cabinets, tanks and booster pumps, a compressor with air receiver and dryers and oilers at the outlets, a heat pump with buffer tank and hot water circulation, temperature, pressure and level sensors, a PV system with generator junction box, Victron MultiPlus-II and Inverter RS on the wall, Cerbo GX and Pylontech batteries in a 19-inch rack."),
        build: building::project,
    },
    Example {
        key: "connector_handling",
        level: 12,
        title: tr_const!("12 · Connectors, pin-outs and ribbon cables"),
        description: tr_const!("Plug connectors with their parts: a sensor on M12 and a valve on M8, a fan and its thermostat on GX16 and GX12 aviation connectors, a buzzer on a D-sub 9 and an operator panel on a ribbon cable between two IDC sockets; the plugs window shows the pin-out of each plug on its mating faces with the wire and core colours."),
        build: connectors::project,
    },
    Example {
        key: "pv_transformer_metering",
        level: 13,
        title: tr_const!("13 · Transformer-rated metering with PV and grid protection"),
        description: tr_const!("The distribution cabinet of an apartment building with an 88 kWp PV system, built from Hager univers parts: busbars with surge arrester and connection point zone, consumption and generation metering with NH1 fuse switch disconnectors on component modules, current transformers, voltage paths and meter cabinets with test terminal blocks, heat pump and inverter feeders, grid and system protection with voltage and frequency relay, buffered 24 V supply, fire brigade switch and a coupling switch with motor operator, undervoltage release and auxiliary switch built in; the cabinet comes with its plinth and lifting eyes."),
        build: metering::project,
    },
    Example {
        key: "industrial_main_distribution",
        level: 14,
        title: tr_const!("14 · Main low-voltage distribution of an industrial plant"),
        description: tr_const!("Two bayed Hager univers cabinets: two aluminium supplies on busbar terminals for Al, transformer metering isolated by NH strips with solid links, a power analyser for the zero export of the battery storage, feeders on NH fuse switch disconnector strips of a 185 mm busbar system with current transformers clipped in behind them and energy meters on the Tree bus (DC charging station, sub-distributions, air conditioning), heat pumps behind residual current devices of type B, PV inverters and battery storage behind two coupling switches with motor operators, undervoltage releases and auxiliary switches, cable entry flanges for the large cables."),
        build: nshv::project,
    },
    Example {
        key: "plant_buildings",
        level: 15,
        title: tr_const!("15 · Plant with buildings, levels and a shaft"),
        description: tr_const!("The pump room in a plant of two buildings: the main building with the electrical room on its ground level and the control room above, joined by a shaft; the pump house with pump motor 2, linked by a cable trench between the building entries through which its motor cable is routed; façade lights on the outside walls and a weather station on the roof."),
        build: site::project,
    },
    Example {
        key: "office_building",
        level: 16,
        title: tr_const!("16 · Office building with roof, stair and installation"),
        description: tr_const!("The plant of level 15 with an office building: the office on the ground level and the attic under a gable roof whose slopes cut its ceiling down to the knee wall, a stair turned at a quarter landing, roof windows in the front slope; Gira socket outlets, a network outlet and a switch, an MDT KNX push button and room temperature controller, a TV outlet at their mounting heights, a COSMO heating circuit manifold and a single-room control box."),
        build: house::project,
    },
];

/// The example project of a name of [`EXAMPLES`].
pub fn example(key: &str) -> Option<Project> {
    EXAMPLES.iter().find(|e| e.key == key).map(|e| (e.build)())
}

/// Device data with the parts of each tag; the parts are copied into the project.
fn assign_parts(prj: &mut Project, db: &PartsDb, devices: &[(&str, &[&str])]) {
    for (tag, parts) in devices {
        let refs = parts.iter().map(|n| PartRef { number: n.to_string(), count: 1, total:false }).collect();
        prj.devices.insert(tag.to_string(), DeviceData { parts: refs, ..Default::default() });
        for n in *parts {
            crate::parts::ensure_in_project(prj, db, n);
        }
    }
}

/// Renumbers the wires of a finished example.
fn number_wires(prj: &mut Project) {
    let open = crate::netlist::Netlist::build(prj, false);
    let bridged = crate::netlist::Netlist::build(prj, true);
    crate::wirenum::renumber(prj, &open, &bridged, false);
}

struct Pg<'a> {
    prj: &'a mut Project,
    page: usize,
}

impl Pg<'_> {
    #[allow(clippy::too_many_arguments)]
    fn sym(&mut self, key: &str, x: f32, y: f32, rot: u8, tag: &str, label: &str, ftext: &str) -> &mut Self {
        let def = self.prj.symbol(key).expect("symbol");
        let id = self.prj.new_id();
        // Tags take the missing structure aspects from the page or the structure box
        // they are in (`-K1` → `=A1+CAB1-K1`).
        let tag = if tag.is_empty() { String::new() } else { crate::structure::complete_at(&self.prj.pages[self.page], p(x, y), tag) };
        let s = SymbolInst {
            id,
            sym: key.into(),
            pos: p(x, y),
            rot,
            mirror: false,
            tag,
            label: if label.is_empty() { def.default_label.into() } else { label.into() },
            function_text: ftext.into(),
            pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
            ..Default::default()
        };
        self.prj.pages[self.page].symbols.push(s);
        self
    }

    fn pins(&mut self, pins: &[&str]) -> &mut Self {
        if let Some(s) = self.prj.pages[self.page].symbols.last_mut() {
            s.pins = pins.iter().map(|x| x.to_string()).collect();
        }
        self
    }

    fn mirror(&mut self) -> &mut Self {
        if let Some(s) = self.prj.pages[self.page].symbols.last_mut() {
            s.mirror = true;
        }
        self
    }

    fn w(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut Self {
        let id = self.prj.new_id();
        self.prj.pages[self.page].wires.push(Wire { id, a: p(x1, y1), b: p(x2, y2), ..Default::default() });
        self
    }

    /// Wire through several points.
    fn path(&mut self, pts: &[(f32, f32)]) -> &mut Self {
        for s in pts.windows(2) {
            self.w(s[0].0, s[0].1, s[1].0, s[1].1);
        }
        self
    }

    fn text(&mut self, x: f32, y: f32, t: &str, size: f32) -> &mut Self {
        let id = self.prj.new_id();
        self.prj.pages[self.page].texts.push(TextItem { id, pos: P { x, y }, text: t.into(), size, ..Default::default() });
        self
    }
}

fn add_page(prj: &mut Project, name: &str, desc: &str, kind: PageKind) -> usize {
    let id = prj.new_id();
    prj.pages.push(Page::new(id, name, desc, kind));
    prj.pages.len() - 1
}

pub fn demo_project() -> Project {
    let mut prj = Project { pages: Vec::new(), ..Default::default() };
    prj.info = ProjectInfo {
        name: "Demo conveyor".into(),
        description: "Direct-on-line motor starter\nwith PLC controlled valve".into(),
        company: "OpenECraft".into(),
        creator: "OpenECraft".into(),
        date: "2026-10-19".into(),
        revision: "A".into(),
        ..Default::default()
    };

    prj.apply_preset(crate::potentials::Preset::Iec60204);
    add_page(&mut prj, "1", "Cover sheet", PageKind::Cover);

    // ------------------------------------------------------------------ power
    let page = add_page(&mut prj, "2", "Power supply conveyor motor", PageKind::Schematic);
    let mut g = Pg { prj: &mut prj, page };
    let rails = [(24.0, "pot_l1"), (32.0, "pot_l2"), (40.0, "pot_l3"), (48.0, "pot_n"), (56.0, "pot_pe")];
    for (y, key) in rails {
        g.sym(key, 20.0, y, 3, "", "", "").w(24.0, y, 400.0, y);
    }
    // Motor feeder
    g.sym("mps3", 60.0, 80.0, 0, "-Q1", "", "Motor protection");
    g.w(60.0, 24.0, 60.0, 76.0).w(68.0, 32.0, 68.0, 76.0).w(76.0, 40.0, 76.0, 76.0);
    g.sym("main3", 60.0, 112.0, 0, "-K1", "", "");
    g.w(60.0, 84.0, 60.0, 108.0).w(68.0, 84.0, 68.0, 108.0).w(76.0, 84.0, 76.0, 108.0);
    for (i, x) in [60.0, 68.0, 76.0].into_iter().enumerate() {
        g.sym("term", x, 140.0, 0, "-X1", &(i + 1).to_string(), "");
        g.w(x, 116.0, x, 136.0).w(x, 144.0, x, 168.0);
    }
    g.sym("term_pe", 84.0, 140.0, 0, "-X1", "PE", "");
    // The motor cable gets room for its tag, cores and the wire numbers.
    g.w(84.0, 56.0, 84.0, 136.0).w(84.0, 144.0, 84.0, 168.0);
    g.sym("motor3", 60.0, 172.0, 0, "-M1", "", "Conveyor\n0.75 kW");
    // The motor feeder behind -Q1 is a power circuit, not a control circuit (project default).
    for w in g.prj.pages[page].wires.iter_mut().filter(|w| [60.0, 68.0, 76.0].contains(&w.a.x) && w.a.y >= 84.0) {
        w.color = "BK".into();
        w.cross_section = "1.5 mm²".into();
    }
    // 24 V power supply
    g.sym("cb1", 180.0, 64.0, 0, "-F1", "", "");
    g.w(180.0, 24.0, 180.0, 60.0).w(180.0, 68.0, 180.0, 76.0);
    g.sym("psu", 180.0, 80.0, 0, "-T1", "", "24 V DC");
    g.w(188.0, 48.0, 188.0, 76.0).w(196.0, 56.0, 196.0, 76.0);
    g.path(&[(180.0, 96.0), (180.0, 112.0)]).sym("intr", 180.0, 112.0, 0, "", "24V", "");
    g.path(&[(188.0, 96.0), (188.0, 104.0), (204.0, 104.0), (204.0, 112.0)]).sym("intr", 204.0, 112.0, 0, "", "0V", "");
    g.text(260.0, 90.0, "Supply 400 V / 50 Hz, TN-S", 3.0);

    // ------------------------------------------------------------------ control
    let page = add_page(&mut prj, "3", "Control circuit conveyor", PageKind::Schematic);
    let mut g = Pg { prj: &mut prj, page };
    g.sym("intr", 20.0, 24.0, 1, "", "24V", "").w(20.0, 24.0, 400.0, 24.0);
    g.sym("intr", 20.0, 236.0, 1, "", "0V", "").w(20.0, 236.0, 400.0, 236.0);
    g.sym("estop", 40.0, 40.0, 0, "-S1", "", "Emergency stop").w(40.0, 24.0, 40.0, 36.0);
    g.sym("pb_nc", 40.0, 56.0, 0, "-S2", "", "Stop").w(40.0, 44.0, 40.0, 52.0);
    g.sym("pb_no", 40.0, 76.0, 0, "-S3", "", "Start").w(40.0, 60.0, 40.0, 72.0);
    g.sym("no", 64.0, 76.0, 0, "-K1", "", "").pins(&["13", "14"]);
    g.path(&[(40.0, 64.0), (64.0, 64.0), (64.0, 72.0)]);
    g.path(&[(64.0, 80.0), (64.0, 88.0), (40.0, 88.0)]).w(40.0, 80.0, 40.0, 88.0);
    g.sym("ol_nc", 40.0, 104.0, 0, "-Q1", "", "").pins(&["21", "22"]);
    g.w(40.0, 88.0, 40.0, 100.0);
    g.sym("coil", 40.0, 128.0, 0, "-K1", "", "Conveyor motor");
    g.w(40.0, 108.0, 40.0, 124.0).w(40.0, 132.0, 40.0, 236.0);
    // Indicator
    g.sym("no", 120.0, 76.0, 0, "-K1", "", "").pins(&["53", "54"]);
    g.w(120.0, 24.0, 120.0, 72.0).w(120.0, 80.0, 120.0, 124.0);
    g.sym("lamp", 120.0, 128.0, 0, "-P1", "", "Motor running").w(120.0, 132.0, 120.0, 236.0);
    // Feedback to PLC
    g.sym("no", 200.0, 76.0, 0, "-K1", "", "").pins(&["83", "84"]);
    g.w(200.0, 24.0, 200.0, 72.0).w(200.0, 80.0, 200.0, 120.0).sym("intr", 200.0, 120.0, 0, "", "K1.FB", "");

    // ------------------------------------------------------------------ PLC
    let page = add_page(&mut prj, "4", "PLC inputs / outputs", PageKind::Schematic);
    let mut g = Pg { prj: &mut prj, page };
    g.sym("intr", 20.0, 24.0, 1, "", "24V", "").w(20.0, 24.0, 400.0, 24.0);
    g.sym("intr", 20.0, 236.0, 1, "", "0V", "").w(20.0, 236.0, 400.0, 236.0);
    g.sym("prox", 60.0, 60.0, 0, "-B1", "", "Part present").w(60.0, 24.0, 60.0, 56.0);
    g.w(60.0, 64.0, 60.0, 80.0).sym("plc_in", 60.0, 84.0, 0, "-A1", "I0.0", "Part present");
    g.sym("pb_no", 100.0, 60.0, 0, "-S4", "", "Reset").w(100.0, 24.0, 100.0, 56.0);
    g.w(100.0, 64.0, 100.0, 80.0).sym("plc_in", 100.0, 84.0, 0, "-A1", "I0.1", "Reset");
    g.sym("intr", 140.0, 72.0, 2, "", "K1.FB", "").w(140.0, 72.0, 140.0, 80.0);
    g.sym("plc_in", 140.0, 84.0, 0, "-A1", "I0.2", "Motor running");
    g.sym("plc_out", 240.0, 120.0, 0, "-A2", "Q0.0", "Valve").w(240.0, 124.0, 240.0, 156.0);
    g.sym("valve", 240.0, 160.0, 0, "-Y1", "", "Ejector valve").w(240.0, 164.0, 240.0, 236.0);
    g.sym("plc_out", 300.0, 120.0, 0, "-A2", "Q0.1", "Fault").w(300.0, 124.0, 300.0, 156.0);
    g.sym("lamp", 300.0, 160.0, 0, "-P2", "", "Fault").w(300.0, 164.0, 300.0, 236.0);
    g.text(220.0, 100.0, "Digital outputs -A2", 2.5);
    g.text(40.0, 112.0, "Digital inputs -A1", 2.5);

    add_page(&mut prj, "5", "Bill of materials", PageKind::Report(ReportKind::BillOfMaterials.into()));
    add_page(&mut prj, "6", "Terminal diagram -X1", PageKind::Report(ReportKind::TerminalDiagram.into()));
    add_page(&mut prj, "7", "Device list", PageKind::Report(ReportKind::DeviceList.into()));

    let db = PartsDb::default();
    assign_parts(
        &mut prj,
        &db,
        &[
            ("-Q1", &["GEN.MPS-2.5-4A", "GEN.MPS-AUX-1NO1NC"][..]),
            ("-K1", &["GEN.CT-9A-24DC", "GEN.AUX-2NO2NC"]),
            ("-M1", &["GEN.MOT-0.75KW"]),
            ("-X1", &["GEN.TB-2.5", "GEN.TB-PE-2.5"]),
            ("-F1", &["GEN.MCB-1P-C6"]),
            ("-T1", &["GEN.PS-24DC-5A"]),
            ("-S1", &["GEN.ESTOP-1NC"]),
            ("-S2", &["GEN.PB-RD-1NC"]),
            ("-S3", &["GEN.PB-GN-1NO"]),
            ("-S4", &["GEN.PB-GN-1NO"]),
            ("-P1", &["GEN.LMP-GN-24"]),
            ("-P2", &["GEN.LMP-RD-24"]),
            ("-B1", &["GEN.PROX-M12-PNP"]),
            ("-Y1", &["GEN.SV-24DC"]),
            ("-A1", &["GEN.PLC-DI16"]),
            ("-A2", &["GEN.PLC-DO16"]),
        ],
    );
    add_panel_layout(&mut prj, &db);
    add_motor_cable(&mut prj, &db);
    number_wires(&mut prj);
    add_door_harness(&mut prj);
    prj
}

/// Command and signal devices on the door of +CAB1 are wired with harness -WH1.
const DOOR_DEVICES: [(&str, f32, f32); 6] =
    [("-S1", 280.0, 110.0), ("-S2", 180.0, 230.0), ("-S3", 240.0, 230.0), ("-S4", 300.0, 230.0), ("-P1", 360.0, 230.0), ("-P2", 420.0, 230.0)];

fn add_door_harness(prj: &mut Project) {
    let id = prj.new_id();
    prj.harnesses.push(crate::harness::Harness { id, tag: "-WH1".into(), description: "Door harness +CAB1".into(), length: 1.5, ..Default::default() });
    // Every connection with a door device at one of its ends.
    let nl = crate::netlist::Netlist::build(prj, true);
    let open = crate::netlist::Netlist::build(prj, false);
    let door: Vec<&str> = DOOR_DEVICES.iter().map(|d| d.0).collect();
    let ends: Vec<(String, String)> = crate::netlist::connections(prj, &nl, &open)
        .into_iter()
        .filter(|c| [c.a, c.b].iter().any(|p| door.contains(&prj.pages[p.page].symbols[p.sym].tag.as_str())))
        .map(|c| (crate::netlist::pin_designation(prj, c.a), crate::netlist::pin_designation(prj, c.b)))
        .collect();
    for (a, b) in ends {
        crate::harness::assign(prj, &a, &b, "-WH1");
    }
}

/// Cable -W1 from terminal strip -X1 to the motor -M1 (page 2).
fn add_motor_cable(prj: &mut Project, db: &PartsDb) {
    let part = db.get("GEN.CBL-4G1.5").expect("cable part");
    let id = prj.new_id();
    prj.cables.push(Cable {
        id,
        tag: "-W1".into(),
        cable_type: part.type_number.clone(),
        cores: part.tech_value("Cores").unwrap_or_default().into(),
        cross_section: part.tech_value("Cross-section").unwrap_or_default().into(),
        length: 15.0,
        part: part.number.clone(),
        description: "Conveyor motor".into(),
        ..Default::default()
    });
    crate::parts::ensure_in_project(prj, db, "GEN.CBL-4G1.5");
    let page = prj.pages.iter().position(|p| p.name == "2").expect("page 2");
    let wires = prj.pages[page].wires.iter().filter(|w| w.a.y == 144.0 && w.b.y == 168.0).map(|w| w.id).collect();
    crate::ops::assign_cores(prj, page, &wires, "-W1");
}

/// Enclosure +CAB1: cable duct frame, three mounting rails and the cabinet devices.
fn add_panel_layout(prj: &mut Project, db: &PartsDb) {
    use crate::panel::{self, ItemKind};
    for part in [panel::ENCLOSURE_PART, panel::RAIL_PART, panel::DUCT_PART] {
        crate::parts::ensure_in_project(prj, db, part);
    }
    let enc = panel::new_enclosure(prj, panel::ENCLOSURE_PART, "+CAB1");
    let enc_id = enc.id;
    prj.layout.enclosures.push(enc);
    // Plate 550 x 750 mm: duct frame plus two horizontal ducts between the rails.
    let ducts = [
        ((0.0, 20.0), (550.0, 20.0)),
        ((0.0, 730.0), (550.0, 730.0)),
        ((20.0, 40.0), (20.0, 710.0)),
        ((530.0, 40.0), (530.0, 710.0)),
        ((40.0, 270.0), (510.0, 270.0)),
        ((40.0, 500.0), (510.0, 500.0)),
    ];
    for (a, b) in ducts {
        let item = panel::new_profile(prj, ItemKind::Duct, panel::DUCT_PART, p(a.0, a.1), p(b.0, b.1));
        prj.layout.enclosures[0].items.push(item);
    }
    for y in [145.0, 385.0, 615.0] {
        let item = panel::new_profile(prj, ItemKind::Rail, panel::RAIL_PART, p(60.0, y), p(490.0, y));
        prj.layout.enclosures[0].items.push(item);
    }
    for (tag, x, rail_y) in [
        ("-Q1", 70.0, 145.0),
        ("-T1", 150.0, 145.0),
        ("-K1", 70.0, 385.0),
        ("-F1", 135.0, 385.0),
        ("-A1", 180.0, 385.0),
        ("-A2", 215.0, 385.0),
        ("-X1", 70.0, 615.0),
    ] {
        let h = panel::device_size(prj, tag).map(|d| d.h).unwrap_or(40.0);
        panel::place_device(prj, 0, tag, p(x, rail_y - h / 2.0));
    }
    for (tag, x, y) in DOOR_DEVICES {
        panel::place_device_on(prj, 0, tag, p(x, y), panel::Face::Door);
    }
    // Door ducts: one along the hinge side, one above and one below the row of
    // command devices; the door wires run through them to the hinge.
    crate::parts::ensure_in_project(prj, db, panel::DOOR_DUCT_PART);
    for (a, b) in [((50.0, 60.0), (50.0, 740.0)), ((62.5, 190.0), (480.0, 190.0)), ((62.5, 295.0), (480.0, 295.0))] {
        let item = panel::new_profile(prj, ItemKind::Duct, panel::DOOR_DUCT_PART, p(a.0, a.1), p(b.0, b.1));
        prj.layout.enclosures[0].items.push(panel::PanelItem { face: panel::Face::Door, ..item });
    }
    let id = prj.new_id();
    let at = prj.pages.len();
    prj.pages.push(Page::new(id, &(at + 1).to_string(), "Mounting layout +CAB1", PageKind::Mounting(enc_id)));
}

/// `motor` drives `machine` on the floor of enclosure `ei`, raised by `riser` mm.
fn drive(prj: &mut Project, ei: usize, machine: &str, motor: &str, riser: f32) {
    prj.devices.entry(machine.into()).or_default().values.insert(crate::panel::DRIVEN_BY.into(), motor.into());
    if let Some(it) = prj.layout.enclosures[ei].items.iter_mut().find(|i| i.tag == motor) {
        it.z = riser;
    }
}

#[cfg(test)]
mod tests;
