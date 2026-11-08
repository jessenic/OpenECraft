//! Editing operations on a page: placement defaults, move, rotate, delete, copy/paste.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::geom::{on_segment_interior, P, R, GRID};
use crate::model::*;
use crate::sections;
use crate::symlib::Symbols;
use crate::symbols::{ContactKind, Func, PinKind, SymbolDef};
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub use crate::model::ops::Clip;

/// Context remembered between placements (e.g. the coil contacts belong to).
#[derive(Default, Clone)]
pub struct PlaceCtx {
    /// identifying letter -> last used main function tag
    pub last_main: HashMap<String, String>,
    pub last_strip: Option<String>,
    /// symbol key -> last PLC card
    pub last_plc: HashMap<String, String>,
    pub plc_article: Option<String>,
    pub last_plug: Option<String>,
}

impl PlaceCtx {
    pub fn remember(&mut self, prj: &Project, s: &SymbolInst) {
        let Some(def) = prj.symbol(&s.sym) else { return };
        if s.tag.is_empty() {
            return;
        }
        match def.func {
            Func::Main { .. } => {
                self.last_main.insert(def.letter.to_string(), s.tag.clone());
            }
            Func::Terminal => self.last_strip = Some(s.tag.clone()),
            Func::Plc => {
                if def.plc_channel.is_none() {
                    self.last_plc.insert(s.sym.clone(), s.tag.clone());
                }
            }
            Func::Plug(_) => self.last_plug = Some(s.tag.clone()),
            _ => {}
        }
    }
}

fn symbols_with_tag<'a>(prj: &'a Project, tag: &'a str) -> impl Iterator<Item = &'a SymbolInst> + 'a {
    prj.pages.iter().flat_map(|pg| pg.symbols.iter()).filter(move |s| s.tag == tag)
}

/// Connection point designations for the next auxiliary contact of a device
/// (13/14, 21/22, 31/32/34 ...; first digit = ordinal number).
pub fn next_contact_pins(prj: &Project, tag: &str, kind: ContactKind, exclude: Option<u64>) -> Vec<String> {
    let mut used = BTreeSet::new();
    for s in symbols_with_tag(prj, tag) {
        if Some(s.id) == exclude {
            continue;
        }
        if let Some(def) = prj.symbol(&s.sym) {
            if def.auto_number || matches!(def.func, Func::Main { .. }) {
                for pin in &s.pins {
                    if pin.len() == 2 {
                        if let Some(d) = pin.chars().next().and_then(|c| c.to_digit(10)) {
                            used.insert(d);
                        }
                    }
                }
            }
        }
    }
    let o = (1..=9).find(|o| !used.contains(o)).unwrap_or(9);
    match kind {
        ContactKind::No | ContactKind::MainNo => vec![format!("{o}3"), format!("{o}4")],
        ContactKind::Nc => vec![format!("{o}1"), format!("{o}2")],
        ContactKind::Changeover => vec![format!("{o}1"), format!("{o}2"), format!("{o}4")],
        // Time-delayed contacts: x5/x6 (NC), x7/x8 (NO).
        ContactKind::NcOnDelay | ContactKind::NcOffDelay => vec![format!("{o}5"), format!("{o}6")],
        ContactKind::NoOnDelay | ContactKind::NoOffDelay => vec![format!("{o}7"), format!("{o}8")],
    }
}

fn next_terminal(prj: &Project, strip: &str) -> String {
    let max = symbols_with_tag(prj, strip)
        .filter(|s| prj.symbol(&s.sym).map(|d| d.func) == Some(Func::Terminal))
        .filter_map(|s| s.label.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (max + 1).to_string()
}

/// Next free pin number of a plug for pins of this kind (a pair counts as both).
fn next_plug_pin(prj: &Project, plug: &str, kind: PinKind) -> String {
    let max = symbols_with_tag(prj, plug)
        .filter(|s| match prj.symbol(&s.sym).map(|d| d.func) {
            Some(Func::Plug(k)) => k == kind || k == PinKind::Pair || kind == PinKind::Pair,
            _ => false,
        })
        .filter_map(|s| s.label.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (max + 1).to_string()
}

/// Next free address of a card in the scheme of its station.
fn next_plc_address(prj: &Project, card: &str, input: bool) -> String {
    let data = prj.plc_cards.get(card).cloned().unwrap_or_default();
    let start = if input { data.start_byte } else { data.output_byte.unwrap_or(data.start_byte) };
    let max = symbols_with_tag(prj, card)
        .filter_map(|s| crate::plc_hw::parse_address(&s.label))
        .filter(|a| a.input == input)
        .map(|a| if a.slot.is_some() { a.byte * 8 + a.bit } else { a.byte.saturating_sub(start) * 8 + a.bit })
        .max();
    crate::plc_hw::card_address(prj, card, input, max.map(|m| m + 1).unwrap_or(0))
}

/// Name for a new page inserted at `at`: one more than the highest page number of the
/// section it comes into (see [`insert_page`]).
pub fn next_page_name(prj: &Project, at: usize) -> String {
    sections::next_page_name(prj, sections::section_at(prj, at))
}

/// Inserts a page at `at` (clamped), into the section of the page before it. Returns
/// its index.
pub fn insert_page(prj: &mut Project, at: usize, mut page: Page) -> usize {
    let at = at.min(prj.pages.len());
    page.section = sections::section_at(prj, at);
    prj.pages.insert(at, page);
    at
}

/// Moves the page at `from` to position `to` (both existing, in the same section).
/// Returns the index the page has afterwards.
pub fn move_page(prj: &mut Project, from: usize, to: usize) -> usize {
    if from >= prj.pages.len() || to >= prj.pages.len() || sections::of_page(prj, from) != sections::of_page(prj, to) {
        return from;
    }
    let page = prj.pages.remove(from);
    prj.pages.insert(to, page);
    to
}

/// Inserts a new page of `kind` at position `at` (clamped). Returns its index.
pub fn add_page(prj: &mut Project, kind: PageKind, at: usize) -> usize {
    let id = prj.new_id();
    let desc = match kind {
        PageKind::Schematic => tr("Schematic").to_string(),
        PageKind::Cover => tr("Cover sheet").to_string(),
        PageKind::Report(r) => r.title(),
        PageKind::Mounting(id) => {
            let name = prj.layout.enclosures.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
            tr_fmt!("Mounting layout {}", name)
        }
        PageKind::Overview => tr("Overview").to_string(),
        PageKind::Graphic => tr("Graphics").to_string(),
        PageKind::Document => tr("Document").to_string(),
    };
    let page = Page::new(id, &next_page_name(prj, at), &desc, kind);
    insert_page(prj, at, page)
}

/// Adds a new page of `kind` after the pages of a section (0: top level). Returns its
/// index.
pub fn add_page_in(prj: &mut Project, kind: PageKind, section: u64) -> usize {
    let at = add_page(prj, kind, prj.pages.len());
    sections::place_new_page(prj, at, section)
}

/// Inserts a schematic sub-page of page `of` (`3` → `3.1`, then `3.2`, …) after the
/// page and its sub-pages, in the page's structure and section; returns its index.
/// Once the project has sections, sub-pages are lettered (`3.a`, `1.2.a`), so that
/// they cannot be mistaken for the pages of a section.
pub fn add_subpage(prj: &mut Project, of: usize) -> usize {
    let section = sections::of_page(prj, of);
    let local = sections::local_name(prj, of);
    let main = format!("{}{}", sections::prefix(prj, section), local.split_once('.').map_or(local, |(m, _)| m));
    let sub = |name: &str| name.strip_prefix(main.as_str()).and_then(|r| r.strip_prefix('.')).filter(|r| !r.is_empty()).map(str::to_string);
    let subs: Vec<String> = (0..prj.pages.len()).filter(|&i| sections::of_page(prj, i) == section).filter_map(|i| sub(&prj.pages[i].name)).collect();
    let next = if prj.sections.is_empty() {
        (subs.iter().filter_map(|n| n.parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string()
    } else {
        (0..).map(letters).find(|l| !subs.contains(l)).unwrap_or_default()
    };
    let mut at = of + 1;
    while at < prj.pages.len() && sections::of_page(prj, at) == section && sub(&prj.pages[at].name).is_some() {
        at += 1;
    }
    let at = add_page(prj, PageKind::Schematic, at);
    let structure = crate::structure::Dt::of_page(&prj.pages[of]);
    let pg = &mut prj.pages[at];
    pg.name = format!("{main}.{next}");
    structure.apply_to(pg);
    at
}

/// `a` … `z`, `aa`, `ab` … for 0, 1, 2 …
fn letters(n: usize) -> String {
    let mut n = n + 1;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'a' + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Placement defaults without allocating an ID or changing the project.
pub fn symbol_preview(prj: &Project, def: &SymbolDef) -> SymbolInst {
    let mut s = SymbolInst {
        sym: def.key.into(), label: def.default_label.into(),
        pins: def.pins.iter().map(|x| x.label.to_string()).collect(), ..Default::default()
    };
    if let Some(u) = prj.symbols.iter().find(|u| u.key == def.key) {
        s.function_text.clone_from(&u.default_function_text);
        s.values.clone_from(&u.default_values);
    }
    s
}

/// Creates a symbol instance for a page with sensible defaults (tag in the page's
/// structure, numbering, ...).
pub fn new_symbol(prj: &mut Project, page: usize, ctx: &PlaceCtx, def: &SymbolDef, pos: P, rot: u8, mirror: bool) -> SymbolInst {
    let structure = crate::structure::page_structure(&prj.pages[page]);
    let mut s = SymbolInst {
        id: prj.new_id(),
        pos,
        rot,
        mirror,
        ..symbol_preview(prj, def)
    };
    match def.func {
        Func::Main { .. } => s.tag = prj.next_tag(&structure, def.letter),
        Func::Overview | Func::Auxiliary => s.tag = ctx.last_main.get(def.letter).cloned().unwrap_or_else(|| prj.next_tag(&structure, def.letter)),
        Func::Aux(kind) => {
            s.tag = ctx.last_main.get(def.letter).cloned().unwrap_or_else(|| prj.next_tag(&structure, def.letter));
            if def.auto_number {
                s.pins = next_contact_pins(prj, &s.tag, kind, None);
            }
        }
        Func::Terminal => {
            let letter = if def.letter.is_empty() { "X" } else { def.letter };
            s.tag = ctx.last_strip.clone().unwrap_or_else(|| format!("{structure}-{letter}1"));
            if def.default_label.chars().all(|c| c.is_ascii_digit()) {
                s.label = next_terminal(prj, &s.tag);
            }
        }
        Func::Plug(kind) => {
            s.tag = ctx.last_plug.clone().unwrap_or_else(|| prj.next_tag(&structure, def.letter));
            s.label = next_plug_pin(prj, &s.tag, kind);
        }
        Func::Plc => {
            if let Some(c) = &def.plc_channel {
                let previous = prj.latest_imported_card(&c.family, ctx.plc_article.as_deref())
                    .filter(|tag| !prj.channel_placed(tag, &c.family, c.input, c.index));
                s.tag = ctx.last_plc.get(def.key).map(String::as_str).or(previous).map(str::to_owned)
                    .unwrap_or_else(|| prj.next_tag(&structure, def.letter));
                s.label = c.address_for(prj, &s.tag);
                s.function_text = c.function_text.clone();
            } else if !def.plc_channels.is_empty() {
                s.tag=prj.next_tag(&structure,def.letter);
                s.label.clear();
            } else {
                s.tag = ctx.last_plc.get(def.key).cloned().unwrap_or_else(|| prj.next_tag(&structure, def.letter));
                s.label = next_plc_address(prj, &s.tag, !def.default_label.starts_with('Q'));
            }
        }
        Func::Potential | Func::Interruption | Func::Junction | Func::Graphic => {}
    }
    s
}

/// Area of a text as written (placeholders not filled in; `render::text_bounds`
/// knows the project).
pub fn text_bounds(t: &TextItem) -> R {
    crate::text::bounds(t, &t.text)
}

/// Bounding box of any item on a page.
pub fn item_bounds(symbols: &Symbols, page: &Page, id: u64) -> Option<R> {
    if let Some(s) = page.symbols.iter().find(|s| s.id == id) {
        return symbols.get(&s.sym).map(|d| d.world_bounds(s.pos, s.rot, s.mirror));
    }
    if let Some(w) = page.wires.iter().find(|w| w.id == id) {
        return Some(R::from_points(w.a, w.b));
    }
    if let Some(t) = page.texts.iter().find(|t| t.id == id) {
        return Some(text_bounds(t));
    }
    if let Some(s) = page.shapes.iter().find(|s| s.id == id) {
        return Some(crate::graphics::shape_bounds(s));
    }
    if let Some(d) = page.dimensions.iter().find(|d| d.id == id) {
        return Some(crate::graphics::dim_bounds(d));
    }
    if let Some(i) = page.images.iter().find(|i| i.id == id) {
        return Some(R::from_points(i.a, i.b));
    }
    if let Some(c) = page.conn_defs.iter().find(|c| c.id == id) {
        return Some(R::from_points(c.pos, c.pos).expand(1.0));
    }
    if let Some(j) = page.junctions.iter().find(|j| j.id == id) {
        return Some(R::from_points(j.pos, j.pos).expand(1.0));
    }
    page.cable_defs.iter().find(|c| c.id == id).map(|c| R::from_points(c.a, c.b))
}

pub fn all_ids(page: &Page) -> Vec<u64> {
    page.symbols
        .iter()
        .map(|s| s.id)
        .chain(page.wires.iter().map(|w| w.id))
        .chain(page.texts.iter().map(|t| t.id))
        .chain(page.shapes.iter().map(|s| s.id))
        .chain(page.dimensions.iter().map(|s| s.id))
        .chain(page.images.iter().map(|s| s.id))
        .chain(page.conn_defs.iter().map(|s| s.id))
        .chain(page.cable_defs.iter().map(|s| s.id))
        .chain(page.junctions.iter().map(|s| s.id))
        .collect()
}

/// Applies `f` to the points of the selected items other than symbols and wires.
fn map_items(page: &mut Page, sel: &BTreeSet<u64>, f: impl Fn(P) -> P) {
    for t in page.texts.iter_mut().filter(|t| sel.contains(&t.id)) {
        t.pos = f(t.pos);
    }
    for s in page.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
        crate::graphics::map_shape(s, &f);
    }
    for d in page.dimensions.iter_mut().filter(|d| sel.contains(&d.id)) {
        d.a = f(d.a);
        d.b = f(d.b);
    }
    for i in page.images.iter_mut().filter(|i| sel.contains(&i.id)) {
        let (a, b) = (f(i.a), f(i.b));
        let r = R::from_points(a, b);
        i.a = r.min;
        i.b = r.max;
    }
    for c in page.conn_defs.iter_mut().filter(|c| sel.contains(&c.id)) {
        c.pos = f(c.pos);
    }
    for c in page.cable_defs.iter_mut().filter(|c| sel.contains(&c.id)) {
        c.a = f(c.a);
        c.b = f(c.b);
    }
    for j in page.junctions.iter_mut().filter(|j| sel.contains(&j.id)) {
        j.pos = f(j.pos);
    }
}

/// World positions of all connection points of the selected symbols and the end
/// points of the selected wires.
pub fn selection_points(symbols: &Symbols, page: &Page, sel: &BTreeSet<u64>) -> Vec<P> {
    let mut pts = Vec::new();
    for s in page.symbols.iter().filter(|s| sel.contains(&s.id)) {
        if let Some(def) = symbols.get(&s.sym) {
            for i in 0..def.pins.len() {
                pts.push(def.pin_world(i, s.pos, s.rot, s.mirror));
            }
        }
    }
    for w in page.wires.iter().filter(|w| sel.contains(&w.id)) {
        pts.push(w.a);
        pts.push(w.b);
    }
    pts
}

/// Wire ends (wire id, is end `a`) of unselected wires touching the selection, so they
/// can be stretched ("rubber band") while moving.
pub fn attached_wire_ends(symbols: &Symbols, page: &Page, sel: &BTreeSet<u64>) -> Vec<(u64, bool)> {
    let pts = selection_points(symbols, page, sel);
    let mut v = Vec::new();
    for w in page.wires.iter().filter(|w| !sel.contains(&w.id)) {
        if pts.iter().any(|q| q.same(w.a)) {
            v.push((w.id, true));
        }
        if pts.iter().any(|q| q.same(w.b)) {
            v.push((w.id, false));
        }
    }
    v
}

pub fn translate(page: &mut Page, sel: &BTreeSet<u64>, attached: &[(u64, bool)], d: P) {
    // A T-junction that is not selected follows the point where its wires meet when
    // every wire there moves (the whole wire or that end).
    let moving_end = |w: &Wire, is_a: bool| sel.contains(&w.id) || attached.contains(&(w.id, is_a));
    let follow: Vec<u64> = page
        .junctions
        .iter()
        .filter(|j| !sel.contains(&j.id))
        .filter(|j| {
            let touching: Vec<&Wire> = page.wires.iter().filter(|w| w.a.same(j.pos) || w.b.same(j.pos) || on_segment_interior(j.pos, w.a, w.b)).collect();
            !touching.is_empty()
                && touching.iter().all(|w| if w.a.same(j.pos) { moving_end(w, true) } else if w.b.same(j.pos) { moving_end(w, false) } else { sel.contains(&w.id) })
        })
        .map(|j| j.id)
        .collect();
    for j in page.junctions.iter_mut().filter(|j| follow.contains(&j.id)) {
        j.pos = j.pos + d;
    }
    for s in page.symbols.iter_mut().filter(|s| sel.contains(&s.id)) {
        s.pos = s.pos + d;
    }
    for w in page.wires.iter_mut() {
        if sel.contains(&w.id) {
            w.a = w.a + d;
            w.b = w.b + d;
        } else {
            for &(id, is_a) in attached {
                if id == w.id {
                    if is_a {
                        w.a = w.a + d;
                    } else {
                        w.b = w.b + d;
                    }
                }
            }
        }
    }
    map_items(page, sel, |q| q + d);
}

/// Rotates the selection by 90° clockwise around its (grid snapped) centre.
pub fn rotate(symbols: &Symbols, page: &mut Page, sel: &BTreeSet<u64>) {
    let mut bounds = R::nothing();
    for &id in sel {
        if let Some(b) = item_bounds(symbols, page, id) {
            bounds.union(b);
        }
    }
    if !bounds.is_valid() {
        return;
    }
    // A single symbol rotates around its own origin.
    let single = page.symbols.iter().find(|s| sel.contains(&s.id)).filter(|_| sel.len() == 1);
    let c = single.map(|s| s.pos).unwrap_or_else(|| bounds.center().snap(GRID));
    let attached = attached_wire_ends(symbols, page, sel);
    let before = selection_points(symbols, page, sel);
    let rot = |q: P| c + (q - c).transform(1, false);
    for s in page.symbols.iter_mut().filter(|s| sel.contains(&s.id)) {
        s.pos = rot(s.pos);
        s.rot = (s.rot + 1) % 4;
    }
    for w in page.wires.iter_mut().filter(|w| sel.contains(&w.id)) {
        w.a = rot(w.a);
        w.b = rot(w.b);
    }
    for t in page.texts.iter_mut().filter(|t| sel.contains(&t.id)) {
        t.angle = (t.angle - 90.0).rem_euclid(360.0);
    }
    for j in page.junctions.iter_mut().filter(|j| sel.contains(&j.id)) {
        j.turn();
    }
    map_items(page, sel, rot);
    // Attached wire ends follow their connection point.
    for (id, is_a) in attached {
        if let Some(w) = page.wires.iter_mut().find(|w| w.id == id) {
            let e = if is_a { &mut w.a } else { &mut w.b };
            if before.iter().any(|q| q.same(*e)) {
                *e = rot(*e);
            }
        }
    }
}

pub fn mirror(page: &mut Page, sel: &BTreeSet<u64>) {
    for s in page.symbols.iter_mut().filter(|s| sel.contains(&s.id)) {
        s.mirror = !s.mirror;
    }
    for j in page.junctions.iter_mut().filter(|j| sel.contains(&j.id)) {
        j.mirror();
    }
}

pub fn delete(page: &mut Page, sel: &BTreeSet<u64>) {
    page.symbols.retain(|s| !sel.contains(&s.id));
    page.wires.retain(|w| !sel.contains(&w.id));
    page.texts.retain(|t| !sel.contains(&t.id));
    page.shapes.retain(|s| !sel.contains(&s.id));
    page.dimensions.retain(|s| !sel.contains(&s.id));
    page.images.retain(|s| !sel.contains(&s.id));
    page.conn_defs.retain(|s| !sel.contains(&s.id));
    page.cable_defs.retain(|s| !sel.contains(&s.id));
    page.junctions.retain(|s| !sel.contains(&s.id));
}

/// Removes zero length wires and merges exact duplicates.
pub fn cleanup_wires(page: &mut Page) {
    page.wires.retain(|w| !w.a.same(w.b));
    let mut seen = std::collections::HashSet::new();
    page.wires.retain(|w| {
        let (a, b) = (w.a.key(), w.b.key());
        let k = if a <= b { (a, b) } else { (b, a) };
        seen.insert(k)
    });
}

const CLIP_MAGIC: &str = "openecraft-clipboard-v1";

pub fn copy(page: &Page, sel: &BTreeSet<u64>) -> Clip {
    Clip {
        magic: CLIP_MAGIC.into(),
        anchor: None,
        function: page.function.clone(),
        installation: page.installation.clone(),
        location: page.location.clone(),
        product: page.product.clone(),
        symbols: page.symbols.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        symbol_defs: Default::default(),
        devices: BTreeMap::new(),
        parts: Vec::new(),
        wires: page.wires.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        texts: page.texts.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        shapes: page.shapes.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        dimensions: page.dimensions.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        images: page.images.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        conn_defs: page.conn_defs.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        cable_defs: page.cable_defs.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
        symbol_cache: Default::default(),
        junctions: page.junctions.iter().filter(|s| sel.contains(&s.id)).cloned().collect(),
    }
}

impl Clip {
    /// Resolve a portable definition before consulting the destination project.
    pub fn symbol(&self, project: &Project, key: &str) -> Option<crate::symbols::SymbolRef> {
        self.symbol_cache.table(&self.symbol_defs).get(key).or_else(|| project.symbol(key))
    }

    /// Full visible extent for previews; insertion still uses the clip's anchor.
    pub fn drawing_bounds(&self) -> R {
        let mut bounds = self.bounds();
        let symbols = self.symbol_cache.table(&self.symbol_defs);
        for instance in &self.symbols {
            if let Some(definition) = symbols.get(&instance.sym) {
                let b = definition.bounds;
                for q in [b.min, crate::geom::p(b.max.x,b.min.y), b.max, crate::geom::p(b.min.x,b.max.y)] {
                    bounds.extend(instance.pos + q.transform(instance.rot,instance.mirror));
                }
            }
        }
        bounds
    }
    /// Carry the definitions and catalog records used by these copied instances.
    pub fn include_project_data(&mut self, project: &Project) {
        for instance in &self.symbols {
            if !self.symbol_defs.iter().any(|s| s.key == instance.sym) {
                if let Some(definition) = project.symbols.iter().find(|s| s.key == instance.sym) {
                    self.symbol_defs.push(definition.clone());
                }
            }
            if let Some(device) = project.devices.get(&instance.tag) {
                self.devices.entry(instance.tag.clone()).or_insert_with(|| device.clone());
            }
        }
        for reference in self.devices.values().flat_map(|d| &d.parts) {
            if !self.parts.iter().any(|p| p.number == reference.number) {
                if let Some(part) = project.parts.iter().find(|p| p.number == reference.number) {
                    self.parts.push(part.clone());
                }
            }
        }
    }
    /// An empty clip (to be filled with items to insert).
    pub fn new() -> Clip {
        Clip { magic: CLIP_MAGIC.into(), ..Default::default() }
    }

    /// Structure of the page the clip was copied from.
    pub fn structure(&self) -> crate::structure::Dt {
        crate::structure::Dt {
            function: self.function.clone(),
            installation: self.installation.clone(),
            location: self.location.clone(),
            product: self.product.clone(),
            name: String::new(),
        }
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_text(s: &str) -> Option<Clip> {
        let c: Clip = serde_json::from_str(s).ok()?;
        (c.magic == CLIP_MAGIC).then_some(c)
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
            && self.wires.is_empty()
            && self.texts.is_empty()
            && self.shapes.is_empty()
            && self.dimensions.is_empty()
            && self.images.is_empty()
            && self.conn_defs.is_empty()
            && self.cable_defs.is_empty()
            && self.junctions.is_empty()
    }

    pub fn bounds(&self) -> R {
        let mut r = R::nothing();
        for s in &self.symbols {
            r.extend(s.pos);
        }
        for w in &self.wires {
            r.extend(w.a);
            r.extend(w.b);
        }
        for t in &self.texts {
            r.extend(t.pos);
        }
        for s in &self.shapes {
            r.union(crate::graphics::shape_bounds(s));
        }
        for d in &self.dimensions {
            r.extend(d.a);
            r.extend(d.b);
        }
        for i in &self.images {
            r.extend(i.a);
            r.extend(i.b);
        }
        for c in &self.conn_defs {
            r.extend(c.pos);
        }
        for c in &self.cable_defs {
            r.extend(c.a);
            r.extend(c.b);
        }
        for j in &self.junctions {
            r.extend(j.pos);
        }
        r
    }
}

/// How far [`paste`] moves the items of `clip` for the point `at`: the top left corner
/// goes to `at`, in steps of the connection point grid.
pub fn paste_offset(clip: &Clip, at: P) -> P {
    (at - clip.anchor.unwrap_or_else(|| clip.bounds().min)).snap(GRID)
}

fn paste_devices(prj:&mut Project,clip:&Clip,source:&crate::structure::Dt,target:&crate::structure::Dt,rename:&HashMap<String,String>) {
    let mapped=|tag:&str| {let full=target.complete(&source.relative(tag));rename.get(&full).cloned().unwrap_or(full)};
    for (tag,device) in &clip.devices {
        let mut device=device.clone();
        if let Some(owner)=device.values.get_mut("device.material_owner") {
            *owner=if clip.devices.contains_key(owner.as_str()) || clip.symbols.iter().any(|s|s.tag==*owner) {mapped(owner)} else {String::new()};
        }
        match prj.devices.entry(mapped(tag)) {
            std::collections::btree_map::Entry::Vacant(entry)=>{entry.insert(device);}
            std::collections::btree_map::Entry::Occupied(mut entry)=>{
                let incoming=entry.get_mut();
                if incoming.description.is_empty() {incoming.description=device.description;}
                for reference in device.parts {
                    if !incoming.parts.iter().any(|p|p.number==reference.number && p.total==reference.total) {incoming.parts.push(reference);}
                }
                for (key,value) in device.values {incoming.values.entry(key).or_insert(value);}
            }
        }
    }
}

/// Pastes the clip so that its top-left corner lands on `at`. Tags move into the
/// structure of the target page; device tags of main functions that already exist
/// get new numbers (consistently for coil + contacts). Wire numbers and cable cores
/// are not copied.
pub fn paste(prj: &mut Project, page_i: usize, clip: &Clip, at: P) -> BTreeSet<u64> {
    let source = clip.structure();
    let target = crate::structure::Dt::of_page(&prj.pages[page_i]);
    let b = clip.bounds();
    if !b.is_valid() {
        return BTreeSet::new();
    }
    let mut library_keys = HashMap::new();
    for definition in clip.symbol_defs.iter() {
        let mut incoming = definition.clone();
        let mut suffix = 0;
        loop {
            match prj.symbols.iter().find(|s| s.key == incoming.key) {
                Some(existing) if existing == &incoming => break,
                Some(_) => {
                    suffix += 1;
                    incoming.key = format!("{}_import_{}", definition.key, suffix);
                }
                None => {
                    prj.symbols.push(incoming.clone());
                    break;
                }
            }
        }
        library_keys.insert(definition.key.as_str(), incoming.key);
    }
    for part in &clip.parts {
        if !prj.parts.iter().any(|p| p.number == part.number) {
            let mut part = part.clone();
            if let Some(key) = library_keys.get(part.symbol.as_str()) { part.symbol = key.clone(); }
            prj.parts.push(part);
        }
    }
    let d = paste_offset(clip, at);
    let existing: BTreeSet<String> = prj.all_tags().into_iter().chain(prj.unplaced_tags().map(String::from)).collect();
    let mut reserved:BTreeSet<_>=clip.symbols.iter().map(|s|s.tag.as_str()).chain(clip.devices.keys().map(String::as_str))
        .filter(|tag|!tag.is_empty()).map(|tag|target.complete(&source.relative(tag))).collect();
    let mut rename: HashMap<String, String> = HashMap::new();
    let mut new_ids = BTreeSet::new();
    let mut symbols_out = Vec::new();
    for s in &clip.symbols {
        let mut s = s.clone();
        if let Some(key) = library_keys.get(s.sym.as_str()) {
            s.sym = key.clone();
        }
        s.id = prj.new_id();
        s.pos = s.pos + d;
        if !s.tag.is_empty() {
            s.tag = target.complete(&source.relative(&s.tag));
        }
        if let Some(def) = prj.symbol(&s.sym) {
            match def.func {
                Func::Main { .. } | Func::Aux(_) | Func::Auxiliary if existing.contains(&s.tag) => {
                    if let Some(n) = rename.get(&s.tag) {
                        s.tag = n.clone();
                    } else {
                        // The copy keeps the letters of its tag (`-Q1` → `-Q2`).
                        let dt = crate::structure::Dt::parse(&s.tag);
                        let letter = crate::structure::letters_of(&dt.name).unwrap_or_else(|| def.letter.to_string());
                        let n = prj.next_tag_reserved(&dt.structure(), &letter,&reserved);reserved.insert(n.clone());
                        // Reserve the tag by temporarily recording it.
                        rename.insert(s.tag.clone(), n.clone());
                        s.tag = n;
                    }
                }
                Func::Terminal if s.label.chars().all(|c| c.is_ascii_digit()) => {
                    s.label = next_terminal(prj, &s.tag);
                }
                Func::Plug(kind) if s.label.chars().all(|c| c.is_ascii_digit()) => {
                    s.label = next_plug_pin(prj, &s.tag, kind);
                }
                _ => {}
            }
        }
        new_ids.insert(s.id);
        // Push immediately so the next `next_tag` / numbering sees it.
        prj.pages[page_i].symbols.push(s.clone());
        symbols_out.push(s.id);
    }
    paste_devices(prj,clip,&source,&target,&rename);
    for w in &clip.wires {
        let mut w = w.clone();
        w.id = prj.new_id();
        w.a = w.a + d;
        w.b = w.b + d;
        w.number.clear();
        w.number_fixed = false;
        w.cable.clear();
        w.core.clear();
        new_ids.insert(w.id);
        prj.pages[page_i].wires.push(w);
    }
    for t in &clip.texts {
        let mut t = t.clone();
        t.id = prj.new_id();
        t.pos = t.pos + d;
        new_ids.insert(t.id);
        prj.pages[page_i].texts.push(t);
    }
    for s in &clip.shapes {
        let mut s = s.clone();
        s.id = prj.new_id();
        crate::graphics::map_shape(&mut s, |q| q + d);
        new_ids.insert(s.id);
        prj.pages[page_i].shapes.push(s);
    }
    for x in &clip.dimensions {
        let mut x = x.clone();
        x.id = prj.new_id();
        x.a = x.a + d;
        x.b = x.b + d;
        new_ids.insert(x.id);
        prj.pages[page_i].dimensions.push(x);
    }
    for x in &clip.images {
        let mut x = x.clone();
        x.id = prj.new_id();
        x.a = x.a + d;
        x.b = x.b + d;
        new_ids.insert(x.id);
        prj.pages[page_i].images.push(x);
    }
    // Connection definitions keep their values except the cable core.
    for x in &clip.conn_defs {
        let mut x = x.clone();
        x.id = prj.new_id();
        x.pos = x.pos + d;
        x.number.clear();
        x.cable.clear();
        x.core.clear();
        new_ids.insert(x.id);
        prj.pages[page_i].conn_defs.push(x);
    }
    for x in &clip.cable_defs {
        let mut x = x.clone();
        x.id = prj.new_id();
        x.a = x.a + d;
        x.b = x.b + d;
        new_ids.insert(x.id);
        prj.pages[page_i].cable_defs.push(x);
    }
    for x in &clip.junctions {
        let mut x = x.clone();
        x.id = prj.new_id();
        x.pos = x.pos + d;
        new_ids.insert(x.id);
        prj.pages[page_i].junctions.push(x);
    }
    new_ids
}

/// A device tag starts with its prefix (`-K1`, or with its structure `=A1+CAB1-K1`)
/// and holds no blanks or list separators; the message says what is wrong.
pub fn check_tag(tag: &str) -> Result<(), String> {
    if tag.trim().is_empty() {
        return Err(openecraft_i18n::tr("A device tag must not be empty").to_string());
    }
    if !tag.starts_with(['-', '=', '+']) {
        return Err(openecraft_i18n::tr_fmt!("Device tag {} must start with -, = or +", tag));
    }
    if tag.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, ';' | '|' | ',')) {
        return Err(openecraft_i18n::tr_fmt!("Device tag {} must not contain blanks or the separators ; | ,", tag));
    }
    Ok(())
}

/// Renames a device everywhere (all functions with that tag, device, strip, plug and
/// PLC card data, panel layout, harnesses, fluid device groups and links).
/// Why device `old` cannot be renamed to `new` (None: it can): an invalid or used tag,
/// or a function on a released page, which must stay as it is.
pub fn rename_problem(prj: &Project, old: &str, new: &str) -> Option<String> {
    if new == old {
        return None;
    }
    if let Err(e) = check_tag(new) {
        return Some(e);
    }
    if prj.all_tags().iter().any(|t| t == new) || prj.unplaced_tags().any(|t|t==new) {
        return Some(tr_fmt!("Device {} already exists", new));
    }
    let released = prj.pages.iter().find(|pg| pg.locked && pg.symbols.iter().any(|s| s.tag == old));
    released.map(|pg| tr_fmt!("{} is on page {}, which is released: renaming it would change the page", old, pg.name))
}

pub fn rename_device(prj: &mut Project, old: &str, new: &str) {
    if old == new || old.is_empty() {
        return;
    }
    for pg in &mut prj.pages {
        for s in &mut pg.symbols {
            if s.tag == old {
                s.tag = new.to_string();
            }
        }
    }
    if let Some(d) = prj.devices.remove(old) {
        prj.devices.entry(new.to_string()).or_insert(d);
    }
    for device in prj.devices.values_mut() {
        if device.values.get("device.material_owner").is_some_and(|v|v==old) {device.values.insert("device.material_owner".into(),new.into());}
    }
    for enclosure in &mut prj.layout.enclosures {
        if enclosure.device_tag==old {enclosure.device_tag=new.into();}
        if enclosure.owner_device==old {enclosure.owner_device=new.into();}
        for it in &mut enclosure.items {
            if it.tag == old {it.tag = new.to_string();}
            if it.owner_device==old {it.owner_device=new.into();}
        }
    }
    crate::harness::rename_device(prj, old, new);
    if let Some(d) = prj.strips.remove(old) {
        prj.strips.entry(new.to_string()).or_insert(d);
    }
    if let Some(d) = prj.plugs.remove(old) {
        prj.plugs.entry(new.to_string()).or_insert(d);
    }
    if let Some(d) = prj.plc_cards.remove(old) {
        prj.plc_cards.entry(new.to_string()).or_insert(d);
    }
    crate::plc_hw::rename_device(prj, old, new);
    crate::electrofluid::rename_device(prj, old, new);
    prj.field.rename(old, new);
    // Lists of device tags elsewhere, and the symbols of inactive options.
    let rename = |tags: &mut Vec<String>| {
        let mut changed = false;
        for t in tags.iter_mut().filter(|t| *t == old) {
            *t = new.to_string();
            changed = true;
        }
        changed
    };
    for l in &mut prj.pct_loops {
        rename(&mut l.devices);
    }
    for s in prj.electrical.safety.iter_mut().flat_map(|f| f.subsystems.iter_mut()) {
        rename(&mut s.devices);
        rename(&mut s.channel2);
    }
    for r in &mut prj.review.requests {
        if rename(&mut r.devices) {
            // A new revision of the record, so that shared projects take it over.
            r.rev += 1;
        }
    }
    for p in prj.model_options.options.iter_mut().flat_map(|o| o.parked.iter_mut()) {
        let symbols: Vec<&mut SymbolInst> = match &mut p.item {
            crate::model::options::Item::Symbol(s) => vec![s.as_mut()],
            crate::model::options::Item::Page(pg) => pg.symbols.iter_mut().collect(),
            _ => Vec::new(),
        };
        for s in symbols.into_iter().filter(|s| s.tag == old) {
            s.tag = new.to_string();
        }
    }
}

/// Deletes a page; released pages and the last page stay.
pub fn delete_page(prj: &mut Project, page: usize) -> Result<Page, &'static str> {
    if prj.pages[page].locked {
        return Err(tr_const!("Released pages cannot be deleted"));
    }
    if prj.pages.len() == 1 {
        return Err(tr_const!("The last page cannot be deleted"));
    }
    Ok(prj.pages.remove(page))
}

/// Adds a cable with the next free tag (`W`) in `structure`. Returns its tag.
pub fn add_cable(prj: &mut Project, structure: &str) -> String {
    let tag = prj.next_tag(structure, "W");
    let id = prj.new_id();
    prj.cables.push(Cable { id, tag: tag.clone(), cores: "BN BK GY GNYE".into(), cross_section: "1.5 mm²".into(), ..Default::default() });
    tag
}

/// The wires of cable `old` move to `new` (after the cable was renamed).
pub fn rename_cable(prj: &mut Project, old: &str, new: &str) {
    for w in prj.pages.iter_mut().flat_map(|pg| pg.wires.iter_mut()).filter(|w| w.cable == old) {
        w.cable = new.to_string();
    }
    for c in prj.plc.connections.iter_mut().filter(|c| c.cable == old) {
        c.cable = new.to_string();
    }
    prj.field.rename(old, new);
}

/// Deletes a cable; its wires lose cable and core.
pub fn remove_cable(prj: &mut Project, index: usize) -> Cable {
    let cable = prj.cables.remove(index);
    for w in prj.pages.iter_mut().flat_map(|pg| pg.wires.iter_mut()).filter(|w| w.cable == cable.tag) {
        w.cable.clear();
        w.core.clear();
    }
    for c in prj.plc.connections.iter_mut().filter(|c| c.cable == cable.tag) {
        c.cable.clear();
    }
    cable
}

/// Takes part number, type, cores and cross section of a cable part.
pub fn apply_cable_part(cable: &mut Cable, part: &Part) {
    cable.part = part.number.clone();
    cable.cable_type = part.type_number.clone();
    if let Some(cores) = part.tech_value("Cores") {
        cable.cores = cores.to_string();
    }
    if let Some(cs) = part.tech_value("Cross-section") {
        cable.cross_section = cs.to_string();
    }
}

/// Assigns wires of a page to the free cores of a cable in order (left to right,
/// top to bottom). The wires of one connection (joined at their ends) share a core,
/// also one they already have of the cable. Returns the number of wires that got a
/// core.
pub fn assign_cores(prj: &mut Project, page: usize, wires: &BTreeSet<u64>, cable: &str) -> usize {
    let Some(c) = prj.cable(cable) else { return 0 };
    let cores = c.core_list();
    let used: Vec<String> = prj
        .pages
        .iter()
        .enumerate()
        .flat_map(|(pi, pg)| pg.wires.iter().map(move |w| (pi, w)))
        .filter(|(pi, w)| w.cable == cable && !(*pi == page && wires.contains(&w.id)))
        .map(|(_, w)| w.core.clone())
        .collect();
    let mut free = cores.into_iter().filter(|c| !used.contains(c));
    let group = wire_groups(&prj.pages[page].wires);
    let mut core_of: HashMap<usize, String> = HashMap::new();
    for (i, w) in prj.pages[page].wires.iter().enumerate() {
        if w.cable == cable && !w.core.is_empty() && !wires.contains(&w.id) {
            core_of.entry(group[i]).or_insert_with(|| w.core.clone());
        }
    }
    let mut sel: Vec<(usize, &mut Wire)> = prj.pages[page].wires.iter_mut().enumerate().filter(|(_, w)| wires.contains(&w.id)).collect();
    sel.sort_by(|(_, a), (_, b)| a.a.x.min(a.b.x).total_cmp(&b.a.x.min(b.b.x)).then(a.a.y.min(a.b.y).total_cmp(&b.a.y.min(b.b.y))));
    let mut n = 0;
    for (i, w) in sel {
        let core = match core_of.get(&group[i]) {
            Some(core) => core.clone(),
            None => {
                let Some(core) = free.next() else { continue };
                core_of.insert(group[i], core.clone());
                core
            }
        };
        w.cable = cable.to_string();
        w.core = core;
        n += 1;
    }
    n
}

/// Connection of each wire: wires joined at an end or ending on another one share
/// one (index of a representative wire).
fn wire_groups(wires: &[Wire]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..wires.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut at_end: HashMap<(i64, i64), usize> = HashMap::new();
    for (i, w) in wires.iter().enumerate() {
        for q in [w.a, w.b] {
            let j = *at_end.entry(q.key()).or_insert(i);
            let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
            parent[ri] = rj;
            for (k, v) in wires.iter().enumerate() {
                if k != i && crate::geom::on_segment_interior(q, v.a, v.b) {
                    let (ri, rk) = (root(&mut parent, i), root(&mut parent, k));
                    parent[ri] = rk;
                }
            }
        }
    }
    (0..wires.len()).map(|i| root(&mut parent, i)).collect()
}

/// Changes the structure of a page; with `move_devices` the devices on the page
/// move along (all their functions, also on other pages, see [`rename_device`]).
pub fn set_page_structure(prj: &mut Project, page: usize, structure: &crate::structure::Dt, move_devices: bool) {
    let old = prj.pages[page].clone();
    structure.apply_to(&mut prj.pages[page]);
    if !move_devices {
        return;
    }
    let new = prj.pages[page].clone();
    let mut tags: Vec<String> = old.symbols.iter().filter(|s| !s.tag.is_empty()).map(|s| s.tag.clone()).collect();
    tags.sort();
    tags.dedup();
    for tag in tags {
        let moved = crate::structure::complete(&new, &crate::structure::relative(&old, &tag));
        if moved != tag && !prj.all_tags().contains(&moved) {
            rename_device(prj, &tag, &moved);
        }
    }
}

#[cfg(test)]
mod tests;
