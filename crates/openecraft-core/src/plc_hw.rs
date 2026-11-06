//! PLC hardware configuration: stations (controllers, remote I/O, drives, HMIs) with
//! racks and modules (the PLC cards of the schematic), bus systems with the port
//! connections between the stations, the address schemes of the PLC types and the
//! automatic configuration from the schematic.

pub use crate::devdesc::{BusKind, DescPort, DeviceDescription, ModuleKind};
use crate::model::*;
use crate::issue::{Issue, Severity};
use openecraft_i18n::{tr, tr_fmt};
use std::collections::{BTreeMap, HashMap};
pub use crate::model::plc_hw::{Address, BusConnection, BusSystem, Hardware, PortRef, Scheme, Station, StationKind};

impl Scheme {
    pub const ALL: [Scheme; 3] = [Scheme::Siemens, Scheme::Iec, Scheme::Rockwell];

    pub fn title(self) -> &'static str {
        match self {
            Scheme::Siemens => tr("Siemens (I0.0 / Q0.0)"),
            Scheme::Iec => tr("IEC 61131-3 (%IX0.0 / %QX0.0)"),
            Scheme::Rockwell => tr("Rockwell (Local:1:I.Data.0)"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Scheme::Siemens => "siemens",
            Scheme::Iec => "iec",
            Scheme::Rockwell => "rockwell",
        }
    }

    pub fn parse(s: &str) -> Option<Scheme> {
        Scheme::ALL.into_iter().find(|k| k.key().eq_ignore_ascii_case(s.trim()))
    }


}

/// Parses `I1.7`, `E1.7`, `%IX1.7`, `Q0.0`, `A0.0`, `%QX0.0`, `Local:3:I.Data.5`.
pub fn parse_address(addr: &str) -> Option<Address> {
    let a = addr.trim().trim_start_matches('%').to_ascii_uppercase();
    if let Some(rest) = a.strip_prefix("LOCAL:") {
        let (slot, rest) = rest.split_once(':')?;
        let input = match rest.chars().next()? {
            'I' => true,
            'O' => false,
            _ => return None,
        };
        let n: u32 = rest.rsplit('.').next()?.parse().ok()?;
        return Some(Address { input, byte: n / 8, bit: n % 8, slot: Some(slot.parse().ok()?) });
    }
    let prefix: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let input = match prefix.as_str() {
        "I" | "E" | "IX" => true,
        "Q" | "A" | "QX" => false,
        _ => return None,
    };
    let (byte, bit) = a[prefix.len()..].split_once('.')?;
    let bit: u32 = bit.parse().ok()?;
    (bit < 8).then_some(Address { input, byte: byte.parse().ok()?, bit, slot: None })
}

impl StationKind {
    pub const ALL: [StationKind; 5] = [StationKind::Controller, StationKind::IoDevice, StationKind::Drive, StationKind::Hmi, StationKind::Other];

    pub fn title(self) -> &'static str {
        match self {
            StationKind::Controller => tr("Controller"),
            StationKind::IoDevice => tr("Remote I/O"),
            StationKind::Drive => tr("Drive"),
            StationKind::Hmi => tr("HMI"),
            StationKind::Other => tr("Other bus device"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            StationKind::Controller => "controller",
            StationKind::IoDevice => "io_device",
            StationKind::Drive => "drive",
            StationKind::Hmi => "hmi",
            StationKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<StationKind> {
        StationKind::ALL.into_iter().find(|k| k.key().eq_ignore_ascii_case(s.trim()))
    }
}

impl PortRef {
    pub fn new(station: &str, port: &str) -> PortRef {
        PortRef { station: station.into(), port: port.into() }
    }
}

impl std::fmt::Display for PortRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.station, self.port)
    }
}

impl Hardware {
    pub fn is_empty(&self) -> bool {
        self == &Hardware::default()
    }

    pub fn station(&self, name: &str) -> Option<&Station> {
        self.stations.iter().find(|s| s.name == name)
    }

    pub fn network(&self, name: &str) -> Option<&BusSystem> {
        self.networks.iter().find(|n| n.name == name)
    }

    /// Adds or replaces descriptions by id. Returns the number of new ones.
    pub fn add_descriptions(&mut self, list: Vec<DeviceDescription>) -> usize {
        let mut n = 0;
        for d in list {
            match self.descriptions.iter_mut().find(|x| x.id == d.id) {
                Some(x) => *x = d,
                None => {
                    self.descriptions.push(d);
                    n += 1;
                }
            }
        }
        n
    }
}

// ------------------------------------------------------------------ descriptions

/// All descriptions to choose from: imported ones, then the built-in list.
pub fn descriptions(prj: &Project) -> Vec<DeviceDescription> {
    let mut v = prj.plc.descriptions.clone();
    for p in &prj.parts {
        if let Some(d) = crate::devdesc::from_part(p) {
            if !v.iter().any(|x| x.id == d.id) {
                v.push(d);
            }
        }
    }
    for d in crate::devdesc::builtin() {
        if !v.iter().any(|x| x.id == d.id) {
            v.push(d);
        }
    }
    v
}

/// Description by id: imported, project parts, the built-in list, the built-in catalog.
pub fn find_description(prj: &Project, id: &str) -> Option<DeviceDescription> {
    if id.is_empty() {
        return None;
    }
    prj.plc
        .descriptions
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .or_else(|| prj.parts.iter().find(|p| p.number == id).and_then(crate::devdesc::from_part))
        .or_else(|| crate::devdesc::builtin().into_iter().find(|d| d.id == id))
        .or_else(|| crate::parts::catalog().iter().find(|p| p.number == id).and_then(crate::devdesc::from_part))
}

/// Description of a device: the one chosen for the card / station, otherwise from the
/// parts assigned to the device.
pub fn description_of(prj: &Project, tag: &str) -> Option<DeviceDescription> {
    let chosen = prj.plc_cards.get(tag).map(|c| c.device.as_str()).filter(|d| !d.is_empty());
    let chosen = chosen.or_else(|| prj.plc.stations.iter().find(|s| s.tag == tag && !tag.is_empty()).map(|s| s.device.as_str()));
    if let Some(d) = chosen.and_then(|id| find_description(prj, id)) {
        return Some(d);
    }
    let dev = prj.devices.get(tag)?;
    dev.parts.iter().find_map(|r| {
        let part = crate::parts::lookup(prj, None, &r.number).or_else(|| crate::parts::catalog().iter().find(|p| p.number == r.number))?;
        crate::devdesc::from_part(part)
    })
}

/// Description of a station's head.
pub fn station_description(prj: &Project, st: &Station) -> Option<DeviceDescription> {
    find_description(prj, &st.device).or_else(|| (!st.tag.is_empty()).then(|| description_of(prj, &st.tag)).flatten())
}

/// Ports of a station: from its description, otherwise the defaults of its bus.
pub fn ports(prj: &Project, st: &Station) -> Vec<DescPort> {
    let desc = station_description(prj, st).map(|d| d.ports).unwrap_or_default();
    if !desc.is_empty() {
        return desc;
    }
    let bus = prj.plc.network(&st.network).map(|n| n.kind).unwrap_or_default();
    bus.default_ports().iter().map(|p| DescPort { name: p.to_string(), bus }).collect()
}

// ------------------------------------------------------------------ modules

/// A module of a station with its use in the schematic.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub card: String,
    pub rack: u32,
    pub slot: u32,
    pub description: Option<DeviceDescription>,
    pub inputs_used: u32,
    pub outputs_used: u32,
    pub input_start: u32,
    pub output_start: u32,
    pub input_bytes: u32,
    pub output_bytes: u32,
}

fn parse_num(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

/// Number of input / output points placed for a card.
pub fn used(prj: &Project, card: &str) -> (u32, u32) {
    let [inputs, outputs] = point_counts(prj, card);
    (inputs.0, outputs.0)
}

/// (Placed points, digitally addressed points) for each direction. Analog
/// channel names do not allocate bits in the estimated process image.
fn point_counts(prj: &Project, card: &str) -> [(u32, u32); 2] {
    let mut counts = [(0, 0); 2];
    for s in prj.pages.iter().flat_map(|p| &p.symbols).filter(|s| s.tag == card) {
        if let Some(def) = prj.symbol(&s.sym) {
            let digital = def.digital_input();
            if let Some(input) = digital.or_else(|| def.plc_channel.as_ref().map(|c| c.input)) {
                let count = &mut counts[usize::from(!input)];
                count.0 += 1;
                count.1 += u32::from(digital.is_some());
            }
        }
    }
    counts
}

fn module(prj: &Project, card: &str, data: &PlcCard) -> Module {
    let description = description_of(prj, card);
    let [(inputs_used, digital_inputs), (outputs_used, digital_outputs)] = point_counts(prj, card);
    let bytes = |desc: Option<u32>, used: u32| desc.filter(|b| *b > 0).unwrap_or(used.div_ceil(8));
    Module {
        card: card.to_string(),
        rack: parse_num(&data.rack).unwrap_or(0),
        slot: parse_num(&data.slot).unwrap_or(0),
        input_bytes: bytes(description.as_ref().map(|d| d.input_bytes), digital_inputs),
        output_bytes: bytes(description.as_ref().map(|d| d.output_bytes), digital_outputs),
        description,
        inputs_used,
        outputs_used,
        input_start: data.start_byte,
        output_start: data.output_byte.unwrap_or(data.start_byte),
    }
}

/// Modules of a station, by rack and slot.
pub fn modules(prj: &Project, station: &str) -> Vec<Module> {
    let mut v: Vec<Module> = prj.plc_cards.iter().filter(|(_, c)| c.station == station).map(|(t, c)| module(prj, t, c)).collect();
    v.sort_by(|a, b| (a.rack, a.slot).cmp(&(b.rack, b.slot)).then_with(|| natural_cmp(&a.card, &b.card)));
    v
}

/// Slot of the head: 1 for controllers (slot 0: power supply), 0 for Rockwell
/// controllers and remote I/O.
pub fn head_slot(st: &Station) -> u32 {
    u32::from(st.kind == StationKind::Controller && st.scheme != Scheme::Rockwell)
}

/// Station of a card (if assigned to an existing one).
pub fn station_of<'a>(prj: &'a Project, card: &str) -> Option<&'a Station> {
    prj.plc_cards.get(card).and_then(|c| prj.plc.station(&c.station))
}

/// Name of the address space a station's addresses are in: its controller's.
pub fn domain(prj: &Project, st: &Station) -> String {
    if st.kind != StationKind::Controller && prj.plc.station(&st.controller).is_some() {
        st.controller.clone()
    } else {
        st.name.clone()
    }
}

/// Address space of a card ("" for cards without station).
pub fn card_domain(prj: &Project, card: &str) -> String {
    station_of(prj, card).map(|s| domain(prj, s)).unwrap_or_default()
}

/// Scheme of a card (that of its station).
pub fn scheme_of(prj: &Project, card: &str) -> Scheme {
    station_of(prj, card).map(|s| s.scheme).unwrap_or_default()
}

/// Address of channel `n` of a card in the scheme of its station.
pub use crate::model::plc_hw::card_address;

/// Converts the addresses of all points to the schemes of their stations (same
/// channel). Returns the number of changed addresses.
pub fn convert_addresses(prj: &mut Project) -> usize {
    let mut changes = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        for (si, s) in pg.symbols.iter().enumerate().filter(|(_, s)| prj.symbol(&s.sym).and_then(|d| d.digital_input()).is_some()) {
            let Some(a) = parse_address(&s.label) else { continue };
            let data = prj.plc_cards.get(&s.tag).cloned().unwrap_or_default();
            let start = if a.input { data.start_byte } else { data.output_byte.unwrap_or(data.start_byte) };
            // Channel of the card: Rockwell addresses count from the module already.
            let n = if a.slot.is_some() { a.byte * 8 + a.bit } else { a.byte.saturating_sub(start) * 8 + a.bit };
            let new = card_address(prj, &s.tag, a.input, n);
            if new != s.label {
                changes.push((pi, si, new));
            }
        }
    }
    let n = changes.len();
    for (pi, si, a) in changes {
        prj.pages[pi].symbols[si].label = a;
    }
    n
}

/// Renames a device tag in the stations.
pub fn rename_device(prj: &mut Project, old: &str, new: &str) {
    for st in prj.plc.stations.iter_mut().filter(|s| s.tag == old) {
        st.tag = new.to_string();
    }
}

/// Renames a station everywhere (cards, controllers, connections).
pub fn rename_station(prj: &mut Project, old: &str, new: &str) {
    if old == new || old.is_empty() {
        return;
    }
    for st in &mut prj.plc.stations {
        if st.name == old {
            st.name = new.to_string();
        }
        if st.controller == old {
            st.controller = new.to_string();
        }
    }
    for c in prj.plc_cards.values_mut().filter(|c| c.station == old) {
        c.station = new.to_string();
    }
    for p in prj.plc.connections.iter_mut().flat_map(|c| [&mut c.a, &mut c.b]) {
        if p.station == old {
            p.station = new.to_string();
        }
    }
}

/// Removes a station; its cards become unassigned, its connections are removed.
pub fn remove_station(prj: &mut Project, name: &str) {
    prj.plc.stations.retain(|s| s.name != name);
    for c in prj.plc_cards.values_mut().filter(|c| c.station == name) {
        c.station.clear();
    }
    prj.plc.connections.retain(|c| c.a.station != name && c.b.station != name);
}

/// Removes a bus system with its connections.
pub fn remove_network(prj: &mut Project, name: &str) {
    prj.plc.networks.retain(|n| n.name != name);
    prj.plc.connections.retain(|c| c.network != name);
    for st in prj.plc.stations.iter_mut().filter(|s| s.network == name) {
        st.network.clear();
    }
}

/// Connects two ports; replaces connections already on them.
pub fn connect(prj: &mut Project, a: PortRef, b: PortRef, cable: &str) -> Result<(), String> {
    for p in [&a, &b] {
        if prj.plc.station(&p.station).is_none() {
            return Err(tr_fmt!("No station {}", p.station));
        }
    }
    if a == b {
        return Err(tr("A port cannot be connected to itself").into());
    }
    let net = |name: &str| prj.plc.station(name).map(|s| s.network.clone()).unwrap_or_default();
    let network = if net(&a.station).is_empty() { net(&b.station) } else { net(&a.station) };
    prj.plc.connections.retain(|c| !((c.a == a && c.b == b) || (c.a == b && c.b == a)));
    // A full port gives up its oldest cable.
    let limit = prj.plc.network(&network).map(|n| n.kind.cables_per_port()).unwrap_or(1);
    for p in [&a, &b] {
        while prj.plc.connections.iter().filter(|c| c.a == *p || c.b == *p).count() >= limit {
            let i = prj.plc.connections.iter().position(|c| c.a == *p || c.b == *p).unwrap();
            prj.plc.connections.remove(i);
        }
    }
    prj.plc.connections.push(BusConnection { network, a, b, cable: cable.into() });
    Ok(())
}

/// Port with the device tag of its station if there is one (`-A0:X1 P1`).
pub fn port_label(prj: &Project, p: &PortRef) -> String {
    match prj.plc.station(&p.station).filter(|s| !s.tag.is_empty()) {
        Some(s) => format!("{}:{}", s.tag, p.port),
        None => p.to_string(),
    }
}

/// Bus connections using a cable.
pub fn cable_links<'a>(prj: &'a Project, cable: &str) -> Vec<&'a BusConnection> {
    prj.plc.connections.iter().filter(|c| !cable.is_empty() && c.cable == cable).collect()
}

/// Bus kind of a connection: its bus system, otherwise the bus of the first port.
pub fn connection_bus(prj: &Project, c: &BusConnection) -> Option<BusKind> {
    prj.plc.network(&c.network).map(|n| n.kind).or_else(|| {
        let st = prj.plc.station(&c.a.station)?;
        ports(prj, st).into_iter().find(|p| p.name == c.a.port).map(|p| p.bus)
    })
}

/// Creates a bus cable (type, cores and cross-section of the bus) for every bus
/// connection without one. Returns the number of new cables.
pub fn create_cables(prj: &mut Project) -> usize {
    let mut n = 0;
    for i in 0..prj.plc.connections.len() {
        if prj.plc.connections[i].cable.is_empty() {
            add_bus_cable(prj, i);
            n += 1;
        }
    }
    n
}

/// Adds the cable of bus connection `i`: the cable named by the connection if it is
/// missing in the project, otherwise a new one with the next free tag.
pub fn add_bus_cable(prj: &mut Project, i: usize) {
    let c = prj.plc.connections[i].clone();
    let bus = connection_bus(prj, &c).unwrap_or_default();
    let (ty, cores, cs) = bus.cable();
    let description = format!("{} - {}", port_label(prj, &c.a), port_label(prj, &c.b));
    let tag = crate::ops::add_cable(prj, "");
    let cable = prj.cables.iter_mut().find(|x| x.tag == tag).unwrap();
    if !c.cable.is_empty() {
        cable.tag = c.cable.clone();
    }
    cable.cable_type = ty.into();
    cable.cores = cores.into();
    cable.cross_section = cs.into();
    cable.bus = Some(bus);
    cable.description = description;
    prj.plc.connections[i].cable = cable.tag.clone();
}

// ------------------------------------------------------------------ automatic configuration

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoResult {
    pub stations: usize,
    pub networks: usize,
    pub assigned: usize,
    pub addresses: usize,
    pub connections: usize,
    pub cables: usize,
}

fn next_name(prj: &Project, base: &str) -> String {
    (1..).map(|i| format!("{base}{i}")).find(|n| prj.plc.station(n).is_none() && prj.plc.network(n).is_none()).unwrap()
}

fn ip_add(base: &str, n: u32) -> Option<String> {
    let parts: Vec<u32> = base.split('.').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [a, b, c, d] = parts[..] else { return None };
    let v = (a << 24 | b << 16 | c << 8 | d).checked_add(n)?;
    Some(format!("{}.{}.{}.{}", v >> 24, (v >> 16) & 255, (v >> 8) & 255, v & 255))
}

/// Builds the hardware configuration from the schematic and the parts: stations for
/// CPUs and bus couplers (or one generic PLC), bus systems, assignment of the cards,
/// racks and slots, start addresses from the module sizes, addresses of all points,
/// device numbers and IP addresses and a line topology for networks without
/// connections (with `cables`: a bus cable for each connection without one).
/// Existing settings are kept where they are consistent.
pub fn auto_configure(prj: &mut Project, cables: bool) -> AutoResult {
    let mut res = AutoResult::default();
    let cards = crate::plc::cards(prj);

    // Heads (CPUs, couplers) found in the parts of the devices.
    let mut heads: Vec<(String, DeviceDescription)> = Vec::new();
    for tag in prj.devices.keys() {
        if let Some(d) = description_of(prj, tag).filter(|d| d.kind.head()) {
            heads.push((tag.clone(), d));
        }
    }
    for (tag, d) in heads {
        if prj.plc.stations.iter().any(|s| s.tag == tag) {
            continue;
        }
        let (kind, base) = if d.kind == ModuleKind::Cpu { (StationKind::Controller, "PLC") } else { (StationKind::IoDevice, "IO") };
        let name = next_name(prj, base);
        prj.plc.stations.push(Station { name, kind, tag, device: d.id.clone(), ..Default::default() });
        res.stations += 1;
    }
    if prj.plc.stations.is_empty() && !cards.is_empty() {
        let name = next_name(prj, "PLC");
        prj.plc.stations.push(Station { name, device: "GEN.CPU-PN".into(), ..Default::default() });
        res.stations += 1;
    }
    if !prj.plc.stations.iter().any(|s| s.kind == StationKind::Controller) {
        return res;
    }

    // Bus systems: one per bus kind of the station heads.
    for i in 0..prj.plc.stations.len() {
        let st = prj.plc.stations[i].clone();
        if prj.plc.network(&st.network).is_some() {
            continue;
        }
        let Some(bus) = station_description(prj, &st).and_then(|d| d.bus()) else { continue };
        let net = match prj.plc.networks.iter().find(|n| n.kind == bus) {
            Some(n) => n.name.clone(),
            None => {
                let name = next_name(prj, if bus == BusKind::EtherCat { "ECAT_" } else { "BUS_" });
                let subnet = if bus.ethernet() { "192.168.0.0".to_string() } else { String::new() };
                prj.plc.networks.push(BusSystem { name: name.clone(), kind: bus, subnet });
                res.networks += 1;
                name
            }
        };
        prj.plc.stations[i].network = net;
    }
    // Remote I/O without controller: the controller in the same network.
    for i in 0..prj.plc.stations.len() {
        let st = &prj.plc.stations[i];
        if st.kind == StationKind::Controller || prj.plc.station(&st.controller).is_some() {
            continue;
        }
        let ctrl = prj.plc.stations.iter().filter(|c| c.kind == StationKind::Controller);
        let ctrl = ctrl.clone().find(|c| c.network == st.network).or_else(|| ctrl.clone().next()).map(|c| c.name.clone());
        prj.plc.stations[i].controller = ctrl.unwrap_or_default();
    }

    // Cards without (existing) station go to the first controller.
    let first = prj.plc.stations.iter().find(|s| s.kind == StationKind::Controller).map(|s| s.name.clone()).unwrap_or_default();
    for card in &cards {
        let data = prj.plc_cards.entry(card.clone()).or_default();
        if !prj.plc.stations.iter().any(|s| s.name == data.station) {
            data.station = first.clone();
            res.assigned += 1;
        }
    }

    // Slots, consecutive after the head, in the present order.
    let stations = prj.plc.stations.clone();
    for st in &stations {
        let first_slot = head_slot(st) + 1;
        let mut list: Vec<String> = prj.plc_cards.iter().filter(|(_, c)| c.station == st.name).map(|(t, _)| t.clone()).collect();
        list.sort_by(|a, b| {
            let key = |t: &str| parse_num(&prj.plc_cards[t].slot).unwrap_or(u32::MAX);
            key(a).cmp(&key(b)).then_with(|| natural_cmp(a, b))
        });
        for (i, card) in list.iter().enumerate() {
            let c = prj.plc_cards.get_mut(card).unwrap();
            c.rack = "0".into();
            c.slot = (first_slot + i as u32).to_string();
        }
    }

    // Start addresses per address space: controller first, then its remote I/O.
    let mut order: Vec<&Station> = stations.iter().collect();
    order.sort_by(|a, b| {
        let (da, db) = (domain(prj, a), domain(prj, b));
        da.cmp(&db).then((a.kind != StationKind::Controller).cmp(&(b.kind != StationKind::Controller))).then(a.device_number.cmp(&b.device_number)).then_with(|| natural_cmp(&a.name, &b.name))
    });
    let mut next: HashMap<String, (u32, u32)> = HashMap::new();
    for st in order {
        let dom = domain(prj, st);
        for m in modules(prj, &st.name) {
            let (ni, no) = next.entry(dom.clone()).or_insert((0, 0));
            let c = prj.plc_cards.get_mut(&m.card).unwrap();
            let (ib, ob) = (m.input_bytes, m.output_bytes);
            match (ib > 0, ob > 0) {
                (false, true) => {
                    c.start_byte = *no;
                    c.output_byte = None;
                }
                (true, true) if *ni != *no => {
                    c.start_byte = *ni;
                    c.output_byte = Some(*no);
                }
                _ => {
                    c.start_byte = *ni;
                    c.output_byte = None;
                }
            }
            *ni += ib;
            *no += ob;
        }
    }
    for card in prj.plc_cards.keys().cloned().collect::<Vec<_>>() {
        res.addresses += crate::plc::readdress(prj, &card);
    }

    // Device numbers, IP addresses and a line topology per network.
    for net in prj.plc.networks.clone() {
        let mut members: Vec<usize> = (0..prj.plc.stations.len()).filter(|&i| prj.plc.stations[i].network == net.name).collect();
        members.sort_by_key(|&i| (prj.plc.stations[i].kind != StationKind::Controller, prj.plc.stations[i].device_number, i));
        let mut used: Vec<u32> = members.iter().map(|&i| prj.plc.stations[i].device_number).filter(|n| *n > 0).collect();
        for &i in &members {
            let st = &mut prj.plc.stations[i];
            if st.kind != StationKind::Controller && st.device_number == 0 {
                st.device_number = (1..).find(|n| !used.contains(n)).unwrap();
                used.push(st.device_number);
            }
        }
        if net.kind.ethernet() {
            for &i in &members {
                if prj.plc.stations[i].address.is_empty() {
                    let n = prj.plc.stations[i].device_number;
                    let taken: Vec<String> = prj.plc.stations.iter().map(|s| s.address.clone()).collect();
                    let ip = (n + 1..n + 255).filter_map(|k| ip_add(&net.subnet, k)).find(|ip| !taken.contains(ip));
                    prj.plc.stations[i].address = ip.unwrap_or_default();
                }
            }
        }
        if members.len() < 2 || prj.plc.connections.iter().any(|c| c.network == net.name) {
            continue;
        }
        for w in members.windows(2) {
            let (a, b) = (prj.plc.stations[w[0]].clone(), prj.plc.stations[w[1]].clone());
            let (pa, pb) = (ports(prj, &a), ports(prj, &b));
            let out = pa.get(1).or(pa.first()).map(|p| p.name.clone()).unwrap_or_default();
            let inp = pb.first().map(|p| p.name.clone()).unwrap_or_default();
            prj.plc.connections.push(BusConnection { network: net.name.clone(), a: PortRef::new(&a.name, &out), b: PortRef::new(&b.name, &inp), cable: String::new() });
            res.connections += 1;
        }
    }
    if cables {
        res.cables = create_cables(prj);
    }
    res
}

// ------------------------------------------------------------------ checks

fn ranges_overlap(a: (u32, u32), b: (u32, u32)) -> bool {
    a.1 > 0 && b.1 > 0 && a.0 < b.0 + b.1 && b.0 < a.0 + a.1
}

/// Consistency of the hardware configuration.
pub fn problems(prj: &Project) -> Vec<Issue> {
    let hw = &prj.plc;
    let mut v = Vec::new();
    let err = |message: String| Issue::plain(Severity::Error, message, None);
    let mut names = BTreeMap::new();
    for st in &hw.stations {
        *names.entry(st.name.as_str()).or_insert(0) += 1;
    }
    for (name, n) in names {
        if n > 1 {
            v.push(crate::issue!(Severity::Error, None, "Station name {} is used {} times", name, n));
        }
    }
    for st in &hw.stations {
        if !st.network.is_empty() && hw.network(&st.network).is_none() {
            v.push(crate::issue!(Severity::Error, None, "Station {}: bus system {} does not exist", st.name, st.network));
        }
        if st.kind == StationKind::IoDevice && hw.station(&st.controller).is_none() {
            v.push(crate::issue!(Severity::Warning, None, "Remote I/O {} has no controller", st.name));
        }
    }
    for card in crate::plc::cards(prj) {
        if !hw.stations.is_empty() && prj.plc_cards.get(&card).is_none_or(|d| d.station.is_empty()) {
            v.push(crate::issue!(Severity::Warning, None, "PLC card {} is not assigned to a station", card));
        }
    }
    for (card, data) in &prj.plc_cards {
        if !data.station.is_empty() && hw.station(&data.station).is_none() {
            v.push(crate::issue!(Severity::Error, None, "PLC card {}: station {} does not exist", card, data.station));
        }
    }
    // Slots, channels, address ranges.
    // Address space -> card, (input start, bytes), (output start, bytes)
    type Ranges = Vec<(String, (u32, u32), (u32, u32))>;
    let mut ranges: BTreeMap<String, Ranges> = BTreeMap::new();
    for st in &hw.stations {
        let mods = modules(prj, &st.name);
        for (i, a) in mods.iter().enumerate() {
            if let Some(b) = mods[i + 1..].iter().find(|b| (b.rack, b.slot) == (a.rack, a.slot)) {
                v.push(crate::issue!(Severity::Error, None, "{}: slot {} of rack {} is used by {} and {}", st.name, a.slot, a.rack, a.card, b.card));
            }
            if let Some(d) = &a.description {
                if a.inputs_used > d.inputs {
                    v.push(crate::issue!(Severity::Error, None, "{}: {} inputs used, the module has {}", a.card, a.inputs_used, d.inputs));
                }
                if a.outputs_used > d.outputs {
                    v.push(crate::issue!(Severity::Error, None, "{}: {} outputs used, the module has {}", a.card, a.outputs_used, d.outputs));
                }
            }
            ranges.entry(domain(prj, st)).or_default().push((a.card.clone(), (a.input_start, a.input_bytes), (a.output_start, a.output_bytes)));
        }
        if let Some(d) = station_description(prj, st).filter(|d| d.slots > 0) {
            if mods.len() as u32 > d.slots {
                v.push(crate::issue!(Severity::Error, None, "{}: {} modules, the head has {} slots", st.name, mods.len(), d.slots));
            }
        }
    }
    for list in ranges.values() {
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                if ranges_overlap(a.1, b.1) || ranges_overlap(a.2, b.2) {
                    v.push(crate::issue!(Severity::Error, None, "Address ranges of {} and {} overlap", a.0, b.0));
                }
            }
        }
    }
    // Bus: addresses, ports, connections.
    for net in &hw.networks {
        let members: Vec<&Station> = hw.stations.iter().filter(|s| s.network == net.name).collect();
        for (i, a) in members.iter().enumerate() {
            if let Some(b) = members[i + 1..].iter().find(|b| !a.address.is_empty() && b.address == a.address) {
                v.push(crate::issue!(Severity::Error, None, "Address {} is used by {} and {}", a.address, a.name, b.name));
            }
        }
        let linked = hw.connections.iter().any(|c| c.network == net.name);
        for st in &members {
            if linked && !hw.connections.iter().any(|c| c.a.station == st.name || c.b.station == st.name) {
                v.push(crate::issue!(Severity::Warning, None, "Station {} is not connected to bus {}", st.name, net.name));
            }
        }
    }
    // Port -> cables on it, cables the port takes.
    let mut seen: HashMap<&PortRef, (usize, usize)> = HashMap::new();
    let mut cable_use: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &hw.connections {
        let bus = connection_bus(prj, c);
        if !c.cable.is_empty() {
            *cable_use.entry(c.cable.as_str()).or_default() += 1;
            match prj.cable(&c.cable) {
                None => v.push(crate::issue!(Severity::Warning, None, "Bus connection {} - {}: cable {} does not exist", c.a, c.b, c.cable)),
                Some(cable) => {
                    if let (Some(cb), Some(b)) = (cable.bus, bus) {
                        if cb != b {
                            v.push(crate::issue!(Severity::Warning, None, "Cable {} is a {} cable on a {} connection", cable.tag, cb.title(), b.title()));
                        }
                    }
                }
            }
        }
        for p in [&c.a, &c.b] {
            let e = seen.entry(p).or_insert((0, 1));
            e.0 += 1;
            e.1 = e.1.max(bus.map(|b| b.cables_per_port()).unwrap_or(1));
            let Some(st) = hw.station(&p.station) else {
                v.push(crate::issue!(Severity::Error, None, "Bus connection to {}: the station does not exist", p));
                continue;
            };
            let list = ports(prj, st);
            match list.iter().find(|x| x.name == p.port) {
                None => v.push(crate::issue!(Severity::Warning, None, "{}: the station has no port {}", st.name, p.port)),
                Some(port) => {
                    if let Some(net) = hw.network(&c.network).filter(|n| n.kind != port.bus) {
                        v.push(crate::issue!(Severity::Warning, None, "{}: port {} is not a {} port", st.name, p.port, net.kind.title()));
                    }
                }
            }
        }
    }
    let mut full: Vec<(&&PortRef, &(usize, usize))> = seen.iter().filter(|(_, (n, limit))| n > limit).collect();
    full.sort();
    for (p, (_, limit)) in full {
        v.push(err(if *limit > 1 { tr_fmt!("Port {} has more than {} cables", p, limit) } else { tr_fmt!("Port {} is connected more than once", p) }));
    }
    for (cable, n) in cable_use.into_iter().filter(|(_, n)| *n > 1) {
        v.push(crate::issue!(Severity::Warning, None, "Cable {} is used for {} bus connections", cable, n));
    }
    v
}

#[cfg(test)]
mod tests;
