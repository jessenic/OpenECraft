//! Property database: definitions of the properties of projects, pages, devices,
//! functions, terminals, PLC connection points, cables and parts – built-in and user
//! defined – and reading / changing their values.
//!
//! A property is addressed by its key (`device.rated_voltage`); indexed properties
//! by key and index (`device.supplementary[3]`). Values are stored as text with the
//! object (project, page, device, symbol, cable). Reading a property that has no
//! value falls back to the default of the function definition, then to the
//! property's fallback (usually part data), then to the definition's default.

use openecraft_i18n::{tr, tr_fmt};
use crate::functions::{self, FunctionDef};
use crate::model::*;
use crate::symbols::Func;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
pub use crate::model::properties::{DataType, Group, Library, ObjectClass, PropDef, Values};

mod builtin;

impl ObjectClass {
    pub const ALL: [ObjectClass; 8] = [
        ObjectClass::Project,
        ObjectClass::Page,
        ObjectClass::Device,
        ObjectClass::Function,
        ObjectClass::Terminal,
        ObjectClass::PlcPoint,
        ObjectClass::Cable,
        ObjectClass::Part,
    ];

    pub fn title(self) -> &'static str {
        match self {
            ObjectClass::Project => tr("Project"),
            ObjectClass::Page => tr("Page"),
            ObjectClass::Device => tr("Device"),
            ObjectClass::Function => tr("Function"),
            ObjectClass::Terminal => tr("Terminal"),
            ObjectClass::PlcPoint => tr("PLC connection point"),
            ObjectClass::Cable => tr("Cable"),
            ObjectClass::Part => tr("Part"),
        }
    }

    /// Name on the command line.
    pub fn key(self) -> &'static str {
        match self {
            ObjectClass::Project => "project",
            ObjectClass::Page => "page",
            ObjectClass::Device => "device",
            ObjectClass::Function => "function",
            ObjectClass::Terminal => "terminal",
            ObjectClass::PlcPoint => "plc",
            ObjectClass::Cable => "cable",
            ObjectClass::Part => "part",
        }
    }

    pub fn from_key(key: &str) -> Option<ObjectClass> {
        ObjectClass::ALL.into_iter().find(|c| c.key() == key.trim())
    }
}

impl Group {
    pub const ALL: [Group; 24] = [
        Group::General,
        Group::Customer,
        Group::Structure,
        Group::Commercial,
        Group::Electrical,
        Group::Control,
        Group::Contacts,
        Group::Protection,
        Group::Motor,
        Group::Drive,
        Group::Sensor,
        Group::Signal,
        Group::PowerSupply,
        Group::Plc,
        Group::Mechanical,
        Group::Environment,
        Group::Safety,
        Group::Communication,
        Group::Fluid,
        Group::Process,
        Group::Maintenance,
        Group::Documentation,
        Group::Supplementary,
        Group::User,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::General => tr("General"),
            Group::Customer => tr("Customer and plant"),
            Group::Structure => tr("Structure"),
            Group::Commercial => tr("Commercial data"),
            Group::Electrical => tr("Electrical data"),
            Group::Control => tr("Coil and timing"),
            Group::Contacts => tr("Contacts"),
            Group::Protection => tr("Protection"),
            Group::Motor => tr("Motor"),
            Group::Drive => tr("Drive"),
            Group::Sensor => tr("Sensor and measurement"),
            Group::Signal => tr("Signal and operating devices"),
            Group::PowerSupply => tr("Power supply and transformers"),
            Group::Plc => tr("PLC"),
            Group::Mechanical => tr("Mechanical data"),
            Group::Environment => tr("Environment"),
            Group::Safety => tr("Functional safety"),
            Group::Communication => tr("Communication"),
            Group::Fluid => tr("Fluid power"),
            Group::Process => tr("Process engineering"),
            Group::Maintenance => tr("Maintenance"),
            Group::Documentation => tr("Documentation"),
            Group::Supplementary => tr("Supplementary fields"),
            Group::User => tr("User defined"),
        }
    }

    /// Groups shown for every object, whatever its function definition.
    pub fn is_common(self) -> bool {
        matches!(
            self,
            Group::General | Group::Customer | Group::Structure | Group::Commercial | Group::Mechanical | Group::Environment | Group::Maintenance | Group::Documentation | Group::Supplementary | Group::User
        )
    }
}

impl DataType {
    pub const ALL: [DataType; 8] =
        [DataType::Text, DataType::MultiLine, DataType::Integer, DataType::Real, DataType::Bool, DataType::Choice, DataType::Date, DataType::Link];

    pub fn title(self) -> &'static str {
        match self {
            DataType::Text => tr("Text"),
            DataType::MultiLine => tr("Multi-line text"),
            DataType::Integer => tr("Integer"),
            DataType::Real => tr("Number"),
            DataType::Bool => tr("Yes / no"),
            DataType::Choice => tr("Selection list"),
            DataType::Date => tr("Date"),
            DataType::Link => tr("Link"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            DataType::Text => "text",
            DataType::MultiLine => "multiline",
            DataType::Integer => "integer",
            DataType::Real => "number",
            DataType::Bool => "bool",
            DataType::Choice => "choice",
            DataType::Date => "date",
            DataType::Link => "link",
        }
    }

    pub fn from_key(key: &str) -> Option<DataType> {
        DataType::ALL.into_iter().find(|t| t.key() == key.trim())
    }
}

impl PropDef {
    /// Shown name (built-in properties translated).
    pub fn title(&self) -> String {
        match self.builtin.then(|| builtin_name(&self.key)).flatten() {
            Some(name) => tr(name).to_string(),
            None => self.name.clone(),
        }
    }

    /// Value keys of the property: the key itself or `key[1]` … `key[n]`.
    pub fn value_keys(&self) -> Vec<String> {
        if self.indices == 0 {
            vec![self.key.clone()]
        } else {
            (1..=self.indices).map(|i| format!("{}[{i}]", self.key)).collect()
        }
    }

    /// Name of a value key, e.g. "Supplementary field [3]".
    pub fn title_of(&self, value_key: &str) -> String {
        match split_key(value_key).1 {
            Some(i) => format!("{} [{i}]", self.title()),
            None => self.title(),
        }
    }

    /// The value can be changed (stored or written to its model field).
    pub fn editable(&self) -> bool {
        !self.read_only
    }

    /// Checks and normalises a value for this property ("" = no value).
    pub fn check(&self, value: &str) -> Result<String, String> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(String::new());
        }
        let bad = || tr_fmt!("{}: invalid value \"{}\" ({})", self.title(), v, self.ty.title());
        match self.ty {
            DataType::Text | DataType::Link => Ok(v.to_string()),
            DataType::MultiLine => Ok(value.trim_end().to_string()),
            DataType::Integer => v.parse::<i64>().map(|n| n.to_string()).map_err(|_| bad()),
            DataType::Real => v.replace(',', ".").parse::<f64>().map(|_| v.replace(',', ".")).map_err(|_| bad()),
            DataType::Bool => match v.to_lowercase().as_str() {
                "true" | "yes" | "1" | "x" => Ok("true".into()),
                "false" | "no" | "0" => Ok("false".into()),
                _ if v == tr("Yes") => Ok("true".into()),
                _ if v == tr("No") => Ok("false".into()),
                _ => Err(bad()),
            },
            DataType::Choice if self.choices.is_empty() => Ok(v.to_string()),
            DataType::Choice => self.choices.iter().find(|c| c.eq_ignore_ascii_case(v)).cloned().ok_or_else(|| tr_fmt!("{}: \"{}\" is not one of {}", self.title(), v, self.choices.join(", "))),
            DataType::Date => {
                let parts: Vec<&str> = v.split('-').collect();
                let ok = parts.len() == 3 && parts[0].len() == 4 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
                ok.then(|| v.to_string()).ok_or_else(bad)
            }
        }
    }

    /// Value as shown: with unit, yes / no translated.
    pub fn format(&self, value: &str) -> String {
        if value.is_empty() {
            return String::new();
        }
        match self.ty {
            DataType::Bool => match value {
                "true" => tr("Yes").into(),
                "false" => tr("No").into(),
                v => v.into(),
            },
            DataType::Real | DataType::Integer if !self.unit.is_empty() && value.parse::<f64>().is_ok() => format!("{value} {}", self.unit),
            _ => value.into(),
        }
    }
}

fn builtin_name(key: &str) -> Option<&'static str> {
    static NAMES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    NAMES.get_or_init(|| builtin::ROWS.iter().map(|r| (r.key, r.name)).collect()).get(key).copied()
}

/// The built-in property definitions.
pub fn builtin() -> &'static [PropDef] {
    static DEFS: OnceLock<Vec<PropDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        builtin::ROWS
            .iter()
            .map(|r| PropDef {
                key: r.key.into(),
                name: r.name.into(),
                class: r.class,
                group: r.group,
                ty: r.ty,
                unit: r.unit.into(),
                choices: r.choices.iter().map(|c| c.to_string()).collect(),
                indices: r.indices,
                default: String::new(),
                description: String::new(),
                builtin: true,
                read_only: r.read_only,
                field: r.field,
                fallback: r.fallback.into(),
            })
            .collect()
    })
}

impl Library {
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.functions.is_empty() && self.schemes.is_empty()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Library, String> {
        serde_json::from_str(text).map_err(|e| tr_fmt!("Not a valid property library: {}", e))
    }

    /// Adds or replaces the entries of `other` (by key / name). Returns the number of
    /// entries taken over; built-in keys are skipped.
    pub fn merge(&mut self, other: Library) -> usize {
        let mut n = 0;
        for d in other.properties {
            if builtin_def(&d.key).is_some() || check_user_key(&d.key).is_err() {
                continue;
            }
            match self.properties.iter_mut().find(|x| x.key == d.key) {
                Some(x) => *x = d,
                None => self.properties.push(d),
            }
            n += 1;
        }
        for f in other.functions {
            if functions::builtin_def(&f.key).is_some() {
                continue;
            }
            match self.functions.iter_mut().find(|x| x.key == f.key) {
                Some(x) => *x = f,
                None => self.functions.push(f),
            }
            n += 1;
        }
        for s in other.schemes {
            if crate::props::builtin_scheme(&s.name).is_some() {
                continue;
            }
            match self.schemes.iter_mut().find(|x| x.name == s.name) {
                Some(x) => *x = s,
                None => self.schemes.push(s),
            }
            n += 1;
        }
        n
    }
}

fn builtin_def(key: &str) -> Option<&'static PropDef> {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| builtin().iter().enumerate().map(|(i, d)| (d.key.as_str(), i)).collect());
    index.get(key).map(|&i| &builtin()[i])
}

/// A user key must not look like a built-in one and must be usable in placeholders.
pub fn check_user_key(key: &str) -> Result<(), String> {
    let ok = !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')) && !key.contains("..");
    if !ok {
        return Err(tr_fmt!("Invalid property key: {}", key));
    }
    if builtin_def(key).is_some() {
        return Err(tr_fmt!("{} is a built-in property", key));
    }
    Ok(())
}

/// Splits `key[3]` into `("key", Some(3))`.
pub fn split_key(key: &str) -> (&str, Option<u16>) {
    if let Some(open) = key.strip_suffix(']').and_then(|k| k.rfind('[')) {
        if let Ok(i) = key[open + 1..key.len() - 1].parse::<u16>() {
            return (&key[..open], Some(i));
        }
    }
    (key, None)
}

/// Definition of a property (built-in first, then the project's own), for a key
/// with or without index.
pub fn def<'a>(prj: &'a Project, key: &str) -> Option<&'a PropDef> {
    let base = split_key(key).0;
    builtin_def(base).or_else(|| prj.property_db.properties.iter().find(|d| d.key == base))
}

/// A key with an index that the definition does not have is not a property.
pub fn valid_key<'a>(prj: &'a Project, key: &str) -> Option<&'a PropDef> {
    let d = def(prj, key)?;
    match split_key(key).1 {
        None if d.indices == 0 => Some(d),
        Some(i) if i >= 1 && i <= d.indices => Some(d),
        _ => None,
    }
}

/// All definitions: built-in, then the project's own.
pub fn all_defs(prj: &Project) -> impl Iterator<Item = &PropDef> {
    builtin().iter().chain(prj.property_db.properties.iter())
}

/// Adds or replaces a user defined property.
pub fn define(prj: &mut Project, d: PropDef) -> Result<(), String> {
    check_user_key(&d.key)?;
    if d.name.trim().is_empty() {
        return Err(tr("The property needs a name").into());
    }
    if matches!(d.class, ObjectClass::Part) {
        return Err(tr("Part data is maintained in the parts management").into());
    }
    let d = PropDef { builtin: false, read_only: false, field: false, fallback: String::new(), ..d };
    match prj.property_db.properties.iter_mut().find(|x| x.key == d.key) {
        Some(x) => *x = d,
        None => prj.property_db.properties.push(d),
    }
    Ok(())
}

/// Removes a user defined property with all its values. Returns the number of
/// values removed.
pub fn undefine(prj: &mut Project, key: &str) -> Result<usize, String> {
    let Some(i) = prj.property_db.properties.iter().position(|d| d.key == key) else {
        return Err(tr_fmt!("No user defined property {}", key));
    };
    prj.property_db.properties.remove(i);
    let mut n = 0;
    let mut purge = |v: &mut Values| {
        let before = v.len();
        v.retain(|k, _| split_key(k).0 != key);
        n += before - v.len();
    };
    purge(&mut prj.info.properties);
    prj.pages.iter_mut().for_each(|pg| purge(&mut pg.values));
    prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()).for_each(|s| purge(&mut s.values));
    prj.devices.values_mut().for_each(|d| purge(&mut d.values));
    prj.cables.iter_mut().for_each(|c| purge(&mut c.values));
    for s in prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()) {
        s.props.retain(|pp| split_key(&pp.key).0 != key);
    }
    Ok(n)
}

// ----------------------------------------------------------------------- objects

/// An object that has properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Object {
    Project,
    /// Page id.
    Page(u64),
    /// Device tag.
    Device(String),
    /// Symbol id: function, terminal and PLC connection point properties.
    Function(u64),
    /// Cable tag.
    Cable(String),
    /// Part number.
    Part(String),
}

impl Object {
    pub fn title(&self, prj: &Project) -> String {
        match self {
            Object::Project => tr("Project").into(),
            Object::Page(id) => prj.pages.iter().find(|p| p.id == *id).map(|p| tr_fmt!("Page {}", p.name)).unwrap_or_default(),
            Object::Device(tag) => tr_fmt!("Device {}", tag),
            Object::Function(id) => match find_symbol(prj, *id) {
                Some((_, s)) => {
                    let name = prj.symbol(&s.sym).map(|d| tr(d.name)).unwrap_or("?");
                    format!("{} {name} {}", s.tag, if s.label.is_empty() { s.pins.join("-") } else { s.label.clone() }).trim().to_string()
                }
                None => String::new(),
            },
            Object::Cable(tag) => tr_fmt!("Cable {}", tag),
            Object::Part(number) => tr_fmt!("Part {}", number),
        }
    }

    /// The object exists in the project (parts: in the project parts).
    pub fn exists(&self, prj: &Project) -> bool {
        match self {
            Object::Project => true,
            Object::Page(id) => prj.pages.iter().any(|p| p.id == *id),
            Object::Device(tag) => !tag.is_empty() && (prj.devices.contains_key(tag) || prj.all_tags().iter().any(|t| t == tag)),
            Object::Function(id) => find_symbol(prj, *id).is_some(),
            Object::Cable(tag) => prj.cable(tag).is_some(),
            Object::Part(number) => prj.parts.iter().any(|p| p.number == *number),
        }
    }

    /// Classes of the properties this object has.
    pub fn classes(&self, prj: &Project) -> Vec<ObjectClass> {
        match self {
            Object::Project => vec![ObjectClass::Project],
            Object::Page(_) => vec![ObjectClass::Page],
            Object::Device(_) => vec![ObjectClass::Device, ObjectClass::Part],
            Object::Function(id) => {
                let func = find_symbol(prj, *id).and_then(|(_, s)| prj.symbol(&s.sym)).map(|d| d.func);
                match func {
                    Some(Func::Terminal) => vec![ObjectClass::Function, ObjectClass::Terminal, ObjectClass::Part],
                    Some(Func::Plc) => vec![ObjectClass::Function, ObjectClass::PlcPoint, ObjectClass::Part],
                    _ => vec![ObjectClass::Function, ObjectClass::Part],
                }
            }
            Object::Cable(_) => vec![ObjectClass::Cable],
            Object::Part(_) => vec![ObjectClass::Part],
        }
    }

    /// Parses `project`, `page:<name>`, `device:<tag>`, `function:<symbol id>`,
    /// `cable:<tag>`, `part:<number>` (command line).
    pub fn parse(prj: &Project, text: &str) -> Result<Object, String> {
        let (kind, arg) = text.split_once(':').map(|(k, a)| (k.trim(), a.trim())).unwrap_or((text.trim(), ""));
        let obj = match kind {
            "project" => Object::Project,
            "page" => Object::Page(prj.pages.iter().find(|p| p.name == arg).map(|p| p.id).ok_or_else(|| tr_fmt!("No page {}", arg))?),
            "device" => Object::Device(arg.into()),
            "function" | "symbol" => Object::Function(arg.parse().map_err(|_| tr_fmt!("Not a symbol id: {}", arg))?),
            "cable" => Object::Cable(arg.into()),
            "part" => Object::Part(arg.into()),
            _ => return Err(tr_fmt!("Unknown object: {}", text)),
        };
        if !obj.exists(prj) {
            return Err(tr_fmt!("No such object: {}", text));
        }
        Ok(obj)
    }
}

pub fn find_symbol(prj: &Project, id: u64) -> Option<(usize, &SymbolInst)> {
    prj.pages.iter().enumerate().find_map(|(pi, pg)| pg.symbols.iter().find(|s| s.id == id).map(|s| (pi, s)))
}

fn find_symbol_mut(prj: &mut Project, id: u64) -> Option<&mut SymbolInst> {
    prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()).find(|s| s.id == id)
}

/// The symbols of a device, main functions first.
fn device_symbols<'a>(prj: &'a Project, tag: &str) -> Vec<(usize, &'a SymbolInst)> {
    let mut v: Vec<(usize, &SymbolInst)> = prj.pages.iter().enumerate().flat_map(|(pi, pg)| pg.symbols.iter().filter(|s| s.tag == tag).map(move |s| (pi, s))).collect();
    v.sort_by_key(|(_, s)| !matches!(prj.symbol(&s.sym).map(|d| d.func), Some(Func::Main { .. })));
    v
}

/// The function definition that decides which properties an object shows and gives
/// their defaults: the symbol's own, a device's main function's.
pub fn function_def<'a>(prj: &'a Project, obj: &Object) -> Option<&'a FunctionDef> {
    match obj {
        Object::Function(id) => find_symbol(prj, *id).and_then(|(_, s)| functions::of_symbol(prj, s)),
        Object::Device(tag) => device_symbols(prj, tag).first().and_then(|(_, s)| functions::of_symbol(prj, s)),
        _ => None,
    }
}

/// The object a property of class `class` shown at a symbol belongs to.
pub fn object_at_symbol(prj: &Project, page_i: usize, s: &SymbolInst, class: ObjectClass) -> Option<Object> {
    Some(match class {
        ObjectClass::Project => Object::Project,
        ObjectClass::Page => Object::Page(prj.pages.get(page_i)?.id),
        ObjectClass::Device | ObjectClass::Part if !s.tag.is_empty() => Object::Device(s.tag.clone()),
        ObjectClass::Function | ObjectClass::Terminal | ObjectClass::PlcPoint => Object::Function(s.id),
        ObjectClass::Cable if prj.cable(&s.tag).is_some() => Object::Cable(s.tag.clone()),
        _ => return None,
    })
}

fn values<'a>(prj: &'a Project, obj: &Object) -> Option<&'a Values> {
    match obj {
        Object::Project => Some(&prj.info.properties),
        Object::Page(id) => prj.pages.iter().find(|p| p.id == *id).map(|p| &p.values),
        Object::Device(tag) => prj.devices.get(tag).map(|d| &d.values),
        Object::Function(id) => find_symbol(prj, *id).map(|(_, s)| &s.values),
        Object::Cable(tag) => prj.cable(tag).map(|c| &c.values),
        Object::Part(_) => None,
    }
}

fn values_mut<'a>(prj: &'a mut Project, obj: &Object) -> Option<&'a mut Values> {
    match obj {
        Object::Project => Some(&mut prj.info.properties),
        Object::Page(id) => prj.pages.iter_mut().find(|p| p.id == *id).map(|p| &mut p.values),
        Object::Device(tag) => Some(&mut prj.devices.entry(tag.clone()).or_default().values),
        Object::Function(id) => find_symbol_mut(prj, *id).map(|s| &mut s.values),
        Object::Cable(tag) => prj.cables.iter_mut().find(|c| c.tag == *tag).map(|c| &mut c.values),
        Object::Part(_) => None,
    }
}

/// Stored value of a property (no fallbacks).
pub fn stored<'a>(prj: &'a Project, obj: &Object, key: &str) -> Option<&'a str> {
    values(prj, obj)?.get(key).map(String::as_str).filter(|v| !v.is_empty())
}

/// Value of a property ("" = none), with the fallbacks described in the module docs.
pub fn get(prj: &Project, obj: &Object, key: &str) -> String {
    let Some(d) = def(prj, key) else { return String::new() };
    if let Object::Function(id)=obj {
        if let Some((field,index))=crate::model::plc_hw::bindings::property(key) {
            if let Some(value)=crate::model::plc_hw::bindings::value(prj,*id,index,field) {return value;}
        }
    }
    if d.class == ObjectClass::Part {
        return part_value(prj, obj, key);
    }
    if d.read_only || d.field {
        return computed(prj, obj, key).unwrap_or_default();
    }
    if let Some(v) = stored(prj, obj, key) {
        return v.to_string();
    }
    if let Some(v) = function_def(prj, obj).and_then(|f| f.defaults.get(key)).filter(|v| !v.is_empty()) {
        return v.clone();
    }
    if !d.fallback.is_empty() {
        let v = get(prj, obj, &d.fallback);
        if !v.is_empty() {
            return v;
        }
    }
    d.default.clone()
}

/// Value as shown (unit, yes / no).
pub fn display(prj: &Project, obj: &Object, key: &str) -> String {
    let v = get(prj, obj, key);
    def(prj, key).map(|d| d.format(&v)).unwrap_or(v)
}

/// Where a shown value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    None,
    /// Set for this object.
    Stored,
    /// From the model (computed or a field).
    Model,
    FunctionDefinition,
    /// From the fallback property (part data …) or the definition's default.
    Default,
}

pub fn origin(prj: &Project, obj: &Object, key: &str) -> Origin {
    let Some(d) = def(prj, key) else { return Origin::None };
    if d.read_only || d.field || d.class == ObjectClass::Part {
        return if get(prj, obj, key).is_empty() { Origin::None } else { Origin::Model };
    }
    if stored(prj, obj, key).is_some() {
        Origin::Stored
    } else if function_def(prj, obj).and_then(|f| f.defaults.get(key)).is_some_and(|v| !v.is_empty()) {
        Origin::FunctionDefinition
    } else if !get(prj, obj, key).is_empty() {
        Origin::Default
    } else {
        Origin::None
    }
}

/// Sets a property ("" removes the value).
pub fn set(prj: &mut Project, obj: &Object, key: &str, value: &str) -> Result<(), String> {
    let d = valid_key(prj, key).ok_or_else(|| tr_fmt!("Unknown property: {}", key))?.clone();
    if !obj.classes(prj).contains(&d.class) {
        return Err(tr_fmt!("{} is not a property of {}", d.title(), obj.title(prj)));
    }
    if d.read_only {
        return Err(tr_fmt!("{} cannot be changed", d.title()));
    }
    let v = d.check(value)?;
    if let Object::Function(id)=obj {
        if let Some((field,index))=crate::model::plc_hw::bindings::property(key) {
            crate::model::plc_hw::bindings::set(prj,*id,index,field,&v)?;
            return Ok(());
        }
    }
    if d.field {
        return set_field(prj, obj, key, &v);
    }
    let missing = tr_fmt!("No such object: {}", obj.title(prj));
    let values = values_mut(prj, obj).ok_or(missing)?;
    if v.is_empty() {
        values.remove(key);
    } else {
        values.insert(key.to_string(), v);
    }
    if let Object::Device(tag) = obj {
        if prj.devices.get(tag).is_some_and(|d| *d == DeviceData::default()) {
            prj.devices.remove(tag);
        }
    }
    Ok(())
}

/// Property keys of an object with their definitions: all value keys of the
/// definitions of its classes (`relevant`: only groups that fit its function
/// definition; `filled`: only keys with a value).
pub fn keys_of<'a>(prj: &'a Project, obj: &Object, relevant: bool, filled: bool) -> Vec<(String, &'a PropDef)> {
    let classes = obj.classes(prj);
    let groups = function_def(prj, obj).map(|f| f.category.groups());
    let channels=if let Object::Function(id)=obj {find_symbol(prj,*id).and_then(|(_,s)|prj.symbol(&s.sym)).map(|d|d.plc_channels.len()).unwrap_or(0)} else {0};
    let mut out = Vec::new();
    for d in all_defs(prj).filter(|d| classes.contains(&d.class)) {
        if relevant && !d.group.is_common() && !(channels>0 && d.group==Group::Plc) && groups.is_some_and(|g| !g.contains(&d.group)) {
            continue;
        }
        let keys=if d.key.starts_with("function.plc_") {(1..=channels.min(usize::from(d.indices))).map(|i|format!("{}[{i}]",d.key)).collect()} else {d.value_keys()};
        for k in keys {
            if !filled || !get(prj, obj, &k).is_empty() {
                out.push((k, d));
            }
        }
    }
    out
}

/// Stored values of the object whose keys are not defined (e.g. from a library
/// that was removed).
pub fn undefined_values(prj: &Project, obj: &Object) -> Vec<(String, String)> {
    values(prj, obj).map(|v| v.iter().filter(|(k, _)| valid_key(prj, k).is_none()).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

/// Copies all stored values of one object to others (of the same kind).
pub fn copy_values(prj: &mut Project, from: &Object, to: &[Object]) -> usize {
    let src = values(prj, from).cloned().unwrap_or_default();
    let mut n = 0;
    for obj in to.iter().filter(|o| *o != from) {
        if let Some(v) = values_mut(prj, obj) {
            v.extend(src.clone());
            n += 1;
        }
    }
    n
}

// --------------------------------------------------------------- model values

fn join(v: Vec<&str>) -> String {
    let mut v: Vec<&str> = v.into_iter().filter(|t| !t.is_empty()).collect();
    v.dedup();
    v.join(", ")
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Number of the assigned part selected by this function.
pub fn selected_part_number<'a>(prj: &'a Project, s: &SymbolInst) -> Option<&'a str> {
    s.selected_part_number(prj)
}

fn set_function_part_index(prj: &mut Project, id: u64, value: &str) -> Result<(), String> {
    let (_, s) = find_symbol(prj, id).ok_or_else(|| tr_fmt!("No such object: {}", id))?;
    let selected = value.parse::<usize>().ok().and_then(|i| s.assigned_part_number(prj, i)).map(str::to_owned);
    if !value.is_empty() && selected.is_none() {
        return Err(tr_fmt!("Assigned part position {} does not exist on this device", value));
    }
    let s = find_symbol_mut(prj, id).ok_or_else(|| tr_fmt!("No such object: {}", id))?;
    if let Some(number) = selected { s.values.insert("function.part_number".into(), number); }
    else { s.values.remove("function.part_number"); }
    Ok(())
}

/// Part data: of a part, the selected part of a function, or all parts of a device.
fn part_value(prj: &Project, obj: &Object, key: &str) -> String {
    if let Object::Function(id) = obj {
        let number = find_symbol(prj, *id).and_then(|(_, s)| selected_part_number(prj, s));
        if key == "part.number" { return number.unwrap_or_default().to_string(); }
    }
    let parts: Vec<&Part> = match obj {
        Object::Part(number) => crate::parts::lookup(prj, None, number).into_iter().collect(),
        Object::Device(tag) => prj.devices.get(tag).map(|d| d.parts.iter().filter_map(|r| crate::parts::lookup(prj, None, &r.number)).collect()).unwrap_or_default(),
        Object::Function(id) => find_symbol(prj, *id).and_then(|(_, s)| selected_part_number(prj, s)).and_then(|number| crate::parts::lookup(prj, None, number)).into_iter().collect(),
        _ => Vec::new(),
    };
    let field = |p: &Part| -> String {
        let n = |v: f32| if v == 0.0 { String::new() } else { num(v as f64) };
        match key {
            "part.number" => p.number.clone(),
            "part.type_number" => p.type_number.clone(),
            "part.order_number" => p.order_number.clone(),
            "part.manufacturer" => p.manufacturer.clone(),
            "part.supplier" => p.supplier.clone(),
            "part.description" => p.description.clone(),
            "part.note" => p.note.clone(),
            "part.group" => p.group.clone(),
            "part.subgroup" => p.subgroup.clone(),
            "part.identifier" => p.identifier.clone(),
            "part.price" => n(p.price),
            "part.currency" => p.currency.clone(),
            "part.price_unit" => p.price_unit.to_string(),
            "part.pack_quantity" => p.pack_quantity.to_string(),
            "part.quantity_unit" => p.quantity_unit.clone(),
            "part.voltage" => p.voltage.clone(),
            "part.current" => p.current.clone(),
            "part.power" => p.power.clone(),
            "part.tech_data" => p.tech_data.clone(),
            "part.width" => n(p.width),
            "part.height" => n(p.height),
            "part.depth" => n(p.depth),
            "part.weight" => n(p.weight),
            "part.mounting" => p.mounting.clone(),
            "part.functions" => p.template_summary(),
            "part.accessories" => p.accessories.join(", "),
            "part.fits" => p.fits.join(", "),
            "part.certificates" => p.certificates.clone(),
            "part.datasheet" => p.datasheet.clone(),
            _ => String::new(),
        }
    };
    let values: Vec<String> = parts.iter().map(|p| field(p)).collect();
    join(values.iter().map(String::as_str).collect())
}

fn computed(prj: &Project, obj: &Object, key: &str) -> Option<String> {
    let yes_no = |b: bool| if b { "true".to_string() } else { "false".to_string() };
    match obj {
        Object::Project => {
            let i = &prj.info;
            Some(match key {
                "project.name" => i.name.clone(),
                "project.description" => i.description.clone(),
                "project.company" => i.company.clone(),
                "project.creator" => i.creator.clone(),
                "project.date" => i.date.clone(),
                "project.revision" => i.revision.clone(),
                "project.pages" => prj.pages.len().to_string(),
                "project.devices" => prj.all_tags().len().to_string(),
                "project.cables" => prj.cables.len().to_string(),
                _ => return None,
            })
        }
        Object::Page(id) => {
            let pg = prj.pages.iter().find(|p| p.id == *id)?;
            Some(match key {
                "page.name" => pg.name.clone(),
                "page.description" => pg.description.clone(),
                "page.full_name" => format!("{}/{}", crate::structure::page_structure(pg), pg.name),
                "page.type" => pg.kind.label(),
                "page.symbols" => pg.symbols.len().to_string(),
                "page.released" => yes_no(pg.locked),
                "page.function" => pg.function.clone(),
                "page.installation" => pg.installation.clone(),
                "page.location" => pg.location.clone(),
                "page.product" => pg.product.clone(),
                _ => return None,
            })
        }
        Object::Device(tag) => {
            let dev = prj.devices.get(tag);
            let parts: Vec<(&Part, u32)> = dev.map(|d| d.parts.iter().filter_map(|r| crate::parts::lookup(prj, None, &r.number).map(|p| (p, r.count))).collect()).unwrap_or_default();
            let syms = device_symbols(prj, tag);
            let dt = crate::structure::Dt::parse(tag);
            Some(match key {
                "device.tag" => tag.clone(),
                "device.description" => dev.map(|d| d.description.clone()).unwrap_or_default(),
                "device.function_text" => syms.first().map(|(_, s)| s.function_text.clone()).unwrap_or_default(),
                "device.letter" => dt.name.trim_start_matches('-').chars().take_while(|c| c.is_alphabetic()).collect(),
                "device.structure" => dt.structure(),
                "device.function" => dt.function.clone(),
                "device.installation" => dt.installation.clone(),
                "device.location" => dt.location.clone(),
                "device.main_position" => syms.first().map(|(pi, s)| position(prj, *pi, s)).unwrap_or_default(),
                "device.functions" => syms.len().to_string(),
                "device.definition" => function_def(prj, obj).map(|f| f.title()).unwrap_or_default(),
                "device.parts" => join(dev.map(|d| d.parts.iter().map(|r| r.number.as_str()).collect()).unwrap_or_default()),
                "device.type_numbers" => join(parts.iter().map(|(p, _)| p.type_number.as_str()).collect()),
                "device.order_numbers" => join(parts.iter().map(|(p, _)| p.order_number.as_str()).collect()),
                "device.manufacturers" => join(parts.iter().map(|(p, _)| p.manufacturer.as_str()).collect()),
                "device.suppliers" => join(parts.iter().map(|(p, _)| p.supplier.as_str()).collect()),
                "device.part_count" => dev.map(|d| d.parts.iter().map(|r| r.count).sum::<u32>().to_string()).unwrap_or_else(|| "0".into()),
                "device.price" => num(parts.iter().map(|(p, n)| p.total_price(*n as f32) as f64).sum()),
                "device.total_weight" => num(parts.iter().map(|(p, n)| (p.weight * *n as f32) as f64).sum()),
                _ => return None,
            })
        }
        Object::Function(id) => {
            let (pi, s) = find_symbol(prj, *id)?;
            let sd = prj.symbol(&s.sym);
            Some(match key {
                "function.label" | "terminal.number" | "plc.address" => s.label.clone(),
                "function.function_text" => s.function_text.clone(),
                "function.part_index" => match selected_part_number(prj, s) {
                    Some(number) => prj.devices.get(&s.tag).and_then(|d| d.parts.iter().position(|p| p.number == number)).map(|i| (i+1).to_string()).unwrap_or_else(|| "1".into()),
                    None if s.values.contains_key("function.part_number") => "0".into(),
                    None => "1".into(),
                },
                "function.part_number" => selected_part_number(prj, s).unwrap_or_default().to_string(),
                "function.pins" => s.pins.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" "),
                "function.symbol" => sd.map(|d| tr(d.name).to_string()).unwrap_or_default(),
                "function.symbol_key" => s.sym.clone(),
                "function.position" => position(prj, pi, s),
                "function.page" => prj.pages[pi].name.clone(),
                "function.definition" => functions::of_symbol(prj, s).map(|f| f.title()).unwrap_or_default(),
                "function.category" => functions::of_symbol(prj, s).map(|f| f.category.title().to_string()).unwrap_or_default(),
                "function.main" => yes_no(matches!(sd.map(|d| d.func), Some(Func::Main { .. }))),
                "terminal.strip" | "plc.card" => s.tag.clone(),
                _ => return None,
            })
        }
        Object::Cable(tag) => {
            let c = prj.cable(tag)?;
            Some(match key {
                "cable.tag" => c.tag.clone(),
                "cable.type" => c.cable_type.clone(),
                "cable.description" => c.description.clone(),
                "cable.cores" => c.cores.clone(),
                "cable.core_count" => c.core_list().len().to_string(),
                "cable.cross_section" => c.cross_section.clone(),
                "cable.length" => if c.length == 0.0 { String::new() } else { num(c.length as f64) },
                "cable.part" => c.part.clone(),
                "cable.diameter" => if c.diameter == 0.0 { String::new() } else { num(c.diameter as f64) },
                _ => return None,
            })
        }
        Object::Part(_) => None,
    }
}

fn position(prj: &Project, page_i: usize, s: &SymbolInst) -> String {
    format!("{}{}", crate::structure::page_structure(&prj.pages[page_i]), crate::frame::reference(prj, page_i, s.pos))
}

fn set_field(prj: &mut Project, obj: &Object, key: &str, v: &str) -> Result<(), String> {
    let missing = || tr_fmt!("No such object: {}", key);
    let v = v.to_string();
    match obj {
        Object::Project => {
            let i = &mut prj.info;
            match key {
                "project.name" => i.name = v,
                "project.description" => i.description = v,
                "project.company" => i.company = v,
                "project.creator" => i.creator = v,
                "project.date" => i.date = v,
                "project.revision" => i.revision = v,
                _ => return Err(missing()),
            }
        }
        Object::Page(id) => {
            let pg = prj.pages.iter_mut().find(|p| p.id == *id).ok_or_else(missing)?;
            if pg.locked {
                return Err(tr("The page is released").into());
            }
            match key {
                "page.name" if v.is_empty() => return Err(tr("The page needs a number").into()),
                "page.name" => pg.name = v,
                "page.description" => pg.description = v,
                "page.function" => pg.function = v,
                "page.installation" => pg.installation = v,
                "page.location" => pg.location = v,
                "page.product" => pg.product = v,
                _ => return Err(missing()),
            }
        }
        Object::Device(tag) => match key {
            "device.description" => {
                prj.devices.entry(tag.clone()).or_default().description = v;
            }
            "device.function_text" => {
                let id = device_symbols(prj, tag).first().map(|(_, s)| s.id).ok_or_else(missing)?;
                find_symbol_mut(prj, id).ok_or_else(missing)?.function_text = v;
            }
            _ => return Err(missing()),
        },
        Object::Function(id) => {
            if key == "function.part_index" { return set_function_part_index(prj, *id, &v); }
            let s = find_symbol_mut(prj, *id).ok_or_else(missing)?;
            match key {
                "function.label" | "terminal.number" | "plc.address" => s.label = v,
                "function.function_text" => s.function_text = v,
                _ => return Err(missing()),
            }
        }
        Object::Cable(tag) => {
            let c = prj.cables.iter_mut().find(|c| c.tag == *tag).ok_or_else(missing)?;
            let f = || v.parse::<f32>().unwrap_or(0.0);
            match key {
                "cable.type" => c.cable_type = v.clone(),
                "cable.description" => c.description = v.clone(),
                "cable.cores" => c.cores = v.clone(),
                "cable.cross_section" => c.cross_section = v.clone(),
                "cable.length" => c.length = f(),
                "cable.part" => c.part = v.clone(),
                "cable.diameter" => c.diameter = f(),
                _ => return Err(missing()),
            }
        }
        Object::Part(_) => return Err(missing()),
    }
    Ok(())
}

/// Number of property values stored in the project, per object class.
pub fn statistics(prj: &Project) -> BTreeMap<ObjectClass, usize> {
    let mut out = BTreeMap::new();
    let mut count = |v: &Values| {
        for k in v.keys() {
            if let Some(d) = valid_key(prj, k) {
                *out.entry(d.class).or_insert(0) += 1;
            }
        }
    };
    count(&prj.info.properties);
    prj.pages.iter().for_each(|p| count(&p.values));
    prj.pages.iter().flat_map(|p| p.symbols.iter()).for_each(|s| count(&s.values));
    prj.devices.values().for_each(|d| count(&d.values));
    prj.cables.iter().for_each(|c| count(&c.values));
    out
}

#[cfg(test)]
mod tests;
