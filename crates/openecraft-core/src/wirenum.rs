//! Wire numbers. A number belongs to a connection: a net with the terminal sides
//! kept apart (`Netlist::build(prj, false)`), so the wires on both sides of a
//! terminal get their own numbers. All wire segments of a connection carry it.

use crate::model::*;
use crate::netlist::{Net, Netlist};
use std::collections::{HashMap, HashSet};

/// The wire segments of a net.
pub fn wires_of<'a>(prj: &'a Project, net: &Net) -> Vec<&'a Wire> {
    let mut v: Vec<&Wire> =
        net.wires.iter().filter_map(|(pi, id)| prj.pages[*pi].wires.iter().find(|w| w.id == *id)).collect();
    v.sort_by_key(|w| w.id);
    v
}

/// Number of a connection (the first number found on its wires).
pub fn number_of(prj: &Project, net: &Net) -> String {
    wires_of(prj, net).iter().find(|w| !w.number.is_empty()).map(|w| w.number.clone()).unwrap_or_default()
}

/// Different numbers on the wires of one connection (e.g. after joining two).
pub fn numbers_of(prj: &Project, net: &Net) -> Vec<String> {
    let mut v: Vec<String> = wires_of(prj, net).iter().filter(|w| !w.number.is_empty()).map(|w| w.number.clone()).collect();
    v.sort_by(|a, b| natural_cmp(a, b));
    v.dedup();
    v
}

/// Where a connection starts: page, column (1-based) and row label.
/// Schematic order: page, column, top to bottom, left to right.
type Order = (usize, usize, i64, i64);

struct Start {
    net: usize,
    page: usize,
    col: usize,
    row: String,
}

/// Connections that get a number (they have wires and connect something), in
/// schematic order (page, column, top to bottom), with the place where they start.
fn connections_in_order(prj: &Project, open: &Netlist) -> Vec<Start> {
    let mut v: Vec<(Order, usize, String)> = Vec::new();
    for (ni, net) in open.nets.iter().enumerate() {
        if net.wires.is_empty() || net.pins.is_empty() {
            continue;
        }
        let start = net
            .wires
            .iter()
            .filter_map(|(pi, id)| prj.pages[*pi].wires.iter().find(|w| w.id == *id).map(|w| (*pi, w)))
            .flat_map(|(pi, w)| {
                let frame = crate::frame::of_page(prj, pi);
                [w.a, w.b].map(|q| ((pi, frame.column_index(q) + 1, (q.y * 10.0) as i64, (q.x * 10.0) as i64), frame.row_of(q)))
            })
            .min_by(|a, b| a.0.cmp(&b.0));
        if let Some((start, row)) = start {
            v.push((start, ni, row));
        }
    }
    v.sort_by_key(|a| a.0);
    v.into_iter().map(|((page, col, _, _), net, row)| Start { net, page, col, row }).collect()
}

/// The potential (bridged net) a connection belongs to.
fn potential_of<'a>(prj: &Project, net: &Net, bridged: &'a Netlist) -> Option<&'a Net> {
    let pin = net.pins[0];
    let sym = prj.pages[pin.page].symbols[pin.sym].id;
    bridged.pin_net.get(&(sym, pin.pin)).map(|&n| &bridged.nets[n])
}

/// Code of a potential kind in number formats.
fn kind_code(k: crate::model::potentials::PotentialKind) -> &'static str {
    use crate::model::potentials::PotentialKind as K;
    match k {
        K::L => "L",
        K::N => "N",
        K::Pe => "PE",
        K::DcPlus => "+",
        K::DcMinus => "-",
        K::Signal => "S",
        K::Other => "",
    }
}

/// Numbers all connections according to the project's settings; hoses and pipes
/// (fluid nets) are counted apart with the fluid prefix (`H1`, `H2` …). Manually set
/// numbers stay unless `overwrite_fixed`. Connections of skipped potential kinds lose
/// their automatic number. Returns the number of numbered connections.
#[allow(clippy::too_many_lines)]
pub fn renumber(prj: &mut Project, open: &Netlist, bridged: &Netlist, overwrite_fixed: bool) -> usize {
    let settings = prj.wire_numbering.clone();
    let step = settings.step.max(1);
    let digits = settings.digits as usize;
    let count_text = |n: u32| format!("{n:0digits$}");
    let order = connections_in_order(prj, open);
    let fixed = |prj: &Project, net: &Net| wires_of(prj, net).iter().any(|w| w.number_fixed && !w.number.is_empty());
    let def_of = |name: &str| prj.potentials.iter().find(|d| d.name == name);
    let mut used: HashSet<String> = HashSet::new();
    let mut todo = Vec::new();
    let mut skipped = Vec::new();
    for st in order {
        let net = &open.nets[st.net];
        let kind = potential_of(prj, net, bridged).filter(|p| p.named).and_then(|p| def_of(&p.name)).map(|d| d.kind);
        if !overwrite_fixed && fixed(prj, net) {
            used.insert(number_of(prj, net));
        } else if !net.fluid && kind.is_some_and(|k| settings.skip_kinds.contains(&k)) {
            skipped.push(st.net);
        } else {
            todo.push(st);
        }
    }
    let mut counter = settings.start;
    let mut next_counted = |used: &HashSet<String>| loop {
        let n = format!("{}{}", settings.prefix, count_text(counter));
        counter += step;
        if !used.contains(&n) {
            return n;
        }
    };
    let mut groups: HashMap<String, u32> = HashMap::new();
    let mut per_column: HashMap<(usize, usize), u32> = HashMap::new();
    let mut numbers = Vec::new();
    let mut hoses = 0;
    for st in todo {
        let net = &open.nets[st.net];
        let number = match settings.scheme {
            _ if net.fluid => loop {
                hoses += 1;
                let n = format!("{}{hoses}", prj.fluid.prefix);
                if !used.contains(&n) {
                    break n;
                }
            },
            NumberScheme::Sequential => next_counted(&used),
            NumberScheme::PageColumn => loop {
                let k = per_column.entry((st.page, st.col)).or_insert(0);
                *k += 1;
                let n = format!("{}{}.{}.{k}", settings.prefix, prj.pages[st.page].name, st.col);
                if !used.contains(&n) {
                    break n;
                }
            },
            NumberScheme::Potential => match potential_of(prj, net, bridged) {
                // Shared on purpose by all connections of the potential.
                Some(pot) if pot.named => pot.name.clone(),
                _ => next_counted(&used),
            },
            NumberScheme::Format => {
                let pot = potential_of(prj, net, bridged).filter(|p| p.named);
                let def = pot.and_then(|p| def_of(&p.name));
                let w = wires_of(prj, net);
                let color = w.iter().map(|w| w.color.as_str()).find(|c| !c.is_empty()).or(def.map(|d| d.color.as_str())).unwrap_or("");
                let cs = w.iter().map(|w| w.cross_section.as_str()).find(|c| !c.is_empty()).or(def.map(|d| d.cross_section.as_str())).unwrap_or("");
                let format = if settings.format.trim().is_empty() { "{n}" } else { settings.format.as_str() };
                let base = format
                    .replace("{page}", &prj.pages[st.page].name)
                    .replace("{column}", &st.col.to_string())
                    .replace("{row}", &st.row)
                    .replace("{potential}", pot.map(|p| p.name.as_str()).unwrap_or(""))
                    .replace("{kind}", def.map(|d| kind_code(d.kind)).unwrap_or(""))
                    .replace("{color}", color)
                    .replace("{cross_section}", &cs.replace(' ', ""));
                let base = format!("{}{base}", settings.prefix);
                if base.contains("{n}") {
                    let c = groups.entry(base.clone()).or_insert(settings.start);
                    loop {
                        let n = base.replace("{n}", &count_text(*c));
                        *c += step;
                        if !used.contains(&n) {
                            break n;
                        }
                    }
                } else {
                    // Shared by all connections with the same text.
                    base
                }
            }
        };
        used.insert(number.clone());
        numbers.push((st.net, number));
    }
    let count = numbers.len();
    for (ni, number) in numbers {
        set(prj, &open.nets[ni], &number, false);
    }
    for ni in skipped {
        set(prj, &open.nets[ni], "", false);
    }
    count
}

/// Sets the number of all wires of a connection.
pub fn set(prj: &mut Project, net: &Net, number: &str, fixed: bool) {
    for (pi, id) in &net.wires {
        if let Some(w) = prj.pages[*pi].wires.iter_mut().find(|w| w.id == *id) {
            w.number = number.to_string();
            w.number_fixed = fixed && !number.is_empty();
        }
    }
}

/// Removes wire numbers (manually set ones only with `include_fixed`). Returns the
/// number of wires changed.
pub fn clear(prj: &mut Project, include_fixed: bool) -> usize {
    let mut n = 0;
    for w in prj.pages.iter_mut().flat_map(|pg| pg.wires.iter_mut()) {
        if !w.number.is_empty() && (include_fixed || !w.number_fixed) {
            w.number.clear();
            w.number_fixed = false;
            n += 1;
        }
    }
    n
}

/// Wire of a connection on a page that carries its label (the longest one).
pub fn label_wire<'a>(page: &'a Page, page_i: usize, net: &Net) -> Option<&'a Wire> {
    net.wires
        .iter()
        .filter(|(pi, _)| *pi == page_i)
        .filter_map(|(_, id)| page.wires.iter().find(|w| w.id == *id))
        .filter(|w| !w.hide_labels)
        .max_by(|a, b| (a.b - a.a).len().total_cmp(&(b.b - b.a).len()).then(b.id.cmp(&a.id)))
}

#[cfg(test)]
mod tests;
