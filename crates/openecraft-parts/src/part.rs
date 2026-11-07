//! Part data: product groups, function templates, commercial / technical / mounting
//! data and the parts database container.

use crate::portal::{Attachment, Lifecycle};
use crate::{natural_cmp, AccessoryMount, ConnectionPoint, Mesh3d, Model2d, MountingData, Shared};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Product top group; also read in lower case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TopGroup {
    #[default]
    #[serde(alias = "electrical")]
    Electrical,
    #[serde(alias = "fluid")]
    Fluid,
    #[serde(alias = "mechanics")]
    Mechanics,
    #[serde(alias = "process")]
    Process,
}

impl TopGroup {
    pub const ALL: [TopGroup; 4] = [TopGroup::Electrical, TopGroup::Fluid, TopGroup::Mechanics, TopGroup::Process];

    pub fn title(self) -> &'static str {
        match self {
            TopGroup::Electrical => "Electrical engineering",
            TopGroup::Fluid => "Fluid power",
            TopGroup::Mechanics => "Mechanics",
            TopGroup::Process => "Process engineering",
        }
    }

    /// Internal value of the EPLAN generic product group.
    pub fn eplan_id(self) -> u16 {
        self as u16 + 1
    }

    pub fn from_eplan_id(id: u16) -> Option<TopGroup> {
        TopGroup::ALL.into_iter().find(|t| t.eplan_id() == id)
    }
}

/// Suggested product groups / subgroups (free text is allowed as well).
pub const PRODUCT_GROUPS: &[(&str, &[&str])] = &[
    ("Relays, contactors", &["Power contactor", "Auxiliary contact block", "Relay", "Timer relay", "Safety relay"]),
    ("Safety devices", &["Motor protection switch", "Circuit breaker", "Residual current device", "Fuse", "Overload relay", "Auxiliary switch", "Main switch", "Surge protection"]),
    ("Sensors, switches, pushbuttons", &["Pushbutton", "Emergency stop", "Selector switch", "Key switch", "Proximity sensor", "Position switch", "Pressure switch", "Temperature switch", "Level switch", "Transmitter", "Contact block"]),
    ("Signal devices", &["Indicator lamp", "Horn"]),
    ("Measuring instruments", &["Ammeter", "Voltmeter", "Current transformer", "Battery monitor", "System monitoring"]),
    ("Motors", &["Three-phase motor", "Single-phase motor", "Brake motor", "DC motor"]),
    ("Drives", &["Frequency converter", "Soft starter"]),
    ("Power supplies, transformers", &["Power supply", "Control transformer", "Battery", "DC-DC converter", "Inverter / charger", "Solar charge controller"]),
    ("Installation devices", &["Socket outlet", "Enclosure heater", "KNX actuator", "KNX system device"]),
    ("Terminals", &["Feed-through terminal", "PE terminal", "N terminal", "Disconnect terminal", "Fuse terminal", "End bracket", "Jumper", "Accessories"]),
    ("Connectors", &["Plug connector"]),
    ("PLC", &["Digital input module", "Digital output module", "Analog input module", "Analog output module", "CPU", "Bus coupler"]),
    ("Valves", &["Solenoid valve", "Directional valve"]),
    ("Fluid power components", &["Cylinder", "Pump", "Service unit"]),
    ("Passive components", &["Resistor", "Capacitor", "Diode", "Varistor"]),
    (
        "Mounting",
        &[
            "Mounting rail", "Cable duct", "Enclosure", "Busbar system", "Busbar holder", "PE busbar", "Plinth", "Side panel", "Rear wall", "Baying kit",
            "Gland plate", "Cable gland", "Cable clamp rail", "Rail standoff", "Filter fan", "Outlet filter", "Enclosure light", "Door handle",
            "Document holder", "Screw", "Nut", "Washer", "Rivet nut",
        ],
    ),
    ("Cables", &["Power cable", "Control cable"]),
    ("Wiring material", &["Ferrule"]),
    ("Marking", &["Device marker", "Legend plate", "Terminal marker", "Wire marker", "Cable marker"]),
];

/// Kinds of functions in a function template. Placed symbols map onto these kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FnKind {
    Coil,
    MainContacts3,
    MainContacts4,
    MainContact,
    No,
    Nc,
    Changeover,
    NoOnDelay,
    NcOnDelay,
    NoOffDelay,
    NcOffDelay,
    PushbuttonNo,
    PushbuttonNc,
    SwitchNo,
    SwitchNc,
    Disconnector3,
    Sensor,
    SensorNc,
    Transmitter,
    Thermistor,
    Breaker1,
    Breaker2,
    Breaker3,
    Breaker4,
    ResidualCurrent,
    MotorProtection,
    Overload,
    Fuse,
    Fuse3,
    SurgeProtection,
    Motor3,
    Motor1,
    MotorDc,
    Brake,
    FrequencyConverter,
    SoftStarter,
    Heater,
    SocketOutlet,
    Signal,
    Meter,
    PowerSupply,
    Transformer,
    CurrentTransformer,
    Battery,
    PvGenerator,
    Terminal,
    PlcInput,
    PlcOutput,
    PlcAnalogInput,
    PlcAnalogOutput,
    Valve,
    Actuator,
    DirectionalValve,
    Cylinder,
    Pump,
    FluidComponent,
    PlugPin,
    #[default]
    Generic,
}

impl FnKind {
    pub const ALL: [FnKind; 58] = [
        FnKind::Coil,
        FnKind::MainContacts3,
        FnKind::MainContacts4,
        FnKind::MainContact,
        FnKind::No,
        FnKind::Nc,
        FnKind::Changeover,
        FnKind::NoOnDelay,
        FnKind::NcOnDelay,
        FnKind::NoOffDelay,
        FnKind::NcOffDelay,
        FnKind::PushbuttonNo,
        FnKind::PushbuttonNc,
        FnKind::SwitchNo,
        FnKind::SwitchNc,
        FnKind::Disconnector3,
        FnKind::Sensor,
        FnKind::SensorNc,
        FnKind::Transmitter,
        FnKind::Thermistor,
        FnKind::Breaker1,
        FnKind::Breaker2,
        FnKind::Breaker3,
        FnKind::Breaker4,
        FnKind::ResidualCurrent,
        FnKind::MotorProtection,
        FnKind::Overload,
        FnKind::Fuse,
        FnKind::Fuse3,
        FnKind::SurgeProtection,
        FnKind::Motor3,
        FnKind::Motor1,
        FnKind::MotorDc,
        FnKind::Brake,
        FnKind::FrequencyConverter,
        FnKind::SoftStarter,
        FnKind::Heater,
        FnKind::SocketOutlet,
        FnKind::Signal,
        FnKind::Meter,
        FnKind::PowerSupply,
        FnKind::Transformer,
        FnKind::CurrentTransformer,
        FnKind::Battery,
        FnKind::PvGenerator,
        FnKind::Terminal,
        FnKind::PlcInput,
        FnKind::PlcOutput,
        FnKind::PlcAnalogInput,
        FnKind::PlcAnalogOutput,
        FnKind::Valve,
        FnKind::Actuator,
        FnKind::DirectionalValve,
        FnKind::Cylinder,
        FnKind::Pump,
        FnKind::FluidComponent,
        FnKind::PlugPin,
        FnKind::Generic,
    ];

    pub fn title(self) -> &'static str {
        match self {
            FnKind::Coil => "Coil",
            FnKind::MainContacts3 => "Main contacts 3-pole",
            FnKind::MainContacts4 => "Main contacts 4-pole",
            FnKind::MainContact => "Main contact 1-pole",
            FnKind::No => "NO contact",
            FnKind::Nc => "NC contact",
            FnKind::Changeover => "Changeover contact",
            FnKind::NoOnDelay => "NO contact, on-delay",
            FnKind::NcOnDelay => "NC contact, on-delay",
            FnKind::NoOffDelay => "NO contact, off-delay",
            FnKind::NcOffDelay => "NC contact, off-delay",
            FnKind::PushbuttonNo => "Pushbutton NO",
            FnKind::PushbuttonNc => "Pushbutton NC",
            FnKind::SwitchNo => "Switch NO",
            FnKind::SwitchNc => "Switch NC",
            FnKind::Disconnector3 => "Main switch 3-pole",
            FnKind::Sensor => "Sensor",
            FnKind::SensorNc => "Sensor NC",
            FnKind::Transmitter => "Measuring transducer",
            FnKind::Thermistor => "Thermistor (PTC)",
            FnKind::Breaker1 => "Circuit breaker 1-pole",
            FnKind::Breaker2 => "Circuit breaker 2-pole",
            FnKind::Breaker3 => "Circuit breaker 3-pole",
            FnKind::Breaker4 => "Circuit breaker 4-pole",
            FnKind::ResidualCurrent => "Residual current device",
            FnKind::MotorProtection => "Motor protection switch",
            FnKind::Overload => "Overload relay",
            FnKind::Fuse => "Fuse",
            FnKind::Fuse3 => "Fuse 3-pole",
            FnKind::SurgeProtection => "Surge protective device",
            FnKind::Motor3 => "Motor 3-phase",
            FnKind::Motor1 => "Motor 1-phase",
            FnKind::MotorDc => "Motor DC",
            FnKind::Brake => "Brake",
            FnKind::FrequencyConverter => "Frequency converter",
            FnKind::SoftStarter => "Soft starter",
            FnKind::Heater => "Heating element",
            FnKind::SocketOutlet => "Socket outlet",
            FnKind::Signal => "Signal device",
            FnKind::Meter => "Measuring instrument",
            FnKind::PowerSupply => "Power supply",
            FnKind::Transformer => "Transformer",
            FnKind::CurrentTransformer => "Current transformer",
            FnKind::Battery => "Battery",
            FnKind::PvGenerator => "PV generator",
            FnKind::Terminal => "Terminal",
            FnKind::PlcInput => "PLC input",
            FnKind::PlcOutput => "PLC output",
            FnKind::PlcAnalogInput => "PLC analog input",
            FnKind::PlcAnalogOutput => "PLC analog output",
            FnKind::Valve => "Valve coil",
            FnKind::Actuator => "Actuator",
            FnKind::DirectionalValve => "Directional valve",
            FnKind::Cylinder => "Cylinder",
            FnKind::Pump => "Pump",
            FnKind::FluidComponent => "Fluid component",
            FnKind::PlugPin => "Plug pin",
            FnKind::Generic => "Generic function",
        }
    }

    pub(crate) fn short(self) -> &'static str {
        match self {
            FnKind::Coil => "Coil",
            FnKind::MainContacts3 => "Main3",
            FnKind::MainContacts4 => "Main4",
            FnKind::MainContact => "Main1",
            FnKind::No => "NO",
            FnKind::Nc => "NC",
            FnKind::Changeover => "CO",
            FnKind::NoOnDelay => "NOOD",
            FnKind::NcOnDelay => "NCOD",
            FnKind::NoOffDelay => "NOFD",
            FnKind::NcOffDelay => "NCFD",
            FnKind::PushbuttonNo => "PBNO",
            FnKind::PushbuttonNc => "PBNC",
            FnKind::SwitchNo => "SWNO",
            FnKind::SwitchNc => "SWNC",
            FnKind::Disconnector3 => "DISC3",
            FnKind::Sensor => "Sensor",
            FnKind::SensorNc => "SensorNC",
            FnKind::Transmitter => "TX",
            FnKind::Thermistor => "PTC",
            FnKind::Breaker1 => "CB1",
            FnKind::Breaker2 => "CB2",
            FnKind::Breaker3 => "CB3",
            FnKind::Breaker4 => "CB4",
            FnKind::ResidualCurrent => "RCD",
            FnKind::MotorProtection => "MPS",
            FnKind::Overload => "OL",
            FnKind::Fuse => "Fuse",
            FnKind::Fuse3 => "Fuse3",
            FnKind::SurgeProtection => "SPD",
            FnKind::Motor3 => "M3",
            FnKind::Motor1 => "M1",
            FnKind::MotorDc => "MDC",
            FnKind::Brake => "Brake",
            FnKind::FrequencyConverter => "VFD",
            FnKind::SoftStarter => "SoftStart",
            FnKind::Heater => "Heater",
            FnKind::SocketOutlet => "Socket",
            FnKind::Signal => "Signal",
            FnKind::Meter => "Meter",
            FnKind::PowerSupply => "PSU",
            FnKind::Transformer => "Trafo",
            FnKind::CurrentTransformer => "CT",
            FnKind::Battery => "Battery",
            FnKind::PvGenerator => "PV",
            FnKind::Terminal => "Terminal",
            FnKind::PlcInput => "DI",
            FnKind::PlcOutput => "DO",
            FnKind::PlcAnalogInput => "AI",
            FnKind::PlcAnalogOutput => "AO",
            FnKind::Valve => "Valve",
            FnKind::Actuator => "Actuator",
            FnKind::DirectionalValve => "DirValve",
            FnKind::Cylinder => "Cyl",
            FnKind::Pump => "Pump",
            FnKind::FluidComponent => "Fluid",
            FnKind::PlugPin => "Pin",
            FnKind::Generic => "Generic",
        }
    }

    pub(crate) fn from_short(s: &str) -> Option<FnKind> {
        FnKind::ALL.into_iter().find(|k| k.short().eq_ignore_ascii_case(s.trim()))
    }
}

/// One function of a function template.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FunctionTemplate {
    pub kind: FnKind,
    /// Connection point designations in symbol order, separated by blanks ("13 14").
    pub pins: String,
    pub description: String,
}

impl FunctionTemplate {
    pub fn new(kind: FnKind, pins: &str) -> FunctionTemplate {
        FunctionTemplate { kind, pins: pins.into(), description: String::new() }
    }

    pub fn pin_list(&self) -> Vec<String> {
        self.pins.split_whitespace().map(String::from).collect()
    }
}

/// A part of an assembly with its quantity per assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Component {
    pub number: String,
    pub quantity: f32,
}

impl Default for Component {
    fn default() -> Self {
        Component { number: String::new(), quantity: 1.0 }
    }
}

/// `NUM:2|NUM2:1` (quantity 1 may be left out).
pub fn components_to_text(c: &[Component]) -> String {
    c.iter().map(|c| if c.quantity == 1.0 { c.number.clone() } else { format!("{}:{}", c.number, c.quantity) }).collect::<Vec<_>>().join("|")
}

pub fn components_from_text(s: &str) -> Vec<Component> {
    s.split('|')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| match x.rsplit_once(':') {
            Some((n, q)) if q.trim().replace(',', ".").parse::<f32>().is_ok() => Component { number: n.trim().to_string(), quantity: q.trim().replace(',', ".").parse().unwrap_or(1.0) },
            _ => Component { number: x.to_string(), quantity: 1.0 },
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Part {
    // General
    /// Unique key, conventionally `MANUFACTURER.TYPE`.
    pub number: String,
    pub type_number: String,
    pub order_number: String,
    pub manufacturer: String,
    pub supplier: String,
    pub description: String,
    pub note: String,
    pub top_group: TopGroup,
    pub group: String,
    pub subgroup: String,
    /// Identifying letter of devices using this part (device selection).
    pub identifier: String,
    // Commercial
    pub price: f32,
    pub currency: String,
    /// The price refers to this many units.
    pub price_unit: u32,
    pub pack_quantity: u32,
    pub quantity_unit: String,
    // Technical
    pub voltage: String,
    pub current: String,
    pub power: String,
    pub tech_data: String,
    pub certificates: String,
    pub datasheet: String,
    // Mounting
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub weight: f32,
    pub mounting: String,
    /// Typed mounting and placement data (clearances, holes, cutouts, power loss, …).
    #[serde(skip_serializing_if = "MountingData::is_empty")]
    pub mounting_data: MountingData,
    // Logic
    pub functions: Vec<FunctionTemplate>,
    /// Preferred project symbol key for a vendor-neutral library assignment.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    /// Original vendor records retained alongside the normalized part data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_data: Option<Shared<serde_json::Value>>,
    /// Group symbol macro reference in a vendor's loose parts XML.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub eplan_macro: String,
    /// Portable EPLAN representations including the native graphics and conversion diagnostics.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eplan_variants: Vec<Shared<serde_json::Value>>,
    /// Part numbers of accessories.
    pub accessories: Vec<String>,
    /// The part is an accessory: it only works attached to one of the parts it fits
    /// (see [`crate::accessory`]).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub accessory: bool,
    /// Where the accessory sits on the part it is attached to.
    #[serde(skip_serializing_if = "AccessoryMount::is_auto")]
    pub accessory_mount: AccessoryMount,
    /// Part numbers of the parts this accessory fits (the other direction of
    /// `accessories`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fits: Vec<String>,
    /// Parts this part is assembled from (assembly / module); the bill of materials
    /// lists them under it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    // Graphics
    /// Front view for mounting layouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_2d: Option<Shared<Model2d>>,
    /// 3D model for the panel layout (shared: project snapshots compare and clone it
    /// by pointer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_3d: Option<Shared<Mesh3d>>,
    /// Terminals for wiring in the panel layout; empty: generated from the function
    /// templates ([`Part::connection_points_or_default`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub connection_points: Vec<ConnectionPoint>,
    // Data portal
    /// Catalog of the parts server the part belongs to ([`crate::portal::Catalog::id`]).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub catalog: String,
    #[serde(skip_serializing_if = "Lifecycle::is_active")]
    pub lifecycle: Lifecycle,
    /// Part number of the part replacing this one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub successor: String,
    /// Files stored on the parts server (datasheets, images, models, macros).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// Server revision of the last change of this part (set by the parts server, 0: unknown).
    #[serde(skip_serializing_if = "is_zero")]
    pub revision: u64,
    /// Unix time of the last change on the parts server.
    #[serde(skip_serializing_if = "is_zero")]
    pub modified: u64,
    /// Id of the part on the EPLAN Data Portal it was loaded from (the portal finds
    /// parts by this id, not by number).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub portal_id: String,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl Default for Part {
    fn default() -> Self {
        Part {
            number: String::new(),
            type_number: String::new(),
            order_number: String::new(),
            manufacturer: String::new(),
            supplier: String::new(),
            description: String::new(),
            note: String::new(),
            top_group: TopGroup::Electrical,
            group: String::new(),
            subgroup: String::new(),
            identifier: String::new(),
            price: 0.0,
            currency: "EUR".into(),
            price_unit: 1,
            pack_quantity: 1,
            quantity_unit: "pcs".into(),
            voltage: String::new(),
            current: String::new(),
            power: String::new(),
            tech_data: String::new(),
            certificates: String::new(),
            datasheet: String::new(),
            width: 0.0,
            height: 0.0,
            depth: 0.0,
            weight: 0.0,
            mounting: String::new(),
            mounting_data: MountingData::default(),
            functions: Vec::new(),
            symbol: String::new(),
            source_data: None,
            eplan_macro: String::new(),
            eplan_variants: Vec::new(),
            accessories: Vec::new(),
            accessory: false,
            accessory_mount: AccessoryMount::Auto,
            fits: Vec::new(),
            components: Vec::new(),
            model_2d: None,
            model_3d: None,
            connection_points: Vec::new(),
            catalog: String::new(),
            lifecycle: Lifecycle::Active,
            successor: String::new(),
            attachments: Vec::new(),
            revision: 0,
            modified: 0,
            portal_id: String::new(),
        }
    }
}

impl Part {
    /// Source values take precedence. The supplied Beckhoff XML uses zero even
    /// for housings with a documented off-center clip. Drawings 0000000304,
    /// 0000000305 and 0000004049 place the rail axis 63.8 mm below the top.
    pub fn effective_rail_mount(&self) -> crate::RailMount {
        let mut rail = self.mounting_data.rail;
        if rail.center_offset != 0. || (self.height - 100.).abs() > 0.1 || (self.depth - 68.).abs() > 0.1 {
            return rail;
        }
        let Some(model) = self.number.strip_prefix("BEC.") else { return rail };
        let terminal = (self.width - 12.).abs() < 0.1
            && ["EL1", "EL2", "EL3", "EL4"].iter().any(|prefix| model.starts_with(prefix))
            && self.eplan_variants.iter().any(|v| {
                if v["function_type"].as_u64() != Some(4) {
                    return false;
                }
                let Some(pins) = v["symbol"]["pins"].as_array() else { return false };
                matches!(pins.len(), 8 | 16)
                    && (1..=pins.len()).all(|n| {
                        let label = n.to_string();
                        pins.iter().any(|p| p["label"].as_str() == Some(label.as_str()))
                    })
            });
        let coupler = model == "EK1100" && (self.width - 44.1).abs() < 0.2;
        if terminal || coupler {
            rail.center_offset = 63.8 - self.height / 2.;
        }
        rail
    }

    /// Width, height and depth: the mounting data, or the size of the 3D model when
    /// the part has no mounting data. `None` without either.
    pub fn size(&self) -> Option<[f32; 3]> {
        let model = self.model_3d.as_ref().map(|m| m.size()).filter(|s| s[0] > 0.0 && s[1] > 0.0);
        if self.width > 0.0 && self.height > 0.0 {
            let depth = if self.depth > 0.0 { self.depth } else { model.map(|s| s[2]).unwrap_or(0.0) };
            Some([self.width, self.height, depth])
        } else {
            model
        }
    }

    /// Price of `qty` units, respecting the price unit.
    pub fn total_price(&self, qty: f32) -> f32 {
        self.price / self.price_unit.max(1) as f32 * qty
    }

    pub fn matches_text(&self, f: &str) -> bool {
        if f.is_empty() {
            return true;
        }
        let f = f.to_lowercase();
        [&self.number, &self.type_number, &self.order_number, &self.manufacturer, &self.description, &self.group, &self.subgroup, &self.catalog]
            .iter()
            .any(|s| s.to_lowercase().contains(&f))
    }

    /// Short summary of the function template, e.g. "Coil, Main3, NO".
    /// Value of a `Name: value` line of the technical data (e.g. the cores of a cable).
    pub fn tech_value(&self, name: &str) -> Option<&str> {
        self.tech_data.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then_some(v.trim())
        })
    }

    pub fn template_summary(&self) -> String {
        let mut counts: BTreeMap<FnKind, usize> = BTreeMap::new();
        for f in &self.functions {
            *counts.entry(f.kind).or_default() += 1;
        }
        counts
            .iter()
            .map(|(k, n)| if *n > 1 { format!("{n}× {}", k.title()) } else { k.title().to_string() })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Reference from a device to a part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartRef {
    pub number: String,
    pub count: u32,
}

impl Default for PartRef {
    fn default() -> Self {
        PartRef { number: String::new(), count: 1 }
    }
}

/// The global parts database.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartsDb {
    pub parts: Vec<Part>,
}

impl Default for PartsDb {
    fn default() -> Self {
        PartsDb { parts: crate::default_catalog() }
    }
}

impl PartsDb {
    pub fn get(&self, number: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.number == number)
    }

    /// Inserts or replaces a part (by part number).
    pub fn upsert(&mut self, part: Part) {
        upsert(&mut self.parts, part);
    }

    pub fn sort(&mut self) {
        self.parts.sort_by(|a, b| natural_cmp(&a.number, &b.number));
    }

    /// Adds / replaces parts (e.g. downloaded from a parts server). Returns (added, updated).
    pub fn merge(&mut self, parts: Vec<Part>) -> (usize, usize) {
        let (mut added, mut updated) = (0, 0);
        for part in parts {
            match self.parts.iter_mut().find(|p| p.number == part.number) {
                Some(existing) if *existing != part => {
                    *existing = part;
                    updated += 1;
                }
                Some(_) => {}
                None => {
                    self.parts.push(part);
                    added += 1;
                }
            }
        }
        self.sort();
        (added, updated)
    }
}

pub fn upsert(list: &mut Vec<Part>, part: Part) {
    match list.iter_mut().find(|p| p.number == part.number) {
        Some(existing) => *existing = part,
        None => list.push(part),
    }
}

/// A part number that does not exist yet, based on `base`.
pub fn unique_number(list: &[Part], base: &str) -> String {
    if !list.iter().any(|p| p.number == base) {
        return base.to_string();
    }
    (1..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| !list.iter().any(|p| &p.number == n))
        .unwrap()
}

#[cfg(test)]
mod tests;
