//! Settings schemes (in the spirit of EPLAN's settings schemes): the settings of a
//! project – wire numbering, potentials and wire defaults, wire colours, wire crossings
//! and connections, marking, hose defaults, cross references and the check scheme – as
//! one file to start other projects with the same settings. Every part is optional, so
//! a file may carry only some of them.

use crate::model::checks::Level;
use crate::model::potentials::{PotentialDef, WireDefaults};
use crate::model::{CrossRefs, Project, WireNumbering, WireSettings};
use openecraft_i18n::{tr_const, tr_fmt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsScheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_numbering: Option<WireNumbering>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub potentials: Option<Vec<PotentialDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_defaults: Option<WireDefaults>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_colors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_settings: Option<WireSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marking: Option<crate::model::marking::Settings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fluid: Option<crate::model::fluid::FluidSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_references: Option<CrossRefs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks_off: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_levels: Option<BTreeMap<String, Level>>,
}

/// The settings of the project as a scheme file (JSON).
pub fn export(prj: &Project) -> String {
    let s = SettingsScheme {
        wire_numbering: Some(prj.wire_numbering.clone()),
        potentials: Some(prj.potentials.clone()),
        wire_defaults: Some(prj.wire_defaults.clone()),
        wire_colors: Some(prj.wire_colors),
        wire_settings: Some(prj.wire_settings.clone()),
        marking: Some(prj.marking.clone()),
        fluid: Some(prj.fluid.clone()),
        cross_references: Some(prj.xref.clone()),
        checks_off: Some(prj.check_settings.disabled.clone()),
        message_levels: Some(prj.check_settings.levels.clone()),
    };
    serde_json::to_string_pretty(&s).unwrap_or_default()
}

/// Takes the settings a scheme file carries; returns the names of the parts taken
/// (English, translate where shown).
pub fn apply(prj: &mut Project, json: &str) -> Result<Vec<&'static str>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| tr_fmt!("Not a settings scheme: {}", e))?;
    if !value.is_object() {
        return Err(tr_fmt!("Not a settings scheme: {}", "JSON object expected"));
    }
    let s: SettingsScheme = serde_json::from_value(value).map_err(|e| tr_fmt!("Not a settings scheme: {}", e))?;
    let mut taken = Vec::new();
    if let Some(v) = s.wire_numbering {
        prj.wire_numbering = v;
        taken.push(tr_const!("Wire numbering"));
    }
    if let Some(v) = s.potentials {
        prj.potentials = v;
        taken.push(tr_const!("Potentials"));
    }
    if let Some(v) = s.wire_defaults {
        prj.wire_defaults = v;
        taken.push(tr_const!("Wire defaults"));
    }
    if let Some(v) = s.wire_colors {
        prj.wire_colors = v;
        taken.push(tr_const!("Wire colours"));
    }
    if let Some(v) = s.wire_settings {
        prj.wire_settings = v;
        taken.push(tr_const!("Wire crossings and connections"));
    }
    if let Some(v) = s.marking {
        prj.marking = v;
        taken.push(tr_const!("Marking"));
    }
    if let Some(v) = s.fluid {
        prj.fluid = v;
        taken.push(tr_const!("Hose defaults"));
    }
    if let Some(v) = s.cross_references {
        prj.xref = v;
        taken.push(tr_const!("Cross references"));
    }
    if let Some(v) = s.checks_off {
        prj.check_settings.disabled = v;
        taken.push(tr_const!("Checks"));
    }
    if let Some(v) = s.message_levels {
        prj.check_settings.levels = v;
        taken.push(tr_const!("Message levels"));
    }
    Ok(taken)
}

#[cfg(test)]
mod tests;
