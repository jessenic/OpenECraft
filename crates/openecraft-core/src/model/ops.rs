//! Data of `crate::ops` that the project file holds (see `docs/architecture.md`).

use super::*;

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
}
