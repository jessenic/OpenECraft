//! Data of `crate::symlib` that the project file holds (see `docs/architecture.md`).


use crate::geom::P;
use crate::symbols::{Func, Prim};
use openecraft_parts::Shared;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

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
    pub name: String,
    /// Library (group in the symbol list), e.g. the company name.
    pub library: String,
    /// Identifying letter(s) for new device tags.
    pub letter: String,
    pub func: Func,
    pub default_label: String,
    pub auto_number: bool,
    pub show_tag: bool,
    pub show_pin_labels: bool,
    pub prims: Vec<Prim>,
    pub pins: Vec<UserPin>,
    /// Pins that are connected inside the symbol (terminals).
    pub bridges: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPin {
    pub pos: P,
    /// Default connection point designation.
    pub label: String,
}
