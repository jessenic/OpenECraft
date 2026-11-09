//! The built-in project checks ("Prüfläufe"): finds typical engineering mistakes.
//! Every check is a registry entry ([`Check`]) with a snake_case name and a title, so
//! the front ends list and run them like the checks of plugins; [`builtin`] gives the
//! list, [`run_builtin`] the issues of all of them.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::ext::Check;
use crate::index::Index;
use crate::model::*;
use crate::netlist::{pin_designation, Netlist, PinRef};
use crate::panel::{self, RoutedWire};
use crate::reports::{fill_spot, loc_str};
use crate::analysis::Analysis;
use crate::issue::{Issue, Severity};
use crate::symbols::Func;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

fn entry(name: &'static str, title: &'static str, run: impl Fn(&Project, &Analysis) -> Vec<Issue> + Send + Sync + 'static) -> Check {
    Check { name: name.into(), title: title.into(), run: Arc::new(move |prj, an| Ok(run(prj, an))) }
}

/// The built-in checks in the order their issues are listed.
pub fn builtin() -> Vec<Check> {
    vec![
        entry("devices", tr_const!("Devices and connection points"), |prj, an| check(prj, &an.index, &an.netlist)),
        entry("accessories", tr_const!("Part accessories"), |prj, an| check_accessories(prj, &an.index)),
        entry("mounting_pages", tr_const!("Mounting layout pages"), |prj, _| check_layout(prj)),
        entry("busbars", tr_const!("Busbar systems"), |prj, an| crate::panel::busbars::problems(prj, &an.netlist)),
        entry("wires", tr_const!("Cables and wire numbers"), |prj, an| check_wires(prj, &an.netlist, &an.open)),
        entry("potentials", tr_const!("Potential definitions"), |prj, an| check_potentials(prj, &an.netlist, &an.open)),
        entry("plugs", tr_const!("Plugs"), |prj, _| crate::plugs::problems(prj).into_iter().map(|(message, target)| Issue::plain(Severity::Warning, message, target)).collect()),
        entry("connectors", tr_const!("Connector pin-outs and flat cables"), |prj, _| crate::pinout::problems(prj)),
        entry("terminal_strips_and_plc", tr_const!("Terminal strips and PLC addresses"), |prj, an| check_strips_and_plc(prj, &an.netlist)),
        entry("plc_hardware", tr_const!("PLC hardware"), |prj, _| crate::plc_hw::problems(prj)),
        entry("duct_fill", tr_const!("Duct fill levels"), |prj, an| check_duct_fill(prj, &an.duct_fill)),
        entry("climate", tr_const!("Enclosure climate"), |prj, _| check_climate(prj)),
        entry("connection_definitions", tr_const!("Connection definitions"), |prj, an| crate::conndef::problems(prj, &an.open)),
        entry("connection_symbols", tr_const!("Connection symbols"), |prj, an| crate::netlist::junctions::problems(prj, &an.open)),
        entry("cable_definitions", tr_const!("Cable definitions"), |prj, _| crate::cabledef::problems(prj)),
        entry("cable_sizing", tr_const!("Cable sizing"), |prj, an| crate::sizing::problems(prj, &an.netlist, &an.open, &an.connections)),
        entry("short_circuit", tr_const!("Short circuit and disconnection"), |prj, an| crate::shortcircuit::problems(prj, &an.netlist, &an.connections)),
        entry("loads", tr_const!("Loads and power supplies"), |prj, an| crate::loads::problems(prj, &an.netlist)),
        entry("emc", tr_const!("EMC"), |prj, an| crate::emc::problems(prj, &an.netlist, &an.connections)),
        entry("safety", tr_const!("Safety functions"), |prj, _| crate::safety::problems(prj)),
        entry("rule_sets", tr_const!("Rule set verification"), |prj, an| crate::rules::problems(prj, &an.netlist, &an.connections)),
        entry("fluid_and_process", tr_const!("Fluid power and process"), |prj, an| check_fluid_and_process(prj, &an.index, &an.open)),
        entry("single_line", tr_const!("Single-line / multi-line"), |prj, an| {
            crate::singleline::problems(prj, &an.index, &an.netlist).into_iter().map(|(severity, message, target)| Issue::plain(severity, message, target)).collect()
        }),
        entry("wiring_marks", tr_const!("Wiring marks"), |prj, an| check_wiring_marks(prj, &an.wiring)),
        entry("mounting_marks", tr_const!("Mounting marks"), |prj, _| check_mounting_marks(prj)),
        entry("preplanning", tr_const!("Pre-planning"), |prj, an| crate::preplanning::issues(prj, &an.index)),
        entry("integration", tr_const!("Enterprise integration"), |prj, _| crate::integration::problems(prj)),
        entry("field_wiring", tr_const!("Field wiring"), |_, an| an.field.issues.clone()),
        entry("buildings", tr_const!("Buildings, levels and shafts"), |prj, _| crate::panel::buildings::problems(prj)),
        entry("routing_3d", tr_const!("3D routing"), |_, an| check_routing(&an.routing)),
        entry("structure", tr_const!("Structure identifiers"), |prj, _| {
            let mut v: Vec<Issue> = crate::structure::problems(prj).into_iter().map(|message| Issue::plain(Severity::Warning, message, None)).collect();
            v.extend(crate::structure_boxes::problems(prj));
            v
        }),
        entry("translations", tr_const!("Project languages"), |prj, _| check_translations(prj)),
        entry("harnesses", tr_const!("Harnesses"), check_harnesses),
        entry("open_wire_ends", tr_const!("Open wire ends"), |prj, _| check_open_wire_ends(prj)),
    ]
}

/// Applies the project's check settings to the issues the checks found: changed
/// levels (`Off` removes the message) and accepted messages. The host skips disabled
/// checks before running them ([`is_enabled`]).
pub fn apply_settings(prj: &Project, issues: Vec<Issue>) -> Vec<Issue> {
    use crate::model::checks::Level;
    let s = &prj.check_settings;
    issues
        .into_iter()
        .filter_map(|mut i| {
            match s.levels.get(&i.code()) {
                Some(Level::Off) => return None,
                Some(Level::Error) => i.severity = Severity::Error,
                Some(Level::Warning) => i.severity = Severity::Warning,
                Some(Level::Info) => i.severity = Severity::Info,
                None => {}
            }
            if !s.accepted.is_empty() {
                let id = i.fingerprint();
                i.accepted = s.accepted.iter().any(|a| a.id == id);
            }
            Some(i)
        })
        .collect()
}

/// The check runs (not switched off in the project's check settings).
pub fn is_enabled(prj: &Project, check: &str) -> bool {
    !prj.check_settings.disabled.iter().any(|d| d == check)
}

/// Accepts a message as checked (or takes the acceptance back with `accept` false).
pub fn accept(prj: &mut Project, issue: &Issue, accept: bool, note: &str, user: &str, date: &str) {
    let id = issue.fingerprint();
    prj.check_settings.accepted.retain(|a| a.id != id);
    if accept {
        prj.check_settings.accepted.push(crate::model::checks::Accepted { id, message: issue.message(), note: note.into(), user: user.into(), date: date.into() });
    }
}

/// The check scheme of the project as a file: checks switched off and changed message
/// levels (the accepted messages belong to the project and stay out).
pub fn scheme_json(prj: &Project) -> String {
    let s = &prj.check_settings;
    let scheme = crate::model::checks::CheckSettings { disabled: s.disabled.clone(), levels: s.levels.clone(), accepted: Vec::new() };
    serde_json::to_string_pretty(&scheme).unwrap_or_default()
}

/// Takes the checks switched off and the message levels of a scheme file; the
/// accepted messages of the project stay.
pub fn apply_scheme(prj: &mut Project, json: &str) -> Result<(), String> {
    let not_scheme = |e: String| openecraft_i18n::tr_fmt!("Not a check scheme: {}", e);
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| not_scheme(e.to_string()))?;
    // Any other JSON object (a project, a settings scheme) would pass as an empty scheme.
    let Some(map) = value.as_object() else {
        return Err(not_scheme("JSON object expected".into()));
    };
    if let Some(key) = map.keys().find(|k| !SCHEME_KEYS.contains(&k.as_str())) {
        return Err(not_scheme(format!("unknown field `{key}`")));
    }
    let scheme: crate::model::checks::CheckSettings = serde_json::from_value(value).map_err(|e| not_scheme(e.to_string()))?;
    prj.check_settings.disabled = scheme.disabled;
    prj.check_settings.levels = scheme.levels;
    Ok(())
}

/// The fields of a check scheme file.
const SCHEME_KEYS: [&str; 3] = ["disabled", "levels", "accepted"];

/// Accepted messages that no check reports any more (the object changed or the
/// problem was solved).
pub fn stale_accepted<'a>(prj: &'a Project, issues: &[Issue]) -> Vec<&'a crate::model::checks::Accepted> {
    let live: std::collections::HashSet<String> = issues.iter().map(Issue::fingerprint).collect();
    prj.check_settings.accepted.iter().filter(|a| !live.contains(&a.id)).collect()
}

/// The messages as CSV (`Severity;Code;Message;Page;Item;Accepted`).
pub fn issues_csv(prj: &Project, issues: &[Issue]) -> String {
    let cell = |s: &str| if s.contains([';', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = String::from("Severity;Code;Message;Page;Item;Accepted\n");
    for i in issues {
        let page = i.target.and_then(|t| prj.pages.get(t.0)).map(|p| p.name.clone()).unwrap_or_default();
        let item = i.target.map(|t| t.1).filter(|id| *id != 0).map(|id| id.to_string()).unwrap_or_default();
        out.push_str(&format!("{};{};{};{};{};{}\n", i.severity.key(), cell(&i.code()), cell(&i.message()), cell(&page), item, if i.accepted { "yes" } else { "" }));
    }
    out
}

/// Issues of all built-in checks, errors first.
pub fn run_builtin(prj: &Project, an: &Analysis) -> Vec<Issue> {
    let mut issues: Vec<Issue> = builtin().iter().flat_map(|c| (c.run)(prj, an).unwrap_or_default()).collect();
    issues.sort_by_key(|i| i.severity);
    issues
}

/// Problems of the 3D routing and frozen routes whose length changed.
fn check_routing(routing: &crate::routing3d::Routing) -> Vec<Issue> {
    let mut v: Vec<Issue> = routing
        .problems
        .iter()
        .map(|p| {
            let severity = match p.level {
                crate::routing3d::Level::Error => Severity::Error,
                crate::routing3d::Level::Warning => Severity::Warning,
                crate::routing3d::Level::Info => Severity::Info,
            };
            Issue::plain(severity, p.message.clone(), p.target)
        })
        .collect();
    if !routing.changes.is_empty() {
        v.push(crate::issue!(Severity::Info, None, "{} frozen routes changed their length", routing.changes.len()));
    }
    v
}

/// Project texts without a translation into a target language.
fn check_translations(prj: &Project) -> Vec<Issue> {
    let mut v = Vec::new();
    for lang in &prj.languages.targets {
        let n = crate::translation::missing(prj, lang).len();
        if n > 0 {
            v.push(crate::issue!(Severity::Info, None, "{} texts are not translated into {}", n, crate::translation::language_name(lang)));
        }
    }
    v
}

/// Project check ("Prüfläufe"): finds typical engineering mistakes.
pub fn check(prj: &Project, idx: &Index, nl: &Netlist) -> Vec<Issue> {
    let mut v = check_function_parts(prj);
    let placed = crate::parts::placed_kinds(prj);
    for tag in idx.sorted_tags() {
        let dev = &idx.devices[tag];
        let t = |l: &crate::index::Loc| Some((l.page, l.sym_id));
        if dev.mains.is_empty() && !dev.auxs.is_empty() {
            v.push(crate::issue!(Severity::Error, t(&dev.auxs[0]), "{}: auxiliary function(s) without main function", tag));
        }
        let mains = mains_of_main_parts(prj, tag, &dev.mains);
        if mains.len() > 1 && !poles_of_one_device(prj, tag, &mains) {
            v.push(crate::issue!(Severity::Error, t(&mains[1]), "{}: main function placed {} times", tag, mains.len()));
        }
        if let Some(l) = dev.boxes.first().filter(|_| dev.mains.is_empty()) {
            v.push(crate::issue!(Severity::Warning, t(l), "{}: device box extends or shows a device that is not placed", tag));
        }
        if !dev.others.is_empty() {
            // Duplicate terminal numbers / PLC addresses / device connection points (plug
            // pins: see `plugs::problems`)
            let mut seen: HashMap<String, usize> = HashMap::new();
            let function_pins: Vec<&String> = dev.mains.iter().chain(&dev.auxs)
                .filter_map(|l| prj.pages[l.page].symbols.iter().find(|s| s.id == l.sym_id))
                .flat_map(|s| s.pins.iter()).filter(|d| !d.is_empty()).collect();
            let mut pins: Option<Vec<String>> = None;
            for l in &dev.others {
                let s = prj.pages[l.page].symbols.iter().find(|s| s.id == l.sym_id);
                let func = s.and_then(|s| prj.symbol(&s.sym)).map(|d| d.func);
                if let Some(s) = s.filter(|_| !matches!(func, Some(Func::Plug(_)))) {
                    let designation = if func == Some(Func::Connection) { s.pins.first().cloned().unwrap_or_default() } else { s.label.clone() };
                    if crate::device_boxes::is_view_point(prj, &prj.pages[l.page], s) {
                        if !pins.get_or_insert_with(|| crate::device_boxes::device_pins(prj, tag)).contains(&designation) {
                            v.push(crate::issue!(Severity::Warning, t(l), "{}: device box shows connection point '{}' that the device does not have", tag, designation));
                        }
                        continue;
                    }
                    let c = seen.entry(designation.clone()).or_default();
                    *c += 1 + usize::from(*c == 0 && func == Some(Func::Connection) && function_pins.contains(&&designation));
                    if *c == 2 {
                        v.push(crate::issue!(Severity::Warning, t(l), "{}: designation '{}' used more than once", tag, designation));
                    }
                }
            }
        }
        let data = prj.devices.get(tag);
        for r in data.map(|d| d.parts.as_slice()).unwrap_or(&[]) {
            if !prj.parts.iter().any(|p| p.number == r.number) {
                let l = dev.mains.first().or(dev.others.first()).or(dev.auxs.first());
                v.push(crate::issue!(Severity::Error, l.and_then(t), "{}: part {} is not in the project parts (Parts management › synchronize)", tag, r.number));
            }
        }
        // Function templates: more functions placed than the parts provide?
        let usage = crate::parts::usage_of(prj, None, tag, placed.get(tag.as_str()).map(Vec::as_slice).unwrap_or(&[]));
        let has_templates = usage.values().any(|(_, avail)| *avail > 0);
        if has_templates {
            for (kind, (placed, avail)) in &usage {
                if !matches!(kind, crate::parts::FnKind::Terminal | crate::parts::FnKind::PlugPin) && placed > avail {
                    let l = dev.mains.first().or(dev.auxs.first()).or(dev.others.first());
                    v.push(crate::issue!(Severity::Warning, l.and_then(t), "{}: {}× {} placed, assigned parts provide {}", tag, placed, tr(kind.title()), avail));
                }
            }
        }
        let has_part = data.map(|d| !d.parts.is_empty()).unwrap_or(false);
        if !has_part && (!dev.mains.is_empty() || !dev.others.is_empty()) {
            let l = dev.mains.first().or(dev.others.first()).unwrap();
            v.push(crate::issue!(Severity::Info, t(l), "{}: no part assigned", tag));
        }
    }
    for (name, locs) in &idx.interruptions {
        let label = &prj.pages[locs[0].page].symbols.iter().find(|s| s.id == locs[0].sym_id).unwrap().label;
        if locs.len() == 1 {
            v.push(crate::issue!(Severity::Warning, Some((locs[0].page, locs[0].sym_id)), "Interruption point '{}' has no counterpart", label));
        } else if prj.xref.interruptions == InterruptionMode::Pairs {
            // In pairs the last point of an odd number has no partner.
            if let Some(l) = locs.iter().find(|l| idx.interruption_targets(prj, name, l.sym_id).is_empty()) {
                v.push(crate::issue!(Severity::Warning, Some((l.page, l.sym_id)), "Interruption point '{}' has no counterpart", label));
            }
        }
    }
    for (pi, pg) in prj.pages.iter().enumerate() {
        let (w, h) = crate::frame::of_page(prj, pi).size();
        for s in &pg.symbols {
            let Some(def) = prj.symbol(&s.sym) else {
                v.push(crate::issue!(Severity::Error, Some((pi, s.id)), "Unknown symbol '{}'", s.sym));
                continue;
            };
            if def.func.has_tag() && s.tag.trim().is_empty() {
                v.push(crate::issue!(Severity::Error, Some((pi, s.id)), "{} without device tag on page {}", def.name, pg.name));
            }
            if !(0.0..=w).contains(&s.pos.x) || !(0.0..=h).contains(&s.pos.y) {
                let name = if s.tag.is_empty() { def.name.to_string() } else { s.tag.clone() };
                v.push(crate::issue!(Severity::Warning, Some((pi, s.id)), "{} lies outside page {}", name, pg.name));
            }
        }
    }
    let mut unconnected: Vec<_> = nl.unconnected.clone();
    unconnected.sort();
    for (pi, sid, pin) in unconnected {
        if let Some((si, s)) = prj.pages[pi].symbols.iter().enumerate().find(|(_, s)| s.id == sid) {
            if crate::fluid::vents(prj, PinRef { page: pi, sym: si, pin }) {
                continue; // reported by the fluid check
            }
            let d = pin_designation(prj, PinRef { page: pi, sym: si, pin });
            v.push(crate::issue!(Severity::Warning, Some((pi, sid)), "Open connection point {} ({})", d, loc_str(prj, pi, s.pos)));
        }
    }
    for net in &nl.nets {
        if net.name.contains(" / ") {
            let target = net.pins.first().map(|p| (p.page, prj.pages[p.page].symbols[p.sym].id));
            v.push(crate::issue!(Severity::Error, target, "Short circuit between potentials {}", net.name));
        }
    }
    v.sort_by_key(|i| i.severity);
    v
}

fn check_function_parts(prj: &Project) -> Vec<Issue> {
    let mut issues = Vec::new();
    for (pi, page) in prj.pages.iter().enumerate() {
        for s in &page.symbols {
            if let Some(number) = s.values.get("function.part_number") {
                if s.selected_part_number(prj).is_none() {
                    issues.push(crate::issue!(Severity::Error, Some((pi, s.id)), "{}: selected part {} is no longer assigned to this device", s.tag, number));
                }
            }
        }
    }
    issues
}

/// Accessories (auxiliary and signal contacts, …) only work attached to a part they
/// fit: one on a device without such a part is a mistake.
fn check_accessories(prj: &Project, idx: &Index) -> Vec<Issue> {
    crate::parts::loose_accessories(prj)
        .into_iter()
        .map(|(tag, number)| {
            let target = idx.devices.get(&tag).and_then(|d| d.mains.first().or(d.auxs.first()).or(d.others.first())).map(|l| (l.page, l.sym_id));
            crate::issue!(Severity::Warning, target, "{}: accessory {} fits none of the device's parts", tag, number)
        })
        .collect()
}

/// Several main functions of one device are its poles or channels drawn one by one
/// (a fuse holder per pole, a surge arrester per conductor, the fuses of a fuse
/// block): all of one kind, which its parts provide as often as they are placed.
/// The main functions of a device without those its accessories provide: the
/// undervoltage release and the motor operator of a circuit breaker are coils drawn
/// on their own, not a second circuit breaker.
fn mains_of_main_parts(prj: &Project, tag: &str, mains: &[crate::index::Loc]) -> Vec<crate::index::Loc> {
    let mut spare: HashMap<crate::parts::FnKind, usize> = HashMap::new();
    let refs = prj.devices.get(tag).map(|d| d.parts.as_slice()).unwrap_or_default();
    for r in refs {
        let Some(p) = crate::sizing::part(prj, &r.number).filter(|p| p.accessory) else { continue };
        for f in &p.functions {
            *spare.entry(f.kind).or_default() += r.quantity() as usize;
        }
    }
    let kind = |l: &crate::index::Loc| prj.pages[l.page].symbols.iter().find(|s| s.id == l.sym_id).and_then(|s| crate::parts::kind_of_symbol(&s.sym));
    mains
        .iter()
        .filter(|l| match kind(l).and_then(|k| spare.get_mut(&k)).filter(|n| **n > 0) {
            Some(n) => {
                *n -= 1;
                false
            }
            None => true,
        })
        .cloned()
        .collect()
}

fn poles_of_one_device(prj: &Project, tag: &str, mains: &[crate::index::Loc]) -> bool {
    let kind = |l: &crate::index::Loc| prj.pages[l.page].symbols.iter().find(|s| s.id == l.sym_id).and_then(|s| crate::parts::kind_of_symbol(&s.sym));
    let Some(first) = mains.first().and_then(kind) else { return false };
    let provided = crate::parts::available_functions(prj, None, tag).iter().filter(|f| f.kind == first).count();
    mains.iter().all(|l| kind(l) == Some(first)) && provided >= mains.len()
}

/// Panel layout checks: enclosures of mounting pages and unplaced devices; the
/// checks of the layout itself are the built-in plugin `panel_checks` (openecraft-api).
fn check_layout(prj: &Project) -> Vec<Issue> {
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        if let PageKind::Mounting(id) = pg.kind {
            if !prj.layout.enclosures.iter().any(|e| e.id == id) {
                v.push(crate::issue!(Severity::Warning, Some((pi, 0)), "Page {}: its enclosure no longer exists", pg.name));
            }
        }
    }
    if prj.layout.enclosures.is_empty() {
        return v;
    }
    for d in panel::devices_to_place(prj) {
        let target = prj.pages.iter().enumerate().find_map(|(pi, pg)| pg.symbols.iter().find(|s| s.tag == d.tag).map(|s| (pi, s.id)));
        v.push(crate::issue!(Severity::Info, target, "{}: not placed in the panel layout", d.tag));
    }
    v
}

/// Wires that end in empty space (not on a connection point or another wire). Ends
/// near the left or right edge of the drawing area are rails continued on the next
/// page and stay unreported.
fn check_open_wire_ends(prj: &Project) -> Vec<Issue> {
    const EDGE: f32 = 12.0;
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate().filter(|(_, pg)| pg.kind == PageKind::Schematic) {
        let area = crate::frame::of_page(prj, pi).area();
        let at_edge = |x: f32| x - area.min.x <= EDGE || area.max.x - x <= EDGE;
        for (id, q) in crate::netlist::open_wire_ends(prj, pg).into_iter().filter(|(_, q)| !at_edge(q.x)) {
            v.push(crate::issue!(Severity::Warning, Some((pi, id)), "Wire ends open at {}", loc_str(prj, pi, q)));
        }
    }
    v
}

/// Cable assignments, wire numbers and device tags.
fn check_wires(prj: &Project, nl: &Netlist, open: &Netlist) -> Vec<Issue> {
    let mut v = Vec::new();
    let mut tags: HashMap<&str, usize> = HashMap::new();
    let catalog = crate::parts::catalog();
    for c in &prj.cables {
        *tags.entry(c.tag.as_str()).or_default() += 1;
        if c.tag.trim().is_empty() {
            v.push(crate::issue!(Severity::Error, None, "Cable without tag"));
        }
        if (!c.length.is_finite() || c.length <= 0.0)
            && prj.parts.iter().chain(catalog.iter()).find(|p| p.number == c.part).is_some_and(|p| p.quantity_unit == "m")
        {
            v.push(crate::issue!(Severity::Warning, None, "{}: cable length is missing; material quantity is unknown", c.tag));
        }
    }
    for (tag, n) in tags.iter().filter(|(t, n)| **n > 1 && !t.is_empty()) {
        v.push(crate::issue!(Severity::Error, None, "Cable {} defined {} times", tag, n));
    }
    // Cable cores: known cable, listed core, one connection per core.
    let mut core_use: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (ni, net) in open.nets.iter().enumerate() {
        for (pi, id) in &net.wires {
            let Some(w) = prj.pages[*pi].wires.iter().find(|w| w.id == *id) else { continue };
            if w.cable.is_empty() {
                continue;
            }
            let at = Some((*pi, w.id));
            match prj.cable(&w.cable) {
                None => v.push(crate::issue!(Severity::Error, at, "Wire assigned to unknown cable {}", w.cable)),
                Some(_) if w.core.is_empty() => v.push(crate::issue!(Severity::Warning, at, "{}: wire without core designation", w.cable)),
                Some(c) if !c.core_list().contains(&w.core) => {
                    v.push(crate::issue!(Severity::Warning, at, "{}: core {} is not a core of the cable ({})", w.cable, w.core, c.cores))
                }
                Some(_) => {}
            }
            let users = core_use.entry((w.cable.clone(), w.core.clone())).or_default();
            if !users.contains(&ni) {
                users.push(ni);
            }
        }
    }
    for ((cable, core), nets) in &core_use {
        if nets.len() > 1 && !core.is_empty() {
            let at = open.nets[nets[1]].wires.first().copied();
            v.push(crate::issue!(Severity::Error, at, "{}: core {} used for {} connections", cable, core, nets.len()));
        }
    }
    // Wire numbers: one per connection, not shared by different potentials.
    let mut by_number: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for net in &open.nets {
        let numbers = crate::wirenum::numbers_of(prj, net);
        let at = net.wires.first().copied();
        if numbers.len() > 1 {
            v.push(crate::issue!(Severity::Warning, at, "Different wire numbers in one connection: {}", numbers.join(", ")));
        }
        let potential = net.pins.first().and_then(|p| nl.pin_net.get(&(prj.pages[p.page].symbols[p.sym].id, p.pin)).copied());
        if let (Some(n), Some(pot)) = (numbers.first(), potential) {
            by_number.entry(n.clone()).or_default().insert(pot);
        }
    }
    for (number, pots) in by_number.iter().filter(|(_, p)| p.len() > 1) {
        v.push(crate::issue!(Severity::Warning, None, "Wire number {} is used for {} different potentials", number, pots.len()));
    }
    // Device tags need a device name after the structure.
    for (pi, pg) in prj.pages.iter().enumerate() {
        for s in pg.symbols.iter().filter(|s| !s.tag.is_empty()) {
            if crate::structure::Dt::parse(&s.tag).name.is_empty() {
                v.push(crate::issue!(Severity::Error, Some((pi, s.id)), "{}: device tag without device name", s.tag));
            }
        }
    }
    v
}

/// Potential definitions and the colours / cross-sections of the connections.
fn check_potentials(prj: &Project, nl: &Netlist, open: &Netlist) -> Vec<Issue> {
    use crate::potentials::{is_protective, normalize_color, props_of, Source};
    let mut v = Vec::new();
    let mut names: HashMap<&str, usize> = HashMap::new();
    for d in &prj.potentials {
        *names.entry(d.name.as_str()).or_default() += 1;
    }
    for (name, n) in names.iter().filter(|(_, n)| **n > 1) {
        v.push(crate::issue!(Severity::Error, None, "Potential {} defined {} times", name, n));
    }
    if !prj.potentials.is_empty() {
        let mut undefined: BTreeMap<String, (usize, u64)> = BTreeMap::new();
        for (pi, pg) in prj.pages.iter().enumerate().filter(|(_, p)| p.kind == PageKind::Schematic) {
            for s in &pg.symbols {
                let pot = prj.symbol(&s.sym).map(|d| d.func == Func::Potential).unwrap_or(false);
                if pot && !s.label.is_empty() && prj.potential(&s.label).is_none() {
                    undefined.entry(s.label.clone()).or_insert((pi, s.id));
                }
            }
        }
        for (name, at) in undefined {
            v.push(crate::issue!(Severity::Info, Some(at), "Potential {} has no definition (Connections › Potentials)", name));
        }
    }
    for net in open.nets.iter().filter(|n| !n.fluid) {
        let Some(&(pi, wid)) = net.wires.first() else { continue };
        let at = Some((pi, wid));
        let p = props_of(prj, nl, net);
        if p.potential.contains(" / ") || crate::wirenum::wires_of(prj, net).iter().any(|w| !w.cable.is_empty()) {
            continue;
        }
        let color = normalize_color(&p.color);
        if is_protective(prj, &p.potential) {
            if !color.is_empty() && color != "GNYE" {
                v.push(crate::issue!(Severity::Error, at, "Protective conductor {} with wire colour {} (must be GNYE)", p.potential, p.color));
            }
        } else if color == "GNYE" {
            let name = if p.potential.is_empty() { net.name.clone() } else { p.potential.clone() };
            v.push(crate::issue!(Severity::Error, at, "Wire colour GNYE is reserved for protective conductors ({})", name));
        }
        let Some(def) = prj.potential(&p.potential) else { continue };
        if p.color_source == Source::Manual && !def.color.is_empty() && color != normalize_color(&def.color) {
            v.push(crate::issue!(Severity::Warning, at, "{}: wire colour {} differs from the potential definition ({})", def.name, p.color, def.color));
        }
        let mm2 = crate::production::cross_section_mm2;
        if p.cross_section_source == Source::Manual && !def.cross_section.is_empty() && mm2(&p.cross_section) != mm2(&def.cross_section) {
            v.push(crate::issue!(Severity::Warning, at, "{}: cross-section {} differs from the potential definition ({})", def.name, p.cross_section, def.cross_section));
        }
    }
    v
}

/// Jumpers on missing terminals, PLC addresses used on several cards.
fn check_strips_and_plc(prj: &Project, nl: &Netlist) -> Vec<Issue> {
    let mut v = Vec::new();
    for (strip, data) in &prj.strips {
        let labels: Vec<String> = crate::strips::terminals(prj, strip).into_iter().map(|t| t.label).collect();
        for (label, j) in data.jumpers.iter().filter(|(_, j)| **j > 0) {
            if !labels.contains(label) {
                v.push(crate::issue!(Severity::Warning, None, "{}: jumper {} on terminal {}, which is not placed", strip, j, label));
            }
        }
        for j in crate::strips::strip_jumpers(prj, strip) {
            let Some(message) = j.problem_text(strip) else { continue };
            let severity = if j.problem == Some(crate::strips::JumperProblem::NoPart) { Severity::Info } else { Severity::Warning };
            v.push(Issue::plain(severity, message, None));
        }
        v.extend(crate::strips::level_problems(prj, strip).into_iter().map(|m| Issue::plain(Severity::Warning, m, None)));
    }
    for (addr, points) in crate::plc::duplicate_addresses(prj, nl) {
        let mut cards: Vec<&str> = points.iter().map(|p| p.card.as_str()).collect();
        cards.dedup();
        if cards.len() > 1 {
            v.push(crate::issue!(Severity::Error, Some((points[1].page, points[1].id)), "PLC address {} used on several cards: {}", addr, cards.join(", ")));
        }
    }
    v
}

/// Ducts above the fill limit (warning) or close to it (above 80 % of it, hint); too
/// small for their wires to lie side by side (error).
fn check_duct_fill(prj: &Project, fill: &[panel::DuctFill]) -> Vec<Issue> {
    let limit = prj.layout.duct_fill_limit;
    let mut v = Vec::new();
    for f in fill {
        let duct = prj.layout.enclosures[f.enclosure].item(f.duct);
        if let Some((width, depth)) = duct.map(panel::duct_inside) {
            if !panel::duct_lanes(width, depth, &f.diameters).1 {
                let at = fill_spot(prj, f);
                let name = &prj.layout.enclosures[f.enclosure].name;
                let message = tr_fmt!("{}: cable duct at {} mm too small: its {} wires cannot be routed side by side", name, format!("{:.0} / {:.0}", at.x, at.y), f.diameters.len());
                v.push(Issue::plain(Severity::Error, message, None));
                continue;
            }
        }
        let severity = if f.percent > limit {
            Severity::Warning
        } else if f.percent > 0.8 * limit {
            Severity::Info
        } else {
            continue;
        };
        let at = fill_spot(prj, f);
        let name = &prj.layout.enclosures[f.enclosure].name;
        let message = tr_fmt!(
            "{}: cable duct at {} mm filled {} % (limit {} %)",
            name,
            format!("{:.0} / {:.0}", at.x, at.y),
            format!("{:.0}", f.percent),
            format!("{limit:.0}")
        );
        v.push(Issue::plain(severity, message, None));
    }
    v
}

/// Enclosures that get too warm without cooling (rooms are not calculated).
fn check_climate(prj: &Project) -> Vec<Issue> {
    use crate::climate::Cooling;
    let mut v = Vec::new();
    for e in prj.layout.enclosures.iter().filter(|e| !e.is_room()) {
        let r = crate::climate::calculate(prj, e);
        if r.cooling != Cooling::None {
            v.push(crate::issue!(Severity::Warning, None, "{}: {} W power loss heat the enclosure to {} °C (highest {} °C): {} needed", e.name, format!("{:.0}", r.loss), format!("{:.0}", r.inside), format!("{:.0}", e.climate.inside_max), r.cooling.text()));
        }
    }
    v
}

/// Hoses, fluid device groups, electro-fluid links, P&ID instruments and PCT loops of
/// the pre-planning.
fn check_fluid_and_process(prj: &Project, idx: &Index, open: &Netlist) -> Vec<Issue> {
    let fluid = crate::fluid::problems(prj, open).into_iter().chain(crate::electrofluid::problems(prj, idx)).map(|(severity, message, target)| Issue::plain(severity, message, target));
    let process = crate::process::problems(prj, idx).into_iter().map(|(message, warning, target)| Issue::plain(if warning { Severity::Warning } else { Severity::Info }, message, target));
    fluid.chain(process).collect()
}

/// Wires changed after they were wired, and problems reported from the workshop.
fn check_wiring_marks(prj: &Project, wiring: &[RoutedWire]) -> Vec<Issue> {
    let mut v = Vec::new();
    for w in wiring {
        match crate::wiring::status(prj, w) {
            crate::wiring::Status::Changed => v.push(crate::issue!(Severity::Warning, None, "Wire {} – {} changed after it was wired", w.from, w.to)),
            crate::wiring::Status::Problem => {
                let note = crate::wiring::mark_of(prj, w).map(|m| m.note.clone()).unwrap_or_default();
                v.push(crate::issue!(Severity::Warning, None, "Wiring problem {} – {}: {}", w.from, w.to, note));
            }
            _ => {}
        }
    }
    let stale = crate::wiring::stale(prj, wiring).len();
    if stale > 0 {
        v.push(crate::issue!(Severity::Info, None, "{} wiring marks belong to wires that no longer exist", stale));
    }
    v
}

/// Items moved or changed after they were mounted, mounting problems and marks of
/// items that no longer exist.
fn check_mounting_marks(prj: &Project) -> Vec<Issue> {
    use crate::mounting::{self, Status};
    let mut v = Vec::new();
    for e in &prj.layout.enclosures {
        for it in &e.items {
            match mounting::status(prj, it) {
                Status::Changed => v.push(crate::issue!(Severity::Warning, None, "{}: {} changed after it was mounted", e.name, mounting::item_title(it))),
                Status::Problem => {
                    let note = mounting::mark_of(prj, it.id).map(|m| m.note.clone()).unwrap_or_default();
                    v.push(crate::issue!(Severity::Warning, None, "{}: mounting problem {}: {}", e.name, mounting::item_title(it), note));
                }
                _ => {}
            }
        }
    }
    let stale = mounting::stale(prj).len();
    if stale > 0 {
        v.push(crate::issue!(Severity::Info, None, "{} mounting marks belong to items that no longer exist", stale));
    }
    v
}

/// Assigned connections that no longer exist, harnesses without wires.
fn check_harnesses(prj: &Project, an: &Analysis) -> Vec<Issue> {
    let mut v = Vec::new();
    for (tag, [a, b]) in crate::harness::stale(prj, an) {
        v.push(crate::issue!(Severity::Warning, None, "Harness {}: connection {} – {} does not exist in the schematic", tag, a, b));
    }
    for h in &prj.harnesses {
        if !an.connections.iter().any(|c| c.harness == h.tag) {
            v.push(crate::issue!(Severity::Info, None, "Harness {} has no wires", h.tag));
        }
        use crate::harness::topology::Problem;
        for p in crate::harness::topology::problems(prj, an, h) {
            v.push(match p {
                Problem::DuplicateId(id) => crate::issue!(Severity::Error, None, "Harness {}: the id {} is used more than once", h.tag, id),
                Problem::UnknownNode(seg, node) => crate::issue!(Severity::Error, None, "Harness {}: segment {} ends at node {}, which does not exist", h.tag, seg, node),
                Problem::UnknownSegment(id, seg) => crate::issue!(Severity::Error, None, "Harness {}: {} lies on segment {}, which does not exist", h.tag, id, seg),
                Problem::OutOfRange(id) => crate::issue!(Severity::Warning, None, "Harness {}: the location of {} is outside the segment (0 … 1)", h.tag, id),
                Problem::ConnectorWithoutNode(dev) => crate::issue!(Severity::Warning, None, "Harness {}: connector {} is at no node of the topology", h.tag, dev),
                Problem::NotRouted(number, a, b) => crate::issue!(Severity::Warning, None, "Harness {}: wire {} ({} – {}) has no way through the segments", h.tag, number, a, b),
                Problem::BendRadius(node, r, min) => crate::issue!(Severity::Warning, None, "Harness {}: bend radius {} mm at node {} is below the smallest bend radius {} mm of the bundle", h.tag, format!("{r:.0}"), node, format!("{min:.0}")),
            });
        }
    }
    v
}

#[cfg(test)]
mod tests;
