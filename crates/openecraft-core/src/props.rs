//! Property placement: properties shown as texts beside a symbol, the position of
//! the tag block, and placement schemes – named arrangements of properties that are
//! applied to symbols and updated on all symbols using them.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::geom::{p, P};
use crate::model::*;
use crate::properties;
use crate::symlib::Symbols;
use std::sync::OnceLock;
pub use crate::model::{default_anchor, is_left, migrate};
pub use crate::model::props::Scheme;

/// Text of a placed property ("" if it has no value).
pub fn text(prj: &Project, page_i: usize, s: &SymbolInst, pp: &PlacedProp) -> String {
    let Some(d) = properties::valid_key(prj, &pp.key) else { return String::new() };
    let Some(obj) = properties::object_at_symbol(prj, page_i, s, d.class) else { return String::new() };
    let v = properties::display(prj, &obj, &pp.key);
    if v.is_empty() || !pp.name {
        v
    } else {
        format!("{}: {v}", d.title_of(&pp.key))
    }
}

/// A symbol block resolves part data on its function's selected assignment.
pub fn function_property_text(prj: &Project, id: u64, key: &str) -> String {
    properties::display(prj, &properties::Object::Function(id), key)
}

/// Page and project fields use the drawing context; part fields retain function selection.
pub fn symbol_property_text(prj:&Project,page_i:usize,symbol:&SymbolInst,key:&str)->String {
    if let Some(definition)=properties::valid_key(prj,key)
        .filter(|d|matches!(d.class,properties::ObjectClass::Project|properties::ObjectClass::Page)) {
        return properties::object_at_symbol(prj,page_i,symbol,definition.class)
            .map(|object|properties::display(prj,&object,key)).unwrap_or_default();
    }
    function_property_text(prj,symbol.id,key)
}

/// Reference point of placed properties and schemes: right of and below the
/// symbol graphics, relative to the symbol position.
pub fn reference(symbols: &Symbols, s: &SymbolInst) -> P {
    symbols.get(&s.sym).map(|d| d.world_bounds(s.pos, s.rot, s.mirror).max - s.pos + p(1.5, 2.0)).unwrap_or_default()
}

/// Adds a property text below the ones already placed.
pub fn add(symbols: &Symbols, s: &mut SymbolInst, key: &str) {
    let below = reference(symbols, s);
    let y = s.props.iter().map(|p| p.offset.y + 2.8).fold(below.y, f32::max);
    s.props.push(PlacedProp { key: key.into(), offset: p(below.x, y), ..Default::default() });
}

// ------------------------------------------------------------------- schemes

impl Scheme {
    pub fn title(&self) -> String {
        if self.builtin { tr_static(&self.name) } else { self.name.clone() }
    }
}

const BUILTIN: &[(&str, &[&str])] = &[
    (tr_const!("Description"), &["device.description"]),
    (tr_const!("Description and part"), &["device.description", "device.parts"]),
    (tr_const!("Part data"), &["device.parts", "device.type_numbers", "device.manufacturers"]),
    (tr_const!("Technical data"), &["device.rated_voltage", "device.rated_current", "device.rated_power"]),
    (tr_const!("Motor data"), &["device.rated_power", "device.rated_current", "device.speed"]),
    (tr_const!("Protection setting"), &["device.characteristic", "device.rated_current", "device.current_setting"]),
    (tr_const!("PLC data"), &["plc.symbolic", "plc.comment"]),
    (tr_const!("Terminal data"), &["terminal.description", "terminal.cross_section"]),
];

fn tr_static(name: &str) -> String {
    BUILTIN.iter().find(|b| b.0 == name).map(|b| tr(b.0).to_string()).unwrap_or_else(|| name.to_string())
}

/// The built-in schemes.
pub fn builtin_schemes() -> &'static [Scheme] {
    static SCHEMES: OnceLock<Vec<Scheme>> = OnceLock::new();
    SCHEMES.get_or_init(|| {
        BUILTIN
            .iter()
            .map(|(name, keys)| Scheme {
                name: name.to_string(),
                description: String::new(),
                entries: keys.iter().enumerate().map(|(i, k)| PlacedProp { key: k.to_string(), offset: p(0.0, i as f32 * 2.8), ..Default::default() }).collect(),
                builtin: true,
            })
            .collect()
    })
}

pub fn builtin_scheme(name: &str) -> Option<&'static Scheme> {
    builtin_schemes().iter().find(|s| s.name == name)
}

/// A scheme by name: built-in or the project's.
pub fn scheme<'a>(prj: &'a Project, name: &str) -> Option<&'a Scheme> {
    builtin_scheme(name).or_else(|| prj.property_db.schemes.iter().find(|s| s.name == name))
}

pub fn all_schemes(prj: &Project) -> impl Iterator<Item = &Scheme> {
    builtin_schemes().iter().chain(prj.property_db.schemes.iter())
}

/// A scheme from the properties placed at a symbol.
pub fn capture(symbols: &Symbols, s: &SymbolInst, name: &str) -> Scheme {
    let r = reference(symbols, s);
    let entries = s.props.iter().map(|pp| PlacedProp { offset: pp.offset - r, ..pp.clone() }).collect();
    Scheme { name: name.trim().into(), description: String::new(), entries, builtin: false }
}

/// Places the properties of a scheme at a symbol (replacing the ones placed so far).
pub fn apply_scheme(symbols: &Symbols, s: &mut SymbolInst, scheme: &Scheme) {
    let r = reference(symbols, s);
    s.props = scheme.entries.iter().map(|e| PlacedProp { offset: e.offset + r, ..e.clone() }).collect();
    s.scheme = scheme.name.clone();
}

/// Adds or replaces a project scheme.
pub fn define_scheme(prj: &mut Project, scheme: Scheme) -> Result<(), String> {
    if scheme.name.trim().is_empty() {
        return Err(tr("The scheme needs a name").into());
    }
    if builtin_scheme(&scheme.name).is_some() {
        return Err(tr_fmt!("{} is a built-in scheme", scheme.name));
    }
    if let Some(e) = scheme.entries.iter().find(|e| properties::valid_key(prj, &e.key).is_none()) {
        return Err(tr_fmt!("Unknown property: {}", e.key));
    }
    let scheme = Scheme { builtin: false, ..scheme };
    match prj.property_db.schemes.iter_mut().find(|s| s.name == scheme.name) {
        Some(s) => *s = scheme,
        None => prj.property_db.schemes.push(scheme),
    }
    Ok(())
}

/// Removes a project scheme; symbols using it keep their placed properties.
pub fn remove_scheme(prj: &mut Project, name: &str) {
    prj.property_db.schemes.retain(|s| s.name != name);
    for s in prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()).filter(|s| s.scheme == name) {
        s.scheme.clear();
    }
}

/// Places the scheme again at all symbols of editable pages that use it. Returns
/// the number of symbols updated.
pub fn reapply(prj: &mut Project, name: &str) -> usize {
    let symbols = prj.symbol_table();
    let Some(scheme) = scheme(prj, name).cloned() else { return 0 };
    let mut n = 0;
    for pg in prj.pages.iter_mut().filter(|pg| pg.is_editable()) {
        for s in pg.symbols.iter_mut().filter(|s| s.scheme == name) {
            apply_scheme(&symbols, s, &scheme);
            n += 1;
        }
    }
    n
}

/// Applies a scheme to symbols by id (on editable pages). Returns the number changed.
pub fn apply_to(prj: &mut Project, name: &str, ids: &[u64]) -> Result<usize, String> {
    let symbols = prj.symbol_table();
    let scheme = scheme(prj, name).cloned().ok_or_else(|| tr_fmt!("Unknown scheme: {}", name))?;
    let mut n = 0;
    for pg in prj.pages.iter_mut().filter(|pg| pg.is_editable()) {
        for s in pg.symbols.iter_mut().filter(|s| ids.contains(&s.id)) {
            apply_scheme(&symbols, s, &scheme);
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests;
