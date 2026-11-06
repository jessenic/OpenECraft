//! Functions that exist in the project data but are not placed in the schematic yet
//! (in the spirit of EPLAN's navigators with unplaced functions): the channels of PLC
//! cards with a device description that have no input / output symbol, and the
//! functions of the parts assigned to a device that no symbol uses. Each comes with
//! the symbol, device tag and designation to place it with.

use crate::model::Project;
use crate::parts::{self, FnKind, PartsDb};
use std::collections::HashSet;

/// Something to place: the symbol, the device tag, the label (PLC address) and the
/// connection point designations.
#[derive(Clone, Debug, PartialEq)]
pub struct Unplaced {
    pub tag: String,
    pub symbol: &'static str,
    pub label: String,
    pub pins: Vec<String>,
    /// What it is, e.g. `I0.3` or `NO 13-14`.
    pub title: String,
}

/// The first built-in symbol that stands for a function kind.
pub fn symbol_for(kind: FnKind) -> Option<&'static str> {
    crate::symbols::library().iter().map(|d| d.key).find(|k| parts::kind_of_symbol(k) == Some(kind))
}

/// The symbol a part is placed with in the schematic and its connection point
/// designations: those of its first function that has a symbol (a contactor's coil,
/// a breaker's main contacts). `None` for parts without one (enclosures, rails).
pub fn part_symbol(part: &parts::Part) -> Option<(&'static str, Vec<String>)> {
    part.functions.iter().find_map(|f| symbol_for(f.kind).map(|s| (s, f.pin_list())))
}

/// Channels of the PLC cards (with a device description) without a symbol, per card
/// in address order.
pub fn plc_channels(prj: &Project) -> Vec<Unplaced> {
    let mut out = Vec::new();
    for card in crate::plc::cards(prj) {
        let mut families = HashSet::new();
        let mut occupied = HashSet::new();
        for s in prj.pages.iter().flat_map(|p| &p.symbols).filter(|s| s.tag == card) {
            let Some(def) = prj.symbol(&s.sym) else { continue };
            if let Some(c) = &def.plc_channel {
                families.insert(c.family.clone());
                occupied.insert((c.family.clone(), c.input, c.index));
            }
        }
        for symbol in &*prj.symbols {
            let Some(c) = symbol.plc_channel.as_ref().filter(|c| families.contains(&c.family)) else { continue };
            if !occupied.insert((c.family.clone(), c.input, c.index)) { continue; }
            let Some(def) = prj.symbol(&symbol.key) else { continue };
            let label = c.address_for(prj, &card);
            let pin = c.analog.as_ref().map(|a| a.terminal.as_str()).or_else(|| symbol.pins.first().map(|pin| pin.label.as_str())).unwrap_or("");
            let title = if label == c.designation { format!("{label}  ({pin})") } else { format!("{}  {}  ({pin})", c.designation, label) };
            out.push(Unplaced { tag: card.clone(), symbol: def.key, label, pins: symbol.pins.iter().map(|p| p.label.clone()).collect(), title });
        }
        let Some(desc) = crate::plc_hw::description_of(prj, &card) else { continue };
        let placed: HashSet<crate::plc::AddressKey> =
            prj.pages.iter().flat_map(|p| p.symbols.iter()).filter(|s| s.tag == card).map(|s| crate::plc::address_key(&s.label)).collect();
        for (input, n, symbol) in [(true, desc.inputs, "plc_in"), (false, desc.outputs, "plc_out")] {
            for ch in 0..n {
                let addr = crate::plc_hw::card_address(prj, &card, input, ch);
                if !placed.contains(&crate::plc::address_key(&addr)) && !out.iter().any(|p| p.tag == card && p.label == addr) {
                    out.push(Unplaced { tag: card.clone(), symbol, label: addr.clone(), pins: Vec::new(), title: addr });
                }
            }
        }
    }
    out
}

/// Functions of the assigned parts that no placed symbol of the device uses (the
/// reserve contacts of the contact image, coils and other main functions too).
pub fn device_functions(prj: &Project, db: Option<&PartsDb>) -> Vec<Unplaced> {
    let mut out = Vec::new();
    for tag in prj.devices.keys() {
        // Terminals, plug pins and PLC points have their own numbering (strips, plugs,
        // PLC channels).
        let own = |k: FnKind| matches!(k, FnKind::Terminal | FnKind::PlugPin | FnKind::PlcInput | FnKind::PlcOutput | FnKind::PlcAnalogInput | FnKind::PlcAnalogOutput);
        for f in parts::reserve_functions(prj, db, tag).into_iter().filter(|f| !own(f.kind)) {
            let Some(symbol) = symbol_for(f.kind) else { continue };
            let pins = f.pin_list();
            let title = format!("{} {}", f.kind.title(), pins.join("-")).trim().to_string();
            out.push(Unplaced { tag: tag.clone(), symbol, label: String::new(), pins, title });
        }
    }
    out
}

/// Potentials defined in the project that no potential connection point carries yet,
/// with the potential symbol of their name (the custom one otherwise).
pub fn potentials(prj: &Project) -> Vec<Unplaced> {
    let used: HashSet<&str> = prj
        .pages
        .iter()
        .flat_map(|p| p.symbols.iter())
        .filter(|s| prj.symbol(&s.sym).is_some_and(|d| d.func == crate::symbols::Func::Potential))
        .map(|s| s.label.as_str())
        .collect();
    prj.potentials
        .iter()
        .filter(|p| !p.name.trim().is_empty() && !used.contains(p.name.as_str()))
        .map(|p| {
            let symbol = crate::symbols::library()
                .iter()
                .find(|d| d.func == crate::symbols::Func::Potential && d.key.starts_with("pot") && d.default_label == p.name)
                .map(|d| d.key)
                .unwrap_or("pot");
            let title = if p.description.is_empty() { p.name.clone() } else { format!("{}  {}", p.name, p.description) };
            Unplaced { tag: String::new(), symbol, label: p.name.clone(), pins: Vec::new(), title }
        })
        .collect()
}

#[cfg(test)]
mod tests;
