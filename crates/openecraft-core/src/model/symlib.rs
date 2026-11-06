//! Data of `crate::symlib` that the project file holds (see `docs/architecture.md`).


use crate::geom::P;
use crate::symbols::{Func, Prim};
use serde::{Deserialize, Serialize};

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
