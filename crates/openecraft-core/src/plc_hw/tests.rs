use super::*;
use crate::netlist::Netlist;
use crate::sample::demo_project;

#[test]
fn an_imported_cpu_without_network_data_does_not_gain_profinet_ports() {
    let mut prj=Project::default();
    prj.plc.descriptions.push(DeviceDescription {id:"SourceCPU".into(),kind:ModuleKind::Cpu,..Default::default()});
    let mut station=Station {name:"Source".into(),device:"SourceCPU".into(),..Default::default()};
    assert!(ports(&prj,&station).is_empty());
    prj.plc.networks.push(BusSystem {name:"Configured".into(),kind:BusKind::EtherCat,..Default::default()});
    station.network="Configured".into();
    assert_eq!(ports(&prj,&station).iter().map(|p|p.name.as_str()).collect::<Vec<_>>(),["IN","OUT"]);
    assert!(ports(&prj,&station).iter().all(|p|p.bus==BusKind::EtherCat));
}

fn addresses(prj: &Project) -> Vec<(String, String)> {
    crate::plc::points(prj, &Netlist::build(prj, true)).into_iter().map(|p| (p.card, p.address)).collect()
}

fn messages(prj: &Project) -> Vec<String> {
    problems(prj).into_iter().map(|i| i.message()).collect()
}

#[test]
fn schemes_format_and_parse_addresses() {
    assert_eq!(Scheme::Siemens.format(true, 2, 9, 0), "I3.1");
    assert_eq!(Scheme::Iec.format(false, 0, 3, 0), "%QX0.3");
    assert_eq!(Scheme::Rockwell.format(true, 4, 9, 3), "Local:3:I.Data.9");
    assert_eq!(parse_address("%IX3.1"), Some(Address { input: true, byte: 3, bit: 1, slot: None }));
    assert_eq!(parse_address("A1.0"), Some(Address { input: false, byte: 1, bit: 0, slot: None }));
    assert_eq!(parse_address("Local:3:O.Data.9"), Some(Address { input: false, byte: 1, bit: 1, slot: Some(3) }));
    assert_eq!(parse_address("I1.8"), None);
    assert_eq!(parse_address("M0.0"), None);
}

#[test]
fn automatic_configuration_of_the_demo() {
    let mut prj = demo_project();
    assert!(messages(&prj).is_empty(), "no checks without hardware");
    let res = auto_configure(&mut prj, false);
    assert_eq!((res.stations, res.networks, res.assigned), (1, 1, 2));
    let st = &prj.plc.stations[0];
    assert_eq!((st.name.as_str(), st.kind, st.device.as_str(), st.network.as_str()), ("PLC1", StationKind::Controller, "GEN.CPU-PN", "BUS_1"));
    assert_eq!(st.address, "192.168.0.1");
    let mods = modules(&prj, "PLC1");
    assert_eq!(mods.iter().map(|m| (m.card.as_str(), m.slot)).collect::<Vec<_>>(), [("-A1", 2), ("-A2", 3)]);
    // DI16 takes input bytes 0 and 1, DO16 output bytes 0 and 1.
    assert_eq!((mods[0].input_start, mods[0].input_bytes, mods[1].output_start, mods[1].output_bytes), (0, 2, 0, 2));
    assert!(messages(&prj).is_empty(), "{:?}", messages(&prj));
    assert!(crate::analysis::Analysis::checked(&prj).issues.iter().all(|i| i.severity != Severity::Error));
    // Nothing left to do.
    assert_eq!(auto_configure(&mut prj, false), AutoResult::default());
}

#[test]
fn remote_io_shares_the_controller_address_space() {
    let mut prj = demo_project();
    prj.plc.stations.push(Station { name: "CPU".into(), device: "GEN.CPU-PN".into(), ..Default::default() });
    prj.plc.stations.push(Station { name: "ET1".into(), kind: StationKind::IoDevice, device: "GEN.IM-PN".into(), ..Default::default() });
    prj.plc_cards.insert("-A1".into(), PlcCard { station: "CPU".into(), ..Default::default() });
    prj.plc_cards.insert("-A2".into(), PlcCard { station: "ET1".into(), output_byte: Some(7), ..Default::default() });
    let res = auto_configure(&mut prj, false);
    assert_eq!((res.stations, res.networks, res.assigned, res.connections), (0, 1, 0, 1));
    let et = prj.plc.station("ET1").unwrap();
    assert_eq!((et.controller.as_str(), et.device_number, et.address.as_str()), ("CPU", 1, "192.168.0.2"));
    assert_eq!(prj.plc_cards["-A2"].slot, "1", "remote I/O: modules from slot 1");
    let c = &prj.plc.connections[0];
    assert_eq!((c.a.to_string(), c.b.to_string()), ("CPU:X1 P2".to_string(), "ET1:X1 P1".to_string()));
    assert_eq!(card_domain(&prj, "-A2"), "CPU");
    let a = addresses(&prj);
    assert!(a.contains(&("-A2".into(), "Q0.0".into())), "{a:?}");

    // Overlapping ranges in one address space are found, also by the address check.
    prj.plc_cards.get_mut("-A2").unwrap().start_byte = 0;
    prj.plc_cards.get_mut("-A1").unwrap().output_byte = Some(1);
    prj.plc_cards.get_mut("-A1").unwrap().start_byte = 0;
    let mut prj2 = prj.clone();
    prj2.plc_cards.get_mut("-A1").unwrap().device = "GEN.PLC-DIO16".into();
    prj2.plc_cards.get_mut("-A1").unwrap().output_byte = Some(0);
    assert!(messages(&prj2).iter().any(|m| m == "Address ranges of -A1 and -A2 overlap"), "{:?}", messages(&prj2));
}

#[test]
fn schemes_change_the_addresses_of_the_points() {
    let mut prj = demo_project();
    auto_configure(&mut prj, false);
    prj.plc.stations[0].scheme = Scheme::Iec;
    assert_eq!(convert_addresses(&mut prj), 5);
    let a = addresses(&prj);
    assert!(a.contains(&("-A1".into(), "%IX0.2".into())) && a.contains(&("-A2".into(), "%QX0.1".into())), "{a:?}");
    prj.plc.stations[0].scheme = Scheme::Rockwell;
    convert_addresses(&mut prj);
    let a = addresses(&prj);
    assert!(a.contains(&("-A2".into(), "Local:3:O.Data.1".into())), "{a:?}");
    prj.plc.stations[0].scheme = Scheme::Siemens;
    convert_addresses(&mut prj);
    assert!(addresses(&prj).contains(&("-A1".into(), "I0.1".into())));
    // New points continue in the scheme of the station.
    prj.plc.stations[0].scheme = Scheme::Iec;
    convert_addresses(&mut prj);
    let mut ctx = crate::ops::PlaceCtx::default();
    ctx.last_plc.insert("plc_in".into(), "-A1".into());
    let def = prj.symbol("plc_in").unwrap();
    let s = crate::ops::new_symbol(&mut prj, 0, &ctx, &def, crate::geom::p(200.0, 200.0), 0, false);
    assert_eq!((s.tag.as_str(), s.label.as_str()), ("-A1", "%IX0.3"));
}

#[test]
fn checks_of_the_hardware() {
    let mut prj = demo_project();
    auto_configure(&mut prj, false);
    prj.plc_cards.get_mut("-A2").unwrap().slot = "2".into();
    prj.plc_cards.get_mut("-A1").unwrap().device = "GEN.PLC-DI8".into();
    prj.plc.stations.push(Station { name: "HMI".into(), kind: StationKind::Hmi, network: "BUS_1".into(), address: "192.168.0.1".into(), ..Default::default() });
    prj.plc.connections.push(BusConnection { network: "BUS_1".into(), a: PortRef::new("PLC1", "X1 P1"), b: PortRef::new("PLC1", "X9"), cable: String::new() });
    prj.plc.connections.push(BusConnection { network: "BUS_1".into(), a: PortRef::new("PLC1", "X1 P1"), b: PortRef::new("NONE", "P1"), cable: String::new() });
    let m = messages(&prj);
    for expected in [
        "PLC1: slot 2 of rack 0 is used by -A1 and -A2",
        "Address 192.168.0.1 is used by PLC1 and HMI",
        "Station HMI is not connected to bus BUS_1",
        "PLC1: the station has no port X9",
        "Bus connection to NONE:P1: the station does not exist",
        "Port PLC1:X1 P1 is connected more than once",
    ] {
        assert!(m.iter().any(|x| x == expected), "{expected}: {m:#?}");
    }
    // 3 inputs on an 8 channel module are fine; 9 are not.
    assert!(!m.iter().any(|x| x.contains("inputs used")));
    prj.plc_cards.insert("-A9".into(), PlcCard { station: "GONE".into(), ..Default::default() });
    assert!(messages(&prj).iter().any(|x| x == "PLC card -A9: station GONE does not exist"));
}

#[test]
fn renaming_stations_and_devices() {
    let mut prj = demo_project();
    auto_configure(&mut prj, false);
    prj.plc.stations[0].tag = "-A0".into();
    rename_station(&mut prj, "PLC1", "Main");
    assert_eq!(prj.plc_cards["-A1"].station, "Main");
    crate::ops::rename_device(&mut prj, "-A0", "-A10");
    assert_eq!(prj.plc.stations[0].tag, "-A10");
    connect(&mut prj, PortRef::new("Main", "X1 P1"), PortRef::new("Main", "X1 P2"), "-W1").unwrap();
    assert!(connect(&mut prj, PortRef::new("Main", "X1 P1"), PortRef::new("Other", "X1 P2"), "").is_err());
    remove_station(&mut prj, "Main");
    assert!(prj.plc.connections.is_empty() && prj.plc_cards["-A1"].station.is_empty());
}

/// Controller and two remote I/O stations on one bus system of `kind`.
fn line(kind: BusKind) -> Project {
    let mut prj = demo_project();
    prj.plc.networks.push(BusSystem { name: "NET".into(), kind, subnet: String::new() });
    prj.plc.stations.push(Station { name: "CPU".into(), tag: "-A0".into(), network: "NET".into(), ..Default::default() });
    for n in ["ET1", "ET2"] {
        prj.plc.stations.push(Station { name: n.into(), kind: StationKind::IoDevice, controller: "CPU".into(), network: "NET".into(), ..Default::default() });
    }
    prj
}

#[test]
fn bus_cables_are_created_and_checked() {
    let mut prj = line(BusKind::Profinet);
    let res = auto_configure(&mut prj, true);
    assert_eq!((res.connections, res.cables), (2, 2));
    let c = &prj.plc.connections[0];
    let cable = prj.cable(&c.cable).unwrap().clone();
    assert_eq!((cable.tag.as_str(), cable.bus, cable.cores.as_str()), ("-W2", Some(BusKind::Profinet), "TD+ TD- RD+ RD-"));
    assert_eq!(cable.description, "-A0:X1 P2 - ET1:X1 P1");
    assert!(messages(&prj).is_empty(), "{:?}", messages(&prj));
    assert_eq!(create_cables(&mut prj), 0, "all connections have a cable");

    // Cable reports list the ports as ends and every core.
    let an = crate::analysis::Analysis::checked(&prj);
    let overview = &an.report(&prj, ReportKind::CableOverview).unwrap();
    let row = overview.rows.iter().find(|r| r[0] == "-W2").unwrap();
    assert_eq!((row[2].as_str(), row[5].as_str(), row[6].as_str()), ("4/4", "-A0:X1 P2", "ET1:X1 P1"));
    let diagram = &an.report(&prj, ReportKind::CableDiagram).unwrap();
    assert_eq!(diagram.rows.iter().filter(|r| r[0] == "-W2").count(), 4);

    // Wrong cable kind, one cable for two links, missing cable.
    prj.cables.iter_mut().find(|x| x.tag == "-W2").unwrap().bus = Some(BusKind::ProfibusDp);
    prj.plc.connections[1].cable = "-W2".into();
    let m = messages(&prj);
    assert!(m.iter().any(|x| x == "Cable -W2 is a PROFIBUS DP cable on a PROFINET connection"), "{m:?}");
    assert!(m.iter().any(|x| x == "Cable -W2 is used for 2 bus connections"), "{m:?}");
    prj.plc.connections[1].cable = "-W99".into();
    assert!(messages(&prj).iter().any(|x| x == "Bus connection ET1:X1 P2 - ET2:X1 P1: cable -W99 does not exist"));

    // Renaming and deleting a cable follow on the connections.
    crate::ops::rename_cable(&mut prj, "-W2", "-W20");
    assert_eq!(prj.plc.connections[0].cable, "-W20");
    let i = prj.cables.iter().position(|x| x.tag == "-W2").unwrap();
    prj.cables[i].tag = "-W20".into();
    crate::ops::remove_cable(&mut prj, i);
    assert!(prj.plc.connections[0].cable.is_empty());
}

#[test]
fn line_buses_take_two_cables_per_port() {
    let mut prj = line(BusKind::ProfibusDp);
    let res = auto_configure(&mut prj, false);
    assert_eq!(res.connections, 2);
    // The middle station has its incoming and outgoing cable on X1.
    let on_et1: Vec<String> = prj.plc.connections.iter().flat_map(|c| [&c.a, &c.b]).filter(|p| p.station == "ET1").map(|p| p.port.clone()).collect();
    assert_eq!(on_et1, ["X1", "X1"]);
    assert!(messages(&prj).is_empty(), "{:?}", messages(&prj));
    // A third cable on a full port replaces its oldest one.
    connect(&mut prj, PortRef::new("ET1", "X1"), PortRef::new("ET2", "X1"), "").unwrap();
    connect(&mut prj, PortRef::new("CPU", "X1"), PortRef::new("ET1", "X1"), "").unwrap();
    assert!(messages(&prj).iter().all(|m| !m.contains("more than")), "{:?}", messages(&prj));
    prj.plc.connections.push(BusConnection { network: "NET".into(), a: PortRef::new("ET1", "X1"), b: PortRef::new("CPU", "X1"), cable: String::new() });
    prj.plc.connections.push(BusConnection { network: "NET".into(), a: PortRef::new("ET1", "X1"), b: PortRef::new("ET2", "X1"), cable: String::new() });
    assert!(messages(&prj).iter().any(|m| m == "Port ET1:X1 has more than 2 cables"), "{:?}", messages(&prj));

    // Ethernet ports take one cable.
    let mut prj = line(BusKind::Profinet);
    auto_configure(&mut prj, false);
    prj.plc.connections.push(BusConnection { network: "NET".into(), a: PortRef::new("ET1", "X1 P1"), b: PortRef::new("ET2", "X1 P2"), cable: String::new() });
    assert!(messages(&prj).iter().any(|m| m == "Port ET1:X1 P1 is connected more than once"));
}
