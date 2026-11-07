//! Data of `crate::symlib` that the project file holds (see `docs/architecture.md`).

use crate::geom::P;
use crate::symbols::{Func, Prim, SymbolDomain, PortKind};
use openecraft_parts::Shared;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::collections::BTreeMap;

/// Snapshots share the symbol drawings until an editor changes the library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolList(Shared<Vec<UserSymbol>>);

impl SymbolList {
    pub(crate) fn same_snapshot(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

impl Deref for SymbolList {
    type Target = Vec<UserSymbol>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SymbolList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.make_mut()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSymbol {
    pub key: String,
    /// Fingerprint of the last imported EMA definition; used to preserve local edits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ema_revision: Option<String>,
    /// Stable source identifiers and vendor properties retained by importers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_data: Option<Shared<serde_json::Value>>,
    pub name: String,
    /// Library (group in the symbol list), e.g. the company name.
    pub library: String,
    /// Identifying letter(s) for new device tags.
    pub letter: String,
    pub func: Func,
    pub domain: SymbolDomain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plc_channel: Option<crate::symbols::PlcChannel>,
    pub default_label: String,
    pub auto_number: bool,
    pub show_tag: bool,
    pub show_pin_labels: bool,
    pub prims: Vec<Prim>,
    pub pins: Vec<UserPin>,
    /// Explicit fluid port functions by zero-based pin index; absent = infer from designation.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fluid_ports: BTreeMap<usize, PortKind>,
    /// Allowed side contacts in local coordinates: up 1, right 2, down 4, left 8.
    /// Absent entries accept every direction; saved wire endpoints always attach.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pin_directions: BTreeMap<usize, u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub directed_junction: bool,
    /// Pins that are connected inside the symbol (terminals).
    pub bridges: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPin {
    pub pos: P,
    /// Default connection point designation.
    pub label: String,
    /// Internal bridge attachment rather than a physical device connection point.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub internal: bool,
}

impl super::Project {
    /// Resolve the single Insert center entry for a standalone channel macro.
    /// Its individual definitions still supply the distinct pin locations and graphics.
    pub fn next_picker_symbol(&self, key: &str) -> Option<&'static str> {
        let def = self.symbol(key)?;
        let Some(c) = &def.plc_channel else { return Some(def.key) };
        let card = self.latest_imported_card(&c.family, None);
        let used: std::collections::HashSet<_> = self
            .pages
            .iter()
            .flat_map(|page| &page.symbols)
            .filter(|s| Some(s.tag.as_str()) == card)
            .filter_map(|s| {
                let d = self.symbol(&s.sym)?;
                let point = d.plc_channel.as_ref().filter(|point| point.family == c.family)?;
                Some((point.input, point.index))
            })
            .collect();
        let next = self
            .symbols
            .iter()
            .filter_map(|s| {
                let point = s.plc_channel.as_ref().filter(|point| point.family == c.family)?;
                Some(((used.contains(&(point.input, point.index)), point.index), s))
            })
            .min_by_key(|(order, _)| *order)?
            .1;
        self.symbol(&next.key).map(|d| d.key)
    }

    pub(crate) fn latest_imported_card(&self, family: &str, article: Option<&str>) -> Option<&str> {
        self.pages
            .iter()
            .flat_map(|pg| &pg.symbols)
            .filter(|s| article.is_none_or(|number| self.devices.get(&s.tag).is_some_and(|d| d.parts.iter().any(|p| p.number == number))))
            .filter(|s| self.symbol(&s.sym).is_some_and(|d| d.plc_channel.as_ref().is_some_and(|c| c.family == family)))
            .max_by_key(|s| s.id)
            .map(|s| s.tag.as_str())
    }

    pub fn channel_placed(&self, card: &str, family: &str, input: bool, index: u16) -> bool {
        self.pages.iter().flat_map(|pg| &pg.symbols).filter(|s| s.tag == card).any(|s| {
            self.symbol(&s.sym).is_some_and(|d| d.plc_channel.as_ref().is_some_and(|c| c.family == family && c.input == input && c.index == index))
        })
    }
}
