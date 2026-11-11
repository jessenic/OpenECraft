//! Data of `crate::integration` that the project file holds (see `docs/architecture.md`).


use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Integration {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
    /// Links of the parts, by part number.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub items: BTreeMap<String, ItemLink>,
    /// Links of the pages, by page name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub documents: BTreeMap<String, DocLink>,
    #[serde(skip_serializing_if = "DocLink::is_default")]
    pub project: DocLink,
    /// Values at the last export, per connector: `target|key|field` → value.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub baseline: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<SyncEntry>,
}

/// One exchange with another system.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncEntry {
    pub date: String,
    pub connector: String,
    pub direction: Direction,
    /// Exported records or applied changes.
    pub count: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub conflicts: usize,
}

pub(crate) fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    Export,
    Import,
}

/// Data of a part in the other systems.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemLink {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub erp_number: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub item_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub revision: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lifecycle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead_time: Option<u32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub order_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<f32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub delivery: String,
}

/// Settings of a connection to one system: the connector and the column names the
/// other system uses for the neutral fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub connector: String,
    /// Field code → column name of the other system.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub mapping: BTreeMap<String, String>,
}

/// Neutral field of the exchange. The code is the column / key name in exported
/// files; imports also accept the aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    PartNumber,
    ErpNumber,
    ItemId,
    Revision,
    Lifecycle,
    Description,
    Manufacturer,
    TypeNumber,
    OrderNumber,
    Supplier,
    Quantity,
    Unit,
    Price,
    Currency,
    Stock,
    LeadTime,
    ToOrder,
    Total,
    OrderStatus,
    Ordered,
    Delivery,
    Tag,
    /// Devices that use a part (lists, not a key).
    Devices,
    Enclosure,
    Face,
    X,
    Y,
    Z,
    Width,
    Height,
    Depth,
    Net,
    From,
    To,
    WireNumber,
    Color,
    CrossSection,
    Length,
    StripFrom,
    StripTo,
    FerruleFrom,
    FerruleTo,
    /// The other wire (its far end) crimped into the twin ferrule of the `from` end.
    TwinFrom,
    TwinTo,
    Cable,
    Core,
    Job,
    Status,
    Note,
    Document,
    DocumentNumber,
    State,
}

/// Number, revision and state of a document (the project or a page) in the PDM system.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocLink {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub number: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub revision: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub state: String,
}
