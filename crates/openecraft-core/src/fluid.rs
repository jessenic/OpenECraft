//! Fluid connections: hoses, tubes and pipes between the ports of fluid power and
//! process symbols. They form their own nets ([`crate::netlist::Net::fluid`]); each
//! point-to-point connection is a hose with a medium, a nominal size, a colour, a
//! length and a configuration (line type, part, end fittings, pressure rating), listed
//! in the hose list. The fluid netlist classifies the ports (supply, working, exhaust,
//! pilot …) and the lines they form; tubes between devices placed in the same
//! enclosure are routed through its ducts like wires.

use openecraft_i18n::{tr, tr_fmt};
use crate::model::*;
use crate::netlist::{chained_pins, pin_designation, Net, Netlist, PinRef};
use crate::issue::Severity;
use crate::symbols::{Category, SymbolDef};
use std::collections::{BTreeMap, BTreeSet};
pub use crate::model::fluid::{FluidSettings, HoseConfig, LineType, Medium};
pub use crate::symbols::PortKind;

impl Medium {
    pub const ALL: [Medium; 11] = [
        Medium::CompressedAir,
        Medium::Vacuum,
        Medium::HydraulicOil,
        Medium::Water,
        Medium::HotWater,
        Medium::HeatingWater,
        Medium::Coolant,
        Medium::Lubricant,
        Medium::Gas,
        Medium::Steam,
        Medium::Product,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Medium::CompressedAir => tr("Compressed air"),
            Medium::Vacuum => tr("Vacuum"),
            Medium::HydraulicOil => tr("Hydraulic oil"),
            Medium::Water => tr("Water"),
            Medium::HotWater => tr("Hot water"),
            Medium::HeatingWater => tr("Heating water"),
            Medium::Coolant => tr("Coolant"),
            Medium::Lubricant => tr("Lubricant"),
            Medium::Gas => tr("Gas"),
            Medium::Steam => tr("Steam"),
            Medium::Product => tr("Product"),
        }
    }

    /// Name in files and on the command line.
    pub fn code(self) -> &'static str {
        match self {
            Medium::CompressedAir => "compressed_air",
            Medium::Vacuum => "vacuum",
            Medium::HydraulicOil => "hydraulic_oil",
            Medium::Water => "water",
            Medium::HotWater => "hot_water",
            Medium::HeatingWater => "heating_water",
            Medium::Coolant => "coolant",
            Medium::Lubricant => "lubricant",
            Medium::Gas => "gas",
            Medium::Steam => "steam",
            Medium::Product => "product",
        }
    }

    pub fn from_code(code: &str) -> Option<Medium> {
        let c = code.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Medium::ALL.into_iter().find(|m| m.code() == c)
    }

    /// Exhausts may stay open to the atmosphere.
    pub fn is_pneumatic(self) -> bool {
        matches!(self, Medium::CompressedAir | Medium::Vacuum)
    }
}

impl LineType {
    pub const ALL: [LineType; 3] = [LineType::Hose, LineType::Tube, LineType::Pipe];

    pub fn title(self) -> &'static str {
        match self {
            LineType::Hose => tr("Hose"),
            LineType::Tube => tr("Tube"),
            LineType::Pipe => tr("Pipe"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            LineType::Hose => "hose",
            LineType::Tube => "tube",
            LineType::Pipe => "pipe",
        }
    }

    pub fn from_code(code: &str) -> Option<LineType> {
        let c = code.trim().to_ascii_lowercase();
        LineType::ALL.into_iter().find(|t| t.code() == c)
    }
}

impl HoseConfig {
    pub fn is_default(&self) -> bool {
        *self == HoseConfig::default()
    }
}

impl Default for FluidSettings {
    fn default() -> Self {
        FluidSettings {
            medium: Medium::CompressedAir,
            size: "8 mm".into(),
            color: "BU".into(),
            prefix: "H".into(),
            pressure: 0.0,
            allowance: 0.0,
            groups: Vec::new(),
            links: BTreeMap::new(),
        }
    }
}

impl FluidSettings {
    pub fn is_default(&self) -> bool {
        *self == FluidSettings::default()
    }
}

impl PortKind {
    pub const ALL: [PortKind; 10] = [Self::Supply, Self::Pressure, Self::Working, Self::Exhaust, Self::Return, Self::Pilot, Self::Suction, Self::Leakage, Self::Measuring, Self::Passage];

    pub fn title(self) -> &'static str {
        match self {
            PortKind::Supply => tr("supply"),
            PortKind::Pressure => tr("pressure"),
            PortKind::Working => tr("working"),
            PortKind::Exhaust => tr("exhaust"),
            PortKind::Return => tr("return"),
            PortKind::Pilot => tr("pilot"),
            PortKind::Suction => tr("suction"),
            PortKind::Leakage => tr("leakage"),
            PortKind::Measuring => tr("measuring"),
            PortKind::Passage => tr("in-line"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            PortKind::Supply => "supply",
            PortKind::Pressure => "pressure",
            PortKind::Working => "working",
            PortKind::Exhaust => "exhaust",
            PortKind::Return => "return",
            PortKind::Pilot => "pilot",
            PortKind::Suction => "suction",
            PortKind::Leakage => "leakage",
            PortKind::Measuring => "measuring",
            PortKind::Passage => "in_line",
        }
    }
}

/// What a fluid net is, from the ports it connects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LineKind {
    Pressure,
    Working,
    Return,
    Pilot,
    Suction,
    Leakage,
    Process,
    Other,
}

impl LineKind {
    pub fn title(self) -> &'static str {
        match self {
            LineKind::Pressure => tr("Pressure line"),
            LineKind::Working => tr("Working line"),
            LineKind::Return => tr("Return / exhaust line"),
            LineKind::Pilot => tr("Pilot line"),
            LineKind::Suction => tr("Suction line"),
            LineKind::Leakage => tr("Leakage line"),
            LineKind::Process => tr("Process line"),
            LineKind::Other => tr("Other"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            LineKind::Pressure => "pressure",
            LineKind::Working => "working",
            LineKind::Return => "return",
            LineKind::Pilot => "pilot",
            LineKind::Suction => "suction",
            LineKind::Leakage => "leakage",
            LineKind::Process => "process",
            LineKind::Other => "other",
        }
    }

    fn of(kinds: &[PortKind], process: bool) -> LineKind {
        let has = |k: PortKind| kinds.contains(&k);
        if has(PortKind::Supply) || has(PortKind::Pressure) {
            LineKind::Pressure
        } else if has(PortKind::Pilot) {
            LineKind::Pilot
        } else if has(PortKind::Working) {
            LineKind::Working
        } else if has(PortKind::Exhaust) || has(PortKind::Return) {
            LineKind::Return
        } else if has(PortKind::Suction) {
            LineKind::Suction
        } else if has(PortKind::Leakage) {
            LineKind::Leakage
        } else if process {
            LineKind::Process
        } else {
            LineKind::Other
        }
    }
}

/// Function of the port `label` of a fluid symbol.
pub fn port_kind(def: &SymbolDef, label: &str) -> PortKind {
    if def.category == Category::Process {
        return PortKind::Passage;
    }
    match def.key {
        "fl_source" => return PortKind::Supply,
        "fl_pump" | "fl_compressor" => return if label == "S" { PortKind::Suction } else { PortKind::Supply },
        "fl_hyd_motor" => return PortKind::Working,
        "fl_accumulator" => return PortKind::Pressure,
        "fl_check" | "fl_shutoff" | "fl_flow_valve" | "fl_regulator" | "fl_frl" | "fl_cooler" | "fl_receiver" | "fl_dryer" | "fl_lubricator" => return PortKind::Passage,
        "fl_coupling" => return PortKind::Working,
        "fl_tank" => return PortKind::Return,
        "fl_silencer" => return PortKind::Exhaust,
        "fl_gauge" => return PortKind::Measuring,
        "fl_filter" | "fl_throttle" => return PortKind::Passage,
        "fl_cylinder" | "fl_cylinder_sa" => return PortKind::Working,
        _ => {}
    }
    match label.trim().to_ascii_uppercase().as_str() {
        "P" | "1" => PortKind::Pressure,
        "A" | "B" | "2" | "4" => PortKind::Working,
        "R" | "S" | "3" | "5" => PortKind::Exhaust,
        "T" => PortKind::Return,
        "X" | "Y" | "Z" | "12" | "14" => PortKind::Pilot,
        "L" => PortKind::Leakage,
        _ => PortKind::Passage,
    }
}

/// Function of a connected pin, if it is a port of a fluid symbol.
pub fn kind_of_pin(prj: &Project, pin: PinRef) -> Option<PortKind> {
    let s = &prj.pages[pin.page].symbols[pin.sym];
    let def = prj.symbol(&s.sym).filter(|d| d.is_fluid())?;
    Some(def.fluid_ports.get(&pin.pin).copied().unwrap_or_else(|| port_kind(&def, s.pins.get(pin.pin).map(String::as_str).unwrap_or(""))))
}

/// A port in the fluid netlist.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub pin: PinRef,
    pub designation: String,
    pub kind: PortKind,
}

/// A fluid net with its ports: one hose line (with branches when it connects more
/// than two ports).
#[derive(Clone, Debug, PartialEq)]
pub struct FluidNet {
    /// Index into the nets of the netlist.
    pub index: usize,
    pub net: String,
    pub number: String,
    pub line: LineKind,
    pub medium: Medium,
    pub size: String,
    pub ports: Vec<Port>,
}

impl FluidNet {
    /// Hose number, otherwise the net name.
    pub fn name(&self) -> &str {
        if self.number.is_empty() { &self.net } else { &self.number }
    }

    /// Tees / distributors needed to connect the ports.
    pub fn branches(&self) -> usize {
        self.ports.len().saturating_sub(2)
    }
}

/// The fluid netlist: all fluid nets with wires, in schematic order.
pub fn fluid_nets(prj: &Project, open: &Netlist) -> Vec<FluidNet> {
    let mut out = Vec::new();
    for (index, net) in open.nets.iter().enumerate().filter(|(_, n)| n.fluid && !n.mixed && !n.wires.is_empty()) {
        let ports: Vec<Port> = chained_pins(prj, net)
            .into_iter()
            .filter_map(|pin| Some(Port { pin, designation: pin_designation(prj, pin), kind: kind_of_pin(prj, pin)? }))
            .collect();
        let kinds: Vec<PortKind> = ports.iter().map(|p| p.kind).collect();
        let process = net.pins.iter().any(|p| prj.symbol(&prj.pages[p.page].symbols[p.sym].sym).is_some_and(|d| d.category == Category::Process));
        let (medium, size, _, _) = props_of(prj, net);
        out.push(FluidNet { index, net: net.name.clone(), number: crate::wirenum::number_of(prj, net), line: LineKind::of(&kinds, process), medium, size, ports });
    }
    out
}

/// Routes a hose between two placed devices through the ducts (like the virtual
/// wiring): on the plate, from the door or the walls, and into bayed enclosures
/// through the cable passages. Devices of a placed device group (valve terminal) are
/// reached at the group; hoses inside one group are part of its base (none).
pub fn route(prj: &Project, a: PinRef, b: PinRef) -> Option<TubeRoute> {
    use crate::panel;
    let tag = |p: PinRef| &prj.pages[p.page].symbols[p.sym].tag;
    let (ta, tb) = (tag(a), tag(b));
    if ta == tb {
        return None;
    }
    let (ea, ia) = panel::locate(prj, ta)?;
    let (eb, ib) = panel::locate(prj, tb)?;
    if ia.id == ib.id {
        return None;
    }
    let encs = &prj.layout.enclosures;
    let (wa, la, _) = panel::wiring_end(&encs[ea], ia, None);
    let (wb, lb, _) = panel::wiring_end(&encs[eb], ib, None);
    let bends = |r: &panel::Route| r.points.len().saturating_sub(2);
    let (plate, n, enclosure) = if ea == eb {
        let r = panel::route_ends(&encs[ea], wa, wb);
        (r.plate_length, bends(&r), encs[ea].name.clone())
    } else {
        let pos = panel::positions(&prj.layout);
        let (r, legs) = panel::route_across(&prj.layout, &pos, (ea, wa), (eb, wb)).ok()?;
        let plate = r.plate_length + legs.iter().map(|l| l.route.plate_length + l.passage_length).sum::<f32>();
        let names: Vec<&str> = std::iter::once(ea).chain(legs.iter().map(|l| l.enclosure)).map(|k| encs[k].name.as_str()).collect();
        (plate, bends(&r) + legs.iter().map(|l| bends(&l.route) + 2).sum::<usize>(), names.join(" / "))
    };
    let mm = plate + la + lb + prj.layout.wire_reserve;
    Some(TubeRoute { enclosure, length: mm / 1000.0, bends: n })
}

impl Hose {
    /// The set length, otherwise the routed one (metres, 0 = unknown).
    pub fn effective_length(&self) -> f32 {
        if self.length > 0.0 { self.length } else { self.routed.as_ref().map(|r| r.length).unwrap_or(0.0) }
    }

    /// Length to cut: the effective length plus the allowance at both ends (mm).
    pub fn cut_length(&self, allowance: f32) -> f32 {
        let l = self.effective_length();
        if l > 0.0 { l + 2.0 * allowance / 1000.0 } else { 0.0 }
    }

    pub fn name(&self) -> &str {
        if self.number.is_empty() { &self.net } else { &self.number }
    }
}

/// Medium, size, colour and length of a fluid net: the first value set on its wires,
/// otherwise the project's defaults.
pub fn props_of(prj: &Project, net: &Net) -> (Medium, String, String, f32) {
    let wires = crate::wirenum::wires_of(prj, net);
    let medium = wires.iter().find_map(|w| w.medium).unwrap_or(prj.fluid.medium);
    let pick = |f: &dyn Fn(&Wire) -> &String, default: &String| wires.iter().map(|w| f(w)).find(|v| !v.is_empty()).unwrap_or(default).clone();
    let size = pick(&|w| &w.cross_section, &prj.fluid.size);
    let color = pick(&|w| &w.color, &prj.fluid.color);
    let length = wires.iter().map(|w| w.length).find(|l| *l > 0.0).unwrap_or(0.0);
    (medium, size, color, length)
}

/// Configuration of a fluid net: the first one set on its wires.
pub fn config_of(prj: &Project, net: &Net) -> HoseConfig {
    crate::wirenum::wires_of(prj, net).iter().find_map(|w| w.hose.clone()).unwrap_or_default()
}

/// All hoses of the project in schematic order (`open`: netlist with the terminal
/// sides apart).
pub fn hoses(prj: &Project, open: &Netlist) -> Vec<Hose> {
    let mut out = Vec::new();
    for net in open.nets.iter().filter(|n| n.fluid && !n.mixed) {
        let pins = chained_pins(prj, net);
        let (medium, size, color, length) = props_of(prj, net);
        let config = config_of(prj, net);
        let number = crate::wirenum::number_of(prj, net);
        for pair in pins.windows(2) {
            out.push(Hose {
                number: number.clone(),
                net: net.name.clone(),
                a: pair[0],
                b: pair[1],
                from: pin_designation(prj, pair[0]),
                to: pin_designation(prj, pair[1]),
                medium,
                size: size.clone(),
                color: color.clone(),
                length,
                config: config.clone(),
                routed: route(prj, pair[0], pair[1]),
            });
        }
    }
    out
}

/// Material of the hoses, tubes and pipes: one entry per line type, size, part and
/// medium.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Tubing {
    pub line: LineType,
    pub size: String,
    pub part: String,
    pub medium: Medium,
    pub pieces: usize,
    /// Sum of the cut lengths in metres.
    pub length: f32,
    pub bends: usize,
    /// Fitting → count.
    pub fittings: BTreeMap<String, usize>,
}

pub fn tubing(prj: &Project, hoses: &[Hose]) -> Vec<Tubing> {
    let mut map: BTreeMap<(LineType, String, String, Medium), Tubing> = BTreeMap::new();
    for h in hoses {
        let c = &h.config;
        let t = map.entry((c.line, h.size.clone(), c.part.clone(), h.medium)).or_insert_with(|| Tubing {
            line: c.line,
            size: h.size.clone(),
            part: c.part.clone(),
            medium: h.medium,
            ..Default::default()
        });
        t.pieces += 1;
        t.length += h.cut_length(prj.fluid.allowance);
        t.bends += h.routed.as_ref().map(|r| r.bends).unwrap_or(0);
        for f in [&c.fitting_a, &c.fitting_b].into_iter().filter(|f| !f.trim().is_empty()) {
            *t.fittings.entry(f.trim().to_string()).or_default() += 1;
        }
    }
    let mut v: Vec<Tubing> = map.into_values().collect();
    v.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| natural_cmp(&a.size, &b.size)).then_with(|| a.part.cmp(&b.part)));
    v
}

/// Exhaust port of a pneumatic device that is not connected: it vents to the
/// atmosphere, which is no open connection point.
pub fn vents(prj: &Project, pin: PinRef) -> bool {
    prj.fluid.medium.is_pneumatic() && kind_of_pin(prj, pin) == Some(PortKind::Exhaust)
}

fn joined<T: Ord>(set: &BTreeSet<T>, f: impl Fn(&T) -> String) -> String {
    set.iter().map(f).collect::<Vec<_>>().join(", ")
}

/// Fluid problems: ports connected to electrical connection points, pressure lines
/// short-circuited to the return, different media or sizes on one hose or one
/// device, hoses rated below the operating pressure or shorter than their route,
/// hoses without length and exhausts open to the atmosphere.
pub fn problems(prj: &Project, open: &Netlist) -> Vec<Problem> {
    let mut v = Vec::new();
    let target_of = |p: PinRef| Some((p.page, prj.pages[p.page].symbols[p.sym].id));
    for net in open.nets.iter().filter(|n| n.mixed) {
        let names: Vec<String> = net.pins.iter().map(|p| pin_designation(prj, *p)).collect();
        let target = net.wires.first().copied();
        v.push((Severity::Error, tr_fmt!("Fluid port connected to an electrical connection point: {}", names.join(", ")), target));
    }
    let nets = fluid_nets(prj, open);
    for fnet in &nets {
        let net = &open.nets[fnet.index];
        let target = fnet.ports.first().and_then(|p| target_of(p.pin)).or(net.wires.first().copied());
        let ports = |pred: &dyn Fn(PortKind) -> bool| fnet.ports.iter().filter(|p| pred(p.kind)).map(|p| p.designation.clone()).collect::<Vec<_>>();
        let pressure = ports(&|k| matches!(k, PortKind::Supply | PortKind::Pressure));
        let back = ports(&|k| matches!(k, PortKind::Exhaust | PortKind::Return));
        if !pressure.is_empty() && !back.is_empty() {
            v.push((Severity::Error, tr_fmt!("Hose {}: pressure line {} connected to return / exhaust {}", fnet.name(), pressure.join(", "), back.join(", ")), target));
        }
        let wires = crate::wirenum::wires_of(prj, net);
        let media: BTreeSet<Medium> = wires.iter().filter_map(|w| w.medium).collect();
        if media.len() > 1 {
            v.push((Severity::Warning, tr_fmt!("Hose {}: different media on its wires ({})", fnet.name(), joined(&media, |m| m.title().to_string())), target));
        }
        let sizes: BTreeSet<&str> = wires.iter().map(|w| w.cross_section.trim()).filter(|s| !s.is_empty()).collect();
        if sizes.len() > 1 {
            v.push((Severity::Warning, tr_fmt!("Hose {}: different nominal sizes on its wires ({})", fnet.name(), joined(&sizes, |s| s.to_string())), target));
        }
        let config = config_of(prj, net);
        if prj.fluid.pressure > 0.0 && config.pressure > 0.0 && config.pressure < prj.fluid.pressure && fnet.medium != Medium::Vacuum {
            v.push((
                Severity::Error,
                tr_fmt!("Hose {}: rated for {} bar, operating pressure {} bar", fnet.name(), config.pressure, prj.fluid.pressure),
                target,
            ));
        }
    }
    for h in hoses(prj, open) {
        let target = target_of(h.a);
        match &h.routed {
            Some(r) if h.length > 0.0 && h.length + 0.0005 < r.length => v.push((
                Severity::Warning,
                tr_fmt!("Hose {} – {}: length {} m shorter than the routed {} m", h.from, h.to, format!("{:.2}", h.length), format!("{:.2}", r.length)),
                target,
            )),
            _ if h.effective_length() <= 0.0 => v.push((Severity::Info, tr_fmt!("Hose {} – {} without length", h.from, h.to), target)),
            _ => {}
        }
    }
    // Media of the ports of each fluid power device (pilot ports may differ).
    let mut device_media: BTreeMap<&str, (BTreeSet<Medium>, PinRef)> = BTreeMap::new();
    for fnet in &nets {
        for p in fnet.ports.iter().filter(|p| p.kind != PortKind::Pilot) {
            let s = &prj.pages[p.pin.page].symbols[p.pin.sym];
            if prj.symbol(&s.sym).is_some_and(|d| d.category == Category::Fluid) {
                let e = device_media.entry(s.tag.as_str()).or_insert_with(|| (BTreeSet::new(), p.pin));
                e.0.insert(fnet.medium);
            }
        }
    }
    for (tag, (media, pin)) in device_media {
        if media.len() > 1 && !tag.is_empty() {
            v.push((Severity::Warning, tr_fmt!("{}: ports with different media ({})", tag, joined(&media, |m| m.title().to_string())), target_of(pin)));
        }
    }
    let mut vented: Vec<_> = open.unconnected.clone();
    vented.sort();
    for (pi, sid, pin) in vented {
        let Some(si) = prj.pages[pi].symbols.iter().position(|s| s.id == sid) else { continue };
        let pr = PinRef { page: pi, sym: si, pin };
        if vents(prj, pr) {
            v.push((Severity::Info, tr_fmt!("Exhaust {} open to the atmosphere (no silencer)", pin_designation(prj, pr)), Some((pi, sid))));
        }
    }
    v
}

/// Length text for reports.
pub fn length_text(length: f32) -> String {
    if length > 0.0 { format!("{length:.2}") } else { "–".into() }
}

/// (severity, message, jump target)
pub type Problem = (Severity, String, Option<(usize, u64)>);

/// A hose (or tube, pipe) between two ports.
#[derive(Clone, Debug, PartialEq)]
pub struct Hose {
    pub number: String,
    /// Fluid net (`F1` …).
    pub net: String,
    pub a: PinRef,
    pub b: PinRef,
    pub from: String,
    pub to: String,
    pub medium: Medium,
    pub size: String,
    pub color: String,
    /// Metres set on the wires; 0 = not set.
    pub length: f32,
    pub config: HoseConfig,
    /// Route through the panel layout.
    pub routed: Option<TubeRoute>,
}

/// Way of a tube through the ducts of an enclosure (or of the enclosures of a
/// line-up, through their cable passages).
#[derive(Clone, Debug, PartialEq)]
pub struct TubeRoute {
    /// Enclosure name; "+CAB1 / +CAB2" across bayed enclosures.
    pub enclosure: String,
    /// Metres, including the way to the plate and the reserve.
    pub length: f32,
    pub bends: usize,
}

#[cfg(test)]
mod tests;
