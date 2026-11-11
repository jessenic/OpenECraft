//! Integration layer for enterprise systems: ERP, PDM, PLM, ECAD, MCAD, procurement
//! and wiring machines. Every connector speaks the same neutral field vocabulary
//! ([`Field`]); files of the other system are read by header names (CSV) or keys
//! (JSON) with aliases and the column mapping of a [`Profile`]. Exports store the
//! exported values as a baseline, so an import knows which values changed on both
//! sides since (conflicts). Imports never change the project directly: they give a
//! list of [`Change`]s that is reviewed and then applied.

use crate::model::{natural_cmp, PartRef, Project, ReportKind};
use crate::analysis::Analysis;
use crate::issue::{Issue, Severity};
use crate::wiring::{self, Mark};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod connectors;

pub use connectors::exchange;
pub use crate::model::integration::{Direction, DocLink, Field, Integration, ItemLink, Profile, SyncEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum System {
    Erp,
    Pdm,
    Plm,
    Ecad,
    Mcad,
    Procurement,
    WiringMachine,
}

impl System {
    pub const ALL: [System; 7] = [System::Erp, System::Pdm, System::Plm, System::Ecad, System::Mcad, System::Procurement, System::WiringMachine];

    pub fn title(self) -> &'static str {
        match self {
            System::Erp => tr("ERP"),
            System::Pdm => tr("PDM"),
            System::Plm => tr("PLM"),
            System::Ecad => tr("ECAD (PCB)"),
            System::Mcad => tr("MCAD"),
            System::Procurement => tr("Procurement"),
            System::WiringMachine => tr("Wiring machine"),
        }
    }
}

impl Field {
    pub const ALL: [Field; 52] = [
        Field::PartNumber,
        Field::ErpNumber,
        Field::ItemId,
        Field::Revision,
        Field::Lifecycle,
        Field::Description,
        Field::Manufacturer,
        Field::TypeNumber,
        Field::OrderNumber,
        Field::Supplier,
        Field::Quantity,
        Field::Unit,
        Field::Price,
        Field::Currency,
        Field::Stock,
        Field::LeadTime,
        Field::ToOrder,
        Field::Total,
        Field::OrderStatus,
        Field::Ordered,
        Field::Delivery,
        Field::Tag,
        Field::Devices,
        Field::Enclosure,
        Field::Face,
        Field::X,
        Field::Y,
        Field::Z,
        Field::Width,
        Field::Height,
        Field::Depth,
        Field::Net,
        Field::From,
        Field::To,
        Field::WireNumber,
        Field::Color,
        Field::CrossSection,
        Field::Length,
        Field::StripFrom,
        Field::StripTo,
        Field::FerruleFrom,
        Field::FerruleTo,
        Field::TwinFrom,
        Field::TwinTo,
        Field::Cable,
        Field::Core,
        Field::Job,
        Field::Status,
        Field::Note,
        Field::Document,
        Field::DocumentNumber,
        Field::State,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Field::PartNumber => "part_number",
            Field::ErpNumber => "erp_number",
            Field::ItemId => "item_id",
            Field::Revision => "revision",
            Field::Lifecycle => "lifecycle",
            Field::Description => "description",
            Field::Manufacturer => "manufacturer",
            Field::TypeNumber => "type_number",
            Field::OrderNumber => "order_number",
            Field::Supplier => "supplier",
            Field::Quantity => "quantity",
            Field::Unit => "unit",
            Field::Price => "price",
            Field::Currency => "currency",
            Field::Stock => "stock",
            Field::LeadTime => "lead_time_days",
            Field::ToOrder => "to_order",
            Field::Total => "total",
            Field::OrderStatus => "order_status",
            Field::Ordered => "ordered",
            Field::Delivery => "delivery",
            Field::Tag => "tag",
            Field::Devices => "devices",
            Field::Enclosure => "enclosure",
            Field::Face => "face",
            Field::X => "x",
            Field::Y => "y",
            Field::Z => "z",
            Field::Width => "width",
            Field::Height => "height",
            Field::Depth => "depth",
            Field::Net => "potential",
            Field::From => "from",
            Field::To => "to",
            Field::WireNumber => "wire_number",
            Field::Color => "color",
            Field::CrossSection => "cross_section",
            Field::Length => "length_mm",
            Field::StripFrom => "strip_from_mm",
            Field::StripTo => "strip_to_mm",
            Field::FerruleFrom => "ferrule_from",
            Field::FerruleTo => "ferrule_to",
            Field::TwinFrom => "twin_from",
            Field::TwinTo => "twin_to",
            Field::Cable => "cable",
            Field::Core => "core",
            Field::Job => "job",
            Field::Status => "status",
            Field::Note => "note",
            Field::Document => "document",
            Field::DocumentNumber => "document_number",
            Field::State => "state",
        }
    }

    pub fn from_code(code: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.code() == code.trim())
    }

    pub fn title(self) -> &'static str {
        match self {
            Field::PartNumber => tr("Part number"),
            Field::ErpNumber => tr("ERP number"),
            Field::ItemId => tr("Item ID"),
            Field::Revision => tr("Revision"),
            Field::Lifecycle => tr("Lifecycle state"),
            Field::Description => tr("Description"),
            Field::Manufacturer => tr("Manufacturer"),
            Field::TypeNumber => tr("Type number"),
            Field::OrderNumber => tr("Order number"),
            Field::Supplier => tr("Supplier"),
            Field::Quantity => tr("Quantity"),
            Field::Unit => tr("Unit"),
            Field::Price => tr("Price"),
            Field::Currency => tr("Currency"),
            Field::Stock => tr("Stock"),
            Field::LeadTime => tr("Lead time (days)"),
            Field::ToOrder => tr("To order"),
            Field::Total => tr("Total"),
            Field::OrderStatus => tr("Order status"),
            Field::Ordered => tr("Ordered"),
            Field::Delivery => tr("Delivery date"),
            Field::Tag => tr("Device tag"),
            Field::Devices => tr("Devices"),
            Field::Enclosure => tr("Enclosure"),
            Field::Face => tr("Face"),
            Field::X => "X",
            Field::Y => "Y",
            Field::Z => "Z",
            Field::Width => tr("Width"),
            Field::Height => tr("Height"),
            Field::Depth => tr("Depth"),
            Field::Net => tr("Potential"),
            Field::From => tr("From"),
            Field::To => tr("To"),
            Field::WireNumber => tr("Wire number"),
            Field::Color => tr("Colour"),
            Field::CrossSection => tr("Cross-section"),
            Field::Length => tr("Length (mm)"),
            Field::StripFrom => tr("Stripping from (mm)"),
            Field::StripTo => tr("Stripping to (mm)"),
            Field::FerruleFrom => tr("Ferrule from"),
            Field::FerruleTo => tr("Ferrule to"),
            Field::TwinFrom => tr("Twin ferrule with (from)"),
            Field::TwinTo => tr("Twin ferrule with (to)"),
            Field::Cable => tr("Cable"),
            Field::Core => tr("Core"),
            Field::Job => tr("Job"),
            Field::Status => tr("Status"),
            Field::Note => tr("Note"),
            Field::Document => tr("Document"),
            Field::DocumentNumber => tr("Document number"),
            Field::State => tr("State"),
        }
    }

    /// Column names other systems commonly use (compared without case, spaces and
    /// punctuation).
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Field::PartNumber => &["part", "partno", "article", "articlenumber", "material", "materialnumber", "artikel", "artikelnummer", "teilenummer", "articlenr", "partnr", "reference"],
            Field::ErpNumber => &["erp", "erpid", "erpitem", "sapnumber", "sapmaterial", "itemnumber", "itemno", "materialnr", "sachnummer"],
            Field::ItemId => &["plmid", "plmitem", "item", "objectid", "partid", "teileid"],
            Field::Revision => &["rev", "version", "index", "revisionindex", "aenderungsindex", "änderungsindex"],
            Field::Lifecycle => &["lifecyclestate", "lifecyclestatus", "releasestatus", "freigabestatus", "lebenszyklus", "maturity"],
            Field::Description => &["text", "designation", "name", "bezeichnung", "beschreibung", "materialdescription"],
            Field::Manufacturer => &["vendor", "maker", "hersteller", "mfr"],
            Field::TypeNumber => &["type", "typenumber", "typ", "typnummer", "mpn"],
            Field::OrderNumber => &["ordernumber", "orderno", "bestellnummer", "ordercode"],
            Field::Supplier => &["lieferant", "distributor", "creditor", "kreditor"],
            Field::Quantity => &["qty", "count", "amount", "menge", "anzahl", "required"],
            Field::Unit => &["uom", "unitofmeasure", "einheit", "mengeneinheit", "quantityunit"],
            Field::Price => &["unitprice", "netprice", "preis", "einzelpreis", "nettopreis", "cost"],
            Field::Currency => &["curr", "waehrung", "währung"],
            Field::Stock => &["onhand", "inventory", "available", "bestand", "lagerbestand", "stockquantity"],
            Field::LeadTime => &["leadtime", "deliverytime", "lieferzeit", "wiederbeschaffungszeit", "leadtimedays"],
            Field::ToOrder => &["orderquantity", "bestellmenge"],
            Field::Total => &["totalprice", "gesamtpreis", "sum"],
            Field::OrderStatus => &["orderstate", "bestellstatus", "postatus", "purchaseorderstatus"],
            Field::Ordered => &["orderedquantity", "confirmedquantity", "bestellt", "bestaetigtemenge"],
            Field::Delivery => &["deliverydate", "confirmeddate", "eta", "liefertermin", "lieferdatum"],
            Field::Tag => &["device", "devicetag", "bmk", "betriebsmittelkennzeichen", "refdes", "reference designator", "designator", "component"],
            Field::Devices => &["usedby", "devicetags"],
            Field::Enclosure => &["location", "cabinet", "schrank", "ort", "einbauort"],
            Field::Face => &["side", "seite", "mountingface"],
            Field::X => &["posx", "xmm", "positionx"],
            Field::Y => &["posy", "ymm", "positiony"],
            Field::Z => &["posz", "zmm", "positionz"],
            Field::Width => &["w", "breite"],
            Field::Height => &["h", "hoehe", "höhe"],
            Field::Depth => &["d", "tiefe"],
            Field::Net => &["net", "netname", "signal", "potenzial"],
            Field::From => &["source", "von", "start", "endpointa", "sidea", "terminala"],
            Field::To => &["target", "destination", "nach", "ziel", "end", "endpointb", "sideb", "terminalb"],
            Field::WireNumber => &["wire", "wireno", "wireid", "drahtnummer", "aderkennzeichnung", "verbindungsnummer"],
            Field::Color => &["colour", "farbe", "wirecolor"],
            Field::CrossSection => &["crosssection", "csa", "querschnitt", "mm2", "gauge"],
            Field::Length => &["length", "laenge", "länge", "cutlength"],
            Field::StripFrom => &["stripa", "stripfrom", "abisolierlaengea"],
            Field::StripTo => &["stripb", "stripto", "abisolierlaengeb"],
            Field::FerruleFrom => &["ferrulea", "aderendhuelsea"],
            Field::FerruleTo => &["ferruleb", "aderendhuelseb"],
            Field::TwinFrom => &["twina", "doublecrimpa", "zwillingaderendhuelsea"],
            Field::TwinTo => &["twinb", "doublecrimpb", "zwillingaderendhuelseb"],
            Field::Cable => &["cabletag", "kabel"],
            Field::Core => &["coreno", "ader"],
            Field::Job => &["jobno", "order", "auftrag", "position"],
            Field::Status => &["result", "state", "ergebnis", "processingstatus"],
            Field::Note => &["comment", "message", "remark", "bemerkung", "error", "fehler"],
            Field::Document => &["page", "sheet", "blatt", "documentname"],
            Field::DocumentNumber => &["docno", "documentno", "dokumentnummer", "drawingnumber", "zeichnungsnummer"],
            Field::State => &["documentstate", "status", "dokumentstatus", "workflowstate"],
        }
    }
}

fn normalize(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// What an imported value changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    /// A part of the project (commercial data and its links to other systems), by part
    /// number.
    Part,
    /// A device, by device tag.
    Device,
    /// The position of a device in the panel layout, by device tag.
    Placement,
    /// A routed wire, by its two ends (`from|to`).
    Wire,
    /// A page, by page name.
    Document,
    /// The project itself (empty key).
    Project,
}

impl Target {
    pub fn title(self) -> &'static str {
        match self {
            Target::Part => tr("Part"),
            Target::Device => tr("Device"),
            Target::Placement => tr("Placement"),
            Target::Wire => tr("Wire"),
            Target::Document => tr("Document"),
            Target::Project => tr("Project"),
        }
    }
}

/// A connector: the file format of one kind of system, in both directions.
pub struct Connector {
    pub id: &'static str,
    pub system: System,
    pub title: &'static str,
    pub help: &'static str,
    /// File extension of the export.
    pub extension: &'static str,
    /// What imports change and which fields they take over (in this order).
    pub target: Target,
    pub accepts: &'static [Field],
    pub export: fn(&Project, &Analysis) -> String,
}

pub static CONNECTORS: &[Connector] = &[
    Connector {
        id: "erp",
        system: System::Erp,
        title: tr_const!("ERP item master and bill of materials (CSV)"),
        help: tr_const!("Exports the bill of materials with the item master data; imports ERP numbers, prices, stock and lead times"),
        extension: "csv",
        target: Target::Part,
        accepts: &[Field::ErpNumber, Field::Description, Field::Price, Field::Currency, Field::Stock, Field::LeadTime],
        export: connectors::erp,
    },
    Connector {
        id: "pdm",
        system: System::Pdm,
        title: tr_const!("PDM documents and metadata (JSON)"),
        help: tr_const!("Exports the project and page metadata with the revisions; imports document numbers, revisions and states"),
        extension: "json",
        target: Target::Document,
        accepts: &[Field::DocumentNumber, Field::Revision, Field::State],
        export: connectors::pdm,
    },
    Connector {
        id: "plm",
        system: System::Plm,
        title: tr_const!("PLM engineering BOM (JSON)"),
        help: tr_const!("Exports the engineering BOM (locations, devices, parts); imports item IDs, revisions and lifecycle states"),
        extension: "json",
        target: Target::Part,
        accepts: &[Field::ItemId, Field::Revision, Field::Lifecycle],
        export: connectors::plm,
    },
    Connector {
        id: "ecad",
        system: System::Ecad,
        title: tr_const!("ECAD components and netlist (JSON)"),
        help: tr_const!("Exports the devices and point-to-point connections for PCB design; imports part assignments and descriptions of the components"),
        extension: "json",
        target: Target::Device,
        accepts: &[Field::PartNumber, Field::Description],
        export: connectors::ecad,
    },
    Connector {
        id: "mcad",
        system: System::Mcad,
        title: tr_const!("MCAD device placement (CSV)"),
        help: tr_const!("Exports the positions and sizes of the devices in the panel layout; imports positions changed in the MCAD system"),
        extension: "csv",
        target: Target::Placement,
        accepts: &[Field::X, Field::Y, Field::Z],
        export: connectors::mcad,
    },
    Connector {
        id: "procurement",
        system: System::Procurement,
        title: tr_const!("Procurement order list (CSV)"),
        help: tr_const!("Exports the order list per supplier (required quantity less stock, rounded to packs); imports order confirmations"),
        extension: "csv",
        target: Target::Part,
        accepts: &[Field::OrderStatus, Field::Ordered, Field::Delivery, Field::Stock, Field::Price],
        export: connectors::procurement,
    },
    Connector {
        id: "wiring_machine",
        system: System::WiringMachine,
        title: tr_const!("Wiring machine job list (CSV)"),
        help: tr_const!("Exports the wires with cut length, stripping and ferrules for wire processing machines; imports the processing feedback as wiring marks"),
        extension: "csv",
        target: Target::Wire,
        accepts: &[Field::Status, Field::Note],
        export: connectors::wiring_machine,
    },
];

pub fn connector(id: &str) -> Option<&'static Connector> {
    CONNECTORS.iter().find(|c| c.id == id.trim())
}

/// Most entries kept in the log.
pub const LOG_SIZE: usize = 200;

impl DocLink {
    pub fn is_default(&self) -> bool {
        *self == DocLink::default()
    }
}

impl Integration {
    pub fn is_empty(&self) -> bool {
        *self == Integration::default()
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    fn record(&mut self, entry: SyncEntry) {
        self.log.push(entry);
        if self.log.len() > LOG_SIZE {
            let n = self.log.len() - LOG_SIZE;
            self.log.drain(..n);
        }
    }
}

/// Imported data: one record per row / JSON object, by neutral field.
pub type Record = BTreeMap<Field, String>;

/// The field a column name stands for: the profile's mapping first, then the codes
/// and aliases.
pub fn column_field(name: &str, mapping: &BTreeMap<String, String>) -> Option<Field> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    if let Some(f) = mapping.iter().find(|(_, col)| normalize(col) == n).and_then(|(code, _)| Field::from_code(code)) {
        return Some(f);
    }
    Field::ALL.into_iter().find(|f| normalize(f.code()) == n).or_else(|| Field::ALL.into_iter().find(|f| f.aliases().iter().any(|a| normalize(a) == n)))
}

fn value_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn object_record(obj: &serde_json::Map<String, Value>, mapping: &BTreeMap<String, String>) -> Record {
    let mut r = Record::new();
    for (k, v) in obj {
        if let (Some(f), Some(text)) = (column_field(k, mapping), value_text(v)) {
            r.entry(f).or_insert(text);
        }
    }
    r
}

fn json_records(v: &Value, mapping: &BTreeMap<String, String>, out: &mut Vec<Record>) {
    match v {
        Value::Array(list) => {
            for item in list {
                match item {
                    Value::Object(o) => {
                        out.push(object_record(o, mapping));
                        for nested in o.values().filter(|n| n.is_array()) {
                            json_records(nested, mapping, out);
                        }
                    }
                    other => json_records(other, mapping, out),
                }
            }
        }
        Value::Object(o) => {
            // The project object of the PDM export is a record of its own.
            if let Some(Value::Object(p)) = o.get("project") {
                let mut r = object_record(p, mapping);
                r.insert(Field::Document, String::new());
                out.push(r);
            }
            for (k, nested) in o {
                if nested.is_array() && k != "revisions" {
                    json_records(nested, mapping, out);
                }
            }
        }
        _ => {}
    }
}

/// Records of a CSV (header row, `;` or `,`) or JSON file (arrays of objects).
pub fn records(text: &str, mapping: &BTreeMap<String, String>) -> Result<Vec<Record>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| tr_fmt!("Not a valid JSON file: {}", e))?;
        let mut out = Vec::new();
        json_records(&v, mapping, &mut out);
        out.retain(|r| !r.is_empty());
        return Ok(out);
    }
    let rows = openecraft_parts::parse_csv(text);
    let Some((header, body)) = rows.split_first() else { return Ok(Vec::new()) };
    let cols: Vec<Option<Field>> = header.iter().map(|h| column_field(h, mapping)).collect();
    if cols.iter().all(Option::is_none) {
        return Err(tr("No known column in the header row").into());
    }
    Ok(body
        .iter()
        .map(|row| {
            let mut r = Record::new();
            for (c, f) in row.iter().zip(&cols) {
                if let Some(f) = f {
                    r.entry(*f).or_insert_with(|| c.trim().to_string());
                }
            }
            r
        })
        .collect())
}

/// A number written by another system ("1.234,5", "12,5", "3 pcs").
pub fn number(text: &str) -> Option<f32> {
    let t: String = text.trim().chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | ' ')).filter(|c| *c != ' ').collect();
    let t = match (t.rfind(','), t.rfind('.')) {
        (Some(c), Some(d)) if c > d => t.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => t.replace(',', ""),
        (Some(_), None) => t.replace(',', "."),
        _ => t,
    };
    t.parse().ok()
}

/// A number as written in exports and compared by imports.
pub fn fmt_number(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Wiring state of a wire processing feedback: wired, problem or open.
pub fn wire_state(text: &str) -> Option<&'static str> {
    match normalize(text).as_str() {
        "ok" | "done" | "wired" | "finished" | "processed" | "1" | "true" | "yes" | "io" | "fertig" | "erledigt" => Some("wired"),
        "fault" | "error" | "failed" | "problem" | "nok" | "nio" | "rejected" | "0" | "false" | "fehler" => Some("problem"),
        "open" | "" | "pending" | "offen" => Some("open"),
        _ => None,
    }
}

fn part_mut<'a>(prj: &'a mut Project, number: &str) -> Option<&'a mut crate::model::Part> {
    if !prj.parts.iter().any(|p| p.number == number) {
        let p = crate::parts::catalog().iter().find(|p| p.number == number)?.clone();
        prj.parts.push(p);
    }
    prj.parts.iter_mut().find(|p| p.number == number)
}

fn wire_key(w: &crate::panel::RoutedWire) -> String {
    format!("{}|{}", w.from, w.to)
}

fn find_wire<'a>(an: &'a Analysis, key: &str) -> Option<&'a crate::panel::RoutedWire> {
    let (a, b) = key.split_once('|')?;
    an.wiring.iter().find(|w| (w.from == a && w.to == b) || (w.from == b && w.to == a))
}

fn placement(prj: &Project, tag: &str) -> Option<(usize, usize)> {
    prj.layout.enclosures.iter().enumerate().find_map(|(e, enc)| enc.items.iter().position(|i| i.kind == crate::panel::ItemKind::Device && i.tag == tag).map(|i| (e, i)))
}

fn opt_number(v: Option<f32>) -> String {
    v.map(fmt_number).unwrap_or_default()
}

/// Current value of a field in the project, None if the target does not exist.
pub fn current(prj: &Project, an: &Analysis, target: Target, key: &str, field: Field) -> Option<String> {
    let link = || prj.integration.items.get(key).cloned().unwrap_or_default();
    Some(match target {
        Target::Part => {
            let part = prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == key)?;
            match field {
                Field::ErpNumber => link().erp_number,
                Field::ItemId => link().item_id,
                Field::Revision => link().revision,
                Field::Lifecycle => link().lifecycle,
                Field::Stock => opt_number(link().stock),
                Field::LeadTime => link().lead_time.map(|d| d.to_string()).unwrap_or_default(),
                Field::OrderStatus => link().order_status,
                Field::Ordered => opt_number(link().ordered),
                Field::Delivery => link().delivery,
                Field::Description => part.description.clone(),
                Field::Price => fmt_number(part.price),
                Field::Currency => part.currency.clone(),
                Field::OrderNumber => part.order_number.clone(),
                Field::Supplier => part.supplier.clone(),
                _ => return None,
            }
        }
        Target::Device => {
            let d = prj.devices.get(key)?;
            match field {
                Field::PartNumber => d.parts.first().map(|r| r.number.clone()).unwrap_or_default(),
                Field::Description => d.description.clone(),
                _ => return None,
            }
        }
        Target::Placement => {
            let (e, i) = placement(prj, key)?;
            let it = &prj.layout.enclosures[e].items[i];
            match field {
                Field::X => fmt_number(it.x),
                Field::Y => fmt_number(it.y),
                Field::Z => fmt_number(it.z),
                _ => return None,
            }
        }
        Target::Wire => {
            let w = find_wire(an, key)?;
            match field {
                Field::Status => match wiring::mark_of(prj, w).map(|m| m.mark) {
                    None => "open",
                    Some(Mark::Done) => "wired",
                    Some(Mark::Problem) => "problem",
                }
                .to_string(),
                Field::Note => wiring::mark_of(prj, w).map(|m| m.note.clone()).unwrap_or_default(),
                _ => return None,
            }
        }
        Target::Document | Target::Project => {
            let doc = if target == Target::Project {
                prj.integration.project.clone()
            } else {
                prj.pages.iter().find(|p| p.name == key)?;
                prj.integration.documents.get(key).cloned().unwrap_or_default()
            };
            match field {
                Field::DocumentNumber => doc.number,
                Field::Revision => doc.revision,
                Field::State => doc.state,
                _ => return None,
            }
        }
    })
}

/// Keys of all objects a connector's imports can change (for the baseline).
fn keys(prj: &Project, an: &Analysis, target: Target) -> Vec<String> {
    match target {
        Target::Part => bom(prj, an).into_iter().map(|l| l.part).collect(),
        Target::Device => prj.devices.keys().cloned().collect(),
        Target::Placement => prj.layout.enclosures.iter().flat_map(|e| &e.items).filter(|i| i.kind == crate::panel::ItemKind::Device && !i.tag.is_empty()).map(|i| i.tag.clone()).collect(),
        Target::Wire => an.wiring.iter().map(wire_key).collect(),
        Target::Document | Target::Project => std::iter::once(String::new()).chain(prj.pages.iter().map(|p| p.name.clone())).collect(),
    }
}

fn document_target(key: &str) -> Target {
    if key.is_empty() {
        Target::Project
    } else {
        Target::Document
    }
}

fn baseline_key(target: Target, key: &str, field: Field) -> String {
    format!("{target:?}|{key}|{}", field.code())
}

/// A line of the bill of materials.
#[derive(Clone, Debug, PartialEq)]
pub struct BomLine {
    pub part: String,
    pub quantity: f32,
    pub unit: String,
    pub devices: Vec<String>,
}

/// The bill of materials (as in the report) as data.
pub fn bom(prj: &Project, an: &Analysis) -> Vec<BomLine> {
    let Some(t) = an.report(prj, ReportKind::BillOfMaterials) else { return Vec::new() };
    t.rows
        .iter()
        .filter(|r| r.len() > 7 && !r[3].is_empty())
        .map(|r| BomLine {
            part: r[3].clone(),
            quantity: number(&r[1]).unwrap_or(1.0),
            unit: r[2].clone(),
            devices: r[7].split(", ").filter(|s| !s.is_empty()).map(str::to_string).collect(),
        })
        .collect()
}

/// Exports with a connector: the file content. Stores the exported values as the
/// baseline for conflicts of later imports and logs the export.
pub fn export(prj: &mut Project, an: &Analysis, c: &Connector, date: &str) -> String {
    let text = (c.export)(prj, an);
    let mut base = BTreeMap::new();
    let ks = keys(prj, an, c.target);
    for k in &ks {
        let target = if c.target == Target::Document { document_target(k) } else { c.target };
        for &f in c.accepts {
            if let Some(v) = current(prj, an, target, k, f) {
                base.insert(baseline_key(target, k, f), v);
            }
        }
    }
    prj.integration.baseline.insert(c.id.to_string(), base);
    prj.integration.record(SyncEntry { date: date.to_string(), connector: c.id.to_string(), direction: Direction::Export, count: ks.len(), conflicts: 0 });
    text
}

/// A value an import would change.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub target: Target,
    pub key: String,
    pub field: Field,
    pub old: String,
    pub new: String,
    /// The value was also changed in the project since the last export.
    pub conflict: bool,
}

/// Result of reading a file of another system.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    pub changes: Vec<Change>,
    /// Keys of records that match nothing in the project.
    pub unmatched: Vec<String>,
    pub records: usize,
}

impl Preview {
    pub fn conflicts(&self) -> usize {
        self.changes.iter().filter(|c| c.conflict).count()
    }
}

fn record_key(prj: &Project, an: &Analysis, target: Target, r: &Record) -> Option<String> {
    let get = |f: Field| r.get(&f).map(|s| s.trim()).filter(|s| !s.is_empty());
    match target {
        Target::Part => get(Field::PartNumber).map(str::to_string).or_else(|| {
            // ERP / PLM systems may only send their own number.
            let erp = get(Field::ErpNumber).or(get(Field::ItemId))?;
            prj.integration.items.iter().find(|(_, l)| l.erp_number == erp || l.item_id == erp).map(|(k, _)| k.clone())
        }),
        Target::Device | Target::Placement => get(Field::Tag).map(str::to_string),
        Target::Wire => match (get(Field::From), get(Field::To)) {
            (Some(a), Some(b)) => Some(format!("{a}|{b}")),
            _ => {
                let n = get(Field::WireNumber)?;
                let mut hits = an.wiring.iter().filter(|w| w.number == n);
                match (hits.next(), hits.next()) {
                    (Some(w), None) => Some(wire_key(w)),
                    _ => Some(format!("{n}|")),
                }
            }
        },
        Target::Document | Target::Project => r.get(&Field::Document).map(|s| s.trim().to_string()),
    }
}

/// Value of an imported field in the form the project stores.
fn imported_value(field: Field, text: &str) -> Option<String> {
    let t = text.trim();
    match field {
        Field::Price | Field::Stock | Field::Ordered | Field::X | Field::Y | Field::Z => {
            if t.is_empty() {
                Some(String::new())
            } else {
                number(t).map(fmt_number)
            }
        }
        Field::LeadTime => {
            if t.is_empty() {
                Some(String::new())
            } else {
                number(t).map(|d| (d.max(0.0).round() as u32).to_string())
            }
        }
        Field::Status => wire_state(t).map(str::to_string),
        _ => Some(t.to_string()),
    }
}

/// Reads a file of another system with a connector and lists what would change.
pub fn import(prj: &Project, an: &Analysis, c: &Connector, text: &str, mapping: &BTreeMap<String, String>) -> Result<Preview, String> {
    let recs = records(text, mapping)?;
    let base = prj.integration.baseline.get(c.id);
    let mut out = Preview { records: recs.len(), ..Default::default() };
    for r in &recs {
        let Some(key) = record_key(prj, an, c.target, r) else { continue };
        let target = if c.target == Target::Document { document_target(&key) } else { c.target };
        let mut matched = false;
        for &f in c.accepts {
            let Some(text) = r.get(&f) else {
                matched |= current(prj, an, target, &key, f).is_some();
                continue;
            };
            let Some(old) = current(prj, an, target, &key, f) else { continue };
            matched = true;
            // Empty values of other systems do not clear data, except the wiring note.
            if text.trim().is_empty() && f != Field::Note {
                continue;
            }
            let Some(new) = imported_value(f, text) else { continue };
            if new == old {
                continue;
            }
            // Unchanged in the other system since the export: the project's value stays.
            let exported = base.and_then(|b| b.get(&baseline_key(target, &key, f)));
            if exported == Some(&new) {
                continue;
            }
            let conflict = exported.is_some_and(|b| *b != old);
            out.changes.push(Change { target, key: key.clone(), field: f, old, new, conflict });
        }
        if !matched && !c.accepts.iter().all(|f| !r.contains_key(f)) {
            out.unmatched.push(key.replace('|', " – "));
        }
    }
    out.unmatched.sort_by(|a, b| natural_cmp(a, b));
    out.unmatched.dedup();
    Ok(out)
}

fn set(prj: &mut Project, an: &Analysis, ch: &Change) -> bool {
    let num = || number(&ch.new);
    match ch.target {
        Target::Part => {
            let link_field = matches!(ch.field, Field::ErpNumber | Field::ItemId | Field::Revision | Field::Lifecycle | Field::Stock | Field::LeadTime | Field::OrderStatus | Field::Ordered | Field::Delivery);
            if link_field {
                let l = prj.integration.items.entry(ch.key.clone()).or_default();
                match ch.field {
                    Field::ErpNumber => l.erp_number = ch.new.clone(),
                    Field::ItemId => l.item_id = ch.new.clone(),
                    Field::Revision => l.revision = ch.new.clone(),
                    Field::Lifecycle => l.lifecycle = ch.new.clone(),
                    Field::Stock => l.stock = num(),
                    Field::LeadTime => l.lead_time = num().map(|d| d as u32),
                    Field::OrderStatus => l.order_status = ch.new.clone(),
                    Field::Ordered => l.ordered = num(),
                    _ => l.delivery = ch.new.clone(),
                }
                return true;
            }
            let Some(p) = part_mut(prj, &ch.key) else { return false };
            match ch.field {
                Field::Description => p.description = ch.new.clone(),
                Field::Price => p.price = num().unwrap_or(p.price),
                Field::Currency => p.currency = ch.new.clone(),
                Field::OrderNumber => p.order_number = ch.new.clone(),
                Field::Supplier => p.supplier = ch.new.clone(),
                _ => return false,
            }
            true
        }
        Target::Device => {
            let Some(d) = prj.devices.get_mut(&ch.key) else { return false };
            match ch.field {
                Field::PartNumber => match d.parts.first_mut() {
                    Some(r) => r.number = ch.new.clone(),
                    None => d.parts.push(PartRef { number: ch.new.clone(), count: 1, total:false }),
                },
                Field::Description => d.description = ch.new.clone(),
                _ => return false,
            }
            if ch.field == Field::PartNumber && !prj.parts.iter().any(|p| p.number == ch.new) {
                if let Some(p) = crate::parts::catalog().iter().find(|p| p.number == ch.new) {
                    prj.parts.push(p.clone());
                }
            }
            true
        }
        Target::Placement => {
            let (Some((e, i)), Some(v)) = (placement(prj, &ch.key), num()) else { return false };
            let it = &mut prj.layout.enclosures[e].items[i];
            match ch.field {
                Field::X => it.x = v,
                Field::Y => it.y = v,
                Field::Z => it.z = v,
                _ => return false,
            }
            true
        }
        Target::Wire => {
            let Some(w) = find_wire(an, &ch.key) else { return false };
            let note = wiring::mark_of(prj, w).map(|m| m.note.clone()).unwrap_or_default();
            let mark = wiring::mark_of(prj, w).map(|m| m.mark);
            match ch.field {
                Field::Status => {
                    let mark = match ch.new.as_str() {
                        "wired" => Some(Mark::Done),
                        "problem" => Some(Mark::Problem),
                        _ => None,
                    };
                    wiring::set_mark(prj, w, mark, &note);
                }
                Field::Note => match mark {
                    Some(m) => wiring::set_mark(prj, w, Some(m), &ch.new),
                    None => return false,
                },
                _ => return false,
            }
            true
        }
        Target::Document | Target::Project => {
            let doc = if ch.target == Target::Project { &mut prj.integration.project } else { prj.integration.documents.entry(ch.key.clone()).or_default() };
            match ch.field {
                Field::DocumentNumber => doc.number = ch.new.clone(),
                Field::Revision => doc.revision = ch.new.clone(),
                Field::State => doc.state = ch.new.clone(),
                _ => return false,
            }
            true
        }
    }
}

/// Applies the changes of an import (conflicts only with `overwrite`), logs the
/// import and returns the number of changed values.
pub fn apply(prj: &mut Project, an: &Analysis, c: &Connector, changes: &[Change], overwrite: bool, date: &str) -> usize {
    let mut n = 0;
    let mut conflicts = 0;
    for ch in changes {
        if ch.conflict && !overwrite {
            conflicts += 1;
            continue;
        }
        if set(prj, an, ch) {
            n += 1;
            // The applied value is the common state of both systems now.
            if let Some(b) = prj.integration.baseline.get_mut(c.id) {
                b.insert(baseline_key(ch.target, &ch.key, ch.field), ch.new.clone());
            }
        }
    }
    prj.integration.record(SyncEntry { date: date.to_string(), connector: c.id.to_string(), direction: Direction::Import, count: n, conflicts });
    n
}

/// Lifecycle states of parts that should not be used any more.
pub fn blocked_state(state: &str) -> bool {
    matches!(normalize(state).as_str(), "obsolete" | "discontinued" | "blocked" | "phaseout" | "eol" | "endoflife" | "inactive" | "superseded" | "abgekuendigt" | "gesperrt" | "auslauf")
}

/// Project check: devices using parts that the PLM / ERP system marks as obsolete or
/// blocked.
pub fn problems(prj: &Project) -> Vec<Issue> {
    let mut out = Vec::new();
    for (number, link) in prj.integration.items.iter().filter(|(_, l)| blocked_state(&l.lifecycle)) {
        let devices: Vec<&str> = prj.devices.iter().filter(|(_, d)| d.parts.iter().any(|r| &r.number == number)).map(|(t, _)| t.as_str()).collect();
        if devices.is_empty() {
            continue;
        }
        out.push(crate::issue!(Severity::Warning, None, "Part {} is {} in the PLM system (used by {})", number, link.lifecycle, devices.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests;
