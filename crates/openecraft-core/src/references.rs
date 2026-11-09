//! Live function references shared by property fields, drawing and extension queries.

use crate::index::{Index, Loc};
use crate::model::{Project, SymbolInst};
use crate::symbols::{BlockRow, Func, Prim, SymbolDef};

pub const PROPERTY: &str = "function.cross_references";

#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    pub text: String,
    pub page: Option<usize>,
}

/// An authored field replaces the automatic reference annotation.
pub fn has_field(def: &SymbolDef, symbol: &SymbolInst) -> bool {
    symbol.props.iter().any(|p| p.key == PROPERTY) || def.prims.iter().any(|p| matches!(p,
        Prim::PropertyBlock(block) if block.rows.iter().any(|row| matches!(row, BlockRow::Property(key) if key == PROPERTY))))
}

/// References follow the current device index, section names and reference settings.
pub fn of_symbol(prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, symbol: &SymbolInst) -> Vec<Reference> {
    let mut refs = Vec::new();
    let mut add = |loc: &Loc| {
        if loc.sym_id != symbol.id { refs.push(Reference { text: loc.reference_from(prj, page_i), page: Some(loc.page) }); }
    };
    let device = idx.devices.get(&symbol.tag);
    match def.func {
        Func::Aux(_) | Func::Auxiliary => {
            if let Some(device) = device.filter(|d| !d.mains.is_empty()) {
                for loc in &device.mains { add(loc); }
            } else { refs.push(Reference { text: "?".into(), page: None }); }
        }
        Func::Overview => {
            if let Some(device) = device { for loc in &device.mains { add(loc); } }
        }
        Func::Main { .. } if !symbol.box_role.is_device() => {
            if let Some(device) = device { for loc in device.mains.iter().filter(|l| l.page != page_i) { add(loc); } }
        }
        Func::Main { .. } => {
            if let Some(device) = device { for loc in device.auxs.iter().chain(device.boxes.iter().filter(|l| l.page != page_i)) { add(loc); } }
        }
        Func::Interruption => {
            let others = idx.interruption_targets(prj, &symbol.interruption_key(), symbol.id);
            for loc in &others { add(loc); }
            if refs.is_empty() { refs.push(Reference { text: "?".into(), page: None }); }
        }
        _ => {}
    }
    if matches!(def.func, Func::Main { .. }) && !symbol.tag.is_empty() {
        refs.extend(fluid(prj, idx, page_i, &symbol.tag).into_iter().map(|(text, page)| Reference { text, page }));
    }
    refs
}

pub fn text(refs: &[Reference]) -> String {
    refs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join("\n")
}

/// Linked devices of the other discipline, shared with electro-fluid annotations.
pub fn fluid(prj: &Project, idx: &Index, page_i: usize, tag: &str) -> Vec<(String, Option<usize>)> {
    let forward = prj.fluid.links.get(tag).into_iter().flatten();
    let reverse = prj.fluid.links.iter().filter(|(_, tags)| tags.iter().any(|t| t == tag)).map(|(t, _)| t);
    forward.chain(reverse).map(|other| {
        let rel = crate::structure::relative(&prj.pages[page_i], other);
        match idx.devices.get(other).and_then(|d| d.mains.first()) {
            Some(m) => (format!("{rel} {}", m.reference_from(prj, page_i)), Some(m.page)),
            None => (format!("{rel} ?"), None),
        }
    }).collect()
}

/// Individual property queries; batch drawing/query callers reuse their index instead.
pub fn property(prj: &Project, page_i: usize, symbol: &SymbolInst) -> String {
    prj.symbol(&symbol.sym).map(|def| text(&of_symbol(prj, &Index::build(prj), page_i, &def, symbol))).unwrap_or_default()
}

#[cfg(test)]
mod tests;
