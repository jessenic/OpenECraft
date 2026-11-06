//! AutomationML (CAEX 2.15) exchange of the PLC hardware configuration with PLC
//! programming systems (TIA Portal, …), structured after the AutomationML application
//! recommendation "Automation Project Configuration" (AR APC): devices with racks and
//! device items (position number, order number, I/O addresses), subnets with the
//! node and port links, and tag tables.

use crate::model::*;
use crate::netlist::Netlist;
use crate::plc_hw::{self, BusKind, BusSystem, DescPort, DeviceDescription, ModuleKind, PortRef, Scheme, Station, StationKind};
use crate::xml::{self, Element, Writer};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const ROLES: &str = "AutomationProjectConfigurationRoleClassLib";
const INTERFACES: &str = "AutomationProjectConfigurationInterfaceClassLib";

/// Deterministic GUID formatted id from a path.
fn guid(path: &str) -> String {
    let hash = |seed: u64| path.bytes().fold(seed, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let (a, b) = (hash(0xcbf29ce484222325), hash(0x84222325cbf29ce4));
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xffff, b >> 48, b & 0xffff_ffff_ffff)
}

fn attribute(w: &mut Writer, name: &str, ty: &str, value: &str) {
    w.open("Attribute", &[("Name", name), ("AttributeDataType", ty)]);
    w.text("Value", &[], value);
    w.close("Attribute");
}

fn role(w: &mut Writer, class: &str) {
    w.empty("RoleRequirements", &[("RefBaseRoleClassPath", &format!("{ROLES}/{class}"))]);
}

fn export_tag(w: &mut Writer, station: &str, name: &str, point: &crate::plc::Point) {
    w.open("InternalElement", &[("ID", &guid(&format!("{station}/tag/{name}"))), ("Name", name)]);
    let ty = point.analog_type.as_deref().unwrap_or("Bool");
    if !ty.is_empty() {
        attribute(w, "DataType", "xs:string", ty);
    }
    if point.analog_type.is_none() {
        attribute(w, "LogicalAddress", "xs:string", &format!("%{}", point.address.trim_start_matches('%')));
    }
    attribute(w, "Comment", "xs:string", &crate::plc::tag_comment(point));
    role(w, "Tag");
    w.close("InternalElement");
}

fn type_identifier(d: Option<&DeviceDescription>) -> Option<String> {
    d.map(|d| if d.order_number.is_empty() { &d.id } else { &d.order_number }).map(|o| format!("OrderNumber:{o}"))
}

fn subnet_type(kind: BusKind) -> &'static str {
    match kind {
        BusKind::ProfibusDp => "PROFIBUS",
        BusKind::EtherCat => "EtherCAT",
        BusKind::CanOpen => "CANopen",
        _ => "Ethernet",
    }
}

/// Exports stations, modules, bus systems and the PLC tags.
pub fn export(prj: &Project, nl: &Netlist) -> String {
    let mut w = Writer::new();
    let file = format!("{}.aml", prj.info.name);
    w.open(
        "CAEXFile",
        &[("FileName", &file), ("SchemaVersion", "2.15"), ("xsi:noNamespaceSchemaLocation", "CAEX_ClassModel_V2.15.xsd"), ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")],
    );
    w.empty("AdditionalInformation", &[("AutomationMLVersion", "2.0")]);
    w.open("AdditionalInformation", &[("DocumentVersions", "Recommendations")]);
    w.empty("Document", &[("DocumentIdentifier", "AR_APC"), ("Version", "1.2.0")]);
    w.close("AdditionalInformation");
    w.open("AdditionalInformation", &[]);
    w.open("WriterHeader", &[]);
    w.text("WriterName", &[], "OpenECraft");
    w.text("WriterID", &[], "OpenECraft");
    w.text("WriterVendor", &[], "OpenECraft");
    w.text("WriterVersion", &[], env!("CARGO_PKG_VERSION"));
    w.text("WriterProjectTitle", &[], &prj.info.name);
    w.close("WriterHeader");
    w.close("AdditionalInformation");

    let hw = &prj.plc;
    let tags = crate::plc::tags(prj, nl);
    let iface_id = |st: &Station| guid(&format!("{}/interface", st.name));
    w.open("InstanceHierarchy", &[("Name", &prj.info.name)]);
    for st in &hw.stations {
        let desc = plc_hw::station_description(prj, st);
        w.open("InternalElement", &[("ID", &guid(&st.name)), ("Name", &st.name)]);
        attribute(&mut w, "StationKind", "xs:string", st.kind.key());
        attribute(&mut w, "AddressScheme", "xs:string", st.scheme.key());
        if !st.controller.is_empty() {
            attribute(&mut w, "Controller", "xs:string", &st.controller);
        }
        if !st.comment.is_empty() {
            attribute(&mut w, "Comment", "xs:string", &st.comment);
        }
        let modules = plc_hw::modules(prj, &st.name);
        let mut racks: BTreeSet<u32> = modules.iter().map(|m| m.rack).collect();
        racks.insert(0);
        for rack in racks {
            let rack_name = format!("Rack_{rack}");
            w.open("InternalElement", &[("ID", &guid(&format!("{}/{rack_name}", st.name))), ("Name", &rack_name)]);
            attribute(&mut w, "PositionNumber", "xs:int", &rack.to_string());
            if rack == 0 {
                let head = if st.tag.is_empty() { desc.as_ref().map(|d| d.title()).unwrap_or_else(|| st.name.clone()) } else { st.tag.clone() };
                w.open("InternalElement", &[("ID", &guid(&format!("{}/head", st.name))), ("Name", &head)]);
                attribute(&mut w, "PositionNumber", "xs:int", &plc_hw::head_slot(st).to_string());
                if let Some(t) = type_identifier(desc.as_ref()) {
                    attribute(&mut w, "TypeIdentifier", "xs:string", &t);
                }
                if !st.tag.is_empty() {
                    attribute(&mut w, "DeviceTag", "xs:string", &st.tag);
                }
                let bus = hw.network(&st.network).map(|n| n.kind).or(desc.as_ref().and_then(|d| d.bus())).unwrap_or_default();
                w.open("InternalElement", &[("ID", &iface_id(st)), ("Name", &format!("{} interface", bus.title()))]);
                if !st.address.is_empty() {
                    attribute(&mut w, "NetworkAddress", "xs:string", &st.address);
                }
                if st.device_number > 0 {
                    attribute(&mut w, "DeviceNumber", "xs:int", &st.device_number.to_string());
                }
                w.empty("ExternalInterface", &[("ID", &guid(&format!("{}/node", st.name))), ("Name", "Node"), ("RefBaseClassPath", &format!("{INTERFACES}/LogicalEndPoint"))]);
                for p in plc_hw::ports(prj, st) {
                    w.empty("ExternalInterface", &[("ID", &guid(&format!("{}/port/{}", st.name, p.name))), ("Name", &p.name), ("RefBaseClassPath", &format!("{INTERFACES}/CommunicationPortInterface"))]);
                }
                role(&mut w, "DeviceItem");
                w.close("InternalElement");
                role(&mut w, "DeviceItem");
                w.close("InternalElement");
            }
            for m in modules.iter().filter(|m| m.rack == rack) {
                w.open("InternalElement", &[("ID", &guid(&format!("{}/{}", st.name, m.card))), ("Name", &m.card)]);
                attribute(&mut w, "PositionNumber", "xs:int", &m.slot.to_string());
                if let Some(t) = type_identifier(m.description.as_ref()) {
                    attribute(&mut w, "TypeIdentifier", "xs:string", &t);
                }
                attribute(&mut w, "DeviceTag", "xs:string", &m.card);
                if m.input_bytes + m.output_bytes > 0 {
                    w.open("Attribute", &[("Name", "Address")]);
                    let mut i = 0;
                    for (io, start, bytes) in [("Input", m.input_start, m.input_bytes), ("Output", m.output_start, m.output_bytes)] {
                        if bytes == 0 {
                            continue;
                        }
                        i += 1;
                        w.open("Attribute", &[("Name", &i.to_string())]);
                        attribute(&mut w, "StartAddress", "xs:int", &start.to_string());
                        attribute(&mut w, "Length", "xs:int", &(bytes * 8).to_string());
                        attribute(&mut w, "IoType", "xs:string", io);
                        w.close("Attribute");
                    }
                    w.close("Attribute");
                }
                role(&mut w, "DeviceItem");
                w.close("InternalElement");
            }
            role(&mut w, "DeviceItem");
            w.close("InternalElement");
        }
        let own: Vec<&(String, crate::plc::Point)> = tags.iter().filter(|(_, p)| plc_hw::station_of(prj, &p.card).is_some_and(|s| s.name == st.name)).collect();
        if !own.is_empty() {
            w.open("InternalElement", &[("ID", &guid(&format!("{}/tags", st.name))), ("Name", "PLC tags")]);
            for (name, p) in own {
                export_tag(&mut w, &st.name, name, p);
            }
            role(&mut w, "TagTable");
            w.close("InternalElement");
        }
        role(&mut w, "Device");
        w.close("InternalElement");
    }
    for net in &hw.networks {
        w.open("InternalElement", &[("ID", &guid(&format!("subnet/{}", net.name))), ("Name", &net.name)]);
        attribute(&mut w, "Type", "xs:string", subnet_type(net.kind));
        attribute(&mut w, "BusSystem", "xs:string", net.kind.key());
        if !net.subnet.is_empty() {
            attribute(&mut w, "SubnetAddress", "xs:string", &net.subnet);
        }
        w.empty("ExternalInterface", &[("ID", &guid(&format!("subnet/{}/node", net.name))), ("Name", "Node"), ("RefBaseClassPath", &format!("{INTERFACES}/LogicalEndPoint"))]);
        let subnet_node = format!("{}:Node", guid(&format!("subnet/{}", net.name)));
        for st in hw.stations.iter().filter(|s| s.network == net.name) {
            let a = format!("{}:Node", iface_id(st));
            w.empty("InternalLink", &[("Name", &format!("{} - {}", st.name, net.name)), ("RefPartnerSideA", &a), ("RefPartnerSideB", &subnet_node)]);
        }
        for (i, c) in hw.connections.iter().filter(|c| c.network == net.name).enumerate() {
            let side = |p: &PortRef| hw.station(&p.station).map(|s| format!("{}:{}", iface_id(s), p.port)).unwrap_or_default();
            let name = if c.cable.is_empty() { format!("Link_{}", i + 1) } else { c.cable.clone() };
            w.empty("InternalLink", &[("Name", &name), ("RefPartnerSideA", &side(&c.a)), ("RefPartnerSideB", &side(&c.b))]);
        }
        role(&mut w, "Subnet");
        w.close("InternalElement");
    }
    w.close("InstanceHierarchy");
    w.open("InterfaceClassLib", &[("Name", INTERFACES)]);
    for c in ["LogicalEndPoint", "CommunicationPortInterface"] {
        w.empty("InterfaceClass", &[("Name", c)]);
    }
    w.close("InterfaceClassLib");
    w.open("RoleClassLib", &[("Name", ROLES)]);
    for c in ["Device", "DeviceItem", "Subnet", "TagTable", "Tag"] {
        w.empty("RoleClass", &[("Name", c)]);
    }
    w.close("RoleClassLib");
    w.close("CAEXFile");
    w.out
}

// ------------------------------------------------------------------ import

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportResult {
    pub stations: usize,
    pub modules: usize,
    pub networks: usize,
    pub connections: usize,
    /// Changed point addresses.
    pub addresses: usize,
    /// Changed function texts (from the tag tables).
    pub texts: usize,
}

fn role_of(el: &Element) -> String {
    let path = el
        .child("RoleRequirements")
        .and_then(|r| r.attr("RefBaseRoleClassPath"))
        .or_else(|| el.child("SupportedRoleClass").and_then(|r| r.attr("RefRoleClassPath")))
        .unwrap_or("");
    path.rsplit('/').next().unwrap_or("").to_string()
}

fn value(el: &Element, name: &str) -> Option<String> {
    el.children("Attribute").find(|a| a.attr("Name") == Some(name)).and_then(|a| a.child("Value")).map(|v| v.text.trim().to_string()).filter(|v| !v.is_empty())
}

fn number(el: &Element, name: &str) -> Option<u32> {
    value(el, name).and_then(|v| v.parse().ok())
}

/// Order number of a `TypeIdentifier` (`OrderNumber:6ES7 …/V2.0`); `System:` types are
/// no orderable hardware (racks).
fn order_number(el: &Element) -> Option<String> {
    let t = value(el, "TypeIdentifier")?;
    let o = t.strip_prefix("OrderNumber:")?;
    Some(o.split('/').next().unwrap_or(o).trim().to_string()).filter(|o| !o.is_empty())
}

/// I/O ranges of the `Address` attribute: (input, start byte, bytes).
fn io_ranges(el: &Element) -> Vec<(bool, u32, u32)> {
    let Some(addr) = el.children("Attribute").find(|a| a.attr("Name") == Some("Address")) else { return Vec::new() };
    addr.children("Attribute")
        .filter_map(|a| {
            let input = match value(a, "IoType")?.to_ascii_lowercase().as_str() {
                "input" => true,
                "output" => false,
                _ => return None,
            };
            Some((input, number(a, "StartAddress")?, number(a, "Length").unwrap_or(8).div_ceil(8)))
        })
        .collect()
}

fn elements(el: &Element) -> impl Iterator<Item = &Element> {
    el.children("InternalElement")
}

/// What an interface reference points to.
#[derive(Clone, Debug, PartialEq)]
enum End {
    Port(String, String),
    Node(String),
    Subnet(String),
}

struct ParsedModule {
    tag: String,
    station: String,
    rack: u32,
    slot: u32,
    order: Option<String>,
    /// (input, start byte, bytes)
    ranges: Vec<(bool, u32, u32)>,
}

#[derive(Default)]
struct Parsed {
    stations: Vec<Station>,
    modules: Vec<ParsedModule>,
    /// station -> head order number, port names
    heads: HashMap<String, (Option<String>, Vec<String>)>,
    networks: Vec<BusSystem>,
    ends: HashMap<String, End>,
    links: Vec<(String, String, String)>,
    /// station -> address key -> text
    texts: HashMap<String, BTreeMap<crate::plc::AddressKey, String>>,
    /// Stations whose kind is given in the file.
    explicit: BTreeSet<String>,
}

impl Parsed {
    fn interfaces(&mut self, el: &Element, station: &str, ports: &mut Vec<String>) {
        let id = el.attr("ID").unwrap_or("");
        for i in el.children("ExternalInterface") {
            let class = i.attr("RefBaseClassPath").unwrap_or("");
            let name = i.attr("Name").unwrap_or("");
            let end = if class.ends_with("LogicalEndPoint") || name == "Node" {
                End::Node(station.to_string())
            } else if class.ends_with("CommunicationPortInterface") || class.ends_with("PortInterface") {
                // TIA names the port element ("Port_1") and the interface generically.
                let port = if name.is_empty() || name.contains("Interface") || name == "Port" { el.attr("Name").unwrap_or(name) } else { name };
                ports.push(port.to_string());
                End::Port(station.to_string(), port.to_string())
            } else {
                continue;
            };
            if let Some(iid) = i.attr("ID") {
                self.ends.insert(iid.to_string(), end.clone());
            }
            self.ends.insert(format!("{id}:{name}"), end);
        }
    }

    fn device_items(&mut self, el: &Element, st: &mut Station, rack: Option<u32>, ports: &mut Vec<String>) {
        for item in elements(el) {
            self.interfaces(item, &st.name.clone(), ports);
            if let Some(a) = value(item, "NetworkAddress") {
                st.address = a;
            }
            if let Some(n) = number(item, "DeviceNumber") {
                st.device_number = n;
            }
            if role_of(item) == "TagTable" {
                let texts = self.texts.entry(st.name.clone()).or_default();
                for tag in elements(item) {
                    let Some(addr) = value(tag, "LogicalAddress") else { continue };
                    let text = value(tag, "Comment").unwrap_or_else(|| tag.attr("Name").unwrap_or("").to_string());
                    texts.insert(crate::plc::address_key(&addr), text);
                }
                continue;
            }
            let ranges = io_ranges(item);
            let pos = number(item, "PositionNumber");
            let is_rack = rack.is_none() && order_number(item).is_none() && elements(item).any(|c| number(c, "PositionNumber").is_some());
            if is_rack {
                let n = pos.or_else(|| item.attr("Name")?.rsplit('_').next()?.parse().ok()).unwrap_or(0);
                self.device_items(item, st, Some(n), ports);
            } else if !ranges.is_empty() {
                let tag = value(item, "DeviceTag").unwrap_or_else(|| item.attr("Name").unwrap_or("").to_string());
                self.modules.push(ParsedModule { tag, station: st.name.clone(), rack: rack.unwrap_or(0), slot: pos.unwrap_or(0), order: order_number(item), ranges });
            } else if let Some(order) = order_number(item).filter(|_| !self.heads.contains_key(&st.name)) {
                if let Some(t) = value(item, "DeviceTag") {
                    st.tag = t;
                }
                self.heads.insert(st.name.clone(), (Some(order), Vec::new()));
                let mut own = Vec::new();
                self.device_items(item, st, rack.or(Some(0)), &mut own);
                self.heads.get_mut(&st.name).unwrap().1 = own;
            } else {
                self.device_items(item, st, rack, ports);
            }
        }
    }

    fn links(&mut self, el: &Element) {
        for l in el.descendants("InternalLink") {
            let side = |n: &str| l.attr(n).unwrap_or("").to_string();
            self.links.push((l.attr("Name").unwrap_or("").to_string(), side("RefPartnerSideA"), side("RefPartnerSideB")));
        }
    }

    fn walk(&mut self, el: &Element) {
        for item in elements(el) {
            match role_of(item).as_str() {
                "Device" => {
                    let name = item.attr("Name").unwrap_or("").to_string();
                    let mut st = Station {
                        name: name.clone(),
                        kind: value(item, "StationKind").and_then(|k| StationKind::parse(&k)).unwrap_or_default(),
                        scheme: value(item, "AddressScheme").and_then(|k| Scheme::parse(&k)).unwrap_or_default(),
                        controller: value(item, "Controller").unwrap_or_default(),
                        comment: value(item, "Comment").unwrap_or_default(),
                        ..Default::default()
                    };
                    if value(item, "StationKind").is_some() {
                        self.explicit.insert(name.clone());
                    }
                    let mut ports = Vec::new();
                    self.interfaces(item, &name, &mut ports);
                    self.device_items(item, &mut st, None, &mut ports);
                    if let Some(h) = self.heads.get_mut(&name) {
                        h.1.extend(ports);
                    } else {
                        self.heads.insert(name, (None, ports));
                    }
                    self.stations.push(st);
                }
                "Subnet" => {
                    let name = item.attr("Name").unwrap_or("").to_string();
                    let ty = value(item, "Type").unwrap_or_default().to_ascii_lowercase();
                    let kind = value(item, "BusSystem").and_then(|k| BusKind::parse(&k)).unwrap_or(match ty.as_str() {
                        "profibus" => BusKind::ProfibusDp,
                        "ethercat" => BusKind::EtherCat,
                        "canopen" => BusKind::CanOpen,
                        _ => BusKind::Profinet,
                    });
                    let id = item.attr("ID").unwrap_or("");
                    for i in item.children("ExternalInterface") {
                        if let Some(iid) = i.attr("ID") {
                            self.ends.insert(iid.to_string(), End::Subnet(name.clone()));
                        }
                        self.ends.insert(format!("{id}:{}", i.attr("Name").unwrap_or("")), End::Subnet(name.clone()));
                    }
                    self.ends.insert(id.to_string(), End::Subnet(name.clone()));
                    self.networks.push(BusSystem { name, kind, subnet: value(item, "SubnetAddress").unwrap_or_default() });
                }
                _ => self.walk(item),
            }
        }
    }

    fn resolve(&self, r: &str) -> Option<End> {
        self.ends.get(r).cloned().or_else(|| self.ends.get(r.rsplit_once(':')?.0).cloned())
    }
}

/// Description for an order number: a known one, otherwise a new one from the data
/// of the file.
fn description_for(prj: &Project, order: &str, make: impl FnOnce() -> DeviceDescription) -> (DeviceDescription, bool) {
    let known = plc_hw::descriptions(prj).into_iter().find(|d| d.order_number == order || d.id == order).or_else(|| {
        crate::parts::catalog().iter().find(|p| p.order_number == order || p.number == order).and_then(crate::devdesc::from_part)
    });
    match known {
        Some(d) => (d, false),
        None => (make(), true),
    }
}

/// Imports an AutomationML hardware configuration: stations, racks and modules,
/// subnets with their node and port links and tag tables. Stations and bus systems
/// with the same name are replaced; cards are matched by their device tag (or the
/// module name). The points of the imported cards are readdressed.
pub fn import(prj: &mut Project, text: &str) -> Result<ImportResult, String> {
    let root = xml::parse(text)?;
    if root.name != "CAEXFile" {
        return Err(format!("not an AutomationML file (root element {})", root.name));
    }
    let mut p = Parsed::default();
    for ih in root.children("InstanceHierarchy") {
        p.walk(ih);
        p.links(ih);
    }
    if p.stations.is_empty() && p.networks.is_empty() {
        return Err("no devices or subnets found".into());
    }
    let mut res = ImportResult { stations: p.stations.len(), networks: p.networks.len(), modules: p.modules.len(), ..Default::default() };
    let mut new_desc = Vec::new();

    // Links: node links give the bus system, port links the connections.
    let mut stations = p.stations.clone();
    let mut connections = Vec::new();
    for (name, a, b) in &p.links {
        match (p.resolve(a), p.resolve(b)) {
            (Some(End::Node(st)), Some(End::Subnet(net))) | (Some(End::Subnet(net)), Some(End::Node(st))) => {
                if let Some(s) = stations.iter_mut().find(|s| s.name == st) {
                    s.network = net;
                }
            }
            (Some(End::Port(sa, pa)), Some(End::Port(sb, pb))) => {
                let cable = if name.starts_with("Link") { String::new() } else { name.clone() };
                connections.push((PortRef::new(&sa, &pa), PortRef::new(&sb, &pb), cable));
            }
            _ => {}
        }
    }
    for st in &mut stations {
        let (order, ports) = p.heads.get(&st.name).cloned().unwrap_or_default();
        let Some(order) = order else { continue };
        let bus = p.networks.iter().find(|n| n.name == st.network).map(|n| n.kind).unwrap_or_default();
        let (d, new) = description_for(prj, &order, || DeviceDescription {
            id: order.clone(),
            name: order.clone(),
            order_number: order.clone(),
            kind: if st.kind == StationKind::Controller { ModuleKind::Cpu } else { ModuleKind::Coupler },
            slots: 32,
            ports: ports.iter().map(|n| DescPort { name: n.clone(), bus }).collect(),
            source: "AutomationML".into(),
            ..Default::default()
        });
        if new {
            new_desc.push(d.clone());
        }
        if !p.explicit.contains(&st.name) && d.kind == ModuleKind::Coupler {
            st.kind = StationKind::IoDevice;
        }
        st.device = d.id;
    }
    for st in stations {
        match prj.plc.stations.iter_mut().find(|s| s.name == st.name) {
            Some(s) => *s = st,
            None => prj.plc.stations.push(st),
        }
    }
    for net in p.networks.clone() {
        prj.plc.connections.retain(|c| c.network != net.name);
        match prj.plc.networks.iter_mut().find(|n| n.name == net.name) {
            Some(n) => *n = net,
            None => prj.plc.networks.push(net),
        }
    }
    for (a, b, cable) in connections {
        if plc_hw::connect(prj, a, b, &cable).is_ok() {
            res.connections += 1;
        }
    }
    // Cables named in the file become bus cables of the project.
    for i in 0..prj.plc.connections.len() {
        let tag = &prj.plc.connections[i].cable;
        if !tag.is_empty() && prj.cable(tag).is_none() {
            plc_hw::add_bus_cable(prj, i);
        }
    }

    // Modules.
    for ParsedModule { tag, station, rack, slot, order, ranges } in &p.modules {
        let range = |input: bool| ranges.iter().find(|r| r.0 == input).copied();
        let (ri, ro) = (range(true), range(false));
        let device = order.as_ref().map(|order| {
            let (d, new) = description_for(prj, order, || {
                let (ib, ob) = (ri.map(|r| r.2).unwrap_or(0), ro.map(|r| r.2).unwrap_or(0));
                DeviceDescription {
                    id: order.clone(),
                    name: order.clone(),
                    order_number: order.clone(),
                    kind: match (ib > 0, ob > 0) {
                        (true, true) => ModuleKind::DigitalMixed,
                        (true, false) => ModuleKind::DigitalInput,
                        _ => ModuleKind::DigitalOutput,
                    },
                    inputs: ib * 8,
                    outputs: ob * 8,
                    input_bytes: ib,
                    output_bytes: ob,
                    source: "AutomationML".into(),
                    ..Default::default()
                }
            });
            if new && !new_desc.iter().any(|x: &DeviceDescription| x.id == d.id) {
                new_desc.push(d.clone());
            }
            d
        });
        let from_parts = plc_hw::description_of(prj, tag).map(|d| d.order_number);
        let c = prj.plc_cards.entry(tag.clone()).or_default();
        c.station = station.clone();
        c.rack = rack.to_string();
        c.slot = slot.to_string();
        match (ri, ro) {
            (Some(i), Some(o)) => {
                c.start_byte = i.1;
                c.output_byte = (o.1 != i.1).then_some(o.1);
            }
            (Some(r), None) | (None, Some(r)) => {
                c.start_byte = r.1;
                c.output_byte = None;
            }
            (None, None) => {}
        }
        if let Some(d) = device {
            if from_parts.as_deref() != Some(d.order_number.as_str()) {
                c.device = d.id;
            }
        }
    }
    prj.plc.add_descriptions(new_desc);
    for m in &p.modules {
        res.addresses += crate::plc::readdress(prj, &m.tag);
    }
    for (station, texts) in &p.texts {
        let Some(st) = prj.plc.station(station).cloned() else { continue };
        let dom = plc_hw::domain(prj, &st);
        res.texts += crate::plc::apply_texts(prj, Some(&dom), texts);
    }
    Ok(res)
}

#[cfg(test)]
mod tests;
