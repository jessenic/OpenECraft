//! Terminal strips: terminal order, renumbering, jumpers (plug-in, saddle and wire
//! jumpers), multi-level terminals, spare terminals, the part of every terminal and
//! the layout of the strip on the rail.

use openecraft_i18n::{tr, tr_fmt};
use crate::model::*;
use crate::netlist::{pin_designation, Netlist};
use crate::parts::FnKind;
use crate::symbols::Func;

/// A terminal of a strip as placed in the schematic.
#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub page: usize,
    pub sym: usize,
    pub id: u64,
    pub label: String,
}

fn is_terminal(prj: &Project, s: &SymbolInst) -> bool {
    prj.symbol(&s.sym).map(|d| d.func) == Some(Func::Terminal)
}

/// Tags of all terminal strips.
pub fn strip_tags(prj: &Project) -> Vec<String> {
    let mut v: Vec<String> =
        prj.pages.iter().flat_map(|p| p.symbols.iter()).filter(|s| is_terminal(prj, s) && !s.tag.is_empty()).map(|s| s.tag.clone()).collect();
    v.extend(prj.strips.keys().cloned());
    v.sort_by(|a, b| natural_cmp(a, b));
    v.dedup();
    v
}

/// Terminals of a strip in strip order: the stored order first, the rest sorted by
/// designation (then schematic order).
pub fn terminals(prj: &Project, strip: &str) -> Vec<Terminal> {
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        if pg.kind != PageKind::Schematic {
            continue;
        }
        for (si, s) in pg.symbols.iter().enumerate() {
            if s.tag == strip && is_terminal(prj, s) {
                v.push(Terminal { page: pi, sym: si, id: s.id, label: s.label.clone() });
            }
        }
    }
    let order = prj.strips.get(strip).map(|d| d.order.as_slice()).unwrap_or(&[]);
    let rank = |t: &Terminal| order.iter().position(|o| *o == t.label).unwrap_or(usize::MAX);
    v.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| natural_cmp(&a.label, &b.label)).then((a.page, a.sym).cmp(&(b.page, b.sym))));
    v
}

/// Moves a terminal one place up or down and stores the resulting order.
pub fn move_terminal(prj: &mut Project, strip: &str, index: usize, up: bool) {
    let mut labels: Vec<String> = terminals(prj, strip).into_iter().map(|t| t.label).collect();
    let other = if up { index.checked_sub(1) } else { Some(index + 1).filter(|i| *i < labels.len()) };
    if let (Some(o), true) = (other, index < labels.len()) {
        labels.swap(index, o);
        prj.strips.entry(strip.to_string()).or_default().order = labels;
    }
}

/// Numbers the terminals 1, 2, 3 … in strip order; terminals with a non-numeric
/// designation (e.g. PE) keep it. Jumpers and order follow. Returns the number of
/// changed designations.
pub fn renumber(prj: &mut Project, strip: &str) -> usize {
    let list = terminals(prj, strip);
    let mut n = 0;
    let mut changed = 0;
    let mut renamed = Vec::new();
    for t in &list {
        if !t.label.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        n += 1;
        let new = n.to_string();
        if t.label != new {
            renamed.push((t.label.clone(), new.clone()));
            prj.pages[t.page].symbols[t.sym].label = new;
            changed += 1;
        }
    }
    if let Some(data) = prj.strips.get_mut(strip) {
        let map = |l: &String| renamed.iter().find(|(o, _)| o == l).map(|(_, n)| n.clone()).unwrap_or_else(|| l.clone());
        data.order = list.iter().map(|t| map(&t.label)).collect();
        data.jumpers = data.jumpers.iter().map(|(l, j)| (map(l), *j)).collect();
        data.levels = data.levels.iter().map(|(l, v)| (map(l), *v)).collect();
    }
    changed
}

/// One row of the terminal diagram / strip editor.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub terminal: Option<Terminal>,
    pub label: String,
    /// Level of a multi-level terminal (1 = bottom).
    pub level: u8,
    pub jumper: u32,
    pub external: String,
    pub internal: String,
    pub potential: String,
    pub location: String,
    pub function_text: String,
}

fn targets(prj: &Project, nl: &Netlist, net: Option<usize>, exclude: (usize, usize)) -> String {
    let Some(ni) = net else { return String::new() };
    let mut v: Vec<String> = nl.nets[ni].pins.iter().filter(|p| (p.page, p.sym) != exclude).map(|p| pin_designation(prj, *p)).collect();
    v.sort_by(|a, b| natural_cmp(a, b));
    v.dedup();
    v.join(", ")
}

/// Terminal diagram rows of a strip including spare terminals. `nl`: potentials,
/// `open`: terminal sides apart.
pub fn rows(prj: &Project, strip: &str, nl: &Netlist, open: &Netlist) -> Vec<Row> {
    let data = prj.strips.get(strip).cloned().unwrap_or_default();
    let mut out = Vec::new();
    let mut max = 0u32;
    for t in terminals(prj, strip) {
        let s = &prj.pages[t.page].symbols[t.sym];
        max = max.max(t.label.parse().unwrap_or(0));
        out.push(Row {
            level: data.levels.get(&t.label).copied().unwrap_or(1).max(1),
            label: t.label.clone(),
            jumper: data.jumpers.get(&t.label).copied().unwrap_or(0),
            external: targets(prj, open, open.pin_net.get(&(s.id, 0)).copied(), (t.page, t.sym)),
            internal: targets(prj, open, open.pin_net.get(&(s.id, 1)).copied(), (t.page, t.sym)),
            potential: nl.pin_net.get(&(s.id, 0)).map(|&n| nl.nets[n].name.clone()).unwrap_or_default(),
            location: crate::frame::reference(prj, t.page, s.pos),
            function_text: crate::text::function_text(prj, t.page, s).replace('\n', " "),
            terminal: Some(t),
        });
    }
    for i in 1..=data.spares {
        out.push(Row {
            terminal: None,
            label: (max + i).to_string(),
            level: 1,
            jumper: 0,
            external: String::new(),
            internal: String::new(),
            potential: String::new(),
            location: String::new(),
            function_text: "spare".into(),
        });
    }
    out
}

/// Width of a terminal whose part has no mounting data (mm).
pub const TERMINAL_WIDTH: f32 = 5.2;
/// Widths of end brackets and end plates whose parts are unknown (mm).
pub const END_BRACKET_WIDTH: f32 = 9.0;
pub const END_PLATE_WIDTH: f32 = 1.5;

/// Symbol of protective earth terminals.
const PE_SYMBOL: &str = "term_pe";

/// A protective earth terminal part (tech data `Terminal type: PE`).
pub fn is_pe_part(part: &Part) -> bool {
    part.data_value("Terminal type").is_some_and(|t| t.eq_ignore_ascii_case("PE"))
}

/// A part whose functions are all terminals.
pub fn is_terminal_part(part: &Part) -> bool {
    !part.functions.is_empty() && part.functions.iter().all(|f| f.kind == FnKind::Terminal)
}

/// Terminal parts of a strip in the order of its parts list (project parts first,
/// then the built-in catalog).
fn terminal_parts<'a>(prj: &'a Project, strip: &str) -> Vec<&'a Part> {
    let Some(data) = prj.devices.get(strip) else { return Vec::new() };
    data.parts
        .iter()
        .filter_map(|r| prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == r.number))
        .filter(|p| is_terminal_part(p))
        .collect()
}

/// The part of a terminal: PE terminals take the strip's PE terminal part, all others
/// its first other terminal part; either falls back to the first terminal part.
pub fn terminal_part<'a>(prj: &'a Project, strip: &str, pe: bool) -> Option<&'a Part> {
    let parts = terminal_parts(prj, strip);
    parts.iter().find(|p| is_pe_part(p) == pe).or(parts.first()).copied()
}

fn part_by_number<'a>(prj: &'a Project, number: &str) -> Option<&'a Part> {
    prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == number)
}

/// What sits on the rail at one place of a strip.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementKind {
    /// A terminal of the schematic (designation).
    Terminal(String),
    /// A spare terminal (designation).
    Spare(String),
    /// Holds the strip on the rail, at both ends.
    EndBracket,
    /// Covers the open side of the last terminal of a kind.
    EndPlate,
}

/// One element of a terminal strip on the rail.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub kind: ElementKind,
    /// Part number; empty if the strip has no terminal part.
    pub part: String,
    /// Width along the rail (mm).
    pub width: f32,
    /// Further levels of a multi-level terminal block: (level, designation).
    pub levels: Vec<(u8, String)>,
}

impl Element {
    /// Designation on `level` of this block (level 1 is the element's own).
    pub fn at_level(&self, level: u8) -> Option<&str> {
        match (&self.kind, level) {
            (ElementKind::Terminal(l) | ElementKind::Spare(l), 0 | 1) => Some(l),
            _ => self.levels.iter().find(|(v, _)| *v == level).map(|(_, l)| l.as_str()),
        }
    }

    /// Designations of all levels of the block.
    pub fn designations(&self) -> Vec<&str> {
        let own = match &self.kind {
            ElementKind::Terminal(l) | ElementKind::Spare(l) => Some(l.as_str()),
            _ => None,
        };
        own.into_iter().chain(self.levels.iter().map(|(_, l)| l.as_str())).collect()
    }
}

/// Level of a terminal (1 for single-level terminals).
pub fn level(prj: &Project, strip: &str, label: &str) -> u8 {
    prj.strips.get(strip).and_then(|d| d.levels.get(label)).copied().unwrap_or(1).max(1)
}

/// Levels of a terminal part (tech data `Levels: 3`, otherwise 1).
pub fn part_levels(part: &Part) -> u8 {
    part.tech_value("Levels").and_then(|v| v.trim().parse().ok()).unwrap_or(1).max(1)
}

/// Accessory element named by the tech data `key` of a terminal part (`End bracket`,
/// `End plate`).
fn accessory(prj: &Project, terminal_part: &str, key: &str, kind: ElementKind, default_width: f32) -> Option<Element> {
    let number = part_by_number(prj, terminal_part)?.tech_value(key)?.trim().to_string();
    let width = part_by_number(prj, &number).and_then(|p| p.size()).map(|s| s[0]).unwrap_or(default_width);
    Some(Element { kind, part: number, width, levels: Vec::new() })
}

/// The strip on the rail in order, each element with its part and width: an end
/// bracket, the terminals, the spare terminals, an end bracket. An end plate follows
/// the last terminal of each part (where the next terminal has another part, and at
/// the end). The accessory parts come from the tech data of the terminal parts
/// (`End bracket: …`, `End plate: …`), unless the strip has `manual_accessories`.
/// Reports, the bill of materials and the panel layout all work from this.
pub fn strip_layout(prj: &Project, strip: &str) -> Vec<Element> {
    let terminals = terminal_elements(prj, strip);
    if terminals.is_empty() || prj.strips.get(strip).is_some_and(|d| d.manual_accessories) {
        return terminals;
    }
    let bracket = |part: &str| accessory(prj, part, "End bracket", ElementKind::EndBracket, END_BRACKET_WIDTH);
    let mut out: Vec<Element> = bracket(&terminals[0].part).into_iter().collect();
    for (i, e) in terminals.iter().enumerate() {
        out.push(e.clone());
        if terminals.get(i + 1).is_none_or(|next| next.part != e.part) {
            out.extend(accessory(prj, &e.part, "End plate", ElementKind::EndPlate, END_PLATE_WIDTH));
        }
    }
    out.extend(bracket(&terminals[terminals.len() - 1].part));
    out
}

/// Terminals and spare terminals of a strip with their parts.
fn terminal_elements(prj: &Project, strip: &str) -> Vec<Element> {
    let element = |kind, pe, number:Option<&str>| {
        let part = number.and_then(|n|part_by_number(prj,n)).or_else(||terminal_part(prj, strip, pe));
        let width = part.and_then(|p| p.size()).map(|s| s[0]).unwrap_or(TERMINAL_WIDTH);
        Element { kind, part: part.map(|p| p.number.clone()).unwrap_or_default(), width, levels: Vec::new() }
    };
    let mut out: Vec<Element> = Vec::new();
    let mut max = 0u32;
    for t in terminals(prj, strip) {
        let pe = prj.pages[t.page].symbols[t.sym].sym == PE_SYMBOL;
        max = max.max(t.label.parse().unwrap_or(0));
        let lv = level(prj, strip, &t.label);
        // Upper levels sit on the block of the terminal before them.
        if let (true, Some(last)) = (lv > 1, out.last_mut().filter(|e| matches!(e.kind, ElementKind::Terminal(_)))) {
            last.levels.push((lv, t.label));
            continue;
        }
        let symbol=&prj.pages[t.page].symbols[t.sym];
        let number=symbol.values.contains_key("function.part_number").then(||symbol.selected_part_number(prj)).flatten();
        out.push(element(ElementKind::Terminal(t.label), pe,number));
    }
    let spares = prj.strips.get(strip).map(|d| d.spares).unwrap_or(0);
    for i in 1..=spares {
        out.push(element(ElementKind::Spare((max + i).to_string()), false,None));
    }
    out
}

/// Why a jumper cannot be fitted as planned.
#[derive(Clone, Debug, PartialEq)]
pub enum JumperProblem {
    /// The terminal part names no jumper parts (tech data `Jumper: …`).
    NoPart,
    /// It needs more poles than the longest jumper part has.
    TooShort { needed: u32, longest: u32 },
    /// An end plate or end bracket lies between its terminals.
    Blocked,
    /// It connects terminals of different parts.
    MixedParts,
    /// It includes a PE terminal.
    Pe,
    /// A plug-in or saddle jumper between terminals on different levels.
    DifferentLevels,
}

/// A plug-in jumper of a strip: from the first to the last terminal with the same
/// jumper number.
#[derive(Clone, Debug, PartialEq)]
pub struct Jumper {
    pub number: u32,
    pub kind: JumperKind,
    /// Level the jumper bridges (1 for single-level terminals).
    pub level: u8,
    /// Jumper part; empty without a suitable one.
    pub part: String,
    /// Poles of the part (cut to the length of `span` if longer).
    pub poles: u32,
    /// Terminals the jumper spans, in strip order.
    pub span: Vec<String>,
    /// Terminals in the span that are not jumpered: their teeth are broken off.
    pub broken: Vec<String>,
    pub problem: Option<JumperProblem>,
}

impl Jumper {
    /// `3 poles`, or `cut from 10 to 3 poles` for a longer part; wire jumpers: the
    /// number of wires.
    pub fn length_text(&self) -> String {
        if self.kind == JumperKind::Wire {
            return tr_fmt!("{} wires", self.span.len().saturating_sub(1));
        }
        match self.span.len() as u32 {
            n if self.poles > n => tr_fmt!("cut from {} to {} poles", self.poles, n),
            n => tr_fmt!("{} poles", n),
        }
    }

    /// `break off at 2, 4`; empty if every terminal of the span is jumpered.
    pub fn broken_text(&self) -> String {
        if self.broken.is_empty() {
            String::new()
        } else {
            tr_fmt!("break off at {}", self.broken.join(", "))
        }
    }

    /// `terminals 1 – 3`.
    pub fn span_text(&self) -> String {
        tr_fmt!("terminals {} – {}", self.span.first().cloned().unwrap_or_default(), self.span.last().cloned().unwrap_or_default())
    }

    /// The problem as a sentence (project check, terminal strip window).
    pub fn problem_text(&self, strip: &str) -> Option<String> {
        let n = self.number;
        Some(match self.problem.as_ref()? {
            JumperProblem::Blocked => tr_fmt!("{}: jumper {} crosses an end plate or end bracket", strip, n),
            JumperProblem::MixedParts => tr_fmt!("{}: jumper {} connects terminals of different parts", strip, n),
            JumperProblem::Pe => tr_fmt!("{}: jumper {} includes a PE terminal", strip, n),
            JumperProblem::DifferentLevels => tr_fmt!("{}: jumper {} connects terminals on different levels; use a wire jumper", strip, n),
            JumperProblem::TooShort { needed, longest } => tr_fmt!("{}: jumper {} needs {} poles, the longest jumper part has {}", strip, n, needed, longest),
            JumperProblem::NoPart => tr_fmt!("{}: no jumper part for jumper {} (tech data Jumper of the terminal part)", strip, n),
        })
    }
}

impl ElementKind {
    /// Name for lists: the designation of terminals, the kind of accessories.
    pub fn title(&self) -> String {
        match self {
            ElementKind::Terminal(label) | ElementKind::Spare(label) => label.clone(),
            ElementKind::EndBracket => tr("End bracket").into(),
            ElementKind::EndPlate => tr("End plate").into(),
        }
    }
}

/// Name of a jumper kind.
pub fn jumper_kind_title(k: JumperKind) -> &'static str {
    match k {
        JumperKind::PlugIn => tr("Plug-in jumper"),
        JumperKind::Saddle => tr("Saddle jumper"),
        JumperKind::Wire => tr("Wire jumper"),
    }
}

/// Poles of a jumper part (tech data `Poles: 3`).
fn poles(prj: &Project, number: &str) -> u32 {
    part_by_number(prj, number).and_then(|p| p.tech_value("Poles")).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// The jumpers of a strip with their parts: plug-in and saddle jumpers take the
/// shortest jumper part of the terminal part (tech data `Jumper: …` / `Saddle jumper:
/// …`) that spans the group; wire jumpers need no part.
pub fn strip_jumpers(prj: &Project, strip: &str) -> Vec<Jumper> {
    let Some(data) = prj.strips.get(strip) else { return Vec::new() };
    let layout = strip_layout(prj, strip);
    let mut numbers: Vec<u32> = data.jumpers.values().copied().filter(|j| *j > 0).collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut out = Vec::new();
    for number in numbers {
        let kind = data.jumper_kinds.get(&number).copied().unwrap_or_default();
        let group: Vec<&String> = data.jumpers.iter().filter(|(_, j)| **j == number).map(|(l, _)| l).collect();
        let lv = |l: &str| data.levels.get(l).copied().unwrap_or(1).max(1);
        let level = group.first().map_or(1, |l| lv(l));
        let mixed_levels = group.iter().any(|l| lv(l) != level);
        let at: Vec<usize> = layout.iter().enumerate().filter(|(_, e)| e.designations().iter().any(|d| group.iter().any(|g| g.as_str() == *d))).map(|(i, _)| i).collect();
        let (Some(&lo), Some(&hi)) = (at.first(), at.last()) else { continue };
        if group.len() < 2 {
            continue;
        }
        let elements = &layout[lo..=hi];
        let span: Vec<String> = if kind == JumperKind::Wire || mixed_levels {
            elements.iter().flat_map(|e| e.designations()).filter(|d| group.iter().any(|g| g.as_str() == *d)).map(String::from).collect()
        } else {
            elements.iter().filter_map(|e| e.at_level(level)).map(String::from).collect()
        };
        let broken: Vec<String> = if kind == JumperKind::Wire { Vec::new() } else { span.iter().filter(|l| !group.contains(l)).cloned().collect() };
        let first = &layout[lo].part;
        let pe = part_by_number(prj, first).is_some_and(is_pe_part);
        if kind == JumperKind::Wire {
            let problem = pe.then_some(JumperProblem::Pe);
            out.push(Jumper { number, kind, level, part: String::new(), poles: 0, span, broken, problem });
            continue;
        }
        let key = if kind == JumperKind::Saddle { "Saddle jumper" } else { "Jumper" };
        let candidates: Vec<(String, u32)> = part_by_number(prj, first)
            .and_then(|p| p.tech_value(key))
            .map(|list| list.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).map(|n| (n.clone(), poles(prj, &n))).collect())
            .unwrap_or_default();
        let needed = span.len() as u32;
        let fitting = candidates.iter().filter(|c| c.1 >= needed).min_by_key(|c| c.1);
        let (part, poles) = fitting.cloned().unwrap_or_default();
        let problem = if mixed_levels {
            Some(JumperProblem::DifferentLevels)
        } else if elements.iter().any(|e| matches!(e.kind, ElementKind::EndPlate | ElementKind::EndBracket)) {
            Some(JumperProblem::Blocked)
        } else if elements.iter().any(|e| e.part != *first) {
            Some(JumperProblem::MixedParts)
        } else if pe {
            Some(JumperProblem::Pe)
        } else if candidates.is_empty() {
            Some(JumperProblem::NoPart)
        } else if fitting.is_none() {
            Some(JumperProblem::TooShort { needed, longest: candidates.iter().map(|c| c.1).max().unwrap_or(0) })
        } else {
            None
        };
        out.push(Jumper { number, kind, level, part, poles, span, broken, problem });
    }
    out
}

/// Multi-level blocks with more levels than their part has (tech data `Levels`), and
/// terminals on an upper level without a block below them.
pub fn level_problems(prj: &Project, strip: &str) -> Vec<String> {
    let mut v = Vec::new();
    let layout = strip_layout(prj, strip);
    for e in &layout {
        let ElementKind::Terminal(label) = &e.kind else { continue };
        let top = e.levels.iter().map(|(l, _)| *l).max().unwrap_or(1);
        let has = part_by_number(prj, &e.part).map_or(1, part_levels);
        if top > has {
            v.push(tr_fmt!("{}: terminal {} uses {} levels, its part {} has {}", strip, label, top, e.part, has));
        }
        if e.levels.is_empty() && level(prj, strip, label) > 1 {
            v.push(tr_fmt!("{}: terminal {} is on level {}, but no terminal block is before it", strip, label, level(prj, strip, label)));
        }
    }
    v
}

/// Jumper groups of all strips: (strip, jumper number) -> terminal designations.
pub fn jumper_groups(prj: &Project) -> Vec<(String, Vec<String>)> {
    let mut v = Vec::new();
    for (strip, data) in &prj.strips {
        let mut numbers: Vec<u32> = data.jumpers.values().copied().filter(|j| *j > 0).collect();
        numbers.sort();
        numbers.dedup();
        for j in numbers {
            let labels: Vec<String> = data.jumpers.iter().filter(|(_, x)| **x == j).map(|(l, _)| l.clone()).collect();
            if labels.len() > 1 {
                v.push((strip.clone(), labels));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests;
