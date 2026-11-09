//! Project data model: project → pages → placed symbols, wires, texts and graphics.
//!
//! Devices are identified by their device tag (e.g. `-K1`). All symbols that share a
//! tag belong to the same device: one *main function* (e.g. the coil) and any number
//! of *auxiliary functions* (e.g. its contacts). Cross references are derived from that.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::geom::{p, P};
use crate::symbols::{Anchor, SymbolRef};
pub use panel::Layout;
pub use openecraft_parts::natural_cmp;
pub use openecraft_parts::{Part, PartRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::{Mutex, OnceLock};

/// Version of the project file format written by this program (see
/// [`crate::archive::convert`]).
pub mod translation;
pub mod wiring;
pub mod marking;
pub mod fluid;
pub mod process;
pub mod options;
pub mod page_templates;
pub mod graphics;
pub mod potentials;
pub mod checks;
pub mod mounting;
pub mod preplanning;
pub mod macro_box;
pub mod plugs;
pub mod harness;
pub mod config;
pub mod structure;
pub mod subproject;
pub mod review;
pub mod revision;
pub mod compare;
pub mod properties;
pub mod electrical;
pub mod integration;
pub mod field;
pub mod plc_hw;
pub mod devdesc;
pub mod symlib;
pub mod forms;
pub mod report_blocks;
pub mod document;
pub mod routing3d;
pub mod panel;
pub mod ops;
pub mod functions;
pub mod safety;
pub mod rules;
pub mod electrofluid;
pub mod frame;
pub mod manufacturing;
pub mod climate;
pub mod props;
pub const FORMAT: u32 = 2;

fn legacy_format() -> u32 {
    0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    /// File format version; files of older versions have none (0).
    #[serde(default = "legacy_format")]
    pub format: u32,
    pub info: ProjectInfo,
    pub pages: Vec<Page>,
    /// Page sections (a tree, siblings in this order); see [`crate::sections`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    pub parts: Vec<Part>,
    /// Source-format records retained by importers for data without a native field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub import_sources: BTreeMap<String, openecraft_parts::Shared<serde_json::Value>>,
    /// Device level data (article assignment etc.) keyed by device tag.
    pub devices: BTreeMap<String, DeviceData>,
    /// Panel layout (enclosures, mounting plates, rails, ducts, devices).
    pub layout: Layout,
    pub cables: Vec<Cable>,
    pub wire_numbering: WireNumbering,
    /// Potential definitions (kind, voltage, wire colour and cross-section).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub potentials: Vec<potentials::PotentialDef>,
    /// Wire colour and cross-section of nets without a defined potential.
    #[serde(skip_serializing_if = "potentials::WireDefaults::is_default")]
    pub wire_defaults: potentials::WireDefaults,
    /// Wires are drawn in their colour (schematic, PDF, SVG).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub wire_colors: bool,
    /// Terminal strip data (order, jumpers, spare terminals) keyed by strip tag.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub strips: BTreeMap<String, StripData>,
    /// Plug data (pin order) keyed by plug tag.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plugs: BTreeMap<String, plugs::PlugData>,
    /// PLC card data (start address, rack / slot) keyed by card tag.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plc_cards: BTreeMap<String, PlcCard>,
    /// PLC stations, bus systems, bus connections and device descriptions.
    #[serde(skip_serializing_if = "plc_hw::Hardware::is_empty")]
    pub plc: plc_hw::Hardware,
    /// Released revisions, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub revisions: Vec<revision::Revision>,
    /// Wire harnesses with the connections they contain.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub harnesses: Vec<harness::Harness>,
    /// Project symbol library (user symbols).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub symbols: symlib::SymbolList,
    /// Title blocks and report forms.
    pub forms: forms::Forms,
    /// Report blocks: report pages generated and updated together.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub report_blocks: Vec<report_blocks::ReportBlock>,
    /// Options and generation rules of the configurator.
    #[serde(skip_serializing_if = "config::Configuration::is_empty")]
    pub configuration: config::Configuration,
    /// Marker texts and materials.
    #[serde(skip_serializing_if = "marking::Settings::is_default")]
    pub marking: marking::Settings,
    /// Defaults and numbering of hoses / pipes.
    #[serde(skip_serializing_if = "fluid::FluidSettings::is_default")]
    pub fluid: fluid::FluidSettings,
    /// PCT loops of the pre-planning.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pct_loops: Vec<process::PctLoop>,
    /// Source and target languages of the project texts with their dictionary.
    #[serde(skip_serializing_if = "translation::Languages::is_default")]
    pub languages: translation::Languages,
    /// Wired / problem marks of the wiring assistant.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wiring_marks: Vec<wiring::WiringMark>,
    /// Mounted / problem marks of the mounting assistant.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mounting_marks: Vec<mounting::MountingMark>,
    /// Pre-planning: segments with planned devices, segment templates.
    #[serde(skip_serializing_if = "preplanning::Preplanning::is_empty")]
    pub preplanning: preplanning::Preplanning,
    /// Structure management: defined identifiers, aspects of the pages, work areas.
    #[serde(skip_serializing_if = "structure::Structures::is_default")]
    pub structures: structure::Structures,
    /// Page templates defined for the project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub page_templates: Vec<page_templates::PageTemplate>,
    /// Layers of graphics, texts, dimensions and images (visibility, locking, print).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<graphics::Layer>,
    /// Approvers of revisions (release workflow).
    #[serde(skip_serializing_if = "revision::Workflow::is_default")]
    pub workflow: revision::Workflow,
    /// Stored states of the whole project (compare, take over).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub baselines: Vec<compare::Baseline>,
    /// Comments, redline / greenline markup, approvals and change requests.
    #[serde(skip_serializing_if = "review::Review::is_empty")]
    pub review: review::Review,
    /// Supply, installation conditions, safety functions and rule sets.
    #[serde(skip_serializing_if = "electrical::Electrical::is_default")]
    pub electrical: electrical::Electrical,
    /// User defined properties, function definitions and placement schemes.
    #[serde(skip_serializing_if = "properties::Library::is_empty")]
    pub property_db: properties::Library,
    /// Field wiring: plant layout, route network, bonding and cable accessories.
    #[serde(skip_serializing_if = "field::Field::is_empty")]
    pub field: field::Field,
    /// Subprojects taken out of this project, or the project this one was taken from.
    #[serde(skip_serializing_if = "subproject::Subprojects::is_empty")]
    pub subprojects: subproject::Subprojects,
    /// Options that show or hide model content.
    #[serde(skip_serializing_if = "options::ModelOptions::is_empty")]
    pub model_options: options::ModelOptions,
    /// How cross references are shown (format, structure, interruption points).
    #[serde(skip_serializing_if = "CrossRefs::is_default")]
    pub xref: CrossRefs,
    /// Checks switched off, message levels and accepted messages of the project check.
    #[serde(skip_serializing_if = "checks::CheckSettings::is_default")]
    pub check_settings: checks::CheckSettings,
    /// Links to ERP, PDM, PLM, ECAD, MCAD, procurement and wiring machines.
    #[serde(skip_serializing_if = "integration::Integration::is_empty")]
    pub integration: integration::Integration,
    pub next_id: u64,
    /// Lookup table of the user symbols (not stored, never compared).
    #[serde(skip)]
    pub symbol_cache: crate::symlib::SymbolCache,
}

impl Default for Project {
    fn default() -> Self {
        let mut prj = Project {
            format: FORMAT,
            info: ProjectInfo::default(),
            pages: Vec::new(),
            sections: Vec::new(),
            parts: Vec::new(),
            import_sources: BTreeMap::new(),
            devices: BTreeMap::new(),
            layout: Layout::default(),
            cables: Vec::new(),
            wire_numbering: WireNumbering::default(),
            potentials: Vec::new(),
            wire_defaults: Default::default(),
            wire_colors: false,
            strips: BTreeMap::new(),
            plugs: BTreeMap::new(),
            plc_cards: BTreeMap::new(),
            plc: Default::default(),
            revisions: Vec::new(),
            harnesses: Vec::new(),
            symbols: Default::default(),
            forms: Default::default(),
            report_blocks: Vec::new(),
            marking: Default::default(),
            configuration: Default::default(),
            fluid: Default::default(),
            pct_loops: Vec::new(),
            wiring_marks: Vec::new(),
            mounting_marks: Vec::new(),
            preplanning: Default::default(),
            languages: Default::default(),
            structures: Default::default(),
            page_templates: Vec::new(),
            layers: Vec::new(),
            workflow: Default::default(),
            baselines: Vec::new(),
            review: Default::default(),
            electrical: Default::default(),
            property_db: Default::default(),
            field: Default::default(),
            subprojects: Default::default(),
            model_options: Default::default(),
            check_settings: Default::default(),
            xref: Default::default(),
            integration: Default::default(),
            next_id: 1,
            symbol_cache: Default::default(),
        };
        let id = prj.new_id();
        prj.pages.push(Page::new(id, "1", "Schematic", PageKind::Schematic));
        prj
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectInfo {
    pub name: String,
    pub description: String,
    pub company: String,
    pub creator: String,
    pub date: String,
    pub revision: String,
    /// Project property values ([`crate::properties`]) and free project properties,
    /// shown in title blocks as `{prop:Name}`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl Default for ProjectInfo {
    fn default() -> Self {
        ProjectInfo {
            name: "New project".into(),
            description: String::new(),
            company: String::new(),
            creator: String::new(),
            date: String::new(),
            revision: "A".into(),
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageKind {
    Schematic,
    Cover,
    Report(ReportId),
    /// 2D model view of the mounting plate of an enclosure (enclosure id).
    Mounting(u64),
    /// Overview drawing (single-line, block diagram): symbols and wires are drawn
    /// but not evaluated.
    Overview,
    /// Free graphics (layout sketches, notes): not evaluated.
    Graphic,
    /// An embedded file (PDF, Office document, image).
    Document,
}

impl PageKind {
    pub fn label(self) -> String {
        match self {
            PageKind::Schematic => tr("Schematic (multi-line)").into(),
            PageKind::Cover => tr("Cover sheet").into(),
            PageKind::Report(r) => tr_fmt!("Report: {}", r.title()),
            PageKind::Mounting(_) => tr("Mounting layout").into(),
            PageKind::Overview => tr("Overview (not evaluated)").into(),
            PageKind::Graphic => tr("Graphics").into(),
            PageKind::Document => tr("Document (embedded file)").into(),
        }
    }

    /// Pages drawn by hand (symbols, wires, texts, graphics).
    pub fn is_drawing(self) -> bool {
        matches!(self, PageKind::Schematic | PageKind::Overview | PageKind::Graphic)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ReportKind {
    TableOfContents,
    DeviceList,
    BillOfMaterials,
    TerminalDiagram,
    TerminalLineup,
    TerminalStripOverview,
    TerminalConnections,
    PlugDiagram,
    PinConnections,
    ConnectionList,
    DeviceConnections,
    PlcAssignment,
    PotentialList,
    PanelWiring,
    CableOverview,
    CableDiagram,
    CableConnections,
    CableSizing,
    RevisionOverview,
    HarnessData,
    DuctFill,
    PanelLegend,
    Climate,
    Marking,
    HoseList,
    PctLoops,
    FluidNetlist,
    HoseConfiguration,
    TubingList,
    FluidDeviceGroups,
    ElectroFluid,
    RoutingList,
    BundleList,
    ShortCircuit,
    Selectivity,
    LoadCalculation,
    PowerSupplyBudget,
    SafetyFunctions,
    Verification,
    FieldRouting,
    Bonding,
    RackElevation,
    PvStrings,
}

impl ReportKind {
    pub const ALL: [ReportKind; 43] = [
        ReportKind::TableOfContents,
        ReportKind::DeviceList,
        ReportKind::BillOfMaterials,
        ReportKind::TerminalDiagram,
        ReportKind::TerminalLineup,
        ReportKind::TerminalStripOverview,
        ReportKind::TerminalConnections,
        ReportKind::PlugDiagram,
        ReportKind::PinConnections,
        ReportKind::ConnectionList,
        ReportKind::DeviceConnections,
        ReportKind::PlcAssignment,
        ReportKind::PotentialList,
        ReportKind::PanelWiring,
        ReportKind::CableOverview,
        ReportKind::CableDiagram,
        ReportKind::CableConnections,
        ReportKind::CableSizing,
        ReportKind::RevisionOverview,
        ReportKind::HarnessData,
        ReportKind::DuctFill,
        ReportKind::PanelLegend,
        ReportKind::Climate,
        ReportKind::Marking,
        ReportKind::HoseList,
        ReportKind::PctLoops,
        ReportKind::FluidNetlist,
        ReportKind::HoseConfiguration,
        ReportKind::TubingList,
        ReportKind::FluidDeviceGroups,
        ReportKind::ElectroFluid,
        ReportKind::RoutingList,
        ReportKind::BundleList,
        ReportKind::ShortCircuit,
        ReportKind::Selectivity,
        ReportKind::LoadCalculation,
        ReportKind::PowerSupplyBudget,
        ReportKind::SafetyFunctions,
        ReportKind::Verification,
        ReportKind::FieldRouting,
        ReportKind::Bonding,
        ReportKind::RackElevation,
        ReportKind::PvStrings,
    ];

    /// English catalog key of the title.
    pub fn title_key(self) -> &'static str {
        match self {
            ReportKind::TableOfContents => tr_const!("Table of contents"),
            ReportKind::DeviceList => tr_const!("Device list"),
            ReportKind::BillOfMaterials => tr_const!("Bill of materials"),
            ReportKind::TerminalDiagram => tr_const!("Terminal diagram"),
            ReportKind::TerminalLineup => tr_const!("Terminal line-up"),
            ReportKind::TerminalStripOverview => tr_const!("Terminal strip overview"),
            ReportKind::TerminalConnections => tr_const!("Terminal connection diagram"),
            ReportKind::PlugDiagram => tr_const!("Plug diagram"),
            ReportKind::PinConnections => tr_const!("Pin connection diagram"),
            ReportKind::DeviceConnections => tr_const!("Device connection diagram"),
            ReportKind::ConnectionList => tr_const!("Connection list"),
            ReportKind::PlcAssignment => tr_const!("PLC assignment list"),
            ReportKind::PotentialList => tr_const!("Potential overview"),
            ReportKind::PanelWiring => tr_const!("Wire list (panel routing)"),
            ReportKind::CableOverview => tr_const!("Cable overview"),
            ReportKind::CableDiagram => tr_const!("Cable diagram"),
            ReportKind::CableConnections => tr_const!("Cable connection diagram"),
            ReportKind::PanelLegend => tr_const!("Panel legend"),
            ReportKind::CableSizing => tr_const!("Cable sizing (voltage drop)"),
            ReportKind::RevisionOverview => tr_const!("Revision overview (changes)"),
            ReportKind::HarnessData => tr_const!("Harness data"),
            ReportKind::DuctFill => tr_const!("Duct fill levels"),
            ReportKind::Climate => tr_const!("Enclosure climate"),
            ReportKind::Marking => tr_const!("Marking"),
            ReportKind::HoseList => tr_const!("Hose list"),
            ReportKind::PctLoops => tr_const!("PCT loop overview"),
            ReportKind::FluidNetlist => tr_const!("Fluid netlist"),
            ReportKind::HoseConfiguration => tr_const!("Hose configuration"),
            ReportKind::TubingList => tr_const!("Tubing / piping list"),
            ReportKind::FluidDeviceGroups => tr_const!("Fluid device groups"),
            ReportKind::ElectroFluid => tr_const!("Electro-fluid cross reference"),
            ReportKind::RoutingList => tr_const!("Routing list (3D)"),
            ReportKind::BundleList => tr_const!("Bundle list"),
            ReportKind::ShortCircuit => tr_const!("Short circuit and disconnection"),
            ReportKind::Selectivity => tr_const!("Selectivity"),
            ReportKind::LoadCalculation => tr_const!("Load calculation"),
            ReportKind::PowerSupplyBudget => tr_const!("Power supply load budget"),
            ReportKind::SafetyFunctions => tr_const!("Safety functions (ISO 13849-1)"),
            ReportKind::Verification => tr_const!("Rule set verification"),
            ReportKind::FieldRouting => tr_const!("Field cable routing"),
            ReportKind::Bonding => tr_const!("Equipotential bonding"),
            ReportKind::RackElevation => tr_const!("19-inch rack elevation"),
            ReportKind::PvStrings => tr_const!("PV strings"),
        }
    }

    /// Title in the current language.
    pub fn title(self) -> &'static str {
        tr(self.title_key())
    }

    /// Name of the report in files, on the command line and in the registry.
    pub fn name(self) -> &'static str {
        match self {
            ReportKind::TableOfContents => "table_of_contents",
            ReportKind::DeviceList => "device_list",
            ReportKind::BillOfMaterials => "bill_of_materials",
            ReportKind::TerminalDiagram => "terminal_diagram",
            ReportKind::TerminalLineup => "terminal_lineup",
            ReportKind::TerminalStripOverview => "terminal_strip_overview",
            ReportKind::TerminalConnections => "terminal_connections",
            ReportKind::PlugDiagram => "plug_diagram",
            ReportKind::PinConnections => "pin_connections",
            ReportKind::ConnectionList => "connection_list",
            ReportKind::DeviceConnections => "device_connections",
            ReportKind::PlcAssignment => "plc_assignment",
            ReportKind::PotentialList => "potential_list",
            ReportKind::PanelWiring => "panel_wiring",
            ReportKind::CableOverview => "cable_overview",
            ReportKind::CableDiagram => "cable_diagram",
            ReportKind::CableConnections => "cable_connections",
            ReportKind::CableSizing => "cable_sizing",
            ReportKind::RevisionOverview => "revision_overview",
            ReportKind::HarnessData => "harness_data",
            ReportKind::DuctFill => "duct_fill",
            ReportKind::PanelLegend => "panel_legend",
            ReportKind::Climate => "climate",
            ReportKind::Marking => "marking",
            ReportKind::HoseList => "hose_list",
            ReportKind::PctLoops => "pct_loops",
            ReportKind::FluidNetlist => "fluid_netlist",
            ReportKind::HoseConfiguration => "hose_configuration",
            ReportKind::TubingList => "tubing_list",
            ReportKind::FluidDeviceGroups => "fluid_device_groups",
            ReportKind::ElectroFluid => "electro_fluid",
            ReportKind::RoutingList => "routing_list",
            ReportKind::BundleList => "bundle_list",
            ReportKind::ShortCircuit => "short_circuit",
            ReportKind::Selectivity => "selectivity",
            ReportKind::LoadCalculation => "load_calculation",
            ReportKind::PowerSupplyBudget => "power_supply_budget",
            ReportKind::SafetyFunctions => "safety_functions",
            ReportKind::Verification => "verification",
            ReportKind::FieldRouting => "field_routing",
            ReportKind::Bonding => "bonding",
            ReportKind::RackElevation => "rack_elevation",
            ReportKind::PvStrings => "pv_strings",
        }
    }

    /// The built-in kind of a name ([`ReportKind::name`]) or of the variant name that
    /// files written before format 2 hold (`DeviceList`).
    pub fn from_name(name: &str) -> Option<ReportKind> {
        ReportKind::ALL.into_iter().find(|k| k.name() == name || format!("{k:?}") == name)
    }
}

/// Name of a report: a built-in kind ([`ReportKind::name`]) or a report of a plugin.
/// Interned, so that page kinds stay `Copy`; stored in files as the snake_case name
/// (files written before format 2 hold the variant names of the built-in kinds).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReportId(&'static str);

fn intern(name: &str) -> &'static str {
    if let Some(k) = ReportKind::from_name(name) {
        return k.name();
    }
    static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    match names.get(name) {
        Some(s) => s,
        None => {
            let s: &'static str = Box::leak(name.to_string().into_boxed_str());
            names.insert(s);
            s
        }
    }
}

impl ReportId {
    pub fn new(name: &str) -> ReportId {
        ReportId(intern(name.trim()))
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }

    /// The built-in kind, if it is one.
    pub fn builtin(&self) -> Option<ReportKind> {
        ReportKind::ALL.into_iter().find(|k| k.name() == self.0)
    }

    /// Title in the current language: of the built-in kind, otherwise the name (the
    /// front ends know the titles of plugin reports).
    pub fn title(&self) -> String {
        self.builtin().map(|k| k.title().to_string()).unwrap_or_else(|| self.0.to_string())
    }
}

impl From<ReportKind> for ReportId {
    fn from(k: ReportKind) -> ReportId {
        ReportId(k.name())
    }
}

impl fmt::Debug for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReportId({:?})", self.0)
    }
}

impl fmt::Display for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for ReportId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for ReportId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<ReportId, D::Error> {
        let name = String::deserialize(d)?;
        Ok(ReportId::new(&name))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub id: u64,
    /// Page number / name as shown in cross references, e.g. "3".
    pub name: String,
    pub description: String,
    /// Section the page is in (its id); 0 = top level. The name starts with the
    /// section's number.
    #[serde(skip_serializing_if = "is_zero")]
    pub section: u64,
    /// Structure of the page (IEC 81346) without prefix signs: function (`=`),
    /// installation site (`++`), location (`+`) and higher-level product (`-`).
    /// Devices placed on the page inherit it.
    pub function: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub installation: String,
    pub location: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub product: String,
    /// Released page: no changes until it is unlocked.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Inserted by the configurator: replaced when it generates again.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub generated: bool,
    /// Title block of this page; empty = the project's default.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title_block: String,
    /// Plot frame (paper format, columns, rows); empty = the project's default.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub frame: String,
    /// File of a document page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<document::Document>,
    pub kind: PageKind,
    /// Report pages: what part of the report the page shows (report blocks, pages
    /// output from other projects); none: the first rows of the whole report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<report_blocks::ReportPart>,
    pub symbols: Vec<SymbolInst>,
    pub wires: Vec<Wire>,
    pub texts: Vec<TextItem>,
    pub shapes: Vec<Shape>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<Dimension>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageItem>,
    /// Connection definition points: define number, colour, cross-section and
    /// cable core of the connection they sit on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conn_defs: Vec<ConnDef>,
    /// Cable definition lines: the wires they cross are cores of the cable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cable_defs: Vec<CableDef>,
    /// Directed T-junctions: where three wires meet, which wires are the targets of
    /// the connection (`netlist::junctions`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub junctions: Vec<Junction>,
    /// Page property values.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: properties::Values,
    /// Window macros placed on the page (macro boxes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub macro_boxes: Vec<macro_box::MacroBox>,
}

impl Default for Page {
    fn default() -> Self {
        Page::new(0, "1", "", PageKind::Schematic)
    }
}

impl Page {
    /// Its content can be changed: a schematic, overview or graphics page that is
    /// not released. Report, cover and mounting pages are generated.
    pub fn is_editable(&self) -> bool {
        self.kind.is_drawing() && !self.locked
    }

    pub fn new(id: u64, name: &str, description: &str, kind: PageKind) -> Page {
        Page {
            id,
            name: name.into(),
            description: description.into(),
            section: 0,
            function: String::new(),
            installation: String::new(),
            location: String::new(),
            product: String::new(),
            locked: false,
            generated: false,
            title_block: String::new(),
            frame: String::new(),
            document: None,
            kind,
            report: None,
            symbols: Vec::new(),
            wires: Vec::new(),
            texts: Vec::new(),
            shapes: Vec::new(),
            dimensions: Vec::new(),
            images: Vec::new(),
            conn_defs: Vec::new(),
            cable_defs: Vec::new(),
            junctions: Vec::new(),
            values: BTreeMap::new(),
            macro_boxes: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
            && self.wires.is_empty()
            && self.texts.is_empty()
            && self.shapes.is_empty()
            && self.dimensions.is_empty()
            && self.images.is_empty()
            && self.conn_defs.is_empty()
            && self.cable_defs.is_empty()
            && self.junctions.is_empty()
    }
}

/// A section of pages, like a directory: holds pages and subsections. Its full
/// number (`2.3`) is the numbers of its parents and its own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Section {
    pub id: u64,
    /// Parent section; 0 = top level.
    #[serde(skip_serializing_if = "is_zero")]
    pub parent: u64,
    /// Number within the parent, e.g. `3` or `A`.
    pub number: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolInst {
    pub id: u64,
    /// Key into the symbol library.
    pub sym: String,
    pub pos: P,
    /// Quarter turns clockwise.
    pub rot: u8,
    pub mirror: bool,
    /// Device tag, e.g. `-K1`.
    pub tag: String,
    /// Symbol specific designation: terminal number, potential name, PLC address,
    /// interruption point name, ...
    pub label: String,
    pub function_text: String,
    /// Connection point designations, one per symbol pin.
    pub pins: Vec<String>,
    /// Tag block placed by hand: position of its top-left corner relative to `pos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_offset: Option<P>,
    /// Further properties shown beside the symbol (part number, description …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<PlacedProp>,
    /// Placement scheme the placed properties were taken from.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scheme: String,
    /// Function definition; empty = the symbol's built-in one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub function_def: String,
    /// Function, terminal and PLC connection point property values.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: properties::Values,
    /// Shared logical channels shown by several representations of this device.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plc_bindings: BTreeMap<usize,plc_hw::ChannelBinding>,
}

impl SymbolInst {
    /// A cable marker follows its inventory; other explicit part references never fall back.
    pub fn selected_part_number<'a>(&self, prj: &'a Project) -> Option<&'a str> {
        if let Some(tag)=self.values.get("function.cable").filter(|tag|!tag.is_empty()) {
            return prj.cable(tag).filter(|cable|!cable.part.is_empty()).map(|cable|cable.part.as_str())
        }
        if let Some(number) = self.values.get("function.part_number") {
            if let Some(device) = prj.devices.get(&self.tag) {
                if let Some(part)=device.parts.iter().find(|p|p.number==*number) {return Some(part.number.as_str());}
            }
            return prj.cable(&self.tag).filter(|c| c.part == *number).map(|c| c.part.as_str());
        }
        self.assigned_part_number(prj, 1)
    }

    pub(crate) fn assigned_part_number<'a>(&self, prj: &'a Project, index: usize) -> Option<&'a str> {
        let index = index.checked_sub(1)?;
        if let Some(tag)=self.values.get("function.cable").filter(|tag|!tag.is_empty()) {
            return prj.cable(tag).filter(|cable|index==0 && !cable.part.is_empty()).map(|cable|cable.part.as_str())
        }
        if let Some(device) = prj.devices.get(&self.tag) {
            return device.parts.get(index).map(|part| part.number.as_str());
        }
        prj.cable(&self.tag).filter(|c| index == 0 && !c.part.is_empty()).map(|c| c.part.as_str())
    }

    /// Electrical identity of an interruption point; its displayed name stays unchanged.
    pub fn interruption_key(&self) -> String {
        match self.values.get("function.connection_scope").filter(|scope| !scope.is_empty()) {
            Some(scope) => format!("\0{}\0{}", scope, self.label),
            None => self.label.clone(),
        }
    }
}

impl Default for SymbolInst {
    fn default() -> Self {
        SymbolInst {
            id: 0,
            sym: String::new(),
            pos: p(0.0, 0.0),
            rot: 0,
            mirror: false,
            tag: String::new(),
            label: String::new(),
            function_text: String::new(),
            pins: Vec::new(),
            tag_offset: None,
            props: Vec::new(),
            scheme: String::new(),
            function_def: String::new(),
            values: BTreeMap::new(),
            plc_bindings: BTreeMap::new(),
        }
    }
}

/// Fixed placeable properties of older files (now property keys, see
/// [`PropKind::key`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PropKind {
    #[default]
    Description,
    PartNumber,
    TypeNumber,
    Manufacturer,
    Label,
    Pins,
    Structure,
}

/// A property shown at a position relative to a symbol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlacedProp {
    /// Property key (with index), see [`crate::properties`].
    pub key: String,
    /// Anchor point of the text relative to the symbol position.
    pub offset: P,
    pub size: f32,
    #[serde(skip_serializing_if = "is_left", default = "default_anchor")]
    pub anchor: crate::symbols::Anchor,
    /// Shown as `Name: value`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub name: bool,
    /// Fixed property of older files, converted on loading.
    #[serde(rename = "kind", skip_serializing)]
    pub legacy: Option<PropKind>,
}

impl Default for PlacedProp {
    fn default() -> Self {
        PlacedProp { key: String::new(), offset: p(4.0, 4.0), size: 2.0, anchor: crate::symbols::Anchor::Left, name: false, legacy: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Wire {
    pub id: u64,
    pub a: P,
    pub b: P,
    /// Wire colour set manually (e.g. "BK", "BU", "GNYE"); empty = from the
    /// potential definition or the project default.
    pub color: String,
    /// Cross-section set manually (e.g. "1.5 mm²"); empty = automatic. Hoses and
    /// pipes (fluid nets): nominal size, e.g. "8 mm" or "DN25".
    pub cross_section: String,
    /// Conductor construction/type, e.g. H07V-K or MKEM; empty = unspecified.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub wire_type: String,
    /// Medium of a hose / pipe; None = the project's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<fluid::Medium>,
    /// Length of a hose / pipe in metres; 0 = not set.
    #[serde(skip_serializing_if = "is_zero")]
    pub length: f32,
    /// Hose / tube / pipe configuration (line type, part, fittings, rating).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hose: Option<fluid::HoseConfig>,
    /// Wire (connection) number; all wires of a connection carry the same number.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub number: String,
    /// Set manually: automatic numbering keeps it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub number_fixed: bool,
    /// Suppress automatic number and cable/core labels when annotations are placed separately.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hide_labels: bool,
    /// Explicit live labels, retained independently of automatic label visibility.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<wiring::WireLabel>,
    /// Per-wire junction policy; None inherits the page's legacy setting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_junctions: Option<bool>,
    /// Tag of the cable this wire is a core of, and the core designation.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cable: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub core: String,
}

impl Wire {
    pub fn requires_explicit_junctions(&self,page:&Page)->bool {
        self.explicit_junctions.unwrap_or_else(||page.values.get("page.explicit_wire_junctions").is_some_and(|v|v=="true"))
    }

    pub fn junction_policy(&self)->&'static str {
        match self.explicit_junctions {None=>"inherit",Some(true)=>"explicit",Some(false)=>"implicit"}
    }

    pub fn same_properties(&self,other:&Self)->bool {
        self.color==other.color && self.cross_section==other.cross_section && self.wire_type==other.wire_type
            && self.medium==other.medium && self.length==other.length && self.hose==other.hose
            && self.number_fixed==other.number_fixed && (!self.number_fixed || self.number==other.number) && self.hide_labels==other.hide_labels
            && self.cable==other.cable && self.core==other.core && self.explicit_junctions==other.explicit_junctions
    }
}

/// `skip_serializing_if` for numbers at their default.
pub(crate) fn is_zero<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// A cable; its cores are assigned to wires (`Wire::cable` / `Wire::core`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Cable {
    pub id: u64,
    /// Complete device tag, e.g. `=A1-W1`.
    pub tag: String,
    /// Cable type, e.g. "H07RN-F 4G1.5".
    pub cable_type: String,
    /// Core designations separated by blanks, e.g. "BN BK GY GNYE" or "1 2 3 GNYE".
    pub cores: String,
    pub cross_section: String,
    /// Length in metres.
    pub length: f32,
    /// Part number (bill of materials: length × part).
    pub part: String,
    pub description: String,
    /// Network / bus cable for this bus system (`None`: ordinary cable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<devdesc::BusKind>,
    /// The cable has an overall shield; `None`: from the cable type and the bus.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shielded: Option<bool>,
    /// Ends at which the shield is connected.
    #[serde(skip_serializing_if = "routing3d::ShieldEnds::is_both")]
    pub shield_ends: routing3d::ShieldEnds,
    /// Outer diameter in mm; 0 = estimated from cores and cross-section.
    #[serde(skip_serializing_if = "is_zero")]
    pub diameter: f32,
    /// Cable property values.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: properties::Values,
}

impl Cable {
    pub fn core_list(&self) -> Vec<String> {
        self.cores.split_whitespace().map(String::from).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NumberScheme {
    /// 1, 2, 3 … in schematic order.
    #[default]
    Sequential,
    /// Page, column and counter: `3.4.1`.
    PageColumn,
    /// Named potentials (L1, +24V …) keep their name, other connections are counted.
    Potential,
    /// After the format of the numbering settings (`{page}.{column}.{n}` …).
    Format,
}

impl NumberScheme {
    pub const ALL: [NumberScheme; 4] = [NumberScheme::Sequential, NumberScheme::PageColumn, NumberScheme::Potential, NumberScheme::Format];

    pub fn title(self) -> &'static str {
        match self {
            NumberScheme::Sequential => tr("Sequential (1, 2, 3 …)"),
            NumberScheme::PageColumn => tr("Page / column (3.4.1)"),
            NumberScheme::Potential => tr("Potential name (L1, +24V, otherwise counted)"),
            NumberScheme::Format => tr("Format (placeholders)"),
        }
    }
}

/// Terminal strip data beyond the placed terminals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StripData {
    /// Terminal designations in strip order; terminals not listed follow sorted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Terminal designation -> jumper (bridge) number; terminals with the same
    /// number are connected.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub jumpers: BTreeMap<String, u32>,
    /// Spare terminals (not in the schematic) at the end of the strip.
    pub spares: u32,
    /// End brackets and end plates are not placed automatically.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub manual_accessories: bool,
    /// Terminal designation -> level of a multi-level terminal (missing = 1). A terminal
    /// on level 2 or higher sits on the block of the terminal before it in strip order.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub levels: BTreeMap<String, u8>,
    /// Jumper number -> kind of jumper (missing = plug-in jumper).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub jumper_kinds: BTreeMap<u32, JumperKind>,
}

/// How the terminals of a jumper are bridged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum JumperKind {
    /// Plug-in (centre) jumper, cut to length (tech data `Jumper: …` of the terminal part).
    #[default]
    PlugIn,
    /// Saddle (screw) jumper (tech data `Saddle jumper: …`).
    Saddle,
    /// Wire jumper: a wire from terminal to terminal, any level, no jumper part.
    Wire,
}

/// PLC card (I/O module) data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlcCard {
    /// First byte of the card's inputs / outputs (I<start>.0, Q<start>.0).
    pub start_byte: u32,
    /// First byte of the outputs when it differs from the inputs' (mixed modules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_byte: Option<u32>,
    pub rack: String,
    pub slot: String,
    pub description: String,
    /// Station of the hardware configuration the card is plugged into.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub station: String,
    /// Device description (overrides the one from the parts).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub device: String,
}

/// When a cross reference shows the structure of the page it points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrefStructure {
    /// Only if it differs from the page the reference is on.
    #[default]
    Different,
    Always,
    Never,
}

/// Which counterparts an interruption point refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InterruptionMode {
    /// Every other point of the same name.
    #[default]
    All,
    /// The first point (in schematic order) is the source: it refers to all others,
    /// the others to it.
    Star,
    /// In schematic order, the 1st and 2nd, 3rd and 4th … refer to each other.
    Pairs,
}

/// How cross references are shown: `/3.4`, `/3.4.B`, `=A1/3.4` …
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossRefs {
    /// In front of the page name (default `/`).
    pub prefix: String,
    /// Between page and column (default `.`).
    pub separator: String,
    /// The row follows the column (`/3.4.B`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub row: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub structure: XrefStructure,
    #[serde(skip_serializing_if = "is_zero")]
    pub interruptions: InterruptionMode,
    /// Coils draw no contact image.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hide_contact_image: bool,
}

impl Default for CrossRefs {
    fn default() -> Self {
        CrossRefs { prefix: "/".into(), separator: ".".into(), row: false, structure: XrefStructure::Different, interruptions: InterruptionMode::All, hide_contact_image: false }
    }
}

impl CrossRefs {
    pub fn is_default(&self) -> bool {
        *self == CrossRefs::default()
    }
}

/// Settings of the automatic wire numbering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WireNumbering {
    pub scheme: NumberScheme,
    /// Put in front of counted numbers, e.g. "W".
    pub prefix: String,
    pub start: u32,
    /// Format of the scheme `Format`: `{n}` counter (counted per value of the rest of
    /// the format), `{page}`, `{column}`, `{row}`, `{potential}` (name of a named
    /// potential), `{kind}` (L, N, PE … of its definition), `{color}`,
    /// `{cross_section}`. Without `{n}`, connections with the same text share it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub format: String,
    /// Minimum digits of the counter (leading zeros).
    #[serde(skip_serializing_if = "is_zero")]
    pub digits: u8,
    /// Counting step.
    #[serde(skip_serializing_if = "is_one", default = "one")]
    pub step: u32,
    /// Connections of potentials of these kinds get no number (e.g. PE).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skip_kinds: Vec<potentials::PotentialKind>,
}

fn one() -> u32 {
    1
}

fn is_one(v: &u32) -> bool {
    *v == 1
}

impl Default for WireNumbering {
    fn default() -> Self {
        WireNumbering { scheme: NumberScheme::Sequential, prefix: String::new(), start: 1, format: String::new(), digits: 0, step: 1, skip_kinds: Vec::new() }
    }
}

/// Vertical anchor of a text at its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VAnchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextItem {
    pub id: u64,
    pub pos: P,
    /// Placeholders such as `{page}` or `{prop:Name}` are filled in when the text is
    /// drawn (special texts, see `forms::fill`).
    pub text: String,
    pub size: f32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub layer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Rotation in degrees, counter-clockwise (90: reads from bottom to top).
    #[serde(skip_serializing_if = "is_zero")]
    pub angle: f32,
    /// Horizontal alignment of the lines at `pos`.
    #[serde(skip_serializing_if = "is_left", default = "default_anchor")]
    pub align: crate::symbols::Anchor,
    /// Which line `pos` is on: the first, the middle of the block or the last.
    #[serde(skip_serializing_if = "is_zero")]
    pub valign: VAnchor,
    /// Lines are wrapped at this width in mm; 0 = no wrapping.
    #[serde(skip_serializing_if = "is_zero")]
    pub width: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// Border around the text.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub frame: bool,
    /// Opaque background in the paper colour (covers what is behind the text).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
    /// Hyperlink (URL) opened from the text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub link: String,
    /// Path function text: the function text of the functions in its column that
    /// have none of their own.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub path_function: bool,
}

impl Default for TextItem {
    fn default() -> Self {
        TextItem {
            id: 0,
            pos: P::default(),
            text: String::new(),
            size: 3.0,
            layer: String::new(),
            color: None,
            angle: 0.0,
            align: crate::symbols::Anchor::Left,
            valign: VAnchor::Top,
            width: 0.0,
            bold: false,
            italic: false,
            underline: false,
            frame: false,
            background: false,
            link: String::new(),
            path_function: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeKind {
    #[default]
    Line,
    Rect,
    /// Centre `a`, a point of the circle `b`.
    Circle,
    /// Inside the rectangle `a` – `b`.
    Ellipse,
    /// Centre `a`, start point `b`, `sweep` degrees counter-clockwise.
    Arc,
    /// Open line through `pts`.
    Polyline,
    /// Closed area through `pts`.
    Polygon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    pub a: P,
    pub b: P,
    pub dashed: bool,
    /// Corner points of polylines and polygons.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pts: Vec<P>,
    /// Arc angle in degrees.
    #[serde(skip_serializing_if = "is_zero")]
    pub sweep: f32,
    /// Line width in mm; 0 = the normal symbol line.
    #[serde(skip_serializing_if = "is_zero")]
    pub width: f32,
    /// Line (and fill) colour; none = the normal line colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Closed shapes are filled.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fill: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub layer: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DimKind {
    /// Parallel to the measured points.
    #[default]
    Aligned,
    Horizontal,
    Vertical,
}

/// A dimension between the points `a` and `b`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dimension {
    pub id: u64,
    pub kind: DimKind,
    pub a: P,
    pub b: P,
    /// Distance of the dimension line from the measured points (along the normal
    /// of the measuring direction).
    pub offset: f32,
    /// Shown instead of the measured value when not empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Real length per millimetre on the page (drawing scale).
    pub scale: f32,
    pub decimals: u8,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub unit: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub layer: String,
}

impl Default for Dimension {
    fn default() -> Self {
        Dimension {
            id: 0,
            kind: DimKind::Aligned,
            a: P::default(),
            b: P::default(),
            offset: 8.0,
            text: String::new(),
            scale: 1.0,
            decimals: 0,
            unit: String::new(),
            layer: String::new(),
        }
    }
}

/// A bitmap (PNG, JPEG) shown in the rectangle `a` – `b`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ImageItem {
    pub id: u64,
    pub a: P,
    pub b: P,
    pub file: document::Document,
    /// Pixel size of the bitmap (keeps the aspect ratio when resized).
    pub px: [u32; 2],
    #[serde(skip_serializing_if = "String::is_empty")]
    pub layer: String,
}

/// Connection definition point on a wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConnDef {
    pub id: u64,
    pub pos: P,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub number: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cross_section: String,
    /// Cable and core of the wire it sits on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cable: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub core: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Direction the stem of a directed T-junction points to (the bar lies across it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum TeeDir {
    Left,
    Right,
    Up,
    #[default]
    Down,
}

impl TeeDir {
    pub const ALL: [TeeDir; 4] = [TeeDir::Left, TeeDir::Right, TeeDir::Up, TeeDir::Down];

    /// Unit vector of the direction (y downwards).
    pub fn vector(self) -> P {
        match self {
            TeeDir::Left => p(-1.0, 0.0),
            TeeDir::Right => p(1.0, 0.0),
            TeeDir::Up => p(0.0, -1.0),
            TeeDir::Down => p(0.0, 1.0),
        }
    }

    pub fn opposite(self) -> TeeDir {
        match self {
            TeeDir::Left => TeeDir::Right,
            TeeDir::Right => TeeDir::Left,
            TeeDir::Up => TeeDir::Down,
            TeeDir::Down => TeeDir::Up,
        }
    }

    /// Turned by 90° clockwise.
    pub fn turned(self) -> TeeDir {
        match self {
            TeeDir::Left => TeeDir::Up,
            TeeDir::Up => TeeDir::Right,
            TeeDir::Right => TeeDir::Down,
            TeeDir::Down => TeeDir::Left,
        }
    }

    /// Directions of the two arms of the bar across a stem in this direction: the
    /// first arm is the left or upper one.
    pub fn arms(self) -> [TeeDir; 2] {
        match self {
            TeeDir::Up | TeeDir::Down => [TeeDir::Left, TeeDir::Right],
            TeeDir::Left | TeeDir::Right => [TeeDir::Up, TeeDir::Down],
        }
    }

    /// Name in commands and files: `left`, `right`, `up`, `down`.
    pub fn name(self) -> &'static str {
        match self {
            TeeDir::Left => "left",
            TeeDir::Right => "right",
            TeeDir::Up => "up",
            TeeDir::Down => "down",
        }
    }

    pub fn parse(name: &str) -> Option<TeeDir> {
        TeeDir::ALL.into_iter().find(|d| d.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// Kind of a connection symbol ([`Junction`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum JunctionKind {
    /// Three wires: a directed T-junction with target tracing.
    #[default]
    Tee,
    /// Two wires at a right angle: a corner that automatic connections wire into.
    Corner,
    /// Four wires: two T-junctions joined, the straight line through it is the first
    /// target of both its ends, the arms across it their second targets.
    Crossing,
    /// Four wires: a star from the side the connection comes from to the three others,
    /// the opposite side first, then the left / upper, then the right / lower.
    Bridge,
}

/// Connection symbol: a directed T-junction (target tracing) or a corner.
///
/// A T-junction at a point where three wires meet says which of the three devices
/// the two physical wires start from and the order of their targets. Its three ports
/// are the stem (in `dir`) and the two arms of the bar across it; `variant` picks the
/// source port and the order of the two targets (`netlist::junctions`). The semantics
/// stay when it is drawn as a dot.
///
/// A corner has two arms at a right angle: `dir` is its vertical arm (up or down),
/// `variant` 1 puts the horizontal one to the left, 2 to the right.
///
/// A crossing has four arms and connects the wires passing through it: `dir` (up or
/// left) is the straight line through it, `variant` 1 wires the first end of that
/// line (`dir`) to the right / lower arm and the other end to the left / upper one,
/// `variant` 2 the other way round. A bridge has four arms as well: `dir` is the side
/// the connection comes from, wired to the three others.
///
/// Automatic connections draw wires to connection points facing the free arms of
/// every kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Junction {
    pub id: u64,
    pub pos: P,
    #[serde(skip_serializing_if = "is_zero")]
    pub kind: JunctionKind,
    pub dir: TeeDir,
    /// T-junction: 1: from the first arm straight to the second arm, then to the stem;
    /// 2: from the second arm straight to the first arm, then to the stem;
    /// 3: from the stem to the first arm, then to the second arm;
    /// 4: from the stem to the second arm, then to the first arm.
    /// The first arm is the left or upper one. Corner: 1 left, 2 right.
    pub variant: u8,
    /// Drawn as a connection dot instead of the directional symbol.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dot: bool,
}

impl Junction {
    /// The usual symbol of a kind at `pos`: a T-junction with its stem down and the
    /// source there, a corner down / right, a crossing with a vertical line, a bridge
    /// fed from above.
    pub fn new(kind: JunctionKind, pos: P) -> Junction {
        let (dir, variant) = match kind {
            JunctionKind::Tee => (TeeDir::Down, 3),
            JunctionKind::Corner => (TeeDir::Down, 2),
            JunctionKind::Crossing => (TeeDir::Up, 2),
            JunctionKind::Bridge => (TeeDir::Up, 1),
        };
        Junction { id: 0, pos, kind, dir, variant, dot: false }
    }

    /// A corner with the given vertical and horizontal arm.
    pub fn corner(pos: P, vertical: TeeDir, horizontal: TeeDir) -> Junction {
        let mut j = Junction { pos, kind: JunctionKind::Corner, ..Default::default() };
        j.set_corner_arms(vertical, horizontal);
        j
    }

    pub fn is_corner(&self) -> bool {
        self.kind == JunctionKind::Corner
    }

    /// Connects the wires that pass through its point (crossings and bridges).
    pub fn connects(&self) -> bool {
        matches!(self.kind, JunctionKind::Crossing | JunctionKind::Bridge)
    }

    /// The straight line of a crossing: `Up` (vertical) or `Left` (horizontal).
    pub fn line(&self) -> TeeDir {
        match self.dir {
            TeeDir::Up | TeeDir::Down => TeeDir::Up,
            TeeDir::Left | TeeDir::Right => TeeDir::Left,
        }
    }

    /// The directions its wires leave in: a T-junction's stem and two arms, a
    /// corner's vertical and horizontal arm, all four of a crossing and a bridge.
    pub fn arms(&self) -> Vec<TeeDir> {
        match self.kind {
            JunctionKind::Tee => vec![self.dir, self.dir.arms()[0], self.dir.arms()[1]],
            JunctionKind::Corner => {
                let vertical = if matches!(self.dir, TeeDir::Up | TeeDir::Down) { self.dir } else { TeeDir::Down };
                vec![vertical, if self.variant == 1 { TeeDir::Left } else { TeeDir::Right }]
            }
            JunctionKind::Crossing | JunctionKind::Bridge => TeeDir::ALL.to_vec(),
        }
    }

    /// The wires through the symbol as pairs of arms, in the order of their targets.
    pub fn links(&self) -> Vec<(TeeDir, TeeDir)> {
        match self.kind {
            JunctionKind::Tee => {
                let [a, b] = self.dir.arms();
                match self.variant {
                    1 => vec![(a, b), (a, self.dir)],
                    2 => vec![(b, a), (b, self.dir)],
                    4 => vec![(self.dir, b), (self.dir, a)],
                    _ => vec![(self.dir, a), (self.dir, b)],
                }
            }
            JunctionKind::Corner => {
                let arms = self.arms();
                vec![(arms[0], arms[1])]
            }
            JunctionKind::Crossing => {
                let line = self.line();
                let [s1, s2] = line.arms();
                if self.variant == 1 {
                    vec![(line, line.opposite()), (line, s2), (line.opposite(), s1)]
                } else {
                    vec![(line, line.opposite()), (line, s1), (line.opposite(), s2)]
                }
            }
            JunctionKind::Bridge => {
                let [a, b] = self.dir.arms();
                vec![(self.dir, self.dir.opposite()), (self.dir, a), (self.dir, b)]
            }
        }
    }

    fn set_corner_arms(&mut self, vertical: TeeDir, horizontal: TeeDir) {
        self.dir = if matches!(vertical, TeeDir::Up | TeeDir::Down) { vertical } else { TeeDir::Down };
        self.variant = if horizontal == TeeDir::Left { 1 } else { 2 };
    }

    /// The ports keep their wires when the directions change by `map`: the stem
    /// follows, and the arms of a T-junction swap when the first one is no longer the
    /// left or upper.
    fn remap(&mut self, map: impl Fn(TeeDir) -> TeeDir) {
        match self.kind {
            JunctionKind::Corner => {
                let arms: Vec<TeeDir> = self.arms().into_iter().map(map).collect();
                let vertical = arms.iter().copied().find(|d| matches!(d, TeeDir::Up | TeeDir::Down)).unwrap_or(TeeDir::Down);
                let horizontal = arms.iter().copied().find(|d| matches!(d, TeeDir::Left | TeeDir::Right)).unwrap_or(TeeDir::Right);
                self.set_corner_arms(vertical, horizontal);
                return;
            }
            JunctionKind::Bridge => {
                self.dir = map(self.dir);
                return;
            }
            JunctionKind::Crossing => {
                // The line and the end of it wired to the left / upper arm.
                let links: Vec<(TeeDir, TeeDir)> = self.links().into_iter().map(|(a, b)| (map(a), map(b))).collect();
                let line = links[0].0;
                self.dir = if matches!(line, TeeDir::Up | TeeDir::Down) { TeeDir::Up } else { TeeDir::Left };
                let first_side = self.dir.arms()[0];
                let to_first = links[1..].iter().find(|(_, s)| *s == first_side).map(|(end, _)| *end);
                self.variant = if to_first == Some(self.dir) { 2 } else { 1 };
                return;
            }
            JunctionKind::Tee => {}
        }
        let first = map(self.dir.arms()[0]);
        self.dir = map(self.dir);
        if self.dir.arms()[0] != first {
            self.variant = match self.variant {
                1 => 2,
                2 => 1,
                3 => 4,
                4 => 3,
                v => v,
            };
        }
    }

    /// Turned by 90° clockwise with its wires: the same devices stay source and targets.
    pub fn turn(&mut self) {
        self.remap(TeeDir::turned);
    }

    /// Mirrored at a vertical axis with its wires.
    pub fn mirror(&mut self) {
        self.remap(|d| match d {
            TeeDir::Left | TeeDir::Right => d.opposite(),
            d => d,
        });
    }
}

/// Cable definition line from `a` to `b`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CableDef {
    pub id: u64,
    pub a: P,
    pub b: P,
    /// Complete tag of the cable.
    pub cable: String,
    /// Draw a shield (ellipse around the crossing).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shield: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeviceData {
    /// Assigned parts (e.g. contactor + auxiliary contact block).
    pub parts: Vec<PartRef>,
    pub description: String,
    /// Single part number of files written by older versions.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub part: String,
    /// Device property values.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: properties::Values,
}

impl PropKind {
    pub const ALL: [PropKind; 7] =
        [PropKind::Description, PropKind::PartNumber, PropKind::TypeNumber, PropKind::Manufacturer, PropKind::Label, PropKind::Pins, PropKind::Structure];

    /// Property key of the fixed properties of older files.
    pub fn key(self) -> &'static str {
        match self {
            PropKind::Description => "device.description",
            PropKind::PartNumber => "device.parts",
            PropKind::TypeNumber => "device.type_numbers",
            PropKind::Manufacturer => "device.manufacturers",
            PropKind::Label => "function.label",
            PropKind::Pins => "function.pins",
            PropKind::Structure => "device.structure",
        }
    }
}

/// Converts the placed properties of older files (fixed kinds) to property keys.
pub fn migrate(s: &mut SymbolInst) {
    for pp in &mut s.props {
        if let Some(kind) = pp.legacy.take() {
            if pp.key.is_empty() {
                pp.key = kind.key().into();
            }
        }
    }
}

pub fn default_anchor() -> Anchor {
    Anchor::Left
}

pub fn is_left(a: &Anchor) -> bool {
    *a == Anchor::Left
}

impl Project {
    /// A symbol this project can place: a built-in one or one of its user symbols.
    pub fn symbol(&self, key: &str) -> Option<SymbolRef> {
        self.symbol_table().get(key)
    }

    /// The symbols this project can place, as a snapshot that does not borrow the
    /// project (for loops that change it).
    pub fn symbol_table(&self) -> crate::symlib::Symbols {
        self.symbol_cache.table(&self.symbols)
    }

    pub fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Repairs ids after loading (e.g. hand-edited files).
    /// Reads a project file and prepares it for editing ([`Project::normalize`]).
    pub fn from_json(text: &str) -> Result<Project, String> {
        let mut prj: Project = serde_json::from_str(text).map_err(|e| tr_fmt!("Not a valid project file: {}", e))?;
        prj.normalize();
        Ok(prj)
    }

    /// Content of a project file.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn normalize(&mut self) {
        self.format = FORMAT;
        self.fix_next_id();
        // Make sure each symbol has a designation for each pin.
        let symbols = self.symbol_table();
        for pg in &mut self.pages {
            for s in &mut pg.symbols {
                if let Some(def) = symbols.get(&s.sym) {
                    while s.pins.len() < def.pins.len() {
                        s.pins.push(def.pins[s.pins.len()].label.to_string());
                    }
                    s.pins.truncate(def.pins.len());
                }
                migrate(s);
            }
        }
        self.migrate_part_assignments();
        self.refresh_rail_mounts();
        if self.pages.is_empty() {
            let id = self.new_id();
            self.pages.push(Page::new(id, "1", "Schematic", PageKind::Schematic));
        }
    }

    /// Makes `next_id` larger than every id in use.
    pub fn fix_next_id(&mut self) {
        let mut max = 0;
        for pg in &self.pages {
            max = max.max(pg.id);
            max = pg.symbols.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.wires.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.texts.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.shapes.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.dimensions.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.images.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.conn_defs.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.cable_defs.iter().map(|s| s.id).fold(max, u64::max);
            max = pg.junctions.iter().map(|s| s.id).fold(max, u64::max);
        }
        for e in &self.layout.enclosures {
            max = max.max(e.id);
            max = e.items.iter().map(|i| i.id).fold(max, u64::max);
            max = e.inner_walls.iter().map(|w| w.id).chain(e.paths.iter().map(|p| p.id)).fold(max, u64::max);
        }
        for b in &self.layout.buildings {
            max = [b.id].into_iter().chain(b.levels.iter().map(|l| l.id)).chain(b.shafts.iter().map(|s| s.id)).fold(max, u64::max);
        }
        max = self.layout.site_links.iter().map(|l| l.id).fold(max, u64::max);
        max = self.cables.iter().map(|c| c.id).fold(max, u64::max);
        max = self.harnesses.iter().map(|h| h.id).fold(max, u64::max);
        let f = &self.field;
        max = f.nodes.iter().map(|x| x.id).chain(f.segments.iter().map(|x| x.id)).chain(f.outlines.iter().map(|x| x.id)).fold(max, u64::max);
        max = f.bonds.iter().map(|x| x.id).chain(f.accessories.iter().map(|x| x.id)).fold(max, u64::max);
        max = max.max(self.model_options.max_id());
        self.next_id = self.next_id.max(max + 1);
    }

    /// Converts single part assignments of older files to part lists; old generic
    /// part numbers are mapped onto the catalog ("CT-9A-24DC" → "GEN.CT-9A-24DC").
    fn migrate_part_assignments(&mut self) {
        let catalog = openecraft_parts::default_catalog();
        let mut remapped = Vec::new();
        for dev in self.devices.values_mut() {
            let legacy = std::mem::take(&mut dev.part);
            if legacy.is_empty() || !dev.parts.is_empty() {
                continue;
            }
            let gen = format!("GEN.{legacy}");
            let number = if catalog.iter().any(|p| p.number == gen) {
                remapped.push((legacy.clone(), gen.clone()));
                gen
            } else {
                legacy
            };
            dev.parts.push(PartRef { number, count: 1, total:false });
        }
        for (old, new) in remapped {
            self.parts.retain(|p| p.number != old);
            if !self.parts.iter().any(|p| p.number == new) {
                if let Some(p) = catalog.iter().find(|p| p.number == new) {
                    self.parts.push(p.clone());
                }
            }
        }
    }

    /// All device tags used in the project.
    pub fn all_tags(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .pages
            .iter()
            .flat_map(|pg| pg.symbols.iter())
            .filter(|s| !s.tag.is_empty())
            .map(|s| s.tag.clone())
            .collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v.dedup();
        v
    }

    /// Device tags in use apart from the placed symbols, cables and harnesses: devices
    /// of the device list, items of the cabinet layouts, terminal strips, plugs and PLC
    /// cards without placed functions and the symbols of inactive options.
    pub fn unplaced_tags(&self) -> impl Iterator<Item = &str> {
        let parked = self.model_options.options.iter().flat_map(|o| &o.parked).flat_map(|p| match &p.item {
            options::Item::Symbol(s) => vec![s.tag.as_str()],
            options::Item::Page(pg) => pg.symbols.iter().map(|s| s.tag.as_str()).collect(),
            _ => Vec::new(),
        });
        self.devices
            .keys()
            .chain(self.strips.keys())
            .chain(self.plugs.keys())
            .chain(self.plc_cards.keys())
            .map(String::as_str)
            .chain(self.layout.enclosures.iter().flat_map(|e| e.items.iter()).map(|i| i.tag.as_str()))
            .chain(self.layout.enclosures.iter().map(|e|e.device_tag.as_str()))
            .chain(parked)
            .filter(|t| !t.is_empty())
    }

    /// Next free device tag for an identifying letter within a structure, e.g.
    /// `=A1-K3` (counted per structure, as `=A1-K1` and `=A2-K1` are different devices).
    pub fn next_tag(&self, structure: &str, letter: &str) -> String {
        self.next_tag_reserved(structure,letter,&Default::default())
    }

    pub(crate) fn next_tag_reserved(&self,structure:&str,letter:&str,reserved:&std::collections::BTreeSet<String>)->String {
        let prefix = format!("{structure}-{letter}");
        let tags = self.pages.iter().flat_map(|pg| pg.symbols.iter().map(|s| s.tag.as_str()));
        let max = tags
            .chain(self.cables.iter().map(|c| c.tag.as_str()))
            .chain(self.harnesses.iter().map(|h| h.tag.as_str()))
            .chain(self.unplaced_tags())
            .chain(reserved.iter().map(String::as_str))
            .filter_map(|t| t.strip_prefix(&prefix)?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix}{}", max + 1)
    }

    pub fn cable(&self, tag: &str) -> Option<&Cable> {
        self.cables.iter().find(|c| c.tag == tag)
    }
}


#[cfg(test)]
mod tests;
