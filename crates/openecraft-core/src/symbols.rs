//! Symbol libraries: built-in IEC, NFPA / JIC, fluid power, single-line and black
//! box symbols, plus the user symbols of the project ([`crate::symlib`]).
//!
//! Symbols are defined in local millimetre coordinates in vertical orientation
//! (current flows top → bottom). Connection points lie on the 4 mm grid relative to
//! the symbol origin so that placed symbols connect cleanly to wires.

use openecraft_i18n::tr;
use crate::geom::{p, P, R};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

mod iec;
mod national;
pub use iec::kind as iec_kind;
pub use national::iec_equivalent;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactKind {
    No,
    Nc,
    Changeover,
    MainNo,
    /// Time-delayed contacts (timer relays): delayed when the coil is energized
    /// (on-delay) or de-energized (off-delay).
    NoOnDelay,
    NcOnDelay,
    NoOffDelay,
    NcOffDelay,
}

impl ContactKind {
    /// Contact without the delay: NO, NC, changeover or main contact.
    pub fn base(self) -> ContactKind {
        match self {
            ContactKind::NoOnDelay | ContactKind::NoOffDelay => ContactKind::No,
            ContactKind::NcOnDelay | ContactKind::NcOffDelay => ContactKind::Nc,
            k => k,
        }
    }

    /// `Some(true)` = on-delay, `Some(false)` = off-delay.
    pub fn delay(self) -> Option<bool> {
        match self {
            ContactKind::NoOnDelay | ContactKind::NcOnDelay => Some(true),
            ContactKind::NoOffDelay | ContactKind::NcOffDelay => Some(false),
            _ => None,
        }
    }
}

/// Side of a plug pin symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinKind {
    /// Mated pin and socket: socket side (connection point 1) and pin side (2).
    Pair,
    Male,
    Female,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Func {
    /// Main function of a device. `mirror` = draws a contact image (coils, overloads).
    Main { mirror: bool },
    /// Auxiliary function, belongs to the main function with the same tag.
    Aux(ContactKind),
    /// Additional device function without a contact classification.
    Auxiliary,
    /// Local connection node; its internal bridges define the connected paths.
    Junction,
    /// Terminal: tag = terminal strip, label = terminal number.
    Terminal,
    /// PLC connection point: tag = PLC card, label = address.
    Plc,
    /// Plug pin: tag = plug, label = pin designation. Male and female pins of the
    /// same designation are mated.
    Plug(PinKind),
    /// Potential connection point: label = potential name. No device tag.
    Potential,
    /// Interruption point (page connector): label = name. No device tag.
    Interruption,
    /// Single-line / overview representation of a device that is drawn in detail
    /// elsewhere: shows cross references to the main function, its connection
    /// points are not part of the netlist.
    Overview,
    /// Drawing without a device identity or electrical connection points.
    Graphic,
}

impl Func {
    pub fn has_tag(self) -> bool {
        !matches!(self, Func::Potential | Func::Interruption | Func::Junction | Func::Graphic)
    }

    /// Terminals and plug pins: their sides are separate connections that the
    /// potential passes through.
    pub fn is_feedthrough(self) -> bool {
        matches!(self, Func::Terminal | Func::Plug(_))
    }

    /// All functions a symbol can have (symbol editor).
    pub const ALL: [Func; 21] = [
        Func::Main { mirror: false },
        Func::Main { mirror: true },
        Func::Aux(ContactKind::No),
        Func::Aux(ContactKind::Nc),
        Func::Aux(ContactKind::Changeover),
        Func::Aux(ContactKind::MainNo),
        Func::Aux(ContactKind::NoOnDelay),
        Func::Aux(ContactKind::NcOnDelay),
        Func::Aux(ContactKind::NoOffDelay),
        Func::Aux(ContactKind::NcOffDelay),
        Func::Terminal,
        Func::Plc,
        Func::Plug(PinKind::Pair),
        Func::Plug(PinKind::Male),
        Func::Plug(PinKind::Female),
        Func::Potential,
        Func::Interruption,
        Func::Overview,
        Func::Auxiliary,
        Func::Junction,
        Func::Graphic,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Func::Main { mirror: false } => tr("Main function"),
            Func::Main { mirror: true } => tr("Main function with contact image"),
            Func::Aux(ContactKind::No) => tr("Auxiliary function: NO contact"),
            Func::Aux(ContactKind::Nc) => tr("Auxiliary function: NC contact"),
            Func::Aux(ContactKind::Changeover) => tr("Auxiliary function: changeover"),
            Func::Aux(ContactKind::MainNo) => tr("Auxiliary function: main contact"),
            Func::Aux(ContactKind::NoOnDelay) => tr("Auxiliary function: NO contact, on-delay"),
            Func::Aux(ContactKind::NcOnDelay) => tr("Auxiliary function: NC contact, on-delay"),
            Func::Aux(ContactKind::NoOffDelay) => tr("Auxiliary function: NO contact, off-delay"),
            Func::Aux(ContactKind::NcOffDelay) => tr("Auxiliary function: NC contact, off-delay"),
            Func::Terminal => tr("Terminal"),
            Func::Plc => tr("PLC connection point"),
            Func::Plug(PinKind::Pair) => tr("Plug: pin and socket"),
            Func::Plug(PinKind::Male) => tr("Plug: male pin"),
            Func::Plug(PinKind::Female) => tr("Plug: female socket"),
            Func::Potential => tr("Potential"),
            Func::Interruption => tr("Interruption point"),
            Func::Overview => tr("Overview / single-line"),
            Func::Auxiliary => tr("Auxiliary function"),
            Func::Junction => tr("Junction"),
            Func::Graphic => tr("Graphic (no electrical function)"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Coils,
    Contacts,
    Switches,
    Protection,
    Loads,
    Terminals,
    Plugs,
    Plc,
    Potentials,
    Passive,
    /// Electronic sensors (proximity, photoelectric) and controllers.
    Sensors,
    Measuring,
    /// Transformers, converters and rectifiers.
    Power,
    BlackBox,
    SingleLine,
    Nfpa,
    Fluid,
    /// Piping and instrumentation diagrams (ISO 10628, ISA 5.1).
    Process,
    /// Building installation plans (IEC 60617-11).
    Installation,
    /// China (GB/T 4728, device letters of GB/T 7159).
    Gb,
    /// Russia and the CIS (ESKD, device letters of GOST 2.710).
    Gost,
    /// Japan (JIS C 0617, device letters of JIS C 0401).
    Jis,
    /// Symbols of the project symbol library (symbol editor).
    User,
}

impl Category {
    /// Categories of the built-in libraries.
    pub const ALL: [Category; 22] = [
        Category::Coils,
        Category::Contacts,
        Category::Switches,
        Category::Sensors,
        Category::Protection,
        Category::Loads,
        Category::Measuring,
        Category::Power,
        Category::Terminals,
        Category::Plugs,
        Category::Plc,
        Category::Potentials,
        Category::Passive,
        Category::BlackBox,
        Category::SingleLine,
        Category::Nfpa,
        Category::Fluid,
        Category::Process,
        Category::Installation,
        Category::Gb,
        Category::Gost,
        Category::Jis,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Coils => tr("Coils / Relays"),
            Category::Contacts => tr("Contacts"),
            Category::Switches => tr("Switches / Sensors"),
            Category::Protection => tr("Protective devices"),
            Category::Loads => tr("Motors / Loads"),
            Category::Terminals => tr("Terminals"),
            Category::Plugs => tr("Plugs"),
            Category::Plc => tr("PLC"),
            Category::Potentials => tr("Potentials / Connectors"),
            Category::Passive => tr("Passive components"),
            Category::Sensors => tr("Sensors"),
            Category::Measuring => tr("Measuring instruments"),
            Category::Power => tr("Transformers / power supplies"),
            Category::BlackBox => tr("Black boxes / PLC boxes"),
            Category::SingleLine => tr("Single-line / overview"),
            Category::Nfpa => tr("NFPA / JIC"),
            Category::Fluid => tr("Fluid power (ISO 1219)"),
            Category::Process => tr("P&ID / process (ISO 10628)"),
            Category::Installation => tr("Installation plan (IEC 60617-11)"),
            Category::Gb => tr("GB (GB/T 4728)"),
            Category::Gost => tr("GOST (ESKD)"),
            Category::Jis => tr("JIS (JIS C 0617)"),
            Category::User => tr("User symbols"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldKind {
    Label,
    Tag,
    Pin(usize),
    TagName,
}

/// Connectivity domain of an imported or user-defined symbol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolDomain {
    #[default]
    Electrical,
    Fluid,
    Process,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Prim {
    /// Embedded bitmap with local corners in texture order (top left clockwise).
    Image { name: String, data: openecraft_parts::Shared<String>, corners: [P; 4] },
    Line { a: P, b: P, dashed: bool },
    Poly { pts: Vec<P>, closed: bool, fill: bool },
    /// Vector path with an explicit stroke from a symbol library.
    Path { pts: Vec<P>, closed: bool, fill: bool, color: Option<[u8;3]>, width: f32, dashed: bool },
    Circle { c: P, r: f32, fill: bool, #[serde(default, skip_serializing_if = "Option::is_none")] color: Option<[u8;3]> },
    Text { pos: P, text: String, size: f32, anchor: Anchor, #[serde(default, skip_serializing_if = "Option::is_none")] angle: Option<f32>, #[serde(default, skip_serializing_if = "Option::is_none")] color: Option<[u8;3]> },
    /// Instance property drawn at this position (stays horizontal when rotated).
    Field { kind: FieldKind, pos: P, size: f32, anchor: Anchor, #[serde(default, skip_serializing_if = "Option::is_none")] angle: Option<f32> },
}

#[derive(Clone, Debug)]
pub struct PinDef {
    pub pos: P,
    pub label: &'static str,
    pub internal: bool,
}

/// A single channel represented by a vendor macro. The channel index is
/// zero based and independent of its physical terminal designation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlcChannel {
    pub family: String,
    pub input: bool,
    pub index: u16,
    pub designation: String,
    pub address: String,
    pub function_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analog: Option<AnalogSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digital_terminal: Option<String>,
}

impl PlcChannel {
    pub fn signal_terminal(&self) -> Option<&str> {
        self.digital_terminal.as_deref().or_else(|| self.analog.as_ref().map(|a| a.terminal.as_str()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnalogSignal {
    /// Authored physical signal terminal, independent of the order of the pins.
    pub terminal: String,
    pub data_type: String,
}

#[derive(Clone, Debug)]
pub struct SymbolDef {
    pub key: &'static str,
    pub name: &'static str,
    pub category: Category,
    /// Library of a user symbol (group in the symbol list); empty for built-in ones.
    pub library: &'static str,
    /// Default identifying letter (IEC 81346 style).
    pub letter: &'static str,
    pub func: Func,
    pub plc_channel: Option<PlcChannel>,
    pub prims: Vec<Prim>,
    pub pins: Vec<PinDef>,
    /// Pins that are internally connected (terminals).
    pub bridges: Vec<(usize, usize)>,
    pub default_label: &'static str,
    /// Contact numbering (13/14, 21/22, ...) is assigned automatically.
    pub auto_number: bool,
    /// Draw tag / function text next to the symbol.
    pub show_tag: bool,
    pub show_pin_labels: bool,
    pub bounds: R,
}

impl SymbolDef {
    pub fn digital_input(&self) -> Option<bool> {
        match self.key {
            "plc_in" => Some(true),
            "plc_out" => Some(false),
            _ => self.plc_channel.as_ref().filter(|c| c.analog.is_none()).map(|c| c.input),
        }
    }

    pub fn pin_world(&self, idx: usize, pos: P, rot: u8, mirror: bool) -> P {
        pos + self.pins[idx].pos.transform(rot, mirror)
    }

    pub fn world_bounds(&self, pos: P, rot: u8, mirror: bool) -> R {
        let corners = [
            self.bounds.min,
            p(self.bounds.max.x, self.bounds.min.y),
            self.bounds.max,
            p(self.bounds.min.x, self.bounds.max.y),
        ];
        let mut r = R::nothing();
        for c in corners {
            r.extend(pos + c.transform(rot, mirror));
        }
        r
    }

    /// Fluid power and process symbols: their connection points are ports of hoses
    /// and pipes, which form fluid nets apart from the electrical ones.
    pub fn is_fluid(&self) -> bool {
        matches!(self.category, Category::Fluid | Category::Process)
    }

    pub fn contact_kind(&self) -> Option<ContactKind> {
        match self.func {
            Func::Aux(k) => Some(k),
            _ => None,
        }
    }
}

struct B {
    prims: Vec<Prim>,
    pins: Vec<PinDef>,
}

impl B {
    fn new() -> B {
        B { prims: Vec::new(), pins: Vec::new() }
    }
    fn l(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut Self {
        self.prims.push(Prim::Line { a: p(x1, y1), b: p(x2, y2), dashed: false });
        self
    }
    fn dl(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut Self {
        self.prims.push(Prim::Line { a: p(x1, y1), b: p(x2, y2), dashed: true });
        self
    }
    /// Dashed rectangle (black box outline).
    fn drect(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut Self {
        self.dl(x1, y1, x2, y1).dl(x2, y1, x2, y2).dl(x2, y2, x1, y2).dl(x1, y2, x1, y1)
    }
    fn rect(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, fill: bool) -> &mut Self {
        self.prims.push(Prim::Poly {
            pts: vec![p(x1, y1), p(x2, y1), p(x2, y2), p(x1, y2)],
            closed: true,
            fill,
        });
        self
    }
    fn poly(&mut self, pts: &[(f32, f32)], closed: bool, fill: bool) -> &mut Self {
        self.prims.push(Prim::Poly {
            pts: pts.iter().map(|&(x, y)| p(x, y)).collect(),
            closed,
            fill,
        });
        self
    }
    fn circle(&mut self, x: f32, y: f32, r: f32, fill: bool) -> &mut Self {
        self.prims.push(Prim::Circle { c: p(x, y), r, fill, color:None });
        self
    }
    /// Arc from angle `a0` to `a1` in degrees (0° = +x, 90° = down).
    fn arc(&mut self, x: f32, y: f32, r: f32, a0: f32, a1: f32) -> &mut Self {
        let n = 12;
        let pts = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                p(x + r * a.cos(), y + r * a.sin())
            })
            .collect();
        self.prims.push(Prim::Poly { pts, closed: false, fill: false });
        self
    }
    fn text(&mut self, x: f32, y: f32, t: &str, size: f32, anchor: Anchor) -> &mut Self {
        self.prims.push(Prim::Text { pos: p(x, y), text: t.into(), size, anchor, angle: None, color: None });
        self
    }
    fn field(&mut self, kind: FieldKind, x: f32, y: f32, size: f32, anchor: Anchor) -> &mut Self {
        self.prims.push(Prim::Field { kind, pos: p(x, y), size, anchor, angle: None });
        self
    }
    fn pin(&mut self, x: f32, y: f32, label: &'static str) -> &mut Self {
        self.pins.push(PinDef { pos: p(x, y), label, internal: false });
        self
    }

    // --- reusable sub-drawings -------------------------------------------------

    /// Normally open contact at x.
    fn no(&mut self, x: f32) -> &mut Self {
        self.l(x, -4.0, x, -2.0).l(x, 2.0, x, 4.0).l(x, 2.0, x - 2.0, -2.4)
    }
    /// Normally closed contact at x.
    fn nc(&mut self, x: f32) -> &mut Self {
        self.l(x, -4.0, x, -2.0)
            .l(x, -2.0, x + 1.8, -2.0)
            .l(x, 2.0, x, 4.0)
            .l(x, 2.0, x + 2.3, -2.6)
    }
    /// Push button actuator (IEC 60617-02-13-05, a bracket open towards the contact),
    /// linked to the contact blade at `bx`.
    fn push_actuator(&mut self, bx: f32) -> &mut Self {
        self.dl(bx, 0.0, -5.0, 0.0)
            .l(-6.0, -1.5, -6.0, 1.5)
            .l(-6.0, -1.5, -5.0, -1.5)
            .l(-6.0, 1.5, -5.0, 1.5)
            .l(-5.0, 0.0, -6.0, 0.0)
    }
    /// Circuit breaker cross at the fixed contact of a pole at x.
    fn breaker_x(&mut self, x: f32) -> &mut Self {
        self.l(x - 0.7, -2.7, x + 0.7, -1.3).l(x - 0.7, -1.3, x + 0.7, -2.7)
    }
    /// Disconnector bar at the fixed contact of a pole at x.
    fn disconnector_bar(&mut self, x: f32) -> &mut Self {
        self.l(x - 0.8, -2.0, x + 0.8, -2.0)
    }
    /// Delay mark of a time-delayed contact, linked to the blade at `bx`: a double
    /// line ending in a semicircle (open to the left for on-delay).
    fn delay(&mut self, bx: f32, on: bool) -> &mut Self {
        self.l(bx, -0.5, -4.0, -0.5).l(bx, 0.5, -4.0, 0.5);
        if on {
            self.arc(-5.5, 0.0, 1.5, 270.0, 450.0)
        } else {
            self.arc(-4.0, 0.0, 1.5, 90.0, 270.0)
        }
    }
    /// Actuator box with a symbol (pressure, temperature …) linked to the blade at `bx`.
    fn sensing_box(&mut self, bx: f32, t: &str) -> &mut Self {
        self.dl(bx, 0.0, -4.0, 0.0).rect(-8.0, -1.5, -4.0, 1.5, false).text(-6.0, 0.0, t, 1.6, Anchor::Center)
    }
}

/// Fuse link along the open switch blade of a pole at x (single-line fuse switch-disconnector).
fn fuse_blade(b: &mut B, x: f32) {
    let (mx, my) = (x - 1.0, -0.2);
    let (ux, uy) = (-0.414, -0.911);
    let (nx, ny) = (0.911, -0.414);
    let c = |s: f32, t: f32| (mx + ux * s + nx * t, my + uy * s + ny * t);
    b.poly(&[c(-1.7, -0.7), c(1.7, -0.7), c(1.7, 0.7), c(-1.7, 0.7)], true, false);
}

struct Def {
    key: &'static str,
    name: &'static str,
    category: Category,
    letter: &'static str,
    func: Func,
    label: &'static str,
    auto_number: bool,
    show_tag: bool,
    show_pin_labels: bool,
    bridges: Vec<(usize, usize)>,
}

fn def(key: &'static str, name: &'static str, category: Category, letter: &'static str, func: Func) -> Def {
    Def {
        key,
        name,
        category,
        letter,
        func,
        label: "",
        auto_number: false,
        show_tag: func.has_tag(),
        show_pin_labels: matches!(func, Func::Main { .. } | Func::Aux(_)),
        bridges: Vec::new(),
    }
}

impl Def {
    fn label(mut self, l: &'static str) -> Self {
        self.label = l;
        self
    }
    fn auto(mut self) -> Self {
        self.auto_number = true;
        self
    }
    fn build(self, b: &mut B) -> SymbolDef {
        let bounds = bounds_of(&b.prims, b.pins.iter().map(|x| x.pos));
        SymbolDef {
            key: self.key,
            name: self.name,
            category: self.category,
            library: "",
            letter: self.letter,
            func: self.func,
            plc_channel: None,
            prims: std::mem::take(&mut b.prims),
            pins: std::mem::take(&mut b.pins),
            bridges: self.bridges,
            default_label: self.label,
            auto_number: self.auto_number,
            show_tag: self.show_tag,
            show_pin_labels: self.show_pin_labels,
            bounds,
        }
    }
}

/// Bounding box of a symbol's graphics and pins (texts are not included).
pub fn bounds_of(prims: &[Prim], pins: impl Iterator<Item = P>) -> R {
    let mut bounds = R::nothing();
    for prim in prims {
        match prim {
            Prim::Line { a, b, .. } => {
                bounds.extend(*a);
                bounds.extend(*b);
            }
            Prim::Poly { pts, .. } | Prim::Path {pts,..} => pts.iter().for_each(|q| bounds.extend(*q)),
            Prim::Image { corners, .. } => corners.iter().for_each(|q| bounds.extend(*q)),
            Prim::Circle { c, r, .. } => {
                bounds.extend(p(c.x - r, c.y - r));
                bounds.extend(p(c.x + r, c.y + r));
            }
            Prim::Text { .. } | Prim::Field { .. } => {}
        }
    }
    for q in pins {
        bounds.extend(q);
    }
    if bounds.min.x > bounds.max.x {
        bounds = R::from_points(p(-1.0, -1.0), p(1.0, 1.0));
    }
    bounds.expand(0.5)
}

const MAIN: Func = Func::Main { mirror: false };
const MAIN_MIRROR: Func = Func::Main { mirror: true };

#[allow(clippy::too_many_lines)]
fn build_library() -> Vec<SymbolDef> {
    use Anchor::*;
    use Category as C;
    let mut v = Vec::new();
    let mut b = B::new();

    // ---------------------------------------------------------------- coils
    b.l(0.0, -4.0, 0.0, -2.0).rect(-3.0, -2.0, 3.0, 2.0, false).l(0.0, 2.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("coil", "Relay / contactor coil", C::Coils, "K", MAIN_MIRROR).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).rect(-3.0, -2.0, 3.0, 2.0, false).l(0.0, 2.0, 0.0, 4.0);
    b.rect(-6.0, -2.0, -3.0, 2.0, false).l(-6.0, -2.0, -3.0, 2.0).l(-6.0, 2.0, -3.0, -2.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("coil_on", "Timer coil, on-delay", C::Coils, "K", MAIN_MIRROR).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).rect(-3.0, -2.0, 3.0, 2.0, false).l(0.0, 2.0, 0.0, 4.0);
    b.rect(-6.0, -2.0, -3.0, 2.0, true);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("coil_off", "Timer coil, off-delay", C::Coils, "K", MAIN_MIRROR).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).rect(-3.0, -2.0, 3.0, 2.0, false).l(-3.0, 2.0, 3.0, -2.0).l(0.0, 2.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("valve", "Solenoid valve coil", C::Coils, "Y", MAIN).build(&mut b));

    // ---------------------------------------------------------------- contacts
    b.no(0.0).pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("no", "NO contact", C::Contacts, "K", Func::Aux(ContactKind::No)).auto().build(&mut b));

    b.nc(0.0).pin(0.0, -4.0, "21").pin(0.0, 4.0, "22");
    v.push(def("nc", "NC contact", C::Contacts, "K", Func::Aux(ContactKind::Nc)).auto().build(&mut b));

    b.l(0.0, 4.0, 0.0, 2.0).l(0.0, 2.0, 2.3, -2.6); // common + blade
    b.l(0.0, -4.0, 0.0, -2.0); // NO
    b.l(4.0, -4.0, 4.0, -2.0).l(4.0, -2.0, 1.8, -2.0); // NC
    b.pin(0.0, 4.0, "11").pin(4.0, -4.0, "12").pin(0.0, -4.0, "14");
    v.push(
        def("co", "Changeover contact", C::Contacts, "K", Func::Aux(ContactKind::Changeover))
            .auto()
            .build(&mut b),
    );

    for x in [0.0, 8.0, 16.0] {
        b.no(x).arc(x, -1.3, 0.7, 180.0, 360.0);
    }
    b.dl(-0.9, 0.0, 15.1, 0.0);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("main3", "Main contacts 3-pole", C::Contacts, "K", Func::Aux(ContactKind::MainNo)).build(&mut b));

    b.no(0.0).arc(0.0, -1.3, 0.7, 180.0, 360.0).pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("main1", "Main contact 1-pole", C::Contacts, "K", Func::Aux(ContactKind::MainNo)).build(&mut b));

    b.nc(0.0).dl(0.96, 0.0, -5.0, 0.0).rect(-8.0, -1.5, -5.0, 1.5, false);
    b.poly(&[(-7.6, 1.0), (-7.0, 1.0), (-7.0, -1.0), (-6.0, -1.0), (-6.0, 1.0), (-5.4, 1.0)], false, false);
    b.pin(0.0, -4.0, "95").pin(0.0, 4.0, "96");
    v.push(def("ol_nc", "Overload aux contact NC", C::Contacts, "F", Func::Aux(ContactKind::Nc)).build(&mut b));

    // Time-delayed contacts (IEC 60617-07-05), numbered x7/x8 (NO) and x5/x6 (NC).
    b.no(0.0).delay(-0.9, true).pin(0.0, -4.0, "17").pin(0.0, 4.0, "18");
    v.push(def("no_on", "NO contact, on-delay", C::Contacts, "K", Func::Aux(ContactKind::NoOnDelay)).auto().build(&mut b));

    b.nc(0.0).delay(1.0, true).pin(0.0, -4.0, "15").pin(0.0, 4.0, "16");
    v.push(def("nc_on", "NC contact, on-delay", C::Contacts, "K", Func::Aux(ContactKind::NcOnDelay)).auto().build(&mut b));

    b.no(0.0).delay(-0.9, false).pin(0.0, -4.0, "17").pin(0.0, 4.0, "18");
    v.push(def("no_off", "NO contact, off-delay", C::Contacts, "K", Func::Aux(ContactKind::NoOffDelay)).auto().build(&mut b));

    b.nc(0.0).delay(1.0, false).pin(0.0, -4.0, "15").pin(0.0, 4.0, "16");
    v.push(def("nc_off", "NC contact, off-delay", C::Contacts, "K", Func::Aux(ContactKind::NcOffDelay)).auto().build(&mut b));

    for x in [0.0, 8.0, 16.0, 24.0] {
        b.no(x).arc(x, -1.3, 0.7, 180.0, 360.0);
    }
    b.dl(-0.9, 0.0, 23.1, 0.0);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5").pin(24.0, -4.0, "7");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6").pin(24.0, 4.0, "8");
    v.push(def("main4", "Main contacts 4-pole", C::Contacts, "K", Func::Aux(ContactKind::MainNo)).build(&mut b));

    // ---------------------------------------------------------------- switches
    b.no(0.0).push_actuator(-0.9).pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("pb_no", "Push button NO", C::Switches, "S", MAIN).build(&mut b));

    b.nc(0.0).push_actuator(0.96).pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("pb_nc", "Push button NC", C::Switches, "S", MAIN).build(&mut b));

    b.nc(0.0).dl(0.96, 0.0, -4.0, 0.0).l(-4.0, -1.8, -4.0, 1.8).arc(-4.0, 0.0, 1.8, 90.0, 270.0);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("estop", "Emergency stop NC", C::Switches, "S", MAIN).build(&mut b));

    b.no(0.0).dl(-0.9, 0.0, -4.0, 0.0);
    b.poly(&[(-4.0, -1.5), (-5.0, -1.5), (-5.0, 1.5), (-6.0, 1.5)], false, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("sel", "Selector switch NO", C::Switches, "S", MAIN).build(&mut b));

    b.no(0.0).dl(-0.9, 0.0, -5.0, 0.0);
    b.poly(&[(-5.0, 0.0), (-6.5, -1.5), (-8.0, 0.0), (-6.5, 1.5)], true, false);
    b.pin(0.0, -4.0, "BN").pin(0.0, 4.0, "BK");
    v.push(def("prox", "Proximity sensor NO", C::Switches, "B", MAIN).build(&mut b));

    b.no(0.0).dl(-0.9, 0.0, -4.0, 0.0);
    b.poly(&[(-4.0, -1.5), (-6.0, 0.0), (-4.0, 1.5)], false, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("limit", "Position switch NO", C::Switches, "B", MAIN).build(&mut b));

    b.nc(0.0).dl(0.96, 0.0, -4.0, 0.0);
    b.poly(&[(-4.0, -1.5), (-6.0, 0.0), (-4.0, 1.5)], false, false);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("limit_nc", "Position switch NC", C::Switches, "B", MAIN).build(&mut b));

    b.nc(0.0).dl(0.96, 0.0, -4.0, 0.0);
    b.poly(&[(-4.0, -1.5), (-5.0, -1.5), (-5.0, 1.5), (-6.0, 1.5)], false, false);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("sel_nc", "Selector switch NC", C::Switches, "S", MAIN).build(&mut b));

    // Key: bow and shank with a bit.
    b.no(0.0).dl(-0.9, 0.0, -4.0, 0.0).l(-4.0, 0.0, -6.0, 0.0).circle(-7.0, 0.0, 1.0, false).l(-5.0, 0.0, -5.0, 1.0);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("key_no", "Key switch NO", C::Switches, "S", MAIN).build(&mut b));

    b.no(0.0).sensing_box(-0.9, "p");
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("pressure_no", "Pressure switch NO", C::Switches, "B", MAIN).build(&mut b));

    b.nc(0.0).sensing_box(0.96, "θ");
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("temp_nc", "Temperature switch NC", C::Switches, "B", MAIN).build(&mut b));

    // Float on the liquid level.
    b.no(0.0).dl(-0.9, 0.0, -5.0, 0.0).circle(-6.5, 0.0, 1.5, false).l(-9.0, 1.5, -4.0, 1.5);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("level_no", "Float switch NO", C::Switches, "B", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.no(x).disconnector_bar(x);
    }
    b.dl(-5.0, 0.0, 15.1, 0.0).l(-5.0, -1.5, -5.0, 1.5).l(-5.0, -1.5, -6.5, -1.5);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("disc3", "Main switch 3-pole", C::Switches, "Q", MAIN).build(&mut b));

    // 2-wire transmitter: the measured value goes in, a 4 - 20 mA loop current out.
    b.rect(-3.0, -1.0, 11.0, 6.0, false).l(-3.0, 6.0, 11.0, -1.0);
    b.l(0.0, -4.0, 0.0, -1.0).l(8.0, -4.0, 8.0, -1.0);
    b.text(1.0, 4.0, "p", 1.8, Center).text(8.0, 2.0, "I", 1.8, Center);
    b.pin(0.0, -4.0, "+").pin(8.0, -4.0, "-");
    v.push(def("transmitter", "Measuring transducer", C::Switches, "B", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(-3.2, 2.6, -2.2, 2.6).l(-2.2, 2.6, 2.2, -2.6).text(2.6, 2.4, "θ", 1.6, Left);
    b.pin(0.0, -4.0, "T1").pin(0.0, 4.0, "T2");
    v.push(def("ptc", "Thermistor PTC", C::Switches, "B", MAIN).build(&mut b));

    // ---------------------------------------------------------------- protection
    b.l(0.0, -4.0, 0.0, 4.0).rect(-1.2, -2.6, 1.2, 2.6, false);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fuse", "Fuse", C::Protection, "F", MAIN).build(&mut b));

    b.no(0.0).breaker_x(0.0).dl(-0.9, 0.0, -5.0, 0.0).rect(-9.0, -1.5, -5.0, 1.5, false);
    b.text(-7.0, 0.0, "I>", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("cb1", "Circuit breaker 1-pole", C::Protection, "F", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.no(x).breaker_x(x);
    }
    b.dl(-5.0, 0.0, 15.1, 0.0).rect(-9.0, -1.5, -5.0, 1.5, false).text(-7.0, 0.0, "I>", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("cb3", "Circuit breaker 3-pole", C::Protection, "Q", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.no(x).breaker_x(x);
    }
    b.dl(-5.0, 0.0, 15.1, 0.0).rect(-13.0, -1.5, -5.0, 1.5, false).text(-9.0, 0.0, "I>  θ>", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("mps3", "Motor protection switch", C::Protection, "Q", MAIN).build(&mut b));

    b.rect(-2.0, -2.5, 18.0, 2.5, false);
    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, -2.5).l(x, 2.5, x, 4.0);
        b.poly(&[(x, -2.5), (x, -1.0), (x + 1.2, -1.0), (x + 1.2, 1.0), (x, 1.0), (x, 2.5)], false, false);
    }
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("ol3", "Thermal overload relay", C::Protection, "F", MAIN_MIRROR).build(&mut b));

    for x in [0.0, 8.0] {
        b.no(x).breaker_x(x);
    }
    b.dl(-5.0, 0.0, 7.1, 0.0).rect(-9.0, -1.5, -5.0, 1.5, false).text(-7.0, 0.0, "I>", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4");
    v.push(def("cb2", "Circuit breaker 2-pole", C::Protection, "F", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0, 24.0] {
        b.no(x).breaker_x(x);
    }
    b.dl(-5.0, 0.0, 23.1, 0.0).rect(-9.0, -1.5, -5.0, 1.5, false).text(-7.0, 0.0, "I>", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5").pin(24.0, -4.0, "7");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6").pin(24.0, 4.0, "8");
    v.push(def("cb4", "Circuit breaker 4-pole", C::Protection, "Q", MAIN).build(&mut b));

    // Residual current device: 4 switching poles, the summation current transformer
    // around all conductors and the residual current release.
    for x in [0.0, 8.0, 16.0, 24.0] {
        b.no(x);
    }
    b.rect(-1.5, 2.6, 25.5, 3.4, false);
    b.dl(-5.0, 0.0, 23.1, 0.0).rect(-11.0, -1.5, -5.0, 1.5, false).text(-8.0, 0.0, "IΔ", 1.6, Center);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5").pin(24.0, -4.0, "N");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6").pin(24.0, 4.0, "N'");
    v.push(def("rcd4", "Residual current device 4-pole", C::Protection, "F", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, 4.0).rect(x - 1.2, -2.6, x + 1.2, 2.6, false);
    }
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("fuse3", "Fuse 3-pole", C::Protection, "F", MAIN).build(&mut b));

    // Surge arrester: voltage-dependent resistor between the conductor and earth.
    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(-2.4, 3.0, 1.0, -0.6).l(1.0, -0.6, 2.4, -0.6).poly(&[(-2.6, 3.4), (-1.6, 3.1), (-2.2, 2.4)], true, true);
    b.pin(0.0, -4.0, "L").pin(0.0, 4.0, "PE");
    v.push(def("spd", "Surge protective device", C::Protection, "F", MAIN).build(&mut b));

    // ---------------------------------------------------------------- loads
    {
        let (cx, cy, r) = (8.0f32, 8.0f32, 6.0f32);
        let lead = |b: &mut B, x: f32| {
            let start = p(x, 0.0);
            let d = start - p(cx, cy);
            let e = p(cx, cy) + d * (r / d.len());
            b.l(x, -4.0, x, 0.0).l(x, 0.0, e.x, e.y);
        };
        lead(&mut b, 0.0);
        lead(&mut b, 16.0);
        b.l(8.0, -4.0, 8.0, cy - r);
        b.l(24.0, -4.0, 24.0, cy).l(24.0, cy, cx + r, cy);
        b.circle(cx, cy, r, false).text(cx, cy - 1.2, "M", 3.5, Center).text(cx, cy + 2.4, "3~", 2.2, Center);
        b.pin(0.0, -4.0, "U1").pin(8.0, -4.0, "V1").pin(16.0, -4.0, "W1").pin(24.0, -4.0, "PE");
        v.push(def("motor3", "Motor 3-phase", C::Loads, "M", MAIN).build(&mut b));
    }
    {
        let (cx, cy, r) = (4.0f32, 8.0f32, 5.0f32);
        let lead = |b: &mut B, x: f32| {
            let start = p(x, 3.0);
            let d = start - p(cx, cy);
            let e = p(cx, cy) + d * (r / d.len());
            b.l(x, -4.0, x, 3.0).l(x, 3.0, e.x, e.y);
        };
        lead(&mut b, 0.0);
        lead(&mut b, 8.0);
        b.l(16.0, -4.0, 16.0, cy).l(16.0, cy, cx + r, cy);
        b.circle(cx, cy, r, false).text(cx, cy - 1.0, "M", 3.0, Center).text(cx, cy + 2.0, "1~", 2.0, Center);
        b.pin(0.0, -4.0, "L").pin(8.0, -4.0, "N").pin(16.0, -4.0, "PE");
        v.push(def("motor1", "Motor 1-phase", C::Loads, "M", MAIN).build(&mut b));
    }

    let d = 2.5 * std::f32::consts::FRAC_1_SQRT_2;
    b.l(0.0, -4.0, 0.0, -2.5).circle(0.0, 0.0, 2.5, false).l(-d, -d, d, d).l(-d, d, d, -d).l(0.0, 2.5, 0.0, 4.0);
    b.pin(0.0, -4.0, "X1").pin(0.0, 4.0, "X2");
    v.push(def("lamp", "Indicator lamp", C::Loads, "P", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).poly(&[(-2.5, -2.0), (2.5, -2.0), (2.5, 0.5), (-2.5, 0.5)], true, false);
    b.arc(0.0, 0.5, 2.5, 0.0, 180.0).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("horn", "Horn / buzzer", C::Loads, "P", MAIN).build(&mut b));

    b.rect(-3.0, 0.0, 19.0, 12.0, false).l(-3.0, 12.0, 19.0, 0.0);
    b.text(3.0, 3.5, "~", 3.5, Center).text(13.0, 8.5, "=", 3.5, Center);
    b.l(0.0, -4.0, 0.0, 0.0).l(8.0, -4.0, 8.0, 0.0).l(16.0, -4.0, 16.0, 0.0);
    b.l(0.0, 12.0, 0.0, 16.0).l(8.0, 12.0, 8.0, 16.0);
    b.pin(0.0, -4.0, "L").pin(8.0, -4.0, "N").pin(16.0, -4.0, "PE");
    b.pin(0.0, 16.0, "L+").pin(8.0, 16.0, "M");
    v.push(def("psu", "Power supply AC/DC", C::Loads, "T", MAIN).build(&mut b));

    b.circle(4.0, 3.0, 3.6, false).circle(4.0, 7.0, 3.6, false);
    b.l(0.0, -4.0, 0.0, 0.0).l(0.0, 0.0, 1.6, 0.4).l(8.0, -4.0, 8.0, 0.0).l(8.0, 0.0, 6.4, 0.4);
    b.l(0.0, 14.0, 0.0, 10.0).l(0.0, 10.0, 1.6, 9.6).l(8.0, 14.0, 8.0, 10.0).l(8.0, 10.0, 6.4, 9.6);
    b.l(0.0, 14.0, 0.0, 16.0).l(8.0, 14.0, 8.0, 16.0);
    b.pin(0.0, -4.0, "1.1").pin(8.0, -4.0, "1.2").pin(0.0, 16.0, "2.1").pin(8.0, 16.0, "2.2");
    v.push(def("trafo", "Control transformer", C::Loads, "T", MAIN).build(&mut b));

    {
        let (cx, cy, r) = (4.0f32, 8.0f32, 5.0f32);
        for x in [0.0f32, 8.0] {
            let d = p(x, 3.0) - p(cx, cy);
            let e = p(cx, cy) + d * (r / d.len());
            b.l(x, -4.0, x, 3.0).l(x, 3.0, e.x, e.y);
        }
        b.circle(cx, cy, r, false).text(cx, cy - 1.0, "M", 3.0, Center).text(cx, cy + 2.0, "=", 2.0, Center);
        b.pin(0.0, -4.0, "A1").pin(8.0, -4.0, "A2");
        v.push(def("motor_dc", "Motor DC", C::Loads, "M", MAIN).build(&mut b));
    }

    // Electromagnetic brake: winding with the brake shoe.
    b.l(0.0, -4.0, 0.0, -2.0).rect(-3.0, -2.0, 3.0, 2.0, false).l(0.0, 2.0, 0.0, 4.0);
    b.l(-6.0, -2.0, -6.0, 2.0).l(-5.0, -2.0, -5.0, 2.0).l(-5.0, 0.0, -3.0, 0.0);
    b.pin(0.0, -4.0, "BD1").pin(0.0, 4.0, "BD2");
    v.push(def("brake", "Brake (electromagnetic)", C::Loads, "Y", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).rect(-2.0, -3.0, 2.0, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    for y in [-1.5, 0.0, 1.5] {
        b.l(-2.0, y + 1.0, 2.0, y - 1.0);
    }
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("heater", "Heating element", C::Loads, "E", MAIN).build(&mut b));

    b.rect(-3.0, 0.0, 27.0, 12.0, false).l(-3.0, 12.0, 27.0, 0.0);
    b.text(4.0, 3.5, "~", 3.5, Center).text(20.0, 8.5, "~", 3.5, Center);
    for x in [0.0, 8.0, 16.0, 24.0] {
        b.l(x, -4.0, x, 0.0);
    }
    for x in [0.0, 8.0, 16.0] {
        b.l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "L1").pin(8.0, -4.0, "L2").pin(16.0, -4.0, "L3").pin(24.0, -4.0, "PE");
    b.pin(0.0, 16.0, "U").pin(8.0, 16.0, "V").pin(16.0, 16.0, "W");
    v.push(def("vfd", "Frequency converter", C::Loads, "T", MAIN).build(&mut b));

    // Soft starter: semiconductor controller with the starting ramp.
    b.rect(-3.0, 0.0, 19.0, 12.0, false).poly(&[(0.0, 9.0), (5.0, 9.0), (13.0, 3.0), (16.0, 3.0)], false, false);
    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, 0.0).l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "1L1").pin(8.0, -4.0, "3L2").pin(16.0, -4.0, "5L3");
    b.pin(0.0, 16.0, "2T1").pin(8.0, 16.0, "4T2").pin(16.0, 16.0, "6T3");
    v.push(def("softstarter", "Soft starter", C::Loads, "Q", MAIN).build(&mut b));

    // Socket outlet with protective contact: a cup open towards the plug.
    b.arc(8.0, 2.0, 9.0, 0.0, 180.0).l(0.0, -4.0, 0.0, 4.0).l(8.0, -4.0, 8.0, 6.0).l(16.0, -4.0, 16.0, 4.0);
    b.pin(0.0, -4.0, "L").pin(8.0, -4.0, "N").pin(16.0, -4.0, "PE");
    v.push(def("socket", "Socket outlet", C::Loads, "X", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).circle(0.0, 0.0, 3.0, false).text(0.0, 0.0, "A", 2.5, Center).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("ammeter", "Ammeter", C::Loads, "P", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).circle(0.0, 0.0, 3.0, false).text(0.0, 0.0, "V", 2.5, Center).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("voltmeter", "Voltmeter", C::Loads, "P", MAIN).build(&mut b));

    // Current transformer: the primary conductor passes through the core.
    b.l(0.0, -8.0, 0.0, 8.0).circle(0.0, 0.0, 3.0, false);
    b.l(2.1, -2.1, 4.0, -4.0).l(4.0, -4.0, 8.0, -4.0).l(2.1, 2.1, 4.0, 4.0).l(4.0, 4.0, 8.0, 4.0);
    b.pin(0.0, -8.0, "P1").pin(0.0, 8.0, "P2").pin(8.0, -4.0, "S1").pin(8.0, 4.0, "S2");
    v.push(def("ct", "Current transformer", C::Loads, "T", MAIN).build(&mut b));

    // Energy meters: the measured conductors pass through, a transformer-operated
    // meter takes the voltages and the secondary currents of its transformers.
    b.rect(-3.0, 0.0, 27.0, 12.0, false).text(12.0, 6.0, "Wh", 3.0, Center);
    for x in [0.0, 8.0, 16.0, 24.0] {
        b.l(x, -4.0, x, 0.0).l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5").pin(24.0, -4.0, "N");
    b.pin(0.0, 16.0, "2").pin(8.0, 16.0, "4").pin(16.0, 16.0, "6").pin(24.0, 16.0, "N'");
    v.push(def("kwh3", "Energy meter 3-phase", C::Loads, "P", MAIN).build(&mut b));

    b.rect(-3.0, 0.0, 75.0, 12.0, false).text(36.0, 6.0, "Wh", 3.0, Center);
    for (i, label) in ["1S1", "1S2", "2S1", "2S2", "3S1", "3S2", "V1", "V2", "V3", "N"].into_iter().enumerate() {
        let x = i as f32 * 8.0;
        b.l(x, -4.0, x, 0.0).pin(x, -4.0, label);
    }
    v.push(def("kwh3_ct", "Energy meter 3-phase, transformer-operated", C::Loads, "P", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-3.0, -1.0, 3.0, -1.0).l(-1.5, 1.0, 1.5, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.text(3.0, -2.6, "+", 1.8, Left);
    b.pin(0.0, -4.0, "+").pin(0.0, 4.0, "-");
    v.push(def("battery", "Battery", C::Loads, "G", MAIN).build(&mut b));

    // PV generator (IEC 60617-06-17-01): a cell with the diagonal, light falling on it.
    b.l(0.0, -4.0, 0.0, 0.0).rect(-4.0, 0.0, 4.0, 12.0, false).l(-4.0, 12.0, 4.0, 0.0).l(0.0, 12.0, 0.0, 16.0);
    for y in [1.0, 5.0] {
        b.l(-10.0, y - 4.0, -6.0, y).poly(&[(-6.0, y), (-7.6, y - 0.4), (-6.4, y - 1.6)], true, true);
    }
    b.text(2.0, -2.4, "+", 1.8, Left);
    b.pin(0.0, -4.0, "+").pin(0.0, 16.0, "-");
    v.push(def("pv", "PV generator", C::Loads, "G", MAIN).build(&mut b));

    // Hybrid inverter: PV input and battery at the top, alternating current at the bottom.
    b.rect(-3.0, 0.0, 35.0, 12.0, false).l(-3.0, 12.0, 35.0, 0.0);
    b.text(6.0, 3.5, "=", 3.5, Center).text(26.0, 8.5, "~", 3.5, Center);
    for x in [0.0, 8.0, 24.0, 32.0] {
        b.l(x, -4.0, x, 0.0);
    }
    for x in [0.0, 8.0, 16.0] {
        b.l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "PV+").pin(8.0, -4.0, "PV-").pin(24.0, -4.0, "DC+").pin(32.0, -4.0, "DC-");
    b.pin(0.0, 16.0, "L").pin(8.0, 16.0, "N").pin(16.0, 16.0, "PE");
    v.push(def("hybrid_inverter", "Hybrid inverter (PV, battery, AC)", C::Loads, "T", MAIN).build(&mut b));

    b.rect(-3.0, 0.0, 11.0, 12.0, false).l(-3.0, 12.0, 11.0, 0.0);
    b.text(1.0, 3.5, "=", 3.0, Center).text(7.0, 8.5, "=", 3.0, Center);
    for x in [0.0, 8.0] {
        b.l(x, -4.0, x, 0.0).l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "PV+").pin(8.0, -4.0, "PV-").pin(0.0, 16.0, "DC+").pin(8.0, 16.0, "DC-");
    v.push(def("mppt", "Solar charge controller (MPPT)", C::Loads, "T", MAIN).build(&mut b));

    // Inverter / charger: AC input at the top; AC output and battery at the bottom.
    b.rect(-3.0, 0.0, 35.0, 12.0, false).l(-3.0, 12.0, 35.0, 0.0);
    b.text(6.0, 4.0, "~", 3.5, Center).text(26.0, 8.5, "=", 3.5, Center);
    b.poly(&[(12.0, 6.0), (20.0, 6.0)], false, false).poly(&[(20.0, 6.0), (18.6, 5.2), (18.6, 6.8)], true, true).poly(&[(12.0, 6.0), (13.4, 5.2), (13.4, 6.8)], true, true);
    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, 0.0);
    }
    for x in [0.0, 8.0, 24.0, 32.0] {
        b.l(x, 12.0, x, 16.0);
    }
    b.pin(0.0, -4.0, "ACin-L").pin(8.0, -4.0, "ACin-N").pin(16.0, -4.0, "PE");
    b.pin(0.0, 16.0, "ACout1-L").pin(8.0, 16.0, "ACout1-N").pin(24.0, 16.0, "DC+").pin(32.0, 16.0, "DC-");
    v.push(def("inverter_charger", "Inverter / charger", C::Loads, "T", MAIN).build(&mut b));

    // Valve actuator with motor: common, open and close.
    b.rect(-3.0, 0.0, 19.0, 10.0, false).circle(8.0, 5.0, 3.5, false).text(8.0, 5.0, "M", 3.0, Center);
    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, 0.0);
    }
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "2").pin(16.0, -4.0, "3");
    v.push(def("actuator", "Valve actuator (open / close)", C::Loads, "M", MAIN).build(&mut b));

    // ---------------------------------------------------------------- terminals
    b.l(0.0, -4.0, 0.0, -1.0).circle(0.0, 0.0, 1.0, false).l(0.0, 1.0, 0.0, 4.0);
    b.field(FieldKind::Tag, -1.6, 0.0, 2.0, Right).field(FieldKind::Label, 1.6, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("term", "Terminal", C::Terminals, "X", Func::Terminal).label("1");
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).circle(0.0, 0.0, 1.0, false).l(0.0, 1.0, 0.0, 4.0);
    b.l(-1.6, -1.6, -0.7, -0.7).l(-1.6, 1.6, -0.7, 0.7);
    b.field(FieldKind::Tag, -2.2, 0.0, 2.0, Right).field(FieldKind::Label, 1.6, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("term_pe", "PE terminal", C::Terminals, "X", Func::Terminal).label(tr("PE"));
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).circle(0.0, 0.0, 1.0, false).l(0.0, 1.0, 0.0, 4.0).l(-0.7, 0.0, 0.7, 0.0);
    b.field(FieldKind::Tag, -1.6, 0.0, 2.0, Right).field(FieldKind::Label, 1.6, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("term_n", "N terminal", C::Terminals, "X", Func::Terminal).label("N");
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    // Disconnect terminal: knife between the two terminal points.
    b.l(0.0, -4.0, 0.0, -2.6).circle(0.0, -2.0, 0.6, false).l(0.3, -1.5, 1.4, 1.6).circle(0.0, 2.0, 0.6, false).l(0.0, 2.6, 0.0, 4.0);
    b.field(FieldKind::Tag, -1.6, 0.0, 2.0, Right).field(FieldKind::Label, 2.0, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("term_disc", "Disconnect terminal", C::Terminals, "X", Func::Terminal).label("1");
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    b.l(0.0, -4.0, 0.0, 4.0).rect(-1.0, -2.6, 1.0, 2.6, false);
    b.field(FieldKind::Tag, -1.6, 0.0, 2.0, Right).field(FieldKind::Label, 1.6, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("term_fuse", "Fuse terminal", C::Terminals, "X", Func::Terminal).label("1");
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    // ---------------------------------------------------------------- plugs
    // Socket (IEC 60617-03-03-01): a cup open to the pin; pin (03-03-03): a bar.
    b.l(0.0, -4.0, 0.0, -1.6).arc(0.0, 0.0, 1.6, 180.0, 360.0);
    b.rect(-0.5, -0.8, 0.5, 1.6, true).l(0.0, 1.6, 0.0, 4.0);
    b.field(FieldKind::Tag, -2.2, 0.0, 2.0, Right).field(FieldKind::Label, 2.2, 0.0, 2.2, Left);
    b.pin(0.0, -4.0, "").pin(0.0, 4.0, "");
    let mut t = def("plug_pair", "Plug pin and socket", C::Plugs, "X", Func::Plug(PinKind::Pair)).label("1");
    t.show_tag = false;
    t.bridges = vec![(0, 1)];
    v.push(t.build(&mut b));

    b.rect(-0.5, -1.6, 0.5, 0.8, true).l(0.0, 0.8, 0.0, 4.0);
    b.field(FieldKind::Tag, -1.6, 1.6, 2.0, Right).field(FieldKind::Label, 1.6, 1.6, 2.2, Left);
    b.pin(0.0, 4.0, "");
    let mut t = def("plug_m", "Plug male pin", C::Plugs, "X", Func::Plug(PinKind::Male)).label("1");
    t.show_tag = false;
    v.push(t.build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.6).arc(0.0, 0.0, 1.6, 180.0, 360.0);
    b.field(FieldKind::Tag, -2.2, -1.6, 2.0, Right).field(FieldKind::Label, 2.2, -1.6, 2.2, Left);
    b.pin(0.0, -4.0, "");
    let mut t = def("plug_f", "Plug female socket", C::Plugs, "X", Func::Plug(PinKind::Female)).label("1");
    t.show_tag = false;
    v.push(t.build(&mut b));

    // ---------------------------------------------------------------- PLC
    b.l(0.0, -4.0, 0.0, 0.0).rect(-5.0, 0.0, 5.0, 6.0, false).poly(&[(-5.0, 0.0), (-3.5, 1.5), (-5.0, 3.0)], false, false);
    b.field(FieldKind::Label, 0.0, 3.2, 2.0, Center);
    b.field(FieldKind::Tag, 5.6, 4.5, 1.8, Left);
    b.pin(0.0, -4.0, "");
    let mut t = def("plc_in", "PLC digital input", C::Plc, "A", Func::Plc).label("I0.0");
    t.show_tag = false;
    v.push(t.build(&mut b));

    b.l(0.0, 0.0, 0.0, 4.0).rect(-5.0, -6.0, 5.0, 0.0, false).poly(&[(-3.5, -6.0), (-5.0, -4.5), (-3.5, -3.0)], false, false);
    b.field(FieldKind::Label, 0.0, -2.8, 2.0, Center);
    b.field(FieldKind::Tag, 5.6, -4.5, 1.8, Left);
    b.pin(0.0, 4.0, "");
    let mut t = def("plc_out", "PLC digital output", C::Plc, "A", Func::Plc).label("Q0.0");
    t.show_tag = false;
    v.push(t.build(&mut b));

    // Analog points: the same boxes with a wave.
    let wave = |b: &mut B, y: f32| {
        b.poly(&[(-4.4, y), (-3.9, y - 0.6), (-3.4, y), (-2.9, y + 0.6), (-2.4, y)], false, false);
    };
    b.l(0.0, -4.0, 0.0, 0.0).rect(-5.0, 0.0, 5.0, 6.0, false);
    wave(&mut b, 4.8);
    b.field(FieldKind::Label, 0.0, 3.2, 2.0, Center);
    b.field(FieldKind::Tag, 5.6, 4.5, 1.8, Left);
    b.pin(0.0, -4.0, "");
    let mut t = def("plc_ai", "PLC analog input", C::Plc, "A", Func::Plc).label("IW64");
    t.show_tag = false;
    v.push(t.build(&mut b));

    b.l(0.0, 0.0, 0.0, 4.0).rect(-5.0, -6.0, 5.0, 0.0, false);
    wave(&mut b, -4.8);
    b.field(FieldKind::Label, 0.0, -2.8, 2.0, Center);
    b.field(FieldKind::Tag, 5.6, -4.5, 1.8, Left);
    b.pin(0.0, 4.0, "");
    let mut t = def("plc_ao", "PLC analog output", C::Plc, "A", Func::Plc).label("QW64");
    t.show_tag = false;
    v.push(t.build(&mut b));

    // ---------------------------------------------------------------- potentials
    for (key, name, label) in [
        ("pot_l1", "Potential L1", "L1"),
        ("pot_l2", "Potential L2", "L2"),
        ("pot_l3", "Potential L3", "L3"),
        ("pot_n", "Potential N", "N"),
        ("pot_pe", "Potential PE", "PE"),
        ("pot_24", "Potential +24V", "+24V"),
        ("pot_0v", "Potential 0V", "0V"),
        ("pot", "Potential (custom)", "P1"),
    ] {
        b.l(0.0, 0.0, 0.0, 4.0).poly(&[(-1.2, 0.0), (1.2, 0.0), (0.0, 2.0)], true, true);
        b.field(FieldKind::Label, 0.0, -1.6, 2.5, Center);
        b.pin(0.0, 4.0, "");
        v.push(def(key, name, C::Potentials, "", Func::Potential).label(label).build(&mut b));
    }

    b.l(0.0, -4.0, 0.0, 0.0).l(-2.5, 0.0, 2.5, 0.0).l(-1.6, 1.0, 1.6, 1.0).l(-0.7, 2.0, 0.7, 2.0);
    b.pin(0.0, -4.0, "");
    v.push(def("ground", "Protective earth", C::Potentials, "", Func::Potential).label(tr("PE")).build(&mut b));

    // Earth electrode (IEC 60617-02-15-01): a device such as the foundation earth
    // electrode, unlike the potential above.
    b.l(0.0, -4.0, 0.0, 0.0).l(-3.0, 0.0, 3.0, 0.0).l(-2.0, 1.2, 2.0, 1.2).l(-1.0, 2.4, 1.0, 2.4);
    b.pin(0.0, -4.0, "1");
    v.push(def("earth_electrode", "Earth electrode", C::Protection, "E", MAIN).build(&mut b));

    b.l(0.0, 0.0, 0.0, 2.6).poly(&[(-1.5, 1.2), (0.0, 2.7), (1.5, 1.2)], false, false);
    b.field(FieldKind::Label, 2.2, 1.6, 2.5, Left);
    b.pin(0.0, 0.0, "");
    v.push(def("intr", "Interruption point", C::Potentials, "", Func::Interruption).label("1").build(&mut b));

    // ---------------------------------------------------------------- passive
    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("res", "Resistor", C::Passive, "R", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -0.8).l(-2.5, -0.8, 2.5, -0.8).l(-2.5, 0.8, 2.5, 0.8).l(0.0, 0.8, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("cap", "Capacitor", C::Passive, "C", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.5).poly(&[(-1.8, -1.5), (1.8, -1.5), (0.0, 1.5)], true, false);
    b.l(-1.8, 1.5, 1.8, 1.5).l(0.0, 1.5, 0.0, 4.0);
    b.pin(0.0, -4.0, "A").pin(0.0, 4.0, "K");
    v.push(def("diode", "Diode", C::Passive, "V", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.5).poly(&[(-1.8, -1.5), (1.8, -1.5), (0.0, 1.5)], true, false);
    b.l(-1.8, 1.5, 1.8, 1.5).l(0.0, 1.5, 0.0, 4.0);
    for y in [-1.2, 0.4] {
        b.l(2.2, y, 3.8, y - 1.2);
        b.poly(&[(3.8, y - 1.2), (3.0, y - 1.0), (3.6, y - 0.4)], true, true);
    }
    b.pin(0.0, -4.0, "A").pin(0.0, 4.0, "K");
    v.push(def("led", "LED", C::Passive, "P", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).l(0.0, 3.0, 0.0, 4.0);
    for y in [-2.25, -0.75, 0.75, 2.25] {
        b.arc(0.0, y, 0.75, 270.0, 450.0);
    }
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("inductor", "Inductor", C::Passive, "L", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(-3.2, 2.6, -2.2, 2.6).l(-2.2, 2.6, 2.2, -2.6).text(2.6, 2.4, "U", 1.6, Left);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("varistor", "Varistor", C::Passive, "R", MAIN).build(&mut b));

    // Potentiometer: wiper (2) on the side.
    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(4.0, 0.0, 1.8, 0.0).poly(&[(1.2, 0.0), (2.2, -0.6), (2.2, 0.6)], true, true);
    b.pin(0.0, -4.0, "1").pin(4.0, 0.0, "2").pin(0.0, 4.0, "3");
    v.push(def("potentiometer", "Potentiometer", C::Passive, "R", MAIN).build(&mut b));

    // ---------------------------------------------------------------- electronics (IEC 60617)
    // Small filled arrow head at `tip` pointing in direction (dx, dy).
    let arrow = |b: &mut B, tx: f32, ty: f32, dx: f32, dy: f32| {
        let (nx, ny) = (-dy, dx);
        b.poly(&[(tx, ty), (tx - dx * 1.2 + nx * 0.5, ty - dy * 1.2 + ny * 0.5), (tx - dx * 1.2 - nx * 0.5, ty - dy * 1.2 - ny * 0.5)], true, true);
    };
    let diode_body = |b: &mut B| {
        b.l(0.0, -4.0, 0.0, -1.5).poly(&[(-1.8, -1.5), (1.8, -1.5), (0.0, 1.5)], true, false);
    };

    // Electrolytic capacitor: the negative plate filled, + at the positive one.
    b.l(0.0, -4.0, 0.0, -0.8).l(-2.5, -0.8, 2.5, -0.8).rect(-2.5, 0.6, 2.5, 1.2, true).l(0.0, 1.2, 0.0, 4.0);
    b.text(2.2, -2.2, "+", 1.8, Left);
    b.pin(0.0, -4.0, "+").pin(0.0, 4.0, "-");
    v.push(def("cap_pol", "Capacitor, polarized", C::Passive, "C", MAIN).build(&mut b));

    diode_body(&mut b);
    b.l(-1.8, 2.2, -1.8, 1.5).l(-1.8, 1.5, 1.8, 1.5).l(1.8, 1.5, 1.8, 0.8).l(0.0, 1.5, 0.0, 4.0);
    b.pin(0.0, -4.0, "A").pin(0.0, 4.0, "K");
    v.push(def("zener", "Zener diode", C::Passive, "V", MAIN).build(&mut b));

    diode_body(&mut b);
    b.l(-1.8, 1.5, 1.8, 1.5).l(0.0, 1.5, 0.0, 4.0);
    for y in [-1.6, 0.0] {
        b.l(4.0, y - 1.4, 2.4, y);
        arrow(&mut b, 2.2, y + 0.2, -0.75, 0.66);
    }
    b.pin(0.0, -4.0, "A").pin(0.0, 4.0, "K");
    v.push(def("photodiode", "Photodiode", C::Passive, "B", MAIN).build(&mut b));

    // Bipolar transistors: base on the left, collector up, emitter down.
    for (key, name, pnp) in [("npn", "Transistor NPN", false), ("pnp", "Transistor PNP", true)] {
        b.circle(0.0, 0.0, 3.2, false).l(-4.0, 0.0, -1.2, 0.0).l(-1.2, -2.0, -1.2, 2.0);
        b.l(-1.2, -0.9, 0.0, -2.2).l(0.0, -2.2, 0.0, -4.0).l(-1.2, 0.9, 0.0, 2.2).l(0.0, 2.2, 0.0, 4.0);
        if pnp {
            arrow(&mut b, -1.1, 1.0, -0.68, -0.73);
        } else {
            arrow(&mut b, 0.0, 2.2, 0.68, 0.73);
        }
        b.pin(-4.0, 0.0, "B").pin(0.0, -4.0, "C").pin(0.0, 4.0, "E");
        v.push(def(key, name, C::Passive, "V", MAIN).build(&mut b));
    }

    // N-channel MOSFET (enhancement): gate left, drain up, source down, body to source.
    b.circle(0.0, 0.0, 3.2, false).l(-4.0, 0.0, -1.8, 0.0).l(-1.8, -1.8, -1.8, 1.8);
    for (y0, y1) in [(-2.0, -1.0), (-0.5, 0.5), (1.0, 2.0)] {
        b.l(-1.0, y0, -1.0, y1);
    }
    b.l(-1.0, -1.5, 0.0, -1.5).l(0.0, -1.5, 0.0, -4.0).l(-1.0, 1.5, 0.0, 1.5).l(0.0, 1.5, 0.0, 4.0).l(-0.6, 0.0, 0.0, 0.0).l(0.0, 0.0, 0.0, 1.5);
    arrow(&mut b, -1.0, 0.0, -1.0, 0.0);
    b.pin(-4.0, 0.0, "G").pin(0.0, -4.0, "D").pin(0.0, 4.0, "S");
    v.push(def("nmos", "MOSFET N-channel", C::Passive, "V", MAIN).build(&mut b));

    // Thyristor: diode with the gate at the cathode.
    diode_body(&mut b);
    b.l(-1.8, 1.5, 1.8, 1.5).l(0.0, 1.5, 0.0, 4.0).l(0.9, 1.5, 4.0, 4.0);
    b.pin(0.0, -4.0, "A").pin(0.0, 4.0, "K").pin(4.0, 4.0, "G");
    v.push(def("thyristor", "Thyristor", C::Passive, "V", MAIN).build(&mut b));

    // Triac: two triangles in antiparallel between two bars, gate at the bottom.
    b.l(0.0, -4.0, 0.0, -1.5).l(-2.6, -1.5, 2.6, -1.5).l(-2.6, 1.5, 2.6, 1.5).l(0.0, 1.5, 0.0, 4.0);
    b.poly(&[(-2.6, -1.5), (0.0, -1.5), (-1.3, 1.5)], true, false).poly(&[(0.0, 1.5), (2.6, 1.5), (1.3, -1.5)], true, false);
    b.l(1.3, 1.5, 4.0, 4.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2").pin(4.0, 4.0, "G");
    v.push(def("triac", "Triac", C::Passive, "V", MAIN).build(&mut b));

    // NTC thermistor: resistor with the non-linear stroke and the sign of the coefficient.
    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(-3.2, 2.6, -2.2, 2.6).l(-2.2, 2.6, 2.2, -2.6).text(2.0, 2.4, "-t°", 1.4, Left);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("ntc", "Thermistor NTC", C::Passive, "R", MAIN).build(&mut b));

    // Photoresistor (LDR): resistor with two incoming arrows.
    b.l(0.0, -4.0, 0.0, -3.0).rect(-1.2, -3.0, 1.2, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    for y in [-2.2, -0.4] {
        b.l(4.0, y - 1.4, 2.0, y);
        arrow(&mut b, 1.8, y + 0.2, -0.75, 0.66);
    }
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("ldr", "Photoresistor (LDR)", C::Passive, "R", MAIN).build(&mut b));

    // Optocoupler: LED and phototransistor in a common housing.
    b.drect(-7.0, -5.5, 7.0, 5.5);
    b.l(-4.0, -8.0, -4.0, -1.5).poly(&[(-5.8, -1.5), (-2.2, -1.5), (-4.0, 1.5)], true, false).l(-5.8, 1.5, -2.2, 1.5).l(-4.0, 1.5, -4.0, 8.0);
    for y in [-1.2, 0.6] {
        b.l(-1.6, y, 0.6, y);
        arrow(&mut b, 0.8, y, 1.0, 0.0);
    }
    b.l(2.0, -2.0, 2.0, 2.0).l(2.0, -0.9, 4.0, -2.4).l(4.0, -2.4, 4.0, -8.0).l(2.0, 0.9, 4.0, 2.4).l(4.0, 2.4, 4.0, 8.0);
    arrow(&mut b, 4.0, 2.4, 0.8, 0.6);
    b.pin(-4.0, -8.0, "A").pin(-4.0, 8.0, "K").pin(4.0, -8.0, "C").pin(4.0, 8.0, "E");
    v.push(def("optocoupler", "Optocoupler", C::Passive, "U", MAIN).build(&mut b));

    // Bridge rectifier: the four diodes as one block, AC on the sides, + up, - down.
    b.rect(-4.0, -4.0, 4.0, 4.0, false);
    b.poly(&[(-1.4, -0.6), (1.4, -0.6), (0.0, -2.4)], true, false).l(-1.4, -2.4, 1.4, -2.4);
    b.text(-2.6, 2.4, "~", 1.8, Center).text(2.6, 2.4, "~", 1.8, Center);
    b.l(0.0, -8.0, 0.0, -4.0).l(0.0, 4.0, 0.0, 8.0).l(-8.0, 0.0, -4.0, 0.0).l(4.0, 0.0, 8.0, 0.0);
    b.pin(-8.0, 0.0, "~1").pin(8.0, 0.0, "~2").pin(0.0, -8.0, "+").pin(0.0, 8.0, "-");
    v.push(def("bridge_rect", "Bridge rectifier block", C::Passive, "T", MAIN).build(&mut b));

    // ---------------------------------------------------------------- installation plan
    // Single-line symbols of building installation plans with one connection point
    // (the cable from the distribution) at the top.
    let feed = |b: &mut B, to: f32| {
        b.l(0.0, -4.0, 0.0, to);
    };
    feed(&mut b, -1.0);
    b.circle(0.0, 0.0, 1.0, false).l(0.7, -0.7, 3.2, -3.2).l(3.2, -3.2, 4.0, -2.4);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_switch", "Switch, 1-pole (installation)", C::Installation, "S", MAIN).build(&mut b));

    feed(&mut b, -1.0);
    b.circle(0.0, 0.0, 1.0, false).l(0.7, -0.7, 3.2, -3.2).l(3.2, -3.2, 4.0, -2.4).l(-0.7, 0.7, -3.2, 3.2).l(-3.2, 3.2, -4.0, 2.4);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_switch2", "Two-way switch (installation)", C::Installation, "S", MAIN).build(&mut b));

    feed(&mut b, -1.0);
    b.circle(0.0, 0.0, 1.0, false).l(0.7, -0.7, 3.2, -3.2).l(3.2, -3.2, 4.0, -2.4).l(-2.6, 2.0, 1.6, -2.2);
    b.poly(&[(1.6, -2.2), (0.6, -1.9), (1.3, -1.2)], true, true);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_dimmer", "Dimmer (installation)", C::Installation, "S", MAIN).build(&mut b));

    feed(&mut b, -1.6);
    b.circle(0.0, 0.0, 1.6, false).circle(0.0, 0.0, 0.5, true);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_push", "Push button (installation)", C::Installation, "S", MAIN).build(&mut b));

    feed(&mut b, -2.5);
    b.circle(0.0, 0.0, 2.5, false).l(-1.77, -1.77, 1.77, 1.77).l(-1.77, 1.77, 1.77, -1.77);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_lamp", "Lighting outlet", C::Installation, "E", MAIN).build(&mut b));

    feed(&mut b, 0.0);
    b.rect(-6.0, 0.0, 6.0, 1.2, true).l(-6.0, -1.2, -6.0, 2.4).l(6.0, -1.2, 6.0, 2.4);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_luminaire", "Luminaire, fluorescent", C::Installation, "E", MAIN).build(&mut b));

    // Double socket outlet with protective contact: two cups side by side.
    feed(&mut b, 0.0);
    for x in [-2.6, 2.6] {
        b.arc(x, 0.0, 2.4, 0.0, 180.0).l(x - 2.4, 0.0, x + 2.4, 0.0);
    }
    b.l(-5.0, -0.9, 5.0, -0.9).l(0.0, 0.0, 0.0, -0.9);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_socket2", "Double socket outlet with PE", C::Installation, "X", MAIN).build(&mut b));

    feed(&mut b, -1.2);
    b.circle(0.0, 0.0, 1.2, true);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_junction", "Junction box", C::Installation, "X", MAIN).build(&mut b));

    // Motion detector: a half circle with the detection arcs.
    feed(&mut b, -2.0);
    b.l(-2.0, -2.0, 2.0, -2.0).arc(0.0, -2.0, 2.0, 0.0, 180.0).arc(0.0, -2.0, 3.2, 30.0, 150.0).arc(0.0, -2.0, 4.4, 40.0, 140.0);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_motion", "Motion detector", C::Installation, "B", MAIN).build(&mut b));

    // Distribution board: a box half filled.
    feed(&mut b, -2.0);
    b.rect(-4.0, -2.0, 4.0, 2.0, false).poly(&[(-4.0, 2.0), (4.0, -2.0), (4.0, 2.0)], true, true);
    b.pin(0.0, -4.0, "1");
    v.push(def("inst_board", "Distribution board", C::Installation, "A", MAIN).build(&mut b));

    // ---------------------------------------------------------------- black boxes
    b.drect(-4.0, -4.0, 4.0, 4.0).l(0.0, -8.0, 0.0, -4.0).l(0.0, 4.0, 0.0, 8.0);
    b.pin(0.0, -8.0, "1").pin(0.0, 8.0, "2");
    v.push(def("box1", "Black box 1 + 1 connections", C::BlackBox, "A", MAIN).build(&mut b));

    b.drect(-4.0, -4.0, 12.0, 4.0);
    b.l(0.0, -8.0, 0.0, -4.0).l(8.0, -8.0, 8.0, -4.0).l(0.0, 4.0, 0.0, 8.0).l(8.0, 4.0, 8.0, 8.0);
    b.pin(0.0, -8.0, "1").pin(8.0, -8.0, "2").pin(0.0, 8.0, "3").pin(8.0, 8.0, "4");
    v.push(def("box2", "Black box 2 + 2 connections", C::BlackBox, "A", MAIN).build(&mut b));

    b.drect(-4.0, -4.0, 28.0, 4.0);
    for (i, x) in [0.0, 8.0, 16.0, 24.0].into_iter().enumerate() {
        b.l(x, -8.0, x, -4.0).l(x, 4.0, x, 8.0);
        b.pin(x, -8.0, ["1", "2", "3", "4"][i]);
    }
    for (i, x) in [0.0, 8.0, 16.0, 24.0].into_iter().enumerate() {
        b.pin(x, 8.0, ["5", "6", "7", "8"][i]);
    }
    v.push(def("box4", "Black box 4 + 4 connections", C::BlackBox, "A", MAIN).build(&mut b));

    // PLC boxes: 8 channels on one side, supply on the other.
    const CH: [&str; 8] = ["0", "1", "2", "3", "4", "5", "6", "7"];
    b.rect(-4.0, -6.0, 60.0, 6.0, false).text(28.0, 0.0, "DI 8 x 24 V DC", 2.5, Center);
    for (i, label) in CH.into_iter().enumerate() {
        let x = i as f32 * 8.0;
        b.l(x, -12.0, x, -6.0).pin(x, -12.0, label);
    }
    b.l(0.0, 6.0, 0.0, 12.0).l(8.0, 6.0, 8.0, 12.0).pin(0.0, 12.0, "L+").pin(8.0, 12.0, "M");
    v.push(def("plcbox_di8", "PLC card 8 digital inputs", C::BlackBox, "A", MAIN).build(&mut b));

    b.l(0.0, -12.0, 0.0, -6.0).l(8.0, -12.0, 8.0, -6.0).pin(0.0, -12.0, "L+").pin(8.0, -12.0, "M");
    b.rect(-4.0, -6.0, 60.0, 6.0, false).text(28.0, 0.0, "DO 8 x 24 V DC 0.5 A", 2.5, Center);
    for (i, label) in CH.into_iter().enumerate() {
        let x = i as f32 * 8.0;
        b.l(x, 6.0, x, 12.0).pin(x, 12.0, label);
    }
    v.push(def("plcbox_do8", "PLC card 8 digital outputs", C::BlackBox, "A", MAIN).build(&mut b));

    // ---------------------------------------------------------------- single-line
    const OVERVIEW: Func = Func::Overview;
    // Three strokes on the lead mark a 3-phase connection.
    let three = |b: &mut B| {
        for y in [-6.6, -5.6, -4.6] {
            b.l(-1.0, y + 0.4, 1.0, y - 0.4);
        }
    };
    b.l(0.0, -8.0, 0.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(0.0, 2.0, -2.0, -2.4).breaker_x(0.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_cb", "Circuit breaker (single-line)", C::SingleLine, "Q", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(0.0, 2.0, -2.0, -2.4).disconnector_bar(0.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_disc", "Switch-disconnector (single-line)", C::SingleLine, "Q", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(0.0, 2.0, -2.0, -2.4).breaker_x(0.0);
    b.dl(-1.0, 0.0, -4.0, 0.0).rect(-11.0, -1.5, -4.0, 1.5, false).text(-7.5, 0.0, "I> θ>", 1.4, Center);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_mps", "Motor protection switch (single-line)", C::SingleLine, "Q", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -3.0).rect(-2.0, -3.0, 2.0, 3.0, false).l(0.0, 3.0, 0.0, 8.0);
    b.poly(&[(0.0, -3.0), (0.0, -1.0), (1.2, -1.0), (1.2, 1.0), (0.0, 1.0), (0.0, 3.0)], false, false);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_ol", "Overload relay (single-line)", C::SingleLine, "F", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(0.0, 2.0, -2.0, -2.4).arc(0.0, -1.3, 0.7, 180.0, 360.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_contactor", "Contactor (single-line)", C::SingleLine, "K", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).rect(-1.2, -2.0, 1.2, 4.0, false).l(0.0, 4.0, 0.0, 8.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_fuse", "Fuse (single-line)", C::SingleLine, "F", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).circle(0.0, 3.0, 5.0, false);
    b.text(0.0, 1.8, "M", 3.0, Center).text(0.0, 5.0, "3~", 2.0, Center);
    three(&mut b);
    b.pin(0.0, -8.0, "");
    v.push(def("sl_motor", "Motor (single-line)", C::SingleLine, "M", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -6.0).circle(0.0, -2.5, 3.5, false).circle(0.0, 2.5, 3.5, false).l(0.0, 6.0, 0.0, 8.0);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_trafo", "Transformer (single-line)", C::SingleLine, "T", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(0.0, 2.0, -2.0, -2.4).disconnector_bar(0.0);
    fuse_blade(&mut b, 0.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_fuse_switch", "Fuse switch-disconnector (single-line)", C::SingleLine, "Q", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -3.0).rect(-3.0, -3.0, 3.0, 3.0, false).text(0.0, 0.0, "Wh", 1.8, Center).l(0.0, 3.0, 0.0, 8.0);
    three(&mut b);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_meter", "Energy meter (single-line)", C::SingleLine, "P", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -4.0).rect(-4.0, -4.0, 4.0, 4.0, false).l(-4.0, 4.0, 4.0, -4.0).l(0.0, 4.0, 0.0, 8.0);
    b.text(-1.8, -1.8, "=", 2.2, Center).text(1.8, 1.8, "~", 2.2, Center);
    b.pin(0.0, -8.0, "").pin(0.0, 8.0, "");
    v.push(def("sl_inverter", "Inverter (single-line)", C::SingleLine, "T", OVERVIEW).build(&mut b));

    b.rect(-4.0, -4.0, 4.0, 4.0, false).l(-4.0, 4.0, 4.0, -4.0).l(0.0, 4.0, 0.0, 8.0);
    b.l(-9.0, -7.0, -5.5, -3.5).poly(&[(-5.5, -3.5), (-7.1, -3.9), (-5.9, -5.1)], true, true);
    b.pin(0.0, 8.0, "");
    v.push(def("sl_pv", "PV generator (single-line)", C::SingleLine, "G", OVERVIEW).build(&mut b));

    b.l(0.0, -8.0, 0.0, -1.0).l(-3.0, -1.0, 3.0, -1.0).l(-1.5, 1.0, 1.5, 1.0).l(0.0, 1.0, 0.0, 3.0);
    b.pin(0.0, -8.0, "");
    v.push(def("sl_battery", "Battery (single-line)", C::SingleLine, "G", OVERVIEW).build(&mut b));

    // ---------------------------------------------------------------- NFPA / JIC
    b.l(0.0, -4.0, 0.0, -3.0).circle(0.0, 0.0, 3.0, false).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("nfpa_coil", "Relay coil (NFPA)", C::Nfpa, "K", MAIN_MIRROR).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_no", "NO contact (NFPA)", C::Nfpa, "K", Func::Aux(ContactKind::No)).auto().build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.l(-2.4, 2.4, 2.4, -2.4);
    b.pin(0.0, -4.0, "21").pin(0.0, 4.0, "22");
    v.push(def("nfpa_nc", "NC contact (NFPA)", C::Nfpa, "K", Func::Aux(ContactKind::Nc)).auto().build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    b.l(-2.0, -3.2, -2.0, 3.2).l(-2.0, 0.0, -4.5, 0.0).l(-4.5, -1.5, -4.5, 1.5);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_pb_no", "Push button NO (NFPA)", C::Nfpa, "S", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    b.l(0.6, -3.2, 0.6, 3.2).l(0.6, 0.0, -4.5, 0.0).l(-4.5, -1.5, -4.5, 1.5);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("nfpa_pb_nc", "Push button NC (NFPA)", C::Nfpa, "S", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).arc(0.0, -1.0, 1.0, 270.0, 450.0).arc(0.0, 1.0, 1.0, 270.0, 450.0).l(0.0, 2.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("nfpa_ol", "Overload heater (NFPA)", C::Nfpa, "F", MAIN_MIRROR).build(&mut b));

    let r = 2.5 * std::f32::consts::FRAC_1_SQRT_2;
    b.l(0.0, -4.0, 0.0, -2.5).circle(0.0, 0.0, 2.5, false).l(0.0, 2.5, 0.0, 4.0);
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.l(sx * (r + 0.4), sy * (r + 0.4), sx * (r + 1.4), sy * (r + 1.4));
    }
    b.field(FieldKind::Label, 0.0, 0.0, 2.2, Center);
    b.pin(0.0, -4.0, "X1").pin(0.0, 4.0, "X2");
    v.push(def("nfpa_lamp", "Pilot light (NFPA)", C::Nfpa, "P", MAIN).label("R").build(&mut b));

    // Timed contacts: NOTC (normally open, timed closed) and NCTO (normally closed, timed open).
    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.text(3.0, 0.0, "TC", 1.4, Left);
    b.pin(0.0, -4.0, "17").pin(0.0, 4.0, "18");
    v.push(def("nfpa_notc", "NO contact, timed closing (NFPA)", C::Nfpa, "K", Func::Aux(ContactKind::NoOnDelay)).auto().build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.l(-2.4, 2.4, 2.4, -2.4).text(3.0, 0.0, "TO", 1.4, Left);
    b.pin(0.0, -4.0, "15").pin(0.0, 4.0, "16");
    v.push(def("nfpa_ncto", "NC contact, timed opening (NFPA)", C::Nfpa, "K", Func::Aux(ContactKind::NcOnDelay)).auto().build(&mut b));

    // Limit switches: blade with the actuating wedge.
    b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    b.l(0.5, 2.0, -2.0, -2.4).poly(&[(-1.2, -1.0), (-2.6, -0.4), (-0.9, 0.0)], true, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_ls_no", "Limit switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    b.l(0.5, 2.0, 1.0, -3.2).poly(&[(0.8, -0.6), (-0.6, -0.2), (0.7, 0.4)], true, false);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("nfpa_ls_nc", "Limit switch NC (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    b.l(0.5, 2.0, -2.0, -2.4).l(-1.0, -0.4, -4.0, -0.4).l(-4.0, -0.4, -5.0, -1.8);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_ss", "Selector switch (NFPA)", C::Nfpa, "S", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).rect(-1.4, -2.0, 1.4, 2.0, false).arc(0.0, -1.0, 1.0, 90.0, 270.0).arc(0.0, 1.0, 1.0, 270.0, 450.0).l(0.0, 2.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("nfpa_fuse", "Fuse (NFPA)", C::Nfpa, "F", MAIN).build(&mut b));

    // Circuit breaker and disconnect: open contacts with arcs (breaker) or blades
    // (disconnect), operated together.
    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, -3.1).circle(x, -2.5, 0.6, false).circle(x, 2.5, 0.6, false).l(x, 3.1, x, 4.0);
        b.arc(x + 1.5, 0.0, 2.6, 125.0, 235.0);
    }
    b.dl(-1.1, 0.0, 14.9, 0.0);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("nfpa_cb3", "Circuit breaker 3-pole (NFPA)", C::Nfpa, "Q", MAIN).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, -3.1).circle(x, -2.5, 0.6, false).circle(x, 2.5, 0.6, false).l(x, 3.1, x, 4.0);
        b.l(x + 0.5, 2.0, x - 2.0, -2.4);
    }
    b.dl(-1.2, 0.0, 14.8, 0.0);
    b.pin(0.0, -4.0, "1").pin(8.0, -4.0, "3").pin(16.0, -4.0, "5");
    b.pin(0.0, 4.0, "2").pin(8.0, 4.0, "4").pin(16.0, 4.0, "6");
    v.push(def("nfpa_disc3", "Disconnect switch 3-pole (NFPA)", C::Nfpa, "Q", MAIN).build(&mut b));

    b.circle(8.0, 8.0, 6.0, false).text(8.0, 8.0, "MTR", 2.2, Center);
    b.l(0.0, -4.0, 0.0, 8.0).l(0.0, 8.0, 2.0, 8.0).l(8.0, -4.0, 8.0, 2.0);
    b.l(16.0, -4.0, 16.0, 8.0).l(16.0, 8.0, 14.0, 8.0).l(24.0, -4.0, 24.0, 14.0).l(24.0, 14.0, 8.0, 14.0);
    b.pin(0.0, -4.0, "T1").pin(8.0, -4.0, "T2").pin(16.0, -4.0, "T3").pin(24.0, -4.0, "GND");
    v.push(def("nfpa_motor", "Motor (NFPA)", C::Nfpa, "M", MAIN).build(&mut b));

    // Further NFPA / JIC devices: the open switch body with its operator.
    let sw = |b: &mut B| {
        b.l(0.0, -4.0, 0.0, -3.1).circle(0.0, -2.5, 0.6, false).circle(0.0, 2.5, 0.6, false).l(0.0, 3.1, 0.0, 4.0);
    };
    sw(&mut b);
    b.l(0.6, -3.2, 0.6, 3.2).l(0.6, 0.0, -4.0, 0.0).arc(-5.5, 0.0, 1.5, 90.0, 270.0).l(-5.5, -1.5, -5.5, 1.5);
    b.pin(0.0, -4.0, "11").pin(0.0, 4.0, "12");
    v.push(def("nfpa_estop", "Emergency stop, mushroom head (NFPA)", C::Nfpa, "S", MAIN).build(&mut b));

    sw(&mut b);
    b.l(0.5, 2.0, -2.0, -2.4).l(-1.0, -0.4, -4.0, -0.4).poly(&[(-4.0, -0.4), (-5.5, -1.6), (-6.0, 0.6), (-4.5, 0.8)], true, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_foot_no", "Foot switch NO (NFPA)", C::Nfpa, "S", MAIN).build(&mut b));

    sw(&mut b);
    b.l(0.5, 2.0, -2.0, -2.4).l(-0.8, -0.2, -3.5, -0.2).l(-3.5, -0.2, -3.5, 1.5).circle(-3.5, 2.5, 1.0, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_fs_no", "Float switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    sw(&mut b);
    b.l(0.5, 2.0, -2.0, -2.4).l(-0.8, -0.2, -3.0, -0.2).arc(-4.5, -0.2, 1.5, 90.0, 270.0).l(-4.5, -1.7, -4.5, 1.3);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_ps_no", "Pressure switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    sw(&mut b);
    b.l(0.5, 2.0, -2.0, -2.4).l(-0.8, -0.2, -2.5, -0.2);
    b.poly(&[(-2.5, -0.2), (-3.0, -1.2), (-3.6, 0.8), (-4.2, -1.2), (-4.8, 0.8), (-5.3, -0.2)], false, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_ts_no", "Temperature switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    sw(&mut b);
    b.l(0.5, 2.0, -2.0, -2.4).l(-0.8, -0.2, -3.0, -0.2).poly(&[(-3.0, -0.2), (-3.0, -2.2), (-5.0, -1.2)], true, false);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_flow_no", "Flow switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.poly(&[(-6.0, 0.0), (-4.5, -1.5), (-3.0, 0.0), (-4.5, 1.5)], true, false).l(-3.0, 0.0, -2.4, 0.0);
    b.pin(0.0, -4.0, "13").pin(0.0, 4.0, "14");
    v.push(def("nfpa_prox_no", "Proximity switch NO (NFPA)", C::Nfpa, "B", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).arc(0.0, -2.0, 1.0, -90.0, 90.0).arc(0.0, 0.0, 1.0, -90.0, 90.0).arc(0.0, 2.0, 1.0, -90.0, 90.0).l(0.0, 3.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("nfpa_sol", "Solenoid (NFPA)", C::Nfpa, "Y", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).circle(0.0, 0.0, 3.0, false).l(0.0, 3.0, 0.0, 4.0).text(0.0, 0.0, "TR", 1.6, Center);
    b.pin(0.0, -4.0, "A1").pin(0.0, 4.0, "A2");
    v.push(def("nfpa_tr_coil", "Timer coil (NFPA)", C::Nfpa, "K", MAIN_MIRROR).build(&mut b));

    for i in 0..4 {
        let x = 1.0 + 2.0 * i as f32;
        b.arc(x, -2.0, 1.0, 180.0, 360.0).arc(x, 2.0, 1.0, 0.0, 180.0);
    }
    b.l(0.0, -0.4, 8.0, -0.4).l(0.0, 0.4, 8.0, 0.4);
    b.l(0.0, -8.0, 0.0, -2.0).l(8.0, -8.0, 8.0, -2.0).l(0.0, 2.0, 0.0, 8.0).l(8.0, 2.0, 8.0, 8.0);
    b.pin(0.0, -8.0, "H1").pin(8.0, -8.0, "H2").pin(0.0, 8.0, "X1").pin(8.0, 8.0, "X2");
    v.push(def("nfpa_xfmr", "Control transformer (NFPA)", C::Nfpa, "T", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -2.0).poly(&[(-2.0, -2.0), (2.0, -2.0), (3.5, 2.0), (-3.5, 2.0)], true, false).l(0.0, 2.0, 0.0, 4.0);
    b.pin(0.0, -4.0, "X1").pin(0.0, 4.0, "X2");
    v.push(def("nfpa_horn", "Horn (NFPA)", C::Nfpa, "P", MAIN).build(&mut b));

    sw(&mut b);
    b.arc(1.5, 0.0, 2.6, 125.0, 235.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("nfpa_cb1", "Circuit breaker 1-pole (NFPA)", C::Nfpa, "Q", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).l(-2.2, -1.0, 2.2, -1.0).l(-2.2, 1.0, 2.2, 1.0).l(0.0, 1.0, 0.0, 4.0);
    b.l(-2.4, 2.4, 2.4, -2.4).text(3.0, 0.0, "OL", 1.4, Left);
    b.pin(0.0, -4.0, "95").pin(0.0, 4.0, "96");
    v.push(def("nfpa_ol_nc", "Overload contact NC (NFPA)", C::Nfpa, "F", Func::Aux(ContactKind::Nc)).build(&mut b));

    for x in [0.0, 8.0, 16.0] {
        b.l(x, -4.0, x, -1.0).l(x - 2.2, -1.0, x + 2.2, -1.0).l(x - 2.2, 1.0, x + 2.2, 1.0).l(x, 1.0, x, 4.0);
    }
    b.dl(2.4, 0.0, 5.6, 0.0).dl(10.4, 0.0, 13.6, 0.0);
    b.pin(0.0, -4.0, "L1").pin(8.0, -4.0, "L2").pin(16.0, -4.0, "L3");
    b.pin(0.0, 4.0, "T1").pin(8.0, 4.0, "T2").pin(16.0, 4.0, "T3");
    v.push(def("nfpa_main3", "Main contacts 3-pole (NFPA)", C::Nfpa, "K", Func::Aux(ContactKind::MainNo)).build(&mut b));

    // ---------------------------------------------------------------- fluid power
    // Small filled arrow head at `tip` pointing in direction (dx, dy).
    let head = |b: &mut B, tx: f32, ty: f32, dx: f32, dy: f32| {
        let (nx, ny) = (-dy, dx);
        b.poly(&[(tx, ty), (tx - dx * 1.4 + nx * 0.7, ty - dy * 1.4 + ny * 0.7), (tx - dx * 1.4 - nx * 0.7, ty - dy * 1.4 - ny * 0.7)], true, true);
    };
    b.l(0.0, -8.0, 0.0, -4.0).circle(0.0, 0.0, 4.0, false).l(0.0, 4.0, 0.0, 8.0);
    b.poly(&[(0.0, -4.0), (-1.6, -1.4), (1.6, -1.4)], true, true);
    b.pin(0.0, -8.0, "P").pin(0.0, 8.0, "S");
    v.push(def("fl_pump", "Hydraulic pump", C::Fluid, "GP", MAIN).build(&mut b));

    b.rect(-4.0, -6.0, 20.0, 0.0, false).l(4.0, -6.0, 4.0, 0.0).l(4.0, -3.0, 28.0, -3.0);
    b.l(0.0, 0.0, 0.0, 4.0).l(16.0, 0.0, 16.0, 4.0);
    b.pin(0.0, 4.0, "1").pin(16.0, 4.0, "2");
    v.push(def("fl_cylinder", "Cylinder, double-acting", C::Fluid, "MM", MAIN).build(&mut b));

    // 4/2 directional valve, solenoid operated, spring return; right box = rest position.
    b.rect(-12.0, -4.0, 0.0, 4.0, false).rect(0.0, -4.0, 12.0, 4.0, false);
    b.l(4.0, 4.0, 4.0, -3.0);
    head(&mut b, 4.0, -4.0, 0.0, -1.0);
    b.l(8.0, -4.0, 8.0, 3.0);
    head(&mut b, 8.0, 4.0, 0.0, 1.0);
    b.l(-8.0, 4.0, -4.6, -3.2);
    head(&mut b, -4.0, -4.0, 0.5, -1.0);
    b.l(-4.0, 4.0, -7.4, -3.2);
    head(&mut b, -8.0, -4.0, -0.5, -1.0);
    b.rect(-16.0, -2.0, -12.0, 2.0, false).l(-16.0, 2.0, -12.0, -2.0);
    b.poly(&[(12.0, 0.0), (12.8, -1.5), (13.6, 1.5), (14.4, -1.5), (15.2, 1.5), (16.0, 0.0)], false, false);
    b.l(4.0, -8.0, 4.0, -4.0).l(8.0, -8.0, 8.0, -4.0).l(4.0, 4.0, 4.0, 8.0).l(8.0, 4.0, 8.0, 8.0);
    b.pin(4.0, -8.0, "A").pin(8.0, -8.0, "B").pin(4.0, 8.0, "P").pin(8.0, 8.0, "T");
    v.push(def("fl_valve42", "4/2 directional valve", C::Fluid, "QM", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).poly(&[(0.0, -3.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 0.0)], true, false).l(0.0, 3.0, 0.0, 4.0);
    b.dl(-3.0, 0.0, 3.0, 0.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_filter", "Filter", C::Fluid, "HQ", MAIN).build(&mut b));

    b.circle(0.0, 0.0, 2.5, false).l(0.0, 2.5, 0.0, 4.0).l(-1.4, 1.4, 1.2, -1.2);
    head(&mut b, 1.7, -1.7, 0.7, -0.7);
    b.pin(0.0, 4.0, "1");
    v.push(def("fl_gauge", "Pressure gauge", C::Fluid, "BP", MAIN).build(&mut b));

    b.poly(&[(-3.0, 0.0), (-3.0, 4.0), (3.0, 4.0), (3.0, 0.0)], false, false).l(0.0, -4.0, 0.0, 3.0);
    b.pin(0.0, -4.0, "1");
    v.push(def("fl_tank", "Tank", C::Fluid, "CM", MAIN).build(&mut b));

    // Pressure source (compressed air supply).
    b.circle(0.0, 0.0, 2.5, false).circle(0.0, 0.0, 0.8, true).l(0.0, 2.5, 0.0, 4.0);
    b.pin(0.0, 4.0, "1");
    v.push(def("fl_source", "Compressed air supply", C::Fluid, "GQ", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.0).rect(-2.0, -1.0, 2.0, 3.0, false).dl(-2.0, 0.0, 2.0, 0.0).dl(-2.0, 1.0, 2.0, 1.0).dl(-2.0, 2.0, 2.0, 2.0);
    b.pin(0.0, -4.0, "3");
    v.push(def("fl_silencer", "Silencer", C::Fluid, "RN", MAIN).build(&mut b));

    // One-way flow control valve: throttle with a check valve in parallel.
    b.l(0.0, -8.0, 0.0, 8.0).arc(-2.6, 0.0, 2.4, -50.0, 50.0).arc(2.6, 0.0, 2.4, 130.0, 230.0);
    b.l(0.0, -5.0, 6.0, -5.0).l(6.0, -5.0, 6.0, -1.6).circle(6.0, 0.0, 1.2, false).l(4.6, -1.6, 6.0, 0.6).l(7.4, -1.6, 6.0, 0.6);
    b.l(6.0, 1.2, 6.0, 5.0).l(6.0, 5.0, 0.0, 5.0);
    b.pin(0.0, -8.0, "1").pin(0.0, 8.0, "2");
    v.push(def("fl_throttle", "One-way flow control valve", C::Fluid, "RN", MAIN).build(&mut b));

    // 5/2 directional valve, solenoid operated, spring return.
    b.rect(-12.0, -4.0, 0.0, 4.0, false).rect(0.0, -4.0, 12.0, 4.0, false);
    b.l(4.0, 4.0, 4.0, -3.0);
    head(&mut b, 4.0, -4.0, 0.0, -1.0);
    b.l(8.0, -4.0, 8.0, 3.0);
    head(&mut b, 8.0, 4.0, 0.0, 1.0);
    b.l(-8.0, 4.0, -4.6, -3.2);
    head(&mut b, -4.0, -4.0, 0.5, -1.0);
    b.l(-4.0, 4.0, -7.4, -3.2);
    head(&mut b, -8.0, -4.0, -0.5, -1.0);
    b.l(-2.0, 4.0, -2.0, 2.6).l(-3.0, 2.6, -1.0, 2.6).l(10.0, 4.0, 10.0, 2.6).l(9.0, 2.6, 11.0, 2.6);
    b.rect(-16.0, -2.0, -12.0, 2.0, false).l(-16.0, 2.0, -12.0, -2.0);
    b.poly(&[(12.0, 0.0), (12.8, -1.5), (13.6, 1.5), (14.4, -1.5), (15.2, 1.5), (16.0, 0.0)], false, false);
    b.l(4.0, -8.0, 4.0, -4.0).l(8.0, -8.0, 8.0, -4.0).l(0.0, 4.0, 0.0, 8.0).l(4.0, 4.0, 4.0, 8.0).l(8.0, 4.0, 8.0, 8.0);
    b.pin(4.0, -8.0, "4").pin(8.0, -8.0, "2").pin(0.0, 8.0, "5").pin(4.0, 8.0, "1").pin(8.0, 8.0, "3");
    v.push(def("fl_valve52", "5/2 directional valve", C::Fluid, "QM", MAIN).build(&mut b));

    b.rect(-4.0, -6.0, 20.0, 0.0, false).l(4.0, -6.0, 4.0, 0.0).l(4.0, -3.0, 28.0, -3.0);
    b.poly(&[(6.0, -3.0), (7.0, -5.0), (8.0, -1.0), (9.0, -5.0), (10.0, -1.0), (11.0, -5.0), (12.0, -3.0)], false, false);
    b.l(0.0, 0.0, 0.0, 4.0);
    b.pin(0.0, 4.0, "1");
    v.push(def("fl_cylinder_sa", "Cylinder, single-acting", C::Fluid, "MM", MAIN).build(&mut b));

    // 3/2 directional valve, solenoid operated, spring return, normally closed.
    b.rect(-12.0, -4.0, 0.0, 4.0, false).rect(0.0, -4.0, 12.0, 4.0, false);
    b.l(8.0, 4.0, 4.6, -3.2);
    head(&mut b, 4.0, -4.0, -0.5, -1.0);
    b.l(4.0, 4.0, 4.0, 2.6).l(3.0, 2.6, 5.0, 2.6);
    b.l(-8.0, 4.0, -8.0, -3.0);
    head(&mut b, -8.0, -4.0, 0.0, -1.0);
    b.l(-4.0, 4.0, -4.0, 2.6).l(-5.0, 2.6, -3.0, 2.6);
    b.rect(-16.0, -2.0, -12.0, 2.0, false).l(-16.0, 2.0, -12.0, -2.0);
    b.poly(&[(12.0, 0.0), (12.8, -1.5), (13.6, 1.5), (14.4, -1.5), (15.2, 1.5), (16.0, 0.0)], false, false);
    b.l(4.0, -8.0, 4.0, -4.0).l(4.0, 4.0, 4.0, 8.0).l(8.0, 4.0, 8.0, 8.0);
    b.pin(4.0, -8.0, "2").pin(4.0, 8.0, "1").pin(8.0, 8.0, "3");
    v.push(def("fl_valve32", "3/2 directional valve", C::Fluid, "QM", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -1.7).circle(0.0, -0.5, 1.2, false).l(-1.8, -0.5, 0.0, 1.8).l(1.8, -0.5, 0.0, 1.8).l(0.0, 1.8, 0.0, 4.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_check", "Non-return valve", C::Fluid, "RM", MAIN).build(&mut b));

    // Pressure relief valve: pilot from P against the spring, opens to T.
    b.rect(-3.0, -3.0, 3.0, 3.0, false).l(0.0, 3.0, 0.0, 4.0).l(3.0, 0.0, 4.0, 0.0);
    b.l(-1.0, 2.0, 1.4, -1.4);
    head(&mut b, 2.0, -2.0, 0.7, -0.7);
    b.poly(&[(-3.0, -1.5), (-3.6, -2.4), (-4.2, -0.6), (-4.8, -2.4), (-5.4, -0.6), (-6.0, -1.5)], false, false);
    b.dl(0.0, 3.6, -4.5, 3.6).dl(-4.5, 3.6, -4.5, 1.0);
    b.pin(0.0, 4.0, "P").pin(4.0, 0.0, "T");
    v.push(def("fl_relief", "Pressure relief valve", C::Fluid, "RM", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, -3.0).poly(&[(-2.5, -3.0), (2.5, -3.0), (0.0, 0.0)], true, false).poly(&[(-2.5, 3.0), (2.5, 3.0), (0.0, 0.0)], true, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(0.0, 0.0, -4.0, 0.0).l(-4.0, -1.2, -4.0, 1.2);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_shutoff", "Ball valve", C::Fluid, "QM", MAIN).build(&mut b));

    b.l(0.0, -4.0, 0.0, 4.0).arc(-2.6, 0.0, 2.4, -50.0, 50.0).arc(2.6, 0.0, 2.4, 130.0, 230.0);
    b.l(-2.6, 2.6, 2.6, -2.6);
    head(&mut b, 3.1, -3.1, 0.7, -0.7);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_flow_valve", "Flow control valve, adjustable", C::Fluid, "RN", MAIN).build(&mut b));

    b.rect(-3.0, -3.0, 3.0, 3.0, false).l(0.0, -4.0, 0.0, -3.0).l(0.0, 3.0, 0.0, 4.0).l(0.0, 2.0, 0.0, -1.6);
    head(&mut b, 0.0, -2.4, 0.0, -1.0);
    b.poly(&[(3.0, 1.5), (3.6, 0.6), (4.2, 2.4), (4.8, 0.6), (5.4, 2.4), (6.0, 1.5)], false, false);
    b.dl(0.0, -3.6, 4.5, -3.6).dl(4.5, -3.6, 4.5, -1.0);
    b.pin(0.0, 4.0, "1").pin(0.0, -4.0, "2");
    v.push(def("fl_regulator", "Pressure regulator", C::Fluid, "RP", MAIN).build(&mut b));

    // Service unit: filter, regulator and lubricator in one housing.
    b.rect(-8.0, -3.0, 8.0, 3.0, false).l(-12.0, 0.0, -8.0, 0.0).l(8.0, 0.0, 12.0, 0.0);
    b.poly(&[(-7.0, 0.0), (-5.0, -2.0), (-3.0, 0.0), (-5.0, 2.0)], true, false).dl(-7.0, 0.0, -3.0, 0.0);
    b.l(-1.2, 1.6, 0.8, -1.2);
    head(&mut b, 1.2, -1.8, 0.6, -0.8);
    b.poly(&[(3.0, 0.0), (5.0, -2.0), (7.0, 0.0), (5.0, 2.0)], true, false).circle(5.0, 0.0, 0.5, true);
    b.pin(-12.0, 0.0, "1").pin(12.0, 0.0, "2");
    v.push(def("fl_frl", "Service unit (filter, regulator, lubricator)", C::Fluid, "AZ", MAIN).build(&mut b));

    b.l(-2.5, -1.0, -2.5, 2.0).l(2.5, -1.0, 2.5, 2.0).arc(0.0, -1.0, 2.5, 180.0, 360.0).arc(0.0, 2.0, 2.5, 0.0, 180.0).l(-2.5, 0.5, 2.5, 0.5);
    b.l(0.0, 4.5, 0.0, 8.0);
    b.pin(0.0, 8.0, "1");
    v.push(def("fl_accumulator", "Accumulator", C::Fluid, "CM", MAIN).build(&mut b));

    b.circle(0.0, 0.0, 4.0, false).poly(&[(0.0, -1.2), (-1.6, -3.66), (1.6, -3.66)], true, true);
    b.l(0.0, -8.0, 0.0, -4.0).l(0.0, 4.0, 0.0, 8.0);
    b.pin(0.0, -8.0, "A").pin(0.0, 8.0, "B");
    v.push(def("fl_hyd_motor", "Hydraulic motor", C::Fluid, "MM", MAIN).build(&mut b));

    b.circle(0.0, 0.0, 4.0, false).poly(&[(0.0, -4.0), (-1.6, -1.4), (1.6, -1.4)], true, false);
    b.l(0.0, -8.0, 0.0, -4.0).l(0.0, 4.0, 0.0, 8.0);
    b.pin(0.0, -8.0, "P").pin(0.0, 8.0, "S");
    v.push(def("fl_compressor", "Compressor", C::Fluid, "GQ", MAIN).build(&mut b));

    // Compressed air treatment and distribution (ISO 1219-1).
    // Air receiver: a pressure vessel with rounded ends.
    b.l(0.0, -8.0, 0.0, -6.0).arc(0.0, -3.0, 3.0, 180.0, 360.0).l(-3.0, -3.0, -3.0, 3.0).l(3.0, -3.0, 3.0, 3.0).arc(0.0, 3.0, 3.0, 0.0, 180.0).l(0.0, 6.0, 0.0, 8.0);
    b.pin(0.0, -8.0, "1").pin(0.0, 8.0, "2");
    v.push(def("fl_receiver", "Air receiver", C::Fluid, "CM", MAIN).build(&mut b));
    // Dryer: the diamond of the conditioning devices with the drying agent below.
    b.l(0.0, -4.0, 0.0, -3.0).poly(&[(0.0, -3.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 0.0)], true, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(-3.0, 0.0, 3.0, 0.0).poly(&[(-1.5, 1.5), (1.5, 1.5), (0.0, 3.0)], true, true);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_dryer", "Air dryer", C::Fluid, "HQ", MAIN).build(&mut b));
    // Lubricator (oiler): the diamond with the oil drop feed.
    b.l(0.0, -4.0, 0.0, -3.0).poly(&[(0.0, -3.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 0.0)], true, false).l(0.0, 3.0, 0.0, 4.0);
    b.l(0.0, -3.0, 0.0, 1.0).circle(0.0, 1.6, 0.6, true);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_lubricator", "Lubricator (oiler)", C::Fluid, "HQ", MAIN).build(&mut b));
    // Self-sealing quick coupling: the outlet of a compressed air line.
    b.l(0.0, -4.0, 0.0, -2.0).l(-2.0, -2.0, 2.0, -2.0).arc(0.0, 0.0, 1.4, 180.0, 360.0).l(-2.0, 2.0, 2.0, 2.0).l(-1.4, 0.0, -1.4, 2.0).l(1.4, 0.0, 1.4, 2.0);
    b.pin(0.0, -4.0, "1");
    v.push(def("fl_coupling", "Quick coupling (outlet)", C::Fluid, "XM", MAIN).build(&mut b));

    b.poly(&[(0.0, -3.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 0.0)], true, false).l(0.0, -4.0, 0.0, -3.0).l(0.0, 3.0, 0.0, 4.0);
    b.l(-2.0, 0.0, 2.0, 0.0);
    head(&mut b, 2.8, 0.0, 1.0, 0.0);
    head(&mut b, -2.8, 0.0, -1.0, 0.0);
    b.pin(0.0, -4.0, "1").pin(0.0, 4.0, "2");
    v.push(def("fl_cooler", "Cooler", C::Fluid, "EP", MAIN).build(&mut b));

    // ---------------------------------------------------------------- P&ID
    // Process equipment has ports (pipes); instruments are overview functions of the
    // device that measures or actuates (sensor, valve drive), engineered elsewhere.
    b.l(-6.0, -6.0, -6.0, 6.0).l(6.0, -6.0, 6.0, 6.0).arc(0.0, -6.0, 6.0, 180.0, 360.0).arc(0.0, 6.0, 6.0, 0.0, 180.0);
    b.l(0.0, -16.0, 0.0, -12.0).l(0.0, 12.0, 0.0, 16.0);
    b.pin(0.0, -16.0, "N1").pin(0.0, 16.0, "N2");
    v.push(def("pid_vessel", "Vessel", C::Process, "CM", MAIN).build(&mut b));

    b.circle(0.0, 0.0, 4.0, false).l(-4.0, 0.0, -8.0, 0.0).l(0.0, -4.0, 4.0, -4.0).l(4.0, -4.0, 8.0, -4.0);
    b.l(-2.8, 2.8, -4.0, 6.0).l(2.8, 2.8, 4.0, 6.0).l(-4.0, 6.0, 4.0, 6.0);
    b.pin(-8.0, 0.0, "S").pin(8.0, -4.0, "P");
    v.push(def("pid_pump", "Centrifugal pump", C::Process, "GP", MAIN).build(&mut b));

    // Valves in a horizontal line: two triangles tip to tip.
    let bowtie = |b: &mut B, fill: bool| {
        b.poly(&[(-4.0, -2.5), (0.0, 0.0), (-4.0, 2.5)], true, fill).poly(&[(4.0, -2.5), (0.0, 0.0), (4.0, 2.5)], true, fill);
        b.l(-8.0, 0.0, -4.0, 0.0).l(4.0, 0.0, 8.0, 0.0);
    };
    bowtie(&mut b, false);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2");
    v.push(def("pid_valve", "Shut-off valve", C::Process, "QM", MAIN).build(&mut b));

    bowtie(&mut b, false);
    b.l(0.0, 0.0, 0.0, -4.0).arc(0.0, -4.0, 2.5, 180.0, 360.0).l(-2.5, -4.0, 2.5, -4.0);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2");
    v.push(def("pid_control_valve", "Control valve", C::Process, "QN", MAIN).build(&mut b));

    // Valve with an electric actuator (the actuator is a device of its own).
    bowtie(&mut b, false);
    b.l(0.0, 0.0, 0.0, -4.0).rect(-2.5, -9.0, 2.5, -4.0, false).text(0.0, -6.5, "M", 2.4, Center);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2");
    v.push(def("pid_motor_valve", "Motor-operated valve", C::Process, "QM", MAIN).build(&mut b));

    // In-line flow meter (water meter).
    b.l(-8.0, 0.0, -4.0, 0.0).circle(0.0, 0.0, 4.0, false).text(0.0, 0.0, "FQ", 2.0, Center).l(4.0, 0.0, 8.0, 0.0);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2");
    v.push(def("pid_flow_meter", "Flow meter", C::Process, "BF", MAIN).build(&mut b));

    // Where a pipe comes from or goes to outside the plant (water main, consumers).
    b.poly(&[(-6.0, -3.0), (0.0, -3.0), (3.0, 0.0), (0.0, 3.0), (-6.0, 3.0)], true, false).l(3.0, 0.0, 8.0, 0.0);
    b.pin(8.0, 0.0, "1");
    v.push(def("pid_connection", "Pipe connection (in / out)", C::Process, "UM", MAIN).build(&mut b));

    b.l(-8.0, 0.0, -3.0, 0.0).l(3.0, 0.0, 8.0, 0.0).rect(-3.0, -2.5, 3.0, 2.5, false).l(-3.0, 2.5, 3.0, -2.5);
    b.poly(&[(1.0, -1.6), (3.0, -2.5), (2.2, -0.6)], true, true);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2");
    v.push(def("pid_check_valve", "Check valve", C::Process, "RM", MAIN).build(&mut b));

    b.circle(0.0, 0.0, 4.0, false).poly(&[(-4.0, 0.0), (-1.5, -2.0), (1.5, 2.0), (4.0, 0.0)], false, false);
    b.l(-4.0, 0.0, -8.0, 0.0).l(4.0, 0.0, 8.0, 0.0).l(0.0, -4.0, 0.0, -8.0).l(0.0, 4.0, 0.0, 8.0);
    b.pin(-8.0, 0.0, "1").pin(8.0, 0.0, "2").pin(0.0, -8.0, "3").pin(0.0, 8.0, "4");
    v.push(def("pid_heat_exchanger", "Heat exchanger", C::Process, "EP", MAIN).build(&mut b));

    // Heat pump: heat taken in on one side, given off at a higher temperature to the
    // flow and return on the other.
    b.rect(-6.0, -6.0, 6.0, 6.0, false).l(-6.0, 6.0, 6.0, -6.0);
    b.poly(&[(-4.0, -1.0), (-2.0, -3.0), (0.0, -1.0)], false, false).poly(&[(0.0, 1.0), (2.0, 3.0), (4.0, 1.0)], false, false);
    b.l(6.0, -4.0, 8.0, -4.0).l(6.0, 4.0, 8.0, 4.0);
    b.pin(8.0, -4.0, "1").pin(8.0, 4.0, "2");
    v.push(def("pid_heat_pump", "Heat pump", C::Process, "EQ", MAIN).build(&mut b));

    // Buffer tank: hot water out at the top, cold water in at the bottom, flow and
    // return of the heat generator at the side.
    b.l(-6.0, -6.0, -6.0, 6.0).l(6.0, -6.0, 6.0, 6.0).arc(0.0, -6.0, 6.0, 180.0, 360.0).arc(0.0, 6.0, 6.0, 0.0, 180.0);
    b.l(0.0, -16.0, 0.0, -12.0).l(0.0, 12.0, 0.0, 16.0).l(-8.0, -4.0, -6.0, -4.0).l(-8.0, 4.0, -6.0, 4.0);
    b.dl(-6.0, 0.0, 6.0, 0.0);
    b.pin(0.0, -16.0, "N1").pin(-8.0, -4.0, "N2").pin(-8.0, 4.0, "N3").pin(0.0, 16.0, "N4");
    v.push(def("pid_buffer", "Buffer tank with four nozzles", C::Process, "CM", MAIN).build(&mut b));

    // Instrument bubble: label = function letters and loop number, e.g. "TIC 101".
    b.circle(0.0, 0.0, 4.0, false).l(-4.0, 0.0, 4.0, 0.0).dl(0.0, 4.0, 0.0, 8.0);
    b.field(FieldKind::Label, 0.0, -1.2, 1.8, Center);
    b.pin(0.0, 8.0, "");
    let mut instrument = def("pid_instrument", "Instrument (PCT loop)", C::Process, "B", Func::Overview).label("TI 1");
    instrument.show_tag = true;
    v.push(instrument.build(&mut b));

    iec::add(&mut v);
    national::add(&mut v);
    v
}

/// Built-in symbols.
pub fn library() -> &'static [SymbolDef] {
    static LIB: OnceLock<Vec<SymbolDef>> = OnceLock::new();
    LIB.get_or_init(build_library)
}

/// A built-in symbol. User symbols of a project are found with
/// [`Project::symbol`](crate::model::Project::symbol), which looks here first.
pub fn builtin(key: &str) -> Option<&'static SymbolDef> {
    library().iter().find(|d| d.key == key)
}

/// A symbol definition: a built-in one or a user symbol of a project (shared with
/// the project's symbol table).
#[derive(Clone, Debug)]
pub enum SymbolRef {
    Builtin(&'static SymbolDef),
    User(Arc<SymbolDef>),
}

impl std::ops::Deref for SymbolRef {
    type Target = SymbolDef;

    fn deref(&self) -> &SymbolDef {
        match self {
            SymbolRef::Builtin(d) => d,
            SymbolRef::User(d) => d,
        }
    }
}

#[cfg(test)]
mod tests;
