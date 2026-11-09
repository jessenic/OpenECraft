//! Typed parameters of commands: checked against JSON arguments, described as JSON
//! schema (MCP) and parsed from the command line. Shared by the built-in tools of the
//! command line and the commands of plugins.

use openecraft_i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::borrow::Cow;

/// A text of the API: `Borrowed` texts are English catalog entries (`tr_const!`) and
/// shown translated, `Owned` texts (from external plugins) are shown as they are.
pub type Text = Cow<'static, str>;

/// The text in the current language.
pub fn text(t: &Text) -> &str {
    match t {
        Cow::Borrowed(s) => tr(s),
        Cow::Owned(s) => s,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Str,
    Int,
    Num,
    Bool,
    /// List of item ids.
    Ids,
    /// List of `[x, y]` points in mm.
    Points,
    /// JSON object.
    Object,
}

impl Kind {
    /// Placeholder of a value in help texts.
    pub fn hint(self) -> &'static str {
        match self {
            Kind::Str => "<text>",
            Kind::Int | Kind::Num => "<number>",
            Kind::Bool => "[true|false]",
            Kind::Ids => "<id,id,…>",
            Kind::Points => "<x,y x,y …>",
            Kind::Object => "<json>",
        }
    }

    pub fn fits(self, v: &Value) -> bool {
        let point = |p: &Value| p.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(Value::is_number));
        match self {
            Kind::Str => v.is_string(),
            // Clients written in JavaScript send 90 as 90.0.
            Kind::Int => integer(v).is_some(),
            Kind::Num => v.is_number(),
            Kind::Bool => v.is_boolean(),
            Kind::Ids => v.as_array().is_some_and(|a| a.iter().all(|id| integer(id).is_some_and(|i| i >= 0))),
            Kind::Points => v.as_array().is_some_and(|a| a.iter().all(point)),
            Kind::Object => v.is_object(),
        }
    }

    /// Value typed on the command line or into a form; `None` if it does not fit.
    pub fn parse(self, raw: &str) -> Option<Value> {
        match self {
            Kind::Str => Some(Value::String(raw.into())),
            Kind::Int => raw.trim().parse::<i64>().ok().map(Value::from),
            Kind::Num => raw.trim().parse::<f64>().ok().map(Value::from),
            Kind::Bool => match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => Some(Value::Bool(true)),
                "false" | "no" | "0" => Some(Value::Bool(false)),
                _ => None,
            },
            // `1,2,3` as well as `[1,2,3]`.
            Kind::Ids if !raw.trim_start().starts_with('[') => {
                raw.split(',').map(|s| s.trim().parse::<u64>().ok().map(Value::from)).collect::<Option<Vec<_>>>().map(Value::Array)
            }
            // `x,y x,y …` as well as `[[x,y],…]`.
            Kind::Points if !raw.trim_start().starts_with('[') => raw
                .split_whitespace()
                .map(|pt| {
                    let (x, y) = pt.split_once(',')?;
                    Some(json!([x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?]))
                })
                .collect::<Option<Vec<_>>>()
                .map(Value::Array),
            _ => serde_json::from_str(raw).ok().filter(|v| self.fits(v)),
        }
    }
}

/// An integer, also written as a number without fraction (`90.0`).
fn integer(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_u64().and_then(|u| i64::try_from(u).ok())).or_else(|| {
        v.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9.0e15).map(|f| f as i64)
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: Text,
    pub kind: Kind,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub help: Text,
}

fn yes() -> bool {
    true
}

pub const fn param(name: &'static str, kind: Kind, help: &'static str) -> Param {
    Param { name: Cow::Borrowed(name), kind, required: true, help: Cow::Borrowed(help) }
}

pub const fn optional(name: &'static str, kind: Kind, help: &'static str) -> Param {
    Param { name: Cow::Borrowed(name), kind, required: false, help: Cow::Borrowed(help) }
}

/// Checks JSON arguments against the parameters: no unknown ones, all required ones,
/// each of the declared type.
pub fn check(params: &[Param], values: &Map<String, Value>) -> Result<(), String> {
    for name in values.keys() {
        if !params.iter().any(|p| p.name == name.as_str()) {
            return Err(tr_fmt!("Unknown parameter: {}", name));
        }
    }
    for p in params {
        match values.get(p.name.as_ref()) {
            None | Some(Value::Null) if p.required => return Err(tr_fmt!("Missing parameter: {}", p.name)),
            None | Some(Value::Null) => {}
            Some(v) if !p.kind.fits(v) => return Err(tr_fmt!("Parameter {} has the wrong type", p.name)),
            Some(_) => {}
        }
    }
    Ok(())
}

/// JSON schema of the parameters (MCP `inputSchema`).
pub fn schema(params: &[Param]) -> Value {
    let mut props = Map::new();
    for p in params {
        let mut s = match p.kind {
            Kind::Str => json!({ "type": "string" }),
            Kind::Int => json!({ "type": "integer" }),
            Kind::Num => json!({ "type": "number" }),
            Kind::Bool => json!({ "type": "boolean" }),
            Kind::Ids => json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } }),
            Kind::Points => json!({
                "type": "array",
                "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }
            }),
            Kind::Object => json!({ "type": "object" }),
        };
        s["description"] = p.help.as_ref().into();
        props.insert(p.name.to_string(), s);
    }
    let required: Vec<&str> = params.iter().filter(|p| p.required).map(|p| p.name.as_ref()).collect();
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

/// Arguments from the command line: `--json` first, then `--name value` pairs
/// converted to the parameter's type.
pub fn from_cli(params: &[Param], json: Option<&str>, pairs: &[(String, Option<String>)]) -> Result<Map<String, Value>, String> {
    let mut out = match json {
        Some(text) => match serde_json::from_str(text) {
            Ok(Value::Object(map)) => map,
            _ => return Err(tr("--json needs a JSON object").to_string()),
        },
        None => Map::new(),
    };
    for (name, raw) in pairs {
        let p = params.iter().find(|p| p.name == name.as_str()).ok_or_else(|| tr_fmt!("Unknown parameter: {}", name))?;
        let value = match (p.kind, raw) {
            (Kind::Bool, None) => Value::Bool(true),
            (_, None) => return Err(tr_fmt!("Missing value for --{}", name)),
            (kind, Some(raw)) => kind.parse(raw).ok_or_else(|| tr_fmt!("Parameter {} has the wrong type", name))?,
        };
        out.insert(name.clone(), value);
    }
    Ok(out)
}

/// Typed access to checked arguments.
pub struct Args<'a>(pub &'a Map<String, Value>);

impl Args<'_> {
    pub fn str(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(Value::as_str)
    }

    /// A parameter the command declared as required.
    pub fn req(&self, name: &str) -> &str {
        self.str(name).unwrap_or_default()
    }

    pub fn num(&self, name: &str) -> Option<f32> {
        self.0.get(name).and_then(Value::as_f64).map(|v| v as f32)
    }

    /// A number with the precision of `f64` (geographic coordinates).
    pub fn real(&self, name: &str) -> Option<f64> {
        self.0.get(name).and_then(Value::as_f64)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.0.get(name).and_then(integer)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.0.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn ids(&self, name: &str) -> Vec<u64> {
        self.0.get(name).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| integer(v).and_then(|i| u64::try_from(i).ok())).collect()).unwrap_or_default()
    }

    pub fn points(&self, name: &str) -> Vec<[f32; 2]> {
        let pt = |v: &Value| Some([v.get(0)?.as_f64()? as f32, v.get(1)?.as_f64()? as f32]);
        self.0.get(name).and_then(Value::as_array).map(|a| a.iter().filter_map(pt).collect()).unwrap_or_default()
    }

    pub fn value(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }
}

#[cfg(test)]
mod tests;
