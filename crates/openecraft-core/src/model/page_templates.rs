//! Data of `crate::page_templates` that the project file holds (see `docs/architecture.md`).


use super::PageKind;
use super::ops::Clip;
use super::structure::Dt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageTemplate {
    pub name: String,
    pub kind: PageKind,
    /// Plot frame; empty = the project default.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub frame: String,
    /// Title block; empty = the project default.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title_block: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub structure: Dt,
    /// Initial editable page properties, including fields of imported frames.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String,String>,
    /// Items placed on the new page (positions kept).
    pub content: Clip,
}
