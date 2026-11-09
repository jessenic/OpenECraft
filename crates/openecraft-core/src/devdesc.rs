//! Device descriptions of PLC hardware: CPUs, bus couplers and I/O modules with their
//! channels, process image size, slots and bus ports. They come from the built-in
//! generic list, from parts (product group PLC) or from the manufacturers' description
//! files (GSDML for PROFINET, ESI for EtherCAT).

use crate::parts::{FnKind, Part};
use crate::xml::{self, Element};
use openecraft_i18n::{tr, tr_const};
use std::collections::HashMap;
pub use crate::model::devdesc::BusKind;
pub use crate::model::devdesc::{DescPort, DeviceDescription, ModuleKind};

impl BusKind {
    pub const ALL: [BusKind; 6] = [BusKind::Profinet, BusKind::EtherCat, BusKind::EthernetIp, BusKind::ProfibusDp, BusKind::ModbusTcp, BusKind::CanOpen];

    /// Product name (not translated).
    pub fn title(self) -> &'static str {
        match self {
            BusKind::Profinet => "PROFINET",
            BusKind::EtherCat => "EtherCAT",
            BusKind::EthernetIp => "EtherNet/IP",
            BusKind::ProfibusDp => "PROFIBUS DP",
            BusKind::ModbusTcp => "Modbus TCP",
            BusKind::CanOpen => "CANopen",
        }
    }

    /// Key for files and the command line (`profinet`, `ethercat`, …).
    pub fn key(self) -> &'static str {
        match self {
            BusKind::Profinet => "profinet",
            BusKind::EtherCat => "ethercat",
            BusKind::EthernetIp => "ethernet_ip",
            BusKind::ProfibusDp => "profibus_dp",
            BusKind::ModbusTcp => "modbus_tcp",
            BusKind::CanOpen => "canopen",
        }
    }

    /// From a key or a product name.
    pub fn parse(s: &str) -> Option<BusKind> {
        let n: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        BusKind::ALL.into_iter().find(|k| {
            let key: String = k.key().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            key == n || k.title().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().eq_ignore_ascii_case(&n)
        })
    }

    /// Ethernet based: devices have an IP address.
    pub fn ethernet(self) -> bool {
        matches!(self, BusKind::Profinet | BusKind::EthernetIp | BusKind::ModbusTcp)
    }

    /// Line bus: the incoming and the outgoing cable share one connector, so a port
    /// takes two cables.
    pub fn line_bus(self) -> bool {
        matches!(self, BusKind::ProfibusDp | BusKind::CanOpen)
    }

    /// Cables a port can take.
    pub fn cables_per_port(self) -> usize {
        if self.line_bus() {
            2
        } else {
            1
        }
    }

    /// Default cable: type, cores and cross-section.
    pub fn cable(self) -> (&'static str, &'static str, &'static str) {
        match self {
            BusKind::Profinet => ("PROFINET cable type A 2x2, green", "TD+ TD- RD+ RD-", "0.34 mm²"),
            BusKind::EtherCat => ("EtherCAT cable Cat 5e 2x2", "TD+ TD- RD+ RD-", "0.34 mm²"),
            BusKind::EthernetIp | BusKind::ModbusTcp => ("Industrial Ethernet cable Cat 5e 2x2", "TD+ TD- RD+ RD-", "0.34 mm²"),
            BusKind::ProfibusDp => ("PROFIBUS cable type A 1x2, violet", "A B", "0.34 mm²"),
            BusKind::CanOpen => ("CAN bus cable 2x2 120 Ohm", "CAN_H CAN_L CAN_GND", "0.25 mm²"),
        }
    }

    /// Port names of a device without description: two ports for line topologies.
    pub fn default_ports(self) -> &'static [&'static str] {
        match self {
            BusKind::Profinet => &["X1 P1", "X1 P2"],
            BusKind::EtherCat => &["IN", "OUT"],
            BusKind::EthernetIp | BusKind::ModbusTcp => &["P1", "P2"],
            BusKind::ProfibusDp | BusKind::CanOpen => &["X1"],
        }
    }
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 10] = [
        ModuleKind::Cpu,
        ModuleKind::Coupler,
        ModuleKind::DigitalInput,
        ModuleKind::DigitalOutput,
        ModuleKind::DigitalMixed,
        ModuleKind::AnalogInput,
        ModuleKind::AnalogOutput,
        ModuleKind::Power,
        ModuleKind::Communication,
        ModuleKind::Other,
    ];

    pub fn title(self) -> &'static str {
        match self {
            ModuleKind::Cpu => tr("CPU"),
            ModuleKind::Coupler => tr("Bus coupler"),
            ModuleKind::DigitalInput => tr("Digital input module"),
            ModuleKind::DigitalOutput => tr("Digital output module"),
            ModuleKind::DigitalMixed => tr("Digital input / output module"),
            ModuleKind::AnalogInput => tr("Analog input module"),
            ModuleKind::AnalogOutput => tr("Analog output module"),
            ModuleKind::Power => tr("Power module"),
            ModuleKind::Communication => tr("Communication module"),
            ModuleKind::Other => tr("Other module"),
        }
    }

    /// Head of a station (carries the bus interface and the slots).
    pub fn head(self) -> bool {
        matches!(self, ModuleKind::Cpu | ModuleKind::Coupler)
    }

    fn analog(self) -> bool {
        matches!(self, ModuleKind::AnalogInput | ModuleKind::AnalogOutput)
    }

    /// From a product subgroup or a `PLC module:` value.
    pub fn from_text(s: &str) -> Option<ModuleKind> {
        let s = s.to_lowercase();
        let has = |w: &[&str]| w.iter().any(|w| s.contains(w));
        Some(if has(&["cpu", "controller"]) {
            ModuleKind::Cpu
        } else if has(&["coupler", "interface module", "head"]) {
            ModuleKind::Coupler
        } else if has(&["analog"]) && has(&["input"]) {
            ModuleKind::AnalogInput
        } else if has(&["analog"]) {
            ModuleKind::AnalogOutput
        } else if has(&["input / output", "input/output", "mixed"]) {
            ModuleKind::DigitalMixed
        } else if has(&["input"]) {
            ModuleKind::DigitalInput
        } else if has(&["output"]) {
            ModuleKind::DigitalOutput
        } else if has(&["power"]) {
            ModuleKind::Power
        } else if has(&["communication"]) {
            ModuleKind::Communication
        } else {
            return None;
        })
    }
}

impl DeviceDescription {
    /// Name to show: built-in names are translated.
    pub fn title(&self) -> String {
        match BUILTIN.iter().find(|b| b.0 == self.id && self.source.is_empty()) {
            Some(b) => tr(b.1).to_string(),
            None if self.name.is_empty() => self.id.clone(),
            None => self.name.clone(),
        }
    }

    /// Bus of the first port.
    pub fn bus(&self) -> Option<BusKind> {
        self.ports.first().map(|p| p.bus)
    }
}

/// id, name, kind, inputs, outputs, input bytes, output bytes, slots, bus of the ports.
type Builtin = (&'static str, &'static str, ModuleKind, u32, u32, u32, u32, u32, Option<BusKind>);

const BUILTIN: &[Builtin] = &[
    ("GEN.CPU-PN", tr_const!("Generic PLC CPU with PROFINET interface"), ModuleKind::Cpu, 0, 0, 0, 0, 31, Some(BusKind::Profinet)),
    ("GEN.CPU-ECAT", tr_const!("Generic PLC CPU with EtherCAT master"), ModuleKind::Cpu, 0, 0, 0, 0, 31, Some(BusKind::EtherCat)),
    ("GEN.IM-PN", tr_const!("Generic PROFINET interface module"), ModuleKind::Coupler, 0, 0, 0, 0, 32, Some(BusKind::Profinet)),
    ("GEN.EK-ECAT", tr_const!("Generic EtherCAT bus coupler"), ModuleKind::Coupler, 0, 0, 0, 0, 64, Some(BusKind::EtherCat)),
    ("GEN.PLC-DI8", tr_const!("Generic digital input module 8 channels"), ModuleKind::DigitalInput, 8, 0, 1, 0, 0, None),
    ("GEN.PLC-DI16", tr_const!("Generic digital input module 16 channels"), ModuleKind::DigitalInput, 16, 0, 2, 0, 0, None),
    ("GEN.PLC-DO8", tr_const!("Generic digital output module 8 channels"), ModuleKind::DigitalOutput, 0, 8, 0, 1, 0, None),
    ("GEN.PLC-DO16", tr_const!("Generic digital output module 16 channels"), ModuleKind::DigitalOutput, 0, 16, 0, 2, 0, None),
    ("GEN.PLC-DIO16", tr_const!("Generic digital module 8 inputs / 8 outputs"), ModuleKind::DigitalMixed, 8, 8, 1, 1, 0, None),
    ("GEN.PLC-AI4", tr_const!("Generic analog input module 4 channels"), ModuleKind::AnalogInput, 4, 0, 8, 0, 0, None),
    ("GEN.PLC-AO4", tr_const!("Generic analog output module 4 channels"), ModuleKind::AnalogOutput, 0, 4, 0, 8, 0, None),
];

/// The built-in generic descriptions.
pub fn builtin() -> Vec<DeviceDescription> {
    BUILTIN
        .iter()
        .map(|&(id, name, kind, inputs, outputs, input_bytes, output_bytes, slots, bus)| DeviceDescription {
            id: id.into(),
            name: name.into(),
            manufacturer: "Generic".into(),
            order_number: id.into(),
            kind,
            inputs,
            outputs,
            input_bytes,
            output_bytes,
            slots,
            ports: bus.map(|b| b.default_ports().iter().map(|p| DescPort { name: p.to_string(), bus: b }).collect()).unwrap_or_default(),
            source: String::new(),
        })
        .collect()
}

fn bytes_for(kind: ModuleKind, channels: u32) -> u32 {
    if kind.analog() {
        channels * 2
    } else {
        channels.div_ceil(8)
    }
}

/// Description of a part of the product group PLC (or with PLC input / output
/// functions). Technical data lines refine it: `PLC module: CPU`, `Inputs: 16`,
/// `Outputs: 0`, `Input bytes: 2`, `Output bytes: 0`, `Slots: 32`,
/// `Bus: PROFINET`, `Ports: X1 P1, X1 P2`.
pub fn from_part(part: &Part) -> Option<DeviceDescription> {
    let count = |k: FnKind| part.functions.iter().filter(|f| f.kind == k).count() as u32;
    let (di, dout, ai, ao) = (count(FnKind::PlcInput), count(FnKind::PlcOutput), count(FnKind::PlcAnalogInput), count(FnKind::PlcAnalogOutput));
    let (fi,fo)=(di+ai,dout+ao);
    if !part.group.eq_ignore_ascii_case("PLC") && fi + fo == 0 && part.tech_value("PLC module").is_none() {
        return None;
    }
    let num = |name: &str| part.tech_value(name).and_then(|v| v.split_whitespace().next()?.parse::<u32>().ok());
    let kind = part
        .tech_value("PLC module")
        .and_then(ModuleKind::from_text)
        .or_else(|| ModuleKind::from_text(&part.subgroup))
        .unwrap_or(match (fi > 0, fo > 0) {
            (true,false) if ai>0 && di==0=>ModuleKind::AnalogInput,
            (false,true) if ao>0 && dout==0=>ModuleKind::AnalogOutput,
            _ if ai+ao>0=>ModuleKind::Other,
            (true, true) => ModuleKind::DigitalMixed,
            (true, false) => ModuleKind::DigitalInput,
            (false, true) => ModuleKind::DigitalOutput,
            _ => ModuleKind::Other,
        });
    let inputs = num("Inputs").unwrap_or(fi);
    let outputs = num("Outputs").unwrap_or(fo);
    let bus = part.tech_value("Bus").and_then(BusKind::parse).or(kind.head().then_some(BusKind::Profinet));
    let ports = match (part.tech_value("Ports"), bus) {
        (Some(list), Some(b)) => list.split([',', ';']).map(str::trim).filter(|p| !p.is_empty()).map(|p| DescPort { name: p.into(), bus: b }).collect(),
        (None, Some(b)) => b.default_ports().iter().map(|p| DescPort { name: p.to_string(), bus: b }).collect(),
        _ => Vec::new(),
    };
    Some(DeviceDescription {
        id: part.number.clone(),
        name: part.description.clone(),
        manufacturer: part.manufacturer.clone(),
        order_number: if part.order_number.is_empty() { part.number.clone() } else { part.order_number.clone() },
        kind,
        inputs,
        outputs,
        input_bytes: num("Input bytes").unwrap_or_else(||if num("Inputs").is_none() && fi>0 {di.div_ceil(8)+ai*2} else {bytes_for(kind,inputs)}),
        output_bytes: num("Output bytes").unwrap_or_else(||if num("Outputs").is_none() && fo>0 {dout.div_ceil(8)+ao*2} else {bytes_for(kind,outputs)}),
        slots: num("Slots").unwrap_or(if kind.head() { 32 } else { 0 }),
        ports,
        source: "part".into(),
    })
}

/// Reads a GSDML (PROFINET) or ESI (EtherCAT) device description file.
pub fn import(text: &str, file_name: &str) -> Result<Vec<DeviceDescription>, String> {
    let root = xml::parse(text)?;
    let v = match root.name.as_str() {
        "ISO15745Profile" => gsdml(&root, file_name),
        "EtherCATInfo" => esi(&root, file_name),
        other => return Err(format!("not a GSDML or ESI file (root element {other})")),
    };
    if v.is_empty() {
        return Err("no devices or modules found".into());
    }
    Ok(v)
}

fn gsd_size(item: &Element) -> u32 {
    match item.attr("DataType").unwrap_or("") {
        "Unsigned8" | "Integer8" | "Boolean" => 1,
        "Unsigned16" | "Integer16" | "Unsigned8+Unsigned8" => 2,
        "Unsigned32" | "Integer32" | "Float32" | "Float32+Unsigned8" => 4,
        "Unsigned64" | "Integer64" | "Float64" => 8,
        _ => item.attr("Length").and_then(|l| l.parse().ok()).unwrap_or(1),
    }
}

fn gsdml(root: &Element, file: &str) -> Vec<DeviceDescription> {
    let texts: HashMap<&str, &str> = root
        .descendants("PrimaryLanguage")
        .first()
        .map(|l| l.children("Text").filter_map(|t| Some((t.attr("TextId")?, t.attr("Value")?))).collect())
        .unwrap_or_default();
    let text = |el: &Element, child: &str| -> String {
        el.child(child).and_then(|c| c.attr("TextId")).and_then(|id| texts.get(id)).map(|s| s.to_string()).unwrap_or_default()
    };
    let vendor = root.descendants("VendorName").first().and_then(|v| v.attr("Value")).unwrap_or("").to_string();
    let info = |el: &Element| -> (String, String) {
        let mi = el.child("ModuleInfo");
        let name = mi.map(|m| text(m, "Name")).unwrap_or_default();
        let order = mi.and_then(|m| m.child("OrderNumber")).and_then(|o| o.attr("Value")).unwrap_or("").to_string();
        (name, order)
    };
    let mut v = Vec::new();
    for dap in root.descendants("DeviceAccessPointItem") {
        let (name, order) = info(dap);
        let slots = dap
            .attr("PhysicalSlots")
            .and_then(|s| s.rsplit("..").next()?.trim().parse::<u32>().ok())
            .unwrap_or(32);
        let ports = dap.descendants("PortSubmoduleItem").len().max(1);
        let kind = if name.to_lowercase().contains("cpu") { ModuleKind::Cpu } else { ModuleKind::Coupler };
        v.push(DeviceDescription {
            id: dap.attr("ID").unwrap_or(&order).to_string(),
            name,
            manufacturer: vendor.clone(),
            order_number: order,
            kind,
            slots,
            ports: (1..=ports).map(|i| DescPort { name: format!("X1 P{i}"), bus: BusKind::Profinet }).collect(),
            source: file.to_string(),
            ..Default::default()
        });
    }
    for m in root.descendants("ModuleItem") {
        let (name, order) = info(m);
        let size = |dir: &str| -> u32 { m.descendants(dir).iter().flat_map(|d| d.children("DataItem")).map(gsd_size).sum() };
        let (ib, ob) = (size("Input"), size("Output"));
        let lower = name.to_lowercase();
        let analog = lower.contains("ai ") || lower.contains("aq ") || lower.contains("ao ") || lower.contains("analog");
        let kind = match (ib > 0, ob > 0, analog) {
            (true, false, true) => ModuleKind::AnalogInput,
            (false, true, true) => ModuleKind::AnalogOutput,
            (true, true, _) => ModuleKind::DigitalMixed,
            (true, false, false) => ModuleKind::DigitalInput,
            (false, true, false) => ModuleKind::DigitalOutput,
            _ => ModuleKind::Other,
        };
        let channels = |bytes: u32| if analog { bytes / 2 } else { bytes * 8 };
        v.push(DeviceDescription {
            id: m.attr("ID").unwrap_or(&order).to_string(),
            name,
            manufacturer: vendor.clone(),
            order_number: order,
            kind,
            inputs: channels(ib),
            outputs: channels(ob),
            input_bytes: ib,
            output_bytes: ob,
            source: file.to_string(),
            ..Default::default()
        });
    }
    v
}

fn esi(root: &Element, file: &str) -> Vec<DeviceDescription> {
    let vendor = root.child("Vendor").and_then(|v| v.child("Name")).map(|n| n.text.trim().to_string()).unwrap_or_default();
    let mut v = Vec::new();
    for dev in root.descendants("Device") {
        let Some(ty) = dev.child("Type") else { continue };
        let ty_name = ty.text.trim().to_string();
        let name = dev.child("Name").map(|n| n.text.trim().to_string()).unwrap_or_else(|| ty_name.clone());
        // Channels: one per PDO entry (digital: 1 bit entries).
        let pdo = |dir: &str| -> (u32, u32, bool) {
            let entries: Vec<&Element> = dev.children(dir).flat_map(|p| p.children("Entry")).collect();
            let bits = |e: &&Element| e.child("BitLen").and_then(|b| b.text.trim().parse::<u32>().ok()).unwrap_or(0);
            let used: Vec<&Element> = entries.iter().copied().filter(|e| e.child("Index").is_some_and(|i| i.text.trim() != "#x0" && i.text.trim() != "0")).collect();
            let total: u32 = entries.iter().map(bits).sum();
            let digital = used.iter().all(|e| bits(e) == 1);
            (used.len() as u32, total.div_ceil(8), digital)
        };
        let (ic, ib, idig) = pdo("TxPdo");
        let (oc, ob, odig) = pdo("RxPdo");
        let lower = format!("{ty_name} {name}").to_lowercase();
        let kind = match (ic > 0, oc > 0) {
            (true, true) if idig && odig => ModuleKind::DigitalMixed,
            (true, true) => ModuleKind::Other,
            (true, false) if idig => ModuleKind::DigitalInput,
            (true, false) => ModuleKind::AnalogInput,
            (false, true) if odig => ModuleKind::DigitalOutput,
            (false, true) => ModuleKind::AnalogOutput,
            _ if lower.contains("coupler") || ty_name.starts_with("EK") => ModuleKind::Coupler,
            _ => ModuleKind::Other,
        };
        let physics = dev.attr("Physics").unwrap_or("YY");
        let ports = if kind == ModuleKind::Coupler { physics.len().max(2) } else { 0 };
        let port_names = ["IN", "OUT", "X3", "X4"];
        v.push(DeviceDescription {
            id: ty_name.clone(),
            name,
            manufacturer: vendor.clone(),
            order_number: ty_name,
            kind,
            inputs: ic,
            outputs: oc,
            input_bytes: ib,
            output_bytes: ob,
            slots: if kind == ModuleKind::Coupler { 64 } else { 0 },
            ports: port_names.iter().take(ports).map(|p| DescPort { name: p.to_string(), bus: BusKind::EtherCat }).collect(),
            source: file.to_string(),
        });
    }
    v
}

#[cfg(test)]
mod tests;
