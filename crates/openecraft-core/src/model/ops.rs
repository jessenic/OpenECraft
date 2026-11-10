//! Data of `crate::ops` that the project file holds (see `docs/architecture.md`).

use super::*;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clip {
    pub magic: String,
    /// Explicit insertion point for imported window macros.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<P>,
    /// Structure of the page the clip was copied from.
    pub function: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub installation: String,
    pub location: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub product: String,
    pub symbols: Vec<SymbolInst>,
    /// Definitions needed when this clip is inserted into another project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub symbol_defs: super::symlib::SymbolList,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub devices: BTreeMap<String, DeviceData>,
    /// Explicit literal accessory articles to add when extending an existing device.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub device_part_additions: BTreeMap<String, BTreeSet<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    /// Cable definitions owned by this clip, with their catalog references.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cables: Vec<Cable>,
    pub wires: Vec<Wire>,
    pub texts: Vec<TextItem>,
    pub shapes: Vec<Shape>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<Dimension>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conn_defs: Vec<ConnDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cable_defs: Vec<CableDef>,
    #[serde(skip)]
    pub(crate) symbol_cache: crate::symlib::SymbolCache,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub junctions: Vec<Junction>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub structure_boxes: Vec<super::structure::StructureBox>,
}
