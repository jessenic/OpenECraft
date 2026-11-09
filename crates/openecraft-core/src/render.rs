//! Backend independent page rendering. The same code draws to the egui canvas and
//! to SVG export, so what you see is what you print.

use openecraft_i18n::{tr, tr_fmt};
use crate::geom::{p, P, R};
use crate::index::{Index, Loc};
use crate::model::*;
use crate::netlist;
use crate::panel::{self, Enclosure, Face, ItemKind, PanelItem};
use crate::parts::{Model2d, Prim2d};
use crate::parts::{self, FnKind};
use crate::analysis::Analysis;
use crate::{graphics, structure, text, wirenum};
use crate::symbols::{Anchor, BlockRow, ContactKind, FieldKind, Func, Prim, SymbolDef, SymbolRef};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Line,
    Wire,
    Text,
    Frame,
    FrameText,
    Selected,
    Highlight,
    Ghost,
    Pin,
    CrossRef,
    Error,
    /// Light fill of mounting plates.
    Plate,
    /// Fill of devices / ducts in mounting layouts.
    Fill,
    /// Wire drawn in its own colour (RGB).
    Color([u8; 3]),
    /// Paper colour (opaque backgrounds of texts).
    Paper,
}

pub use crate::model::VAnchor;
pub use crate::text::TextStyle;
pub trait Draw {
    fn line(&mut self, a: P, b: P, width: f32, ink: Ink, dashed: bool);
    fn poly(&mut self, pts: &[P], closed: bool, width: f32, ink: Ink, fill: bool);
    fn circle(&mut self, c: P, r: f32, width: f32, ink: Ink, fill: bool);
    fn text(&mut self, pos: P, text: &str, size: f32, h: Anchor, v: VAnchor, ink: Ink);
    /// Text with rotation and style (free texts); back ends that cannot draw them
    /// show the plain text.
    fn styled_text(&mut self, pos: P, text: &str, style: &TextStyle, ink: Ink) {
        self.text(pos, text, style.size, style.h, style.v, ink);
    }
    fn rect(&mut self, a: P, b: P, width: f32, ink: Ink, fill: bool) {
        self.poly(&[a, p(b.x, a.y), b, p(a.x, b.y)], true, width, ink, fill);
    }
    /// Marks the area `min`..`max` as a link to another page (PDF export).
    fn link(&mut self, _min: P, _max: P, _page: usize) {}
    /// Marks the area `min`..`max` as a hyperlink (PDF and SVG export).
    fn link_url(&mut self, _min: P, _max: P, _url: &str) {}
    /// Draws a bitmap; back ends that cannot show it draw a frame with its name.
    fn image(&mut self, img: &ImageItem) {
        image_placeholder(self, img);
    }
    /// Bitmap on an affine quadrilateral, used by rotated and mirrored symbols.
    fn image_quad(&mut self, img: &ImageItem, corners: &[P; 4]) {
        let mut image = img.clone();
        let mut bounds = R::from_points(corners[0], corners[2]);
        for corner in corners { bounds.extend(*corner); }
        image.a = bounds.min;
        image.b = bounds.max;
        self.image(&image);
    }
    /// Layer of the following elements (DXF layers); None = by ink.
    fn layer(&mut self, _name: Option<&str>) {}
}

/// Frame with crossed diagonals and the file name in place of a bitmap.
pub fn image_placeholder<D: Draw + ?Sized>(d: &mut D, img: &ImageItem) {
    let r = R::from_points(img.a, img.b);
    d.rect(r.min, r.max, SYMBOL_W, Ink::Frame, false);
    d.line(r.min, r.max, SYMBOL_W * 0.5, Ink::Frame, false);
    d.line(p(r.min.x, r.max.y), p(r.max.x, r.min.y), SYMBOL_W * 0.5, Ink::Frame, false);
    d.text(r.center(), &img.file.name, 2.0, Anchor::Center, VAnchor::Middle, Ink::FrameText);
}

/// Estimated width of a text.
fn text_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.62
}

/// Links the area of a text (estimated from its length) to a page.
fn link_text(d: &mut dyn Draw, pos: P, text: &str, size: f32, h: Anchor, page: usize) {
    let w = text_width(text, size);
    let x0 = match h {
        Anchor::Left => pos.x,
        Anchor::Center => pos.x - w / 2.0,
        Anchor::Right => pos.x - w,
    };
    d.link(p(x0, pos.y - size * 0.7), p(x0 + w, pos.y + size * 0.7), page);
}

mod report_graphics;

pub const SYMBOL_W: f32 = 0.25;
pub const WIRE_W: f32 = 0.35;
pub const TAG_SIZE: f32 = 2.5;
pub const SMALL_SIZE: f32 = 1.8;

#[derive(Default)]
pub struct Opts {
    pub selected: BTreeSet<u64>,
    pub highlight_wires: HashSet<u64>,
    pub show_pins: bool,
    /// Revision marks for changes since the last revision are drawn unless hidden.
    pub hide_revision_marks: bool,
    /// Draw the review layer (comments, redline / greenline markup).
    pub review: bool,
    pub review_selected: Option<crate::review::Hit>,
    /// Drawn on screen: layers that are not printed are shown.
    pub screen: bool,
    /// Draw the macro boxes (on screen only).
    pub macro_boxes: bool,
}

/// Draws a complete page including frame and title block.
pub fn draw_page(d: &mut dyn Draw, prj: &Project, idx: &Index, an: &Analysis, page_i: usize, opts: &Opts) {
    let page = &prj.pages[page_i];
    draw_frame(d, prj, page_i);
    let frame = crate::frame::of_page(prj, page_i);
    match page.kind {
        PageKind::Cover => draw_cover(d, prj, &frame),
        PageKind::Report(kind) => {
            if let Some(t) = crate::report_blocks::page_table(prj, an, page_i) {
                if let Some(layout) = &t.layout {
                    crate::forms::draw_layout(d, layout, &t, prj, page_i, frame.area().min);
                } else if !(t.graphic && report_graphics::draw(d, kind, &t, &frame)) {
                    draw_report(d, &t, &frame);
                }
            }
        }
        PageKind::Mounting(id) => draw_mounting_page(d, prj, id, &frame),
        PageKind::Document => crate::document::draw(d, &frame, page.document.as_ref()),
        PageKind::Schematic | PageKind::Overview | PageKind::Graphic => {}
    }
    let shown = |layer: &str| graphics::shown(prj, layer, !opts.screen);
    let item_ink = |id: u64, own: Option<[u8; 3]>, layer: &str, normal: Ink| {
        if opts.selected.contains(&id) {
            Ink::Selected
        } else {
            own.or_else(|| graphics::color(prj, layer)).map(Ink::Color).unwrap_or(normal)
        }
    };
    for img in page.images.iter().filter(|i| shown(&i.layer)) {
        d.layer(Some(&img.layer));
        d.image(img);
        if opts.selected.contains(&img.id) {
            d.rect(img.a, img.b, SYMBOL_W, Ink::Selected, false);
        }
    }
    for s in page.shapes.iter().filter(|s| shown(&s.layer)) {
        d.layer(Some(&s.layer));
        draw_shape(d, s, item_ink(s.id, s.color, &s.layer, Ink::Line));
    }
    for dm in page.dimensions.iter().filter(|x| shown(&x.layer)) {
        d.layer(Some(&dm.layer));
        draw_dimension(d, dm, item_ink(dm.id, None, &dm.layer, Ink::Line));
    }
    d.layer(None);
    let colors = if prj.wire_colors && page.kind == PageKind::Schematic {
        crate::potentials::wire_colors(prj, &an.netlist, &an.open)
    } else {
        Default::default()
    };
    for w in &page.wires {
        let ink = if opts.selected.contains(&w.id) {
            Ink::Selected
        } else if opts.highlight_wires.contains(&w.id) {
            Ink::Highlight
        } else {
            Ink::Wire
        };
        let parts = colors.get(&w.id).map(|c| crate::potentials::color_parts(c)).unwrap_or_default();
        match parts.as_slice() {
            // Black too: the canvas shows it in its line colour when it is dark.
            [c] if ink == Ink::Wire => d.line(w.a, w.b, WIRE_W, Ink::Color(*c), false),
            // Two-colour wires: the second colour dashed over the first.
            [a, b] if ink == Ink::Wire => {
                d.line(w.a, w.b, WIRE_W, Ink::Color(*a), false);
                d.line(w.a, w.b, WIRE_W, Ink::Color(*b), true);
            }
            _ => d.line(w.a, w.b, WIRE_W, ink, false),
        }
    }
    // A directed T-junction replaces the dot where it fits its wires.
    let directed: Vec<P> = page.junctions.iter().filter(|j| !j.dot && !j.is_corner() && netlist::junctions::fits(prj, page, j)).map(|j| j.pos).collect();
    for j in netlist::junctions(prj, page) {
        if !directed.iter().any(|q| q.same(j)) {
            d.circle(j, 0.7, 0.0, Ink::Wire, true);
        }
    }
    for j in &page.junctions {
        let ink = if opts.selected.contains(&j.id) { Ink::Selected } else { Ink::Wire };
        draw_junction(d, prj, page, j, ink);
    }
    draw_wire_labels(d, prj, an, page_i);
    let structured = !structure::page_structure(page).is_empty();
    for s in &page.symbols {
        let Some(def) = prj.symbol(&s.sym) else { continue };
        let ink = if opts.selected.contains(&s.id) { Ink::Selected } else { Ink::Line };
        let tag = if continues_strip(prj, page_i, &def, s) {
            String::new()
        } else if structured {
            structure::relative(page, &s.tag)
        } else {
            s.tag.clone()
        };
        let property = |key: &str| crate::props::symbol_property_text(prj, page_i, s, key);
        if tag != s.tag {
            draw_symbol_resolved(d, &def, &SymbolInst { tag, ..s.clone() }, ink, &property);
        } else {
            draw_symbol_resolved(d, &def, s, ink, &property);
        }
        draw_annotations(d, prj, idx, page_i, &def, s, ink);
        if opts.show_pins {
            for i in 0..def.pins.len() {
                d.circle(def.pin_world(i, s.pos, s.rot, s.mirror), 0.6, 0.15, Ink::Pin, false);
            }
        }
    }
    for cd in &page.cable_defs {
        let ink = if opts.selected.contains(&cd.id) { Ink::Selected } else { Ink::Line };
        draw_cable_def(d, prj, page, cd, ink);
    }
    for cd in &page.conn_defs {
        let ink = if opts.selected.contains(&cd.id) { Ink::Selected } else { Ink::Line };
        draw_conn_def(d, page, cd, ink);
    }
    for t in page.texts.iter().filter(|t| shown(&t.layer)) {
        d.layer(Some(&t.layer));
        draw_text_item(d, prj, page_i, t, item_ink(t.id, t.color, &t.layer, Ink::Text));
    }
    d.layer(None);
    if !opts.hide_revision_marks {
        draw_revision_marks(d, prj, an, page_i);
    }
    if opts.review {
        crate::review::draw(d, prj, page_i, opts.review_selected);
    }
    if opts.screen && opts.macro_boxes {
        for b in &page.macro_boxes {
            if let Some(r) = crate::macro_boxes::bounds(prj, page_i, b) {
                let (a, c) = (p(r.min.x - 2.0, r.min.y - 4.5), p(r.max.x + 2.0, r.max.y + 2.0));
                d.poly(&[a, p(c.x, a.y), c, p(a.x, c.y)], true, 0.2, Ink::Ghost, false);
                d.line(a, p(c.x, a.y), 0.2, Ink::Ghost, true);
                d.text(p(a.x + 1.0, a.y + 2.2), &b.name, 2.0, Anchor::Left, VAnchor::Middle, Ink::Ghost);
            }
        }
    }
}

/// The text of a free text as shown: placeholders such as `{page}` or `{prop:Name}`
/// (special texts) are filled in.
pub fn text_shown(prj: &Project, page_i: usize, t: &TextItem) -> String {
    if t.text.contains('{') {
        crate::forms::fill(&t.text, prj, page_i)
    } else {
        t.text.clone()
    }
}

/// Area of a free text as shown (picking).
pub fn text_bounds(prj: &Project, page_i: usize, t: &TextItem) -> R {
    text::bounds(t, &text_shown(prj, page_i, t))
}

/// Draws a free text: background and frame, its lines with rotation, alignment and
/// style, and its hyperlink.
pub fn draw_text_item(d: &mut dyn Draw, prj: &Project, page_i: usize, t: &TextItem, ink: Ink) {
    let shown = text_shown(prj, page_i, t);
    if t.background || t.frame {
        let c = text::corners(t, &shown, 0.25);
        if t.background {
            d.poly(&c, true, 0.0, Ink::Paper, true);
        }
        if t.frame {
            d.poly(&c, true, SYMBOL_W, if ink == Ink::Text { Ink::Line } else { ink }, false);
        }
    }
    let style = TextStyle::of(t);
    for (q, line) in text::lines(t, &shown) {
        d.styled_text(q, &line, &style, ink);
    }
    if openecraft_parts::api::is_web_link(&t.link) {
        let b = text::bounds(t, &shown);
        d.link_url(b.min, b.max, &t.link);
    }
}

/// Triangle with the upcoming revision index at every item changed since the last
/// revision (at the title block for changed page properties / new pages).
fn draw_revision_marks(d: &mut dyn Draw, prj: &Project, an: &Analysis, page_i: usize) {
    let Some(pc) = an.changes.iter().find(|c| c.page == Some(page_i)) else { return };
    let name = crate::revision::next_name(prj);
    let mut done: Vec<(i64, i64)> = Vec::new();
    for c in &pc.changes {
        let at = if c.what == "Page" { p(203.0, 262.0) } else { c.pos + p(3.5, -3.5) };
        // One mark per spot (e.g. a moved wire split into several).
        if done.contains(&at.key()) {
            continue;
        }
        done.push(at.key());
        let ink = Ink::Highlight;
        d.poly(&[at + p(0.0, -2.4), at + p(2.2, 1.4), at + p(-2.2, 1.4)], true, 0.2, ink, false);
        d.text(at + p(0.0, 0.3), &name, 1.5, Anchor::Center, VAnchor::Middle, ink);
    }
}

pub fn draw_shape(d: &mut dyn Draw, s: &Shape, ink: Ink) {
    let width = if s.width > 0.0 { s.width } else { SYMBOL_W };
    let (pts, closed) = graphics::outline(s);
    if pts.len() < 2 {
        return;
    }
    if s.fill && closed && ink != Ink::Ghost {
        d.poly(&pts, true, 0.0, ink, true);
    }
    if s.dashed || matches!(s.kind, ShapeKind::Line | ShapeKind::Rect) {
        let n = if closed { pts.len() } else { pts.len() - 1 };
        for i in 0..n {
            d.line(pts[i], pts[(i + 1) % pts.len()], width, ink, s.dashed);
        }
    } else {
        d.poly(&pts, closed, width, ink, false);
    }
}

/// Dimension line with arrows, extension lines and the measured value.
pub fn draw_dimension(d: &mut dyn Draw, dm: &Dimension, ink: Ink) {
    let g = graphics::dim_geom(dm);
    let w = SYMBOL_W * 0.7;
    d.line(g.line.0, g.line.1, w, ink, false);
    for (a, b) in g.ext {
        d.line(a, b, w, ink, false);
    }
    if g.line.0.dist(g.line.1) > graphics::ARROW * 1.5 {
        for a in g.arrows {
            d.poly(&a, true, 0.0, ink, true);
        }
    }
    let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
    let text = graphics::dim_text(dm);
    match g.text_side {
        Some(right) => {
            let anchor = if right { Anchor::Left } else { Anchor::Right };
            d.text(g.text_pos, &text, TAG_SIZE, anchor, VAnchor::Middle, text_ink);
        }
        None if g.text_below => d.text(g.text_pos + p(0.0, TAG_SIZE * 0.6), &text, TAG_SIZE, Anchor::Center, VAnchor::Middle, text_ink),
        None => d.text(g.text_pos, &text, TAG_SIZE, Anchor::Center, VAnchor::Bottom, text_ink),
    }
}

/// The directional symbol of a T-junction: a diagonal from its source to its second
/// target (the first target lies straight on); a dot when it is drawn as one. A
/// corner that fits its wires is the bend of the wires and draws nothing. One that
/// does not fit its wires is drawn in the error colour with its arms. The selected one
/// gets a ring.
pub fn draw_junction(d: &mut dyn Draw, prj: &Project, page: &Page, j: &Junction, ink: Ink) {
    let fits = netlist::junctions::fits(prj, page, j);
    let mark = if fits { ink } else { Ink::Error };
    // The dot of a fitting one is the connection dot the wires get anyway.
    junction_symbol(d, j, mark, !fits || !(j.dot || j.is_corner()), ink == Ink::Selected || !fits);
}

/// The symbol of a connection symbol at its position: the diagonal of a T-junction
/// from its source to its second target, or a dot; the two short arms of a corner;
/// with `ring` a circle around it.
pub fn junction_symbol(d: &mut dyn Draw, j: &Junction, ink: Ink, symbol: bool, ring: bool) {
    if symbol && j.is_corner() {
        for a in j.arms() {
            d.line(j.pos, j.pos + a.vector() * JUNCTION_R, WIRE_W, ink, false);
        }
    } else if symbol && j.dot {
        d.circle(j.pos, 0.7, 0.0, ink, true);
    } else if symbol {
        // The first target lies straight on, the others get a diagonal.
        for (a, b) in j.links().into_iter().skip(1) {
            d.line(j.pos + a.vector() * JUNCTION_R, j.pos + b.vector() * JUNCTION_R, WIRE_W, ink, false);
        }
    }
    if ring {
        d.circle(j.pos, JUNCTION_R * 0.7, SYMBOL_W, ink, false);
    }
}

/// Reach of the diagonal of a T-junction (mm).
pub const JUNCTION_R: f32 = 2.0;

/// Diamond on the wire and the defined values beside it.
fn draw_conn_def(d: &mut dyn Draw, page: &Page, cd: &ConnDef, ink: Ink) {
    let q = cd.pos;
    let r = 0.9;
    d.poly(&[q + p(0.0, -r), q + p(r, 0.0), q + p(0.0, r), q + p(-r, 0.0)], true, SYMBOL_W, ink, ink != Ink::Line);
    let text = crate::conndef::label(cd);
    if text.is_empty() {
        return;
    }
    let text_ink = if ink == Ink::Line { Ink::CrossRef } else { ink };
    let vertical = crate::conndef::wire_at(page, q).is_some_and(|w| (w.b.x - w.a.x).abs() < (w.b.y - w.a.y).abs());
    if vertical {
        d.text(q + p(1.6, 0.0), &text, SMALL_SIZE, Anchor::Left, VAnchor::Middle, text_ink);
    } else {
        d.text(q + p(0.0, 3.4), &text, SMALL_SIZE, Anchor::Center, VAnchor::Bottom, text_ink);
    }
}

/// Cable definition line, shield and the cable data at its end.
fn draw_cable_def(d: &mut dyn Draw, prj: &Project, page: &Page, cd: &CableDef, ink: Ink) {
    d.line(cd.a, cd.b, SYMBOL_W, ink, false);
    let v = cd.b - cd.a;
    let len = v.len();
    if len < 0.1 {
        return;
    }
    let u = v * (1.0 / len);
    if cd.shield {
        let c = cd.a + v * 0.5;
        let n = p(-u.y, u.x);
        let pts: Vec<P> = (0..48)
            .map(|i| {
                let t = (i as f32 / 48.0) * std::f32::consts::TAU;
                c + u * (t.cos() * len * 0.42) + n * (t.sin() * 1.6)
            })
            .collect();
        d.poly(&pts, true, SYMBOL_W * 0.7, ink, false);
    }
    let text_ink = if ink == Ink::Line { Ink::CrossRef } else { ink };
    let lines = crate::cabledef::label(prj, page, cd);
    let horizontal = u.x.abs() >= u.y.abs();
    for (i, t) in lines.iter().enumerate() {
        let size = if i == 0 { TAG_SIZE } else { SMALL_SIZE };
        if horizontal {
            let anchor = if u.x >= 0.0 { Anchor::Left } else { Anchor::Right };
            let q = cd.b + u * 1.2 + p(0.0, i as f32 * 3.0 - (lines.len() as f32 - 1.0) * 1.5);
            d.text(q, t, size, anchor, VAnchor::Middle, text_ink);
        } else {
            let q = cd.b + u * 2.0 + p(0.0, if u.y >= 0.0 { i as f32 * 3.0 + 1.0 } else { -((lines.len() - 1 - i) as f32) * 3.0 - 1.0 });
            d.text(q, t, size, Anchor::Center, VAnchor::Middle, text_ink);
        }
    }
}

/// Draws only the symbol graphics (used for canvas, ghost and library previews).
pub fn draw_symbol(d: &mut dyn Draw, def: &SymbolDef, s: &SymbolInst, ink: Ink) {
    draw_symbol_resolved(d, def, s, ink, &|key| s.values.get(key).cloned().unwrap_or_default());
}

fn draw_symbol_resolved(d: &mut dyn Draw, def: &SymbolDef, s: &SymbolInst, ink: Ink, property: &dyn Fn(&str) -> String) {
    if s.pins.len() < def.pins.len() {
        let mut preview = s.clone();
        for pin in def.pins.iter().skip(preview.pins.len()) {
            preview.pins.push(pin.label.into());
        }
        draw_prims_resolved(d, &def.prims, &preview, ink, property);
    } else {
        draw_prims_resolved(d, &def.prims, s, ink, property);
    }
}

/// Symbol graphics placed like the instance `s` (also used by the symbol editor).
pub fn draw_prims(d: &mut dyn Draw, prims: &[Prim], s: &SymbolInst, ink: Ink) {
    draw_prims_resolved(d, prims, s, ink, &|key| s.values.get(key).cloned().unwrap_or_default());
}

fn field_text(s: &SymbolInst, kind: FieldKind) -> String {
    match kind {
        FieldKind::Label => s.label.clone(),
        FieldKind::FunctionText => s.function_text.clone(),
        FieldKind::Tag => s.tag.clone(),
        FieldKind::TagName => crate::structure::Dt::parse(&s.tag).name,
        FieldKind::Pin(i) => s.pins.get(i).cloned().unwrap_or_default(),
        FieldKind::Product(levels) => {
            let dt = crate::structure::Dt::parse(&s.tag);
            let product = if dt.product.is_empty() { dt.name } else { format!("-{}{}", dt.product, dt.name) };
            let start = product.rmatch_indices('-').nth(levels.saturating_sub(1)).map_or(0, |(i, _)| i);
            product[start..].to_owned()
        }
    }
}

fn draw_prims_resolved(d: &mut dyn Draw, prims: &[Prim], s: &SymbolInst, ink: Ink, property: &dyn Fn(&str) -> String) {
    let tf = |q: P| s.pos + q.transform(s.rot, s.mirror);
    let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
    for prim in prims {
        match prim {
            Prim::PropertyBlock(block) => {
                let values: Vec<_> = block.rows.iter().map(|row| match row {
                    BlockRow::Property(key) => property(key),
                    BlockRow::Field(kind) => field_text(s, *kind),
                    BlockRow::Text(text) => text.clone(),
                }).collect();
                let lines: Vec<_> = values.iter().filter(|value| !value.is_empty()).flat_map(|value| value.split('\n')).collect();
                let pen = if ink == Ink::Line { block.color.map(Ink::Color).unwrap_or(text_ink) } else { text_ink };
                let style = TextStyle::plain(block.size, block.anchor, VAnchor::Middle);
                for (i, line) in lines.iter().enumerate() {
                    draw_symbol_text(d, s, block.row_pos(i, lines.len()), line.trim_end_matches('\r'), &style, Some(block.angle), pen);
                }
            }
            Prim::Line { a, b, dashed } => d.line(tf(*a), tf(*b), SYMBOL_W, ink, *dashed),
            Prim::Poly { pts, closed, fill } => {
                let v: Vec<P> = pts.iter().map(|q| tf(*q)).collect();
                d.poly(&v, *closed, SYMBOL_W, ink, *fill);
            }
            Prim::Circle { c, r, fill, color } => d.circle(tf(*c), *r, SYMBOL_W, if ink == Ink::Line {color.map(Ink::Color).unwrap_or(ink)} else {ink}, *fill),
            Prim::Image { name, data, corners } => {
                let world = corners.map(tf);
                let image = ImageItem {
                    a: world[0], b: world[2],
                    file: crate::document::Document { name: name.clone(), data: data.clone() },
                    ..Default::default()
                };
                d.image_quad(&image, &world);
            }
            Prim::Path { pts, closed, fill, width, color, dashed } => {
                let pen = if ink==Ink::Line {color.map(Ink::Color).unwrap_or(ink)} else {ink};
                let points: Vec<_> = pts.iter().map(|q| tf(*q)).collect();
                if *dashed && !*fill {
                    for pair in points.windows(2) { d.line(pair[0],pair[1],*width,pen,true); }
                    if *closed && points.len()>2 { d.line(points[points.len()-1],points[0],*width,pen,true); }
                } else {
                    d.poly(&points,*closed,*width,pen,*fill);
                }
            }
            Prim::Text { pos, text, size, anchor, angle, color } => {
                let text_ink = if ink==Ink::Line {color.map(Ink::Color).unwrap_or(text_ink)} else {text_ink};
                draw_symbol_text(d, s, *pos, text, &TextStyle::plain(*size, *anchor, VAnchor::Middle), *angle, text_ink);
            }
            Prim::Field { kind, pos, size, anchor, angle } => {
                let name = match kind {
                    FieldKind::TagName => crate::structure::Dt::parse(&s.tag).name,
                    FieldKind::Product(levels) => {
                        let dt = crate::structure::Dt::parse(&s.tag);
                        let product = if dt.product.is_empty() { dt.name } else { format!("-{}{}", dt.product, dt.name) };
                        let start = product.rmatch_indices('-').nth(levels.saturating_sub(1)).map_or(0, |(i, _)| i);
                        product[start..].to_owned()
                    }
                    _ => String::new(),
                };
                let t = match kind {
                    FieldKind::Label => &s.label,
                    FieldKind::FunctionText => &s.function_text,
                    FieldKind::Tag => &s.tag,
                    FieldKind::TagName | FieldKind::Product(_) => &name,
                    FieldKind::Pin(index) => {
                        s.pins.get(*index).map_or("", String::as_str)
                    }
                };
                if !t.is_empty() {
                    if *kind == FieldKind::FunctionText {
                        draw_symbol_text(d, s, *pos, t, &TextStyle::plain(*size, *anchor, VAnchor::Middle), *angle, text_ink);
                        continue;
                    }
                    if let Some(angle) = angle {
                        let mut style = TextStyle::plain(*size, *anchor, VAnchor::Middle);
                        style.angle = (if s.mirror { 180. - angle } else { *angle }) - 90. * s.rot as f32;
                        d.styled_text(tf(*pos), t, &style, text_ink);
                    } else {
                        d.text(tf(*pos), t, *size, rotated_anchor(*anchor, s), VAnchor::Middle, text_ink);
                    }
                }
            }
        }
    }
}

fn draw_symbol_text(d: &mut dyn Draw, s: &SymbolInst, pos: P, text: &str, plain: &TextStyle, angle: Option<f32>, ink: Ink) {
    let tf = |q: P| s.pos + q.transform(s.rot, s.mirror);
    if let Some(angle) = angle {
        let mut style = *plain;
        let direction = (if s.mirror { 180. - angle } else { angle }) - 90. * s.rot as f32;
        style.angle = (direction + 180.).rem_euclid(360.) - 180.;
        if style.angle > 90. || style.angle <= -90. {
            style.angle += if style.angle > 0. { -180. } else { 180. };
            style.h = match plain.h {
                Anchor::Left => Anchor::Right,
                Anchor::Right => Anchor::Left,
                Anchor::Center => Anchor::Center,
            };
        }
        let (sin, cos) = angle.to_radians().sin_cos();
        for (i, line) in text.lines().enumerate() {
            let offset = p(sin, cos) * (i as f32 * style.size * 1.25);
            d.styled_text(tf(pos + offset), line, &style, ink);
        }
    } else {
        d.text(tf(pos), text, plain.size, rotated_anchor(plain.h, s), VAnchor::Middle, ink);
    }
}

/// Text stays horizontal when a symbol is rotated, so left/right anchors follow the
/// side the text ends up on.
fn rotated_anchor(a: Anchor, s: &SymbolInst) -> Anchor {
    if a == Anchor::Center || (s.rot.is_multiple_of(4) && !s.mirror) {
        return a;
    }
    let dir = if a == Anchor::Left { p(1.0, 0.0) } else { p(-1.0, 0.0) };
    let w = dir.transform(s.rot, s.mirror);
    if w.x > 0.5 {
        Anchor::Left
    } else if w.x < -0.5 {
        Anchor::Right
    } else {
        Anchor::Center
    }
}

fn find_symbol(prj: &Project, loc: &Loc) -> Option<SymbolRef> {
    prj.pages[loc.page].symbols.iter().find(|s| s.id == loc.sym_id).and_then(|s| prj.symbol(&s.sym))
}

fn find_inst<'a>(prj: &'a Project, loc: &Loc) -> Option<&'a SymbolInst> {
    prj.pages[loc.page].symbols.iter().find(|s| s.id == loc.sym_id)
}

/// Text beside a wire at fraction `t` of its length: left of vertical wires, above
/// horizontal ones (`below`: right of / below the wire).
/// Distance of a wire number from the ends of its wire (mm).
const END_CLEARANCE: f32 = 4.5;
/// Length of a wire taken by the cable tag and core below the cable stroke (mm).
const CORE_LABEL: f32 = 4.0;

fn wire_text(d: &mut dyn Draw, w: &Wire, t: f32, text: &str, below: bool, ink: Ink) {
    let q = p(w.a.x + (w.b.x - w.a.x) * t, w.a.y + (w.b.y - w.a.y) * t);
    if (w.b.x - w.a.x).abs() < (w.b.y - w.a.y).abs() {
        let (dx, anchor) = if below { (1.2, Anchor::Left) } else { (-0.8, Anchor::Right) };
        d.text(p(q.x + dx, q.y), text, SMALL_SIZE, anchor, VAnchor::Middle, ink);
    } else if below {
        d.text(p(q.x, q.y + 2.8), text, SMALL_SIZE, Anchor::Center, VAnchor::Bottom, ink);
    } else {
        d.text(p(q.x, q.y - 0.6), text, SMALL_SIZE, Anchor::Center, VAnchor::Bottom, ink);
    }
}

/// Wire numbers (once per connection and page, on its longest wire) and cable cores
/// (a stroke across the wire, the core and once per page the cable tag).
fn draw_wire_labels(d: &mut dyn Draw, prj: &Project, an: &Analysis, page_i: usize) {
    let page = &prj.pages[page_i];
    for w in &page.wires {
        for label in &w.labels {
            if label.is_valid() && !label.text(w).is_empty() {
                let mut style = TextStyle::plain(label.size, label.anchor, VAnchor::Middle);
                style.angle = label.angle;
                d.styled_text(label.position(w), label.text(w), &style, label.color.map_or(Ink::Text, Ink::Color));
            }
        }
    }
    for net in &an.open.nets {
        let Some(w) = wirenum::label_wire(page, page_i, net) else { continue };
        let number = wirenum::number_of(prj, net);
        if !number.is_empty() {
            // Cable cores are labelled at 30 % of the wire: the number goes into the
            // middle of the rest, clear of the end with the connection point
            // designations.
            let len = (w.b.x - w.a.x).hypot(w.b.y - w.a.y);
            let (from, to) = if w.cable.is_empty() { (END_CLEARANCE, len - END_CLEARANCE) } else { (0.3 * len + CORE_LABEL, len - END_CLEARANCE) };
            let t = if to > from && len > 0.0 { (from + to) / 2.0 / len } else if w.cable.is_empty() { 0.5 } else { 0.72 };
            wire_text(d, w, t, &number, false, Ink::Text);
        }
    }
    let mut cabled: Vec<&Wire> = page.wires.iter().filter(|w| !w.cable.is_empty() && !w.hide_labels).collect();
    cabled.sort_by(|a, b| a.a.x.min(a.b.x).total_cmp(&b.a.x.min(b.b.x)).then(a.a.y.total_cmp(&b.a.y)));
    let mut tagged: Vec<&str> = Vec::new();
    for w in cabled {
        let t = 0.3;
        let q = p(w.a.x + (w.b.x - w.a.x) * t, w.a.y + (w.b.y - w.a.y) * t);
        d.line(q + p(-1.0, 0.8), q + p(1.0, -0.8), SYMBOL_W, Ink::Line, false);
        if !tagged.contains(&w.cable.as_str()) {
            tagged.push(&w.cable);
            wire_text(d, w, t, &structure::relative(page, &w.cable), false, Ink::CrossRef);
        }
        wire_text(d, w, t, &w.core, true, Ink::CrossRef);
    }
}

/// Device tag, function text, connection point designations and cross references.
pub fn draw_annotations(d: &mut dyn Draw, prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst, ink: Ink) {
    let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
    let bounds = def.world_bounds(s.pos, s.rot, s.mirror);
    let vertical = s.rot.is_multiple_of(2);

    // Connection point designations
    if def.show_pin_labels {
        let c = bounds.center();
        for (i, label) in s.pins.iter().enumerate().take(def.pins.len()) {
            if label.is_empty() {
                continue;
            }
            let q = def.pin_world(i, s.pos, s.rot, s.mirror);
            let dlt = q - c;
            if dlt.y.abs() >= dlt.x.abs() {
                let y = q.y - dlt.y.signum() * 1.1;
                d.text(p(q.x - 0.6, y), label, SMALL_SIZE, Anchor::Right, VAnchor::Middle, text_ink);
            } else {
                let x = q.x - dlt.x.signum() * 1.1;
                d.text(p(x, q.y - 0.5), label, SMALL_SIZE, Anchor::Center, VAnchor::Bottom, text_ink);
            }
        }
    }

    // Cross references
    let mut refs: Vec<(String, Option<usize>)> = Vec::new();
    let dev = idx.devices.get(&s.tag);
    match def.func {
        Func::Aux(_) => match dev.map(|d| d.mains.as_slice()) {
            Some([]) | None => refs.push(("?".into(), None)),
            Some(mains) => refs.extend(mains.iter().map(|m| (m.reference_from(prj, page_i), Some(m.page)))),
        },
        Func::Overview => {
            if let Some(dev) = dev {
                refs.extend(dev.mains.iter().map(|m| (m.reference_from(prj, page_i), Some(m.page))));
            }
        }
        Func::Main { mirror: false } => {
            if let Some(dev) = dev {
                refs.extend(dev.auxs.iter().map(|a| (a.reference_from(prj, page_i), Some(a.page))));
            }
        }
        Func::Interruption if idx.interruptions.contains_key(&s.interruption_key()) => {
            let others: Vec<Loc> = idx.interruption_targets(prj, &s.interruption_key(), s.id);
            let q = s.pos + p(2.2, 1.6).transform(s.rot, s.mirror) + p(0.0, 2.6);
            let txt = if others.is_empty() { "?".to_string() } else { others.iter().map(|l| l.reference_from(prj, page_i)).collect::<Vec<_>>().join(" ") };
            let ink = if others.is_empty() { Ink::Error } else { Ink::CrossRef };
            d.text(q, &txt, SMALL_SIZE, Anchor::Left, VAnchor::Middle, ink);
            if let Some(l) = others.first() {
                link_text(d, q, &txt, SMALL_SIZE, Anchor::Left, l.page);
            }
        }
        _ => {}
    }
    if matches!(def.func, Func::Main { .. }) && !s.tag.is_empty() {
        refs.extend(crate::electrofluid::cross_refs(prj, idx, page_i, &s.tag));
    }

    // Tag block
    if def.show_tag {
        let mut lines: Vec<(String, f32, Ink, Option<usize>)> = Vec::new();
        if !s.tag.is_empty() && !continues_strip(prj, page_i, def, s) {
            lines.push((structure::relative(&prj.pages[page_i], &s.tag), TAG_SIZE, text_ink, None));
        }
        if !s.function_text.is_empty() && !def.has_function_text_field() {
            for l in s.function_text.lines() {
                lines.push((l.to_string(), 2.0, text_ink, None));
            }
        }
        for (r, target) in refs {
            let ink = if r == "?" { Ink::Error } else { Ink::CrossRef };
            lines.push((r, 2.0, ink, target));
        }
        let (x, mut y, anchor) = tag_block_origin(def, s, &lines.iter().map(|l| l.1).collect::<Vec<_>>());
        for (t, size, ink, target) in &lines {
            let q = p(x, y + size * 0.675);
            d.text(q, t, *size, anchor, VAnchor::Middle, *ink);
            if let Some(page) = target {
                link_text(d, q, t, *size, anchor, *page);
            }
            y += size * 1.35;
        }
    }

    for pp in &s.props {
        let t = crate::props::text(prj, page_i, s, pp);
        if !t.is_empty() {
            d.text(s.pos + pp.offset, &t, pp.size, pp.anchor, VAnchor::Middle, text_ink);
        }
    }

    // Contact image (mirror) below coils
    if matches!(def.func, Func::Main { mirror: true }) {
        if let Some(dev) = dev {
            if !prj.xref.hide_contact_image {
                draw_contact_image(d, prj, page_i, &s.tag, dev.auxs.as_slice(), &bounds, vertical, text_ink);
            }
        }
    }
}

/// A terminal right beside the one before it of the same strip (same row, at most
/// 20 mm apart): the strip tag is shown once, at the first terminal of the row, or it
/// would run into the terminal numbers.
pub fn continues_strip(prj: &Project, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> bool {
    let is_terminal = |d: &SymbolDef| matches!(d.func, Func::Terminal);
    is_terminal(def)
        && s.tag_offset.is_none()
        && s.rot.is_multiple_of(2)
        && prj.pages[page_i].symbols.iter().any(|o| {
            let dx = s.pos.x - o.pos.x;
            o.id != s.id && o.tag == s.tag && (o.pos.y - s.pos.y).abs() < 0.1 && dx > 0.0 && dx <= 20.0 && o.rot == s.rot && prj.symbol(&o.sym).is_some_and(|d| is_terminal(&d))
        })
}

/// Where the tag block starts: x and top y of the first line and the text anchor
/// (left of vertical symbols, above horizontal ones, or where it was placed).
fn tag_block_origin(def: &SymbolDef, s: &SymbolInst, sizes: &[f32]) -> (f32, f32, Anchor) {
    let bounds = def.world_bounds(s.pos, s.rot, s.mirror);
    let total: f32 = sizes.iter().map(|z| z * 1.35).sum();
    match s.tag_offset {
        Some(o) => (s.pos.x + o.x, s.pos.y + o.y, Anchor::Left),
        None if s.rot.is_multiple_of(2) => (bounds.min.x - 1.2, bounds.center().y - total / 2.0, Anchor::Right),
        None => (bounds.center().x, bounds.min.y - 1.0 - total, Anchor::Center),
    }
}

/// Approximate area of the tag block (tag and function text) of a placed symbol,
/// used to pick and drag it.
pub fn tag_block_bounds(prj: &Project, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> Option<R> {
    if !def.show_tag || (s.tag.is_empty() && (s.function_text.is_empty() || def.has_function_text_field())) {
        return None;
    }
    let mut lines: Vec<(String, f32)> = Vec::new();
    if !s.tag.is_empty() && !continues_strip(prj, page_i, def, s) {
        lines.push((structure::relative(&prj.pages[page_i], &s.tag), TAG_SIZE));
    }
    if !def.has_function_text_field() {
        lines.extend(s.function_text.lines().map(|l| (l.to_string(), 2.0)));
    }
    let (x, y, anchor) = tag_block_origin(def, s, &lines.iter().map(|l| l.1).collect::<Vec<_>>());
    let w = lines.iter().map(|(t, z)| t.chars().count() as f32 * z * 0.62).fold(0.0, f32::max);
    let h: f32 = lines.iter().map(|l| l.1 * 1.35).sum();
    let x0 = match anchor {
        Anchor::Left => x,
        Anchor::Center => x - w / 2.0,
        Anchor::Right => x - w,
    };
    Some(R::from_points(p(x0, y), p(x0 + w, y + h)))
}

/// Area of a placed property text.
pub fn prop_bounds(prj: &Project, page_i: usize, s: &SymbolInst, pp: &PlacedProp) -> R {
    let t = crate::props::text(prj, page_i, s, pp);
    let w = t.chars().count().max(3) as f32 * pp.size * 0.62;
    let q = s.pos + pp.offset;
    let x0 = match pp.anchor {
        Anchor::Left => q.x,
        Anchor::Center => q.x - w / 2.0,
        Anchor::Right => q.x - w,
    };
    R::from_points(p(x0, q.y - pp.size * 0.7), p(x0 + w, q.y + pp.size * 0.7))
}

/// Contact image row: kind, connection designations, cross reference and its page
/// (None = reserve contact).
type ContactRow = (ContactKind, Vec<String>, Option<(String, usize)>);

#[allow(clippy::too_many_arguments)]
fn draw_contact_image(d: &mut dyn Draw, prj: &Project, page_i: usize, tag: &str, auxs: &[Loc], bounds: &R, vertical: bool, ink: Ink) {
    // Rows: contact kind, connection designations, cross reference (None = reserve).
    let mut rows: Vec<ContactRow> = Vec::new();
    for loc in auxs {
        let (Some(def), Some(inst)) = (find_symbol(prj, loc), find_inst(prj, loc)) else { continue };
        let kind = def.contact_kind().unwrap_or(ContactKind::No);
        rows.push((kind, inst.pins.clone(), Some((loc.reference_from(prj, page_i), loc.page))));
    }
    // Unused contacts of the assigned parts (function templates) are shown as reserve.
    for f in parts::reserve_functions(prj, None, tag) {
        let kind = match f.kind {
            FnKind::No => ContactKind::No,
            FnKind::Nc => ContactKind::Nc,
            FnKind::Changeover => ContactKind::Changeover,
            FnKind::NoOnDelay => ContactKind::NoOnDelay,
            FnKind::NcOnDelay => ContactKind::NcOnDelay,
            FnKind::NoOffDelay => ContactKind::NoOffDelay,
            FnKind::NcOffDelay => ContactKind::NcOffDelay,
            FnKind::MainContacts3 | FnKind::MainContacts4 | FnKind::MainContact => ContactKind::MainNo,
            _ => continue,
        };
        rows.push((kind, f.pin_list(), None));
    }
    if rows.is_empty() {
        return;
    }
    // Placed right of the outgoing wire so it does not overlap it; below a turned coil,
    // clear of its wires and connection point designations on both sides.
    let (cx, mut y) = if vertical { (bounds.center().x + 7.5, bounds.max.y + 3.0) } else { (bounds.center().x, bounds.max.y + 3.0) };
    for (kind, pins, reference) in rows {
        let (text_ink, line_ink) = match (&reference, ink) {
            (None, Ink::Text) => (Ink::FrameText, Ink::Frame),
            (_, Ink::Text) => (Ink::Text, Ink::Line),
            _ => (ink, ink),
        };
        let (left, right) = if kind == ContactKind::MainNo && pins.len() > 3 {
            // 3-pole main contacts: "1 3 5" / "2 4 6"
            let half = pins.len() / 2;
            (pins[..half].join(" "), pins[half..].join(" "))
        } else if kind == ContactKind::Changeover {
            // Common pin is listed first for changeover contacts.
            (pins.first().cloned().unwrap_or_default(), pins.get(1..).map(|r| r.join(" ")).unwrap_or_default())
        } else {
            (pins.first().cloned().unwrap_or_default(), pins.get(1).cloned().unwrap_or_default())
        };
        mirror_glyph(d, p(cx, y), kind, line_ink);
        d.text(p(cx - 3.6, y), &left, SMALL_SIZE, Anchor::Right, VAnchor::Middle, text_ink);
        d.text(p(cx + 3.6, y), &right, SMALL_SIZE, Anchor::Left, VAnchor::Middle, text_ink);
        let rx = (cx + 4.2 + right.chars().count() as f32 * SMALL_SIZE * 0.6).max(cx + 7.5);
        match reference {
            Some((r, page)) => {
                d.text(p(rx, y), &r, SMALL_SIZE, Anchor::Left, VAnchor::Middle, Ink::CrossRef);
                link_text(d, p(rx, y), &r, SMALL_SIZE, Anchor::Left, page);
            }
            None => d.text(p(rx, y), "reserve", SMALL_SIZE * 0.9, Anchor::Left, VAnchor::Middle, Ink::FrameText),
        }
        y += 3.0;
    }
}

/// Small horizontal contact symbol used in the contact image.
fn mirror_glyph(d: &mut dyn Draw, c: P, kind: ContactKind, ink: Ink) {
    let w = 0.18;
    d.line(c + p(-3.0, 0.0), c + p(-1.2, 0.0), w, ink, false);
    d.line(c + p(1.2, 0.0), c + p(3.0, 0.0), w, ink, false);
    match kind.base() {
        ContactKind::Nc => {
            d.line(c + p(-1.2, 0.0), c + p(-1.2, 0.9), w, ink, false);
            d.line(c + p(1.2, 0.0), c + p(-1.6, 1.2), w, ink, false);
        }
        ContactKind::Changeover => {
            d.line(c + p(-1.2, 0.0), c + p(-1.2, 0.9), w, ink, false);
            d.line(c + p(1.2, 0.0), c + p(-1.6, 1.2), w, ink, false);
            d.line(c + p(-1.2, 0.0), c + p(-1.2, -0.9), w, ink, false);
        }
        _ => d.line(c + p(1.2, 0.0), c + p(-1.5, -1.1), w, ink, false),
    }
    // Delayed contacts: a short stem from the blade with a bar, pointing up for on-delay.
    if let Some(on) = kind.delay() {
        let (y0, y1) = if on { (-0.4, -1.6) } else { (0.4, 1.6) };
        d.line(c + p(0.0, y0), c + p(0.0, y1), w, ink, false);
        d.line(c + p(-0.6, y1), c + p(0.6, y1), w, ink, false);
    }
}

pub fn draw_frame(d: &mut dyn Draw, prj: &Project, page_i: usize) {
    let page = &prj.pages[page_i];
    let f = crate::frame::of_page(prj, page_i);
    if !f.visible { return; }
    let fi = Ink::Frame;
    let b = f.border();
    let a = f.area();
    d.rect(b.min, b.max, 0.5, fi, false);
    if f.strips && crate::frame::has_strips(page.kind) && a.min.x > b.min.x {
        // Column strip
        let size = ((a.min.y - b.min.y) * 0.5).min(2.5);
        d.line(p(a.min.x, a.min.y), p(b.max.x, a.min.y), 0.25, fi, false);
        d.line(p(a.min.x, b.min.y), p(a.min.x, a.max.y), 0.25, fi, false);
        let cw = f.column_width();
        for c in 0..f.columns.max(1) {
            let x = a.min.x + c as f32 * cw;
            if c > 0 {
                d.line(p(x, b.min.y), p(x, a.min.y), 0.25, fi, false);
                d.line(p(x, a.max.y - 2.0), p(x, a.max.y), 0.25, fi, false);
            }
            d.text(p(x + cw / 2.0, (b.min.y + a.min.y) / 2.0), &f.column_labels.label(c), size, Anchor::Center, VAnchor::Middle, Ink::FrameText);
        }
        let rh = f.row_height();
        for r in 0..f.rows.max(1) {
            let y = a.min.y + r as f32 * rh;
            if r > 0 {
                d.line(p(b.min.x, y), p(a.min.x, y), 0.25, fi, false);
            }
            d.text(p((b.min.x + a.min.x) / 2.0, y + rh / 2.0), &f.row_labels.label(r), size, Anchor::Center, VAnchor::Middle, Ink::FrameText);
        }
    }
    draw_title_block(d, prj, page_i);
}

fn draw_title_block(d: &mut dyn Draw, prj: &Project, page_i: usize) {
    let form = crate::forms::title_block(prj, page_i);
    crate::forms::draw(d, &form, prj, page_i);
}

fn draw_cover(d: &mut dyn Draw, prj: &Project, frame: &crate::frame::PlotFrame) {
    let info = &prj.info;
    let a = frame.area();
    let cx = a.center().x;
    // Laid out for the 240 mm high area of A3; other heights move the blocks.
    let k = a.height() / 240.0;
    let at = |y: f32| a.min.y + (y - 15.0) * k;
    d.text(p(cx, at(80.0)), &info.name, 10.0, Anchor::Center, VAnchor::Middle, Ink::Text);
    for (i, l) in info.description.lines().enumerate() {
        d.text(p(cx, at(100.0) + i as f32 * 8.0), l, 5.0, Anchor::Center, VAnchor::Middle, Ink::Text);
    }
    let rows = [
        (tr("Company"), info.company.as_str()),
        (tr("Created by"), info.creator.as_str()),
        (tr("Date"), info.date.as_str()),
        (tr("Revision"), info.revision.as_str()),
    ];
    let pages = prj.pages.len().to_string();
    let rows: Vec<(&str, &str)> = rows.iter().copied().chain(std::iter::once((tr("Number of pages"), pages.as_str()))).collect();
    // The rows centred in the box, which is wide enough for the longest key and value.
    let (row, size, pad) = (9.0, 3.5, 3.0);
    let top = at(150.0);
    for (i, (k, v)) in rows.iter().enumerate() {
        let y = top + pad + (i as f32 + 0.5) * row;
        d.text(p(cx - 5.0, y), k, size, Anchor::Right, VAnchor::Middle, Ink::FrameText);
        d.text(p(cx + 5.0, y), v, size, Anchor::Left, VAnchor::Middle, Ink::Text);
    }
    let longest = rows.iter().map(|(k, v)| text_width(k, size).max(text_width(v, size))).fold(0.0, f32::max);
    let half = (longest + 5.0 + 2.0 * pad).max(100.0).min(a.width() / 2.0 - 5.0);
    d.rect(p(cx - half, top), p(cx + half, top + rows.len() as f32 * row + 2.0 * pad), 0.25, Ink::Frame, false);
}

pub const REPORT_ROWS_PER_PAGE: usize = crate::report_blocks::ROWS_PER_PAGE;

/// Height of the terminal line-up drawing above the table.
const LINEUP_H: f32 = 58.0;

fn draw_report(d: &mut dyn Draw, t: &crate::report_blocks::PageTable, frame: &crate::frame::PlotFrame) {
    let area = frame.area();
    let (x0, x1) = (area.min.x + 7.0, area.max.x - 10.0);
    let top = area.min.y;
    d.text(p(x0, top + 9.0), &t.title, 6.0, Anchor::Left, VAnchor::Middle, Ink::Text);
    let mut table_top = top + 19.0;
    if !t.lineup.is_empty() {
        draw_lineup(d, &t.lineup, p(x0, table_top), x1 - x0);
        table_top += LINEUP_H;
    }
    let total_w: f32 = t.widths.iter().sum::<f32>().max(0.01);
    let mut xs = vec![x0];
    for w in &t.widths {
        xs.push(xs.last().unwrap() + w / total_w * (x1 - x0));
    }
    // Rows get lower (and their text smaller) when more of them must fit.
    let row_h = (5.0f32).min((area.max.y - table_top - 17.0) / t.per_page.max(1) as f32).max(2.0);
    let size = if t.text_size > 0.0 { t.text_size.min(row_h * 0.8) } else { 2.2 * row_h / 5.0 };
    let mut y = table_top;
    for (i, h) in t.headers.iter().enumerate() {
        d.text(p(xs[i] + 1.2, y + 2.5), h, 2.4, Anchor::Left, VAnchor::Middle, Ink::Text);
    }
    y += 5.0;
    d.line(p(x0, y), p(x1, y), 0.5, Ink::Frame, false);
    for (ri, row) in t.rows.iter().enumerate() {
        if let Some(Some((page, _))) = t.targets.get(ri) {
            d.link(p(x0, y), p(x1, y + row_h), *page);
        }
        for i in 0..t.headers.len() {
            let cell = row.get(i).map(String::as_str).unwrap_or("");
            let maxc = ((xs[i + 1] - xs[i]) / (size * 0.6)) as usize;
            let txt: String = if cell.chars().count() > maxc && maxc > 1 {
                cell.chars().take(maxc - 1).collect::<String>() + "…"
            } else {
                cell.to_string()
            };
            d.text(p(xs[i] + 1.2, y + row_h / 2.0), &txt, size, Anchor::Left, VAnchor::Middle, Ink::Text);
        }
        y += row_h;
        d.line(p(x0, y), p(x1, y), 0.1, Ink::Frame, false);
    }
    for x in &xs {
        d.line(p(*x, table_top), p(*x, y), 0.15, Ink::Frame, false);
    }
    d.rect(p(x0, table_top), p(x1, y), 0.3, Ink::Frame, false);
    let note = if t.rows.is_empty() {
        Some(tr("(no entries)").to_string())
    } else if t.continued {
        Some(tr("Continued on the next page").to_string())
    } else if t.more > 0 {
        Some(tr_fmt!("… {} more rows – see Reports window / CSV export", t.more))
    } else {
        None
    };
    if let Some(note) = note {
        d.text(p(x0 + 2.0, y + 5.0), &note, 2.5, Anchor::Left, VAnchor::Middle, Ink::FrameText);
    }
    if !t.source.is_empty() {
        d.text(p(x1, top + 9.0), &tr_fmt!("Output from project {}", t.source), 2.5, Anchor::Right, VAnchor::Middle, Ink::FrameText);
    }
}

/// Terminal line-up: the elements side by side as on the rail (scaled to fit, at most
/// 4:1), designations inside, jumpers as bars below.
fn draw_lineup(d: &mut dyn Draw, boxes: &[crate::report_blocks::LineupBox], origin: P, width: f32) {
    let total: f32 = boxes.iter().map(|b| b.width).sum::<f32>().max(1.0);
    let k = (width / total).min(4.0);
    let (y0, h) = (origin.y + 6.0, 36.0);
    let mut x = origin.x + (width - total * k) / 2.0;
    let mut spans: Vec<(u32, f32, f32)> = Vec::new();
    d.line(p(origin.x, y0 + h / 2.0), p(origin.x + width, y0 + h / 2.0), 0.15, Ink::Frame, true);
    for b in boxes {
        let w = b.width * k;
        let (a, c) = (p(x, if b.accessory { y0 - 3.0 } else { y0 }), p(x + w, if b.accessory { y0 + h + 3.0 } else { y0 + h }));
        if b.accessory {
            d.rect(a, c, 0.0, Ink::Fill, true);
        }
        if b.spare {
            for (s, e) in [(a, p(c.x, a.y)), (p(c.x, a.y), c), (c, p(a.x, c.y)), (p(a.x, c.y), a)] {
                d.line(s, e, 0.25, Ink::Line, true);
            }
        } else {
            d.rect(a, c, 0.25, Ink::Line, false);
        }
        if !b.label.is_empty() {
            let size = (w * 0.8 / (b.label.chars().count() as f32 * 0.62)).min(2.5);
            if size >= 0.8 {
                d.text(p(x + w / 2.0, y0 + 5.0), &b.label, size, Anchor::Center, VAnchor::Middle, Ink::Text);
            }
        }
        if b.jumper > 0 {
            match spans.iter_mut().find(|s| s.0 == b.jumper) {
                Some(s) => s.2 = x + w / 2.0,
                None => spans.push((b.jumper, x + w / 2.0, x + w / 2.0)),
            }
        }
        x += w;
    }
    for (i, (_, a, b)) in spans.iter().enumerate() {
        let y = y0 + h - 4.0 - (i % 3) as f32 * 2.0;
        d.line(p(*a, y), p(*b, y), 0.6, Ink::Line, false);
        for x in [a, b] {
            d.circle(p(*x, y), 0.6, 0.0, Ink::Line, true);
        }
    }
}

// ---------------------------------------------------------------------- panel layout

/// Draws an enclosure in front view: its mounting plate or its door (seen from the
/// front). `origin` is the page position of the enclosure's top left corner, `k` the
/// scale (page mm per real mm). From the front, the cable ducts behind the door are
/// hidden (dashed); `inside` draws the door as seen from the inside (the caller
/// mirrors it): ducts as they are, door devices from the rear.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn draw_enclosure(
    d: &mut dyn Draw,
    e: &Enclosure,
    face: Face,
    inside: bool,
    origin: P,
    k: f32,
    selected: &BTreeSet<u64>,
    problems: &BTreeSet<u64>,
    prj: &Project,
) {
    let parts = &prj.parts[..];
    let m = e.face_offset(face);
    let tf = |x: f32, y: f32| origin + p((x + m.x) * k, (y + m.y) * k);
    let (vw, vh) = e.view_size(face);
    if !e.is_assembly() {d.rect(origin, origin + p(vw * k, vh * k), 0.5, Ink::Line, false);}
    match face {
        Face::Plate if e.is_assembly() || e.no_implicit_plate || e.items.iter().any(|i|i.kind==ItemKind::MountingPlate) => {}
        Face::Plate if panel::is_rack(e) => {
            // No plate in a rack: its front rails with the rack units, numbered from
            // the bottom.
            use panel::rack::{panel_left, unit_top, RACK_OPENING, RACK_UNIT, RACK_WIDTH};
            let n = panel::rack_units(e);
            let (l, rail) = (panel_left(e), (RACK_WIDTH - RACK_OPENING) / 2.0);
            let (top, bottom) = (unit_top(e, n, 1), unit_top(e, 1, 1) + RACK_UNIT);
            for x in [l, l + RACK_WIDTH - rail] {
                d.rect(tf(x, top), tf(x + rail, bottom), 0.0, Ink::Plate, true);
                d.rect(tf(x, top), tf(x + rail, bottom), 0.25, Ink::Frame, false);
                for u in 1..n {
                    let y = unit_top(e, u, 1);
                    d.line(tf(x, y), tf(x + rail, y), 0.1, Ink::Frame, false);
                }
            }
            for u in 1..=n {
                let y = unit_top(e, u, 1) + RACK_UNIT / 2.0;
                d.text(tf(l - 4.0, y), &u.to_string(), RACK_UNIT * 0.4 * k, Anchor::Right, VAnchor::Middle, Ink::Text);
            }
        }
        Face::Plate => {
            let (pw, ph) = e.plate_size();
            d.rect(tf(0.0, 0.0), tf(pw, ph), 0.0, Ink::Plate, true);
            d.rect(tf(0.0, 0.0), tf(pw, ph), 0.25, Ink::Frame, false);
        }
        Face::Door => {
            // Door frame, hinges on the left, handle on the right.
            let f = panel::DOOR_FRAME;
            let (a, b) = (tf(f, f), tf(e.width - f, e.height - f));
            for (s, t) in [(a, p(b.x, a.y)), (p(b.x, a.y), b), (b, p(a.x, b.y)), (p(a.x, b.y), a)] {
                d.line(s, t, 0.15, Ink::Frame, true);
            }
            for y in [0.15, 0.85] {
                let hy = e.height * y;
                d.rect(tf(-3.0, hy - 25.0), tf(3.0, hy + 25.0), 0.25, Ink::Frame, false);
            }
            let hy = e.height / 2.0;
            d.rect(tf(e.width - 32.0, hy - 50.0), tf(e.width - 14.0, hy + 50.0), 0.0, Ink::Fill, true);
            d.rect(tf(e.width - 32.0, hy - 50.0), tf(e.width - 14.0, hy + 50.0), 0.25, Ink::Line, false);
        }
        wall => {
            // The sheet, its mounting area inside the flanges (dashed) and, for walls
            // that are a separate component, the outline of that part.
            d.rect(origin, origin + p(vw * k, vh * k), 0.0, Ink::Plate, true);
            if e.is_room() {
                // A wall of a room has no flanges: the floor is drawn as a strong line.
                if wall != Face::Roof {
                    d.line(origin + p(0.0, vh * k), origin + p(vw * k, vh * k), 0.6, Ink::Frame, false);
                }
            } else {
                let a = panel::mounting_area(e, wall);
                let (s, t) = (tf(a.min.x, a.min.y), tf(a.max.x, a.max.y));
                for (u, v) in [(s, p(t.x, s.y)), (p(t.x, s.y), t), (t, p(s.x, t.y)), (p(s.x, t.y), s)] {
                    d.line(u, v, 0.15, Ink::Frame, true);
                }
            }
            let open = (wall == Face::Left && e.build.left_open) || (wall == Face::Right && e.build.right_open);
            if open {
                d.line(origin, origin + p(vw * k, vh * k), 0.15, Ink::Frame, false);
                d.line(origin + p(0.0, vh * k), origin + p(vw * k, 0.0), 0.15, Ink::Frame, false);
                d.text(origin + p(vw * k / 2.0, vh * k / 2.0), tr("bayed (open)"), 3.0 * k.max(0.2), Anchor::Center, VAnchor::Middle, Ink::Text);
            }
            if wall == Face::Bottom && e.build.gland_plate {
                let g = 3.0 * e.flange();
                if vw > 2.0 * g && vh > 2.0 * g {
                    d.rect(tf(g, g / 2.0), tf(vw - g, vh - g / 2.0), 0.2, Ink::Frame, false);
                }
            }
        }
    }
    // Cutouts first, then rails, ducts, busbars, devices on top.
    let mut items: Vec<&PanelItem> = e.items.iter().filter(|i| i.face == face).collect();
    items.sort_by_key(|i| i.kind.layer());
    for it in items {
        let ink = if selected.contains(&it.id) {
            Ink::Selected
        } else if problems.contains(&it.id) {
            Ink::Error
        } else {
            Ink::Line
        };
        let (a, b) = (tf(it.x, it.y), tf(it.x + it.w, it.y + it.h));
        let horizontal = it.horizontal();
        let len = it.length();
        match it.kind {
            ItemKind::Rail => {
                d.rect(a, b, 0.0, Ink::Plate, true);
                d.rect(a, b, 0.2, ink, false);
                // Mounting slots
                let mut t = 12.5;
                while t + 10.0 < len {
                    let (s0, s1) = if horizontal {
                        (tf(it.x + t, it.y + it.h / 2.0), tf(it.x + t + 10.0, it.y + it.h / 2.0))
                    } else {
                        (tf(it.x + it.w / 2.0, it.y + t), tf(it.x + it.w / 2.0, it.y + t + 10.0))
                    };
                    d.line(s0, s1, 0.15, Ink::Frame, false);
                    t += 25.0;
                }
            }
            ItemKind::Duct if face == Face::Door && !inside => {
                let ink = if ink == Ink::Line { Ink::Frame } else { ink };
                for (s, t) in [(a, p(b.x, a.y)), (p(b.x, a.y), b), (b, p(a.x, b.y)), (p(a.x, b.y), a)] {
                    d.line(s, t, 0.2, ink, true);
                }
            }
            ItemKind::Duct => {
                d.rect(a, b, 0.0, Ink::Fill, true);
                d.rect(a, b, 0.25, ink, false);
                // Comb of the duct
                let mut t = 10.0;
                while t < len {
                    let pairs = if horizontal {
                        [
                            (tf(it.x + t, it.y), tf(it.x + t, it.y + it.h * 0.25)),
                            (tf(it.x + t, it.y + it.h * 0.75), tf(it.x + t, it.y + it.h)),
                        ]
                    } else {
                        [
                            (tf(it.x, it.y + t), tf(it.x + it.w * 0.25, it.y + t)),
                            (tf(it.x + it.w * 0.75, it.y + t), tf(it.x + it.w, it.y + t)),
                        ]
                    };
                    for (s0, s1) in pairs {
                        d.line(s0, s1, 0.12, Ink::Frame, false);
                    }
                    t += 10.0;
                }
            }
            ItemKind::Busbar => {
                d.rect(a, b, 0.0, Ink::Plate, true);
                d.rect(a, b, 0.2, ink, false);
                // The bars across the system (at their centre distance).
                let horizontal = crate::panel::bars_along_x(parts, it);
                let across = if horizontal { it.h } else { it.w };
                let centres = crate::production::bar_centres(parts, &it.part, across);
                let pitch = centres.windows(2).map(|w| w[1] - w[0]).fold(across, f32::min);
                for off in centres {
                    let half = (pitch * 0.18).min(3.0);
                    let (r0, r1) = if horizontal {
                        (tf(it.x, it.y + off - half), tf(it.x + it.w, it.y + off + half))
                    } else {
                        (tf(it.x + off - half, it.y), tf(it.x + off + half, it.y + it.h))
                    };
                    d.rect(r0, r1, 0.0, Ink::Fill, true);
                    d.rect(r0, r1, 0.15, ink, false);
                }
            }
            ItemKind::Cutout if matches!(it.opening, crate::model::panel::Opening::Door | crate::model::panel::Opening::Window) => {
                // Doors and windows in the wall of a room, seen from inside: a frame,
                // the door's handle on the right, the window's sashes.
                let line_ink = if ink == Ink::Line { Ink::Frame } else { ink };
                d.rect(a, b, 0.0, Ink::Fill, true);
                d.rect(a, b, 0.3, ink, false);
                let f = 60.0_f32.min(it.w / 4.0).min(it.h / 4.0);
                if it.opening == crate::model::panel::Opening::Door {
                    d.rect(tf(it.x + f, it.y + f), tf(it.x + it.w - f, it.y + it.h), 0.15, line_ink, false);
                    let handle = tf(it.x + it.w - f - 60.0, it.y + it.h - 1050.0_f32.min(it.h / 2.0));
                    d.circle(handle, 20.0 * k, 0.2, line_ink, false);
                } else {
                    let (c, m) = (tf(it.x + f, it.y + f), tf(it.x + it.w - f, it.y + it.h - f));
                    d.rect(c, m, 0.15, line_ink, false);
                    let mid = tf(it.x + it.w / 2.0, it.y + it.h / 2.0);
                    d.line(p(mid.x, c.y), p(mid.x, m.y), 0.15, line_ink, false);
                }
            }
            ItemKind::Cutout => {
                let line_ink = if ink == Ink::Line { Ink::Frame } else { ink };
                if it.round {
                    let c = tf(it.x + it.w / 2.0, it.y + it.h / 2.0);
                    d.circle(c, it.w / 2.0 * k, 0.3, ink, false);
                    let r = it.w / 2.0 * k * std::f32::consts::FRAC_1_SQRT_2;
                    d.line(c + p(-r, -r), c + p(r, r), 0.12, line_ink, false);
                    d.line(c + p(-r, r), c + p(r, -r), 0.12, line_ink, false);
                } else {
                    d.rect(a, b, 0.3, ink, false);
                    d.line(a, b, 0.12, line_ink, false);
                    d.line(p(a.x, b.y), p(b.x, a.y), 0.12, line_ink, false);
                }
            }
            ItemKind::MountingPlate => {
                d.rect(a,b,0.0,Ink::Fill,true);
                d.rect(a,b,0.25,ink,false);
            }
            ItemKind::Accessory => {
                // Accessories: their outline with the part number.
                d.rect(a, b, 0.0, Ink::Fill, true);
                let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
                let size = (it.w.min(it.h) * 0.18).clamp(2.0, 7.0) * k;
                d.text(tf(it.x + it.w / 2.0, it.y + it.h / 2.0), it.part.trim_start_matches("GEN."), size, Anchor::Center, VAnchor::Middle, text_ink);
                d.rect(a, b, 0.25, ink, false);
                d.line(a, p(a.x + (b.x - a.x).min(b.y - a.y) * 0.3, a.y), 0.15, ink, false);
            }
            ItemKind::Device if inside => {
                // Rear of a door device: its box and tag.
                d.rect(a, b, 0.0, Ink::Fill, true);
                let size = (it.w.min(it.h) * 0.22).clamp(3.0, 9.0) * k;
                let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
                d.text(tf(it.x + it.w / 2.0, it.y + it.h / 2.0), &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink);
                d.rect(a, b, 0.3, ink, false);
            }
            ItemKind::Device => {
                d.rect(a, b, 0.0, Ink::Fill, true);
                let model = parts.iter().find(|x| x.number == it.part).and_then(|x| prj.symbol_cache.front_view(x));
                let text_ink = if ink == Ink::Line { Ink::Text } else { ink };
                let size = (it.w.min(it.h) * 0.22).clamp(3.0, 9.0) * k;
                let c = tf(it.x + it.w / 2.0, it.y + it.h / 2.0);
                // Terminal strips: every element of the strip in its place.
                let cells = panel::strip_cells(prj, it);
                let mounted = panel::mounted(prj, it);
                let on_bars = panel::busbars::terminals_on_bars(prj, it);
                match model.as_deref() {
                    // Busbar terminals across the bars: each upright on its bar.
                    _ if on_bars.is_some() => {
                        for (sub, number, _) in on_bars.as_ref().unwrap() {
                            let (pa, pb) = (tf(sub.x, sub.y), tf(sub.x + sub.w, sub.y + sub.h));
                            d.rect(pa, pb, 0.0, Ink::Fill, true);
                            match parts.iter().chain(crate::parts::catalog()).find(|x| &x.number == number).and_then(|x| prj.symbol_cache.front_view(x)).as_deref() {
                                Some(m) => draw_model_2d(d, m, sub, &tf, k, ink),
                                None => d.rect(pa, pb, 0.15, ink, false),
                            }
                        }
                        let size = size * 0.8;
                        let half = p(it.tag.chars().count() as f32 * size * 0.33 + size * 0.2, size * 0.6);
                        d.rect(c - half, c + half, 0.0, Ink::Plate, true);
                        d.text(c, &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink);
                    }
                    Some(_) if mounted.is_some() => {
                        // Accessories on the main part: each part's front view in its
                        // box, front accessories over the main part.
                        let a = mounted.as_ref().unwrap();
                        for piece in a.pieces() {
                            let sub = a.piece_item(it, piece);
                            let (pa, pb) = (tf(sub.x, sub.y), tf(sub.x + sub.w, sub.y + sub.h));
                            d.rect(pa, pb, 0.0, Ink::Fill, true);
                            match prj.symbol_cache.front_view(piece.part).as_deref() {
                                Some(m) => draw_model_2d(d, m, &sub, &tf, k, ink),
                                None => d.rect(pa, pb, 0.15, ink, false),
                            }
                        }
                        let main = a.piece_item(it, &a.main);
                        let c = tf(main.x + main.w / 2.0, main.y + main.h / 2.0);
                        let size = size * 0.8;
                        let half = p(it.tag.chars().count() as f32 * size * 0.33 + size * 0.2, size * 0.6);
                        d.rect(c - half, c + half, 0.0, Ink::Plate, true);
                        d.text(c, &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink);
                    }
                    Some(_) if !cells.is_empty() => {
                        draw_strip_2d(d, prj, it, &cells, &tf, k, ink);
                        let size = size * 0.8;
                        let half = p(it.tag.chars().count() as f32 * size * 0.33 + size * 0.2, size * 0.6);
                        d.rect(c - half, c + half, 0.0, Ink::Plate, true);
                        d.text(c, &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink);
                    }
                    Some(m) => {
                        draw_model_2d(d, m, it, &tf, k, ink);
                        // Tag on a small label so it stays readable on the drawing
                        let size = size * 0.8;
                        let half = p(it.tag.chars().count() as f32 * size * 0.33 + size * 0.2, size * 0.6);
                        d.rect(c - half, c + half, 0.0, Ink::Plate, true);
                        d.text(c, &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink);
                    }
                    None => d.text(c, &it.tag, size, Anchor::Center, VAnchor::Middle, text_ink),
                }
                d.rect(a, b, 0.3, ink, false);
            }
        }
    }
    // Cutouts of door devices and accessories (dashed: under the item)
    {
        let under = |o: &crate::production::Opening| e.item(o.item).map(|i| matches!(i.kind, ItemKind::Device | ItemKind::Accessory)).unwrap_or(false);
        for o in crate::production::openings(e, parts, face).iter().filter(|o| under(o)) {
            let c = tf(o.center.x, o.center.y);
            if o.round {
                d.circle(c, o.w / 2.0 * k, 0.15, Ink::Frame, false);
            } else {
                let half = p(o.w / 2.0 * k, o.h / 2.0 * k);
                let (a, b) = (c - half, c + half);
                for (s, t) in [(a, p(b.x, a.y)), (p(b.x, a.y), b), (b, p(a.x, b.y)), (p(a.x, b.y), a)] {
                    d.line(s, t, 0.15, Ink::Frame, true);
                }
            }
        }
    }
    // Drilling pattern (holes hidden under other devices are left out); weld studs as
    // a filled dot.
    for h in crate::production::holes_on(e, parts, face) {
        let covered = e.items.iter().any(|i| i.kind == ItemKind::Device && i.face == face && i.id != h.item && i.rect().contains(h.pos));
        if covered {
            continue;
        }
        let c = tf(h.pos.x, h.pos.y);
        let stud = h.fastening == panel::Fastening::Stud;
        d.circle(c, (h.diameter / 2.0 * k).max(0.25), if stud { 0.0 } else { 0.1 }, Ink::Frame, stud);
    }
}

/// Draws a part's 2D front view into a placed device. Rotated devices (width and
/// height swapped) get the drawing turned by 90°, other size differences are scaled;
/// a strip of equal parts repeats it.
fn draw_model_2d(d: &mut dyn Draw, m: &Model2d, it: &PanelItem, tf: &dyn Fn(f32, f32) -> P, k: f32, ink: Ink) {
    if m.width <= 0.0 || m.height <= 0.0 {
        return;
    }
    let rotation = panel::item_rotation(it, m.height);
    let rotated = rotation.is_quarter();
    let n = if rotated { 1 } else { panel::tiles(it.w, m.width) };
    let cell = (if rotated { it.h } else { it.w }) / n as f32;
    for tile in 0..n {
        draw_model_2d_in(d, m, it, tf, k, ink, tile as f32 * cell, cell);
    }
}

/// A part's 2D front view in the stretch `at .. at + len` along a device.
#[allow(clippy::too_many_arguments)]
fn draw_model_2d_in(d: &mut dyn Draw, m: &Model2d, it: &PanelItem, tf: &dyn Fn(f32, f32) -> P, k: f32, ink: Ink, at: f32, len: f32) {
    if m.width <= 0.0 || m.height <= 0.0 {
        return;
    }
    let rotation = panel::item_rotation(it, m.height);
    let rotated = rotation.is_quarter();
    let (sx, sy) = if rotated { (it.w / m.height, len / m.width) } else { (len / m.width, it.h / m.height) };
    let s = (sx + sy) / 2.0;
    let map = |q: [f32; 2]| {
        let point=panel::front_point(it,rotation,at,len,[q[0]/m.width,q[1]/m.height]);tf(point.x,point.y)
    };
    let angle=if it.rotation.is_some() {-(rotation.degrees() as f32)} else {0.};
    draw_prims_2d_at_angle(d, &m.prims, &map, s * k, 0.15, ink,angle);
}

/// The elements of a terminal strip: each part's front view in its cell, a plain
/// outline where a part has none.
#[allow(clippy::too_many_arguments)]
fn draw_strip_2d(d: &mut dyn Draw, prj: &Project, it: &PanelItem, cells: &[panel::Cell], tf: &dyn Fn(f32, f32) -> P, k: f32, ink: Ink) {
    let rotation=panel::device_rotation(prj,it);
    for c in cells {
        match prj.parts.iter().chain(parts::catalog()).find(|x| x.number == c.part).and_then(|x| prj.symbol_cache.front_view(x)).as_deref() {
            Some(m) => draw_model_2d_in(d, m, it, tf, k, ink, c.at, c.len),
            None => {
                let cell=panel::front_cell(it,rotation,c.at,c.len);
                let (a,b)=(tf(cell.min.x,cell.min.y),tf(cell.max.x,cell.max.y));
                d.rect(a, b, 0.15, ink, false);
            }
        }
    }
}

pub fn draw_prims_2d(d: &mut dyn Draw, prims: &[Prim2d], map: &dyn Fn([f32; 2]) -> P, s: f32, w: f32, ink: Ink) {
    draw_prims_2d_at_angle(d,prims,map,s,w,ink,0.);
}

fn draw_prims_2d_at_angle(d: &mut dyn Draw, prims: &[Prim2d], map: &dyn Fn([f32; 2]) -> P, s: f32, w: f32, ink: Ink,angle:f32) {
    for prim in prims {
        match prim {
            Prim2d::Line { a, b } => d.line(map(*a), map(*b), w, ink, false),
            Prim2d::Rect { min, max, fill } => {
                let corners = [*min, [max[0], min[1]], *max, [min[0], max[1]]].map(map);
                d.poly(&corners, true, if *fill { 0.0 } else { w }, ink, *fill);
            }
            Prim2d::Circle { c, r, fill } => d.circle(map(*c), r * s, if *fill { 0.0 } else { w }, ink, *fill),
            Prim2d::Polyline { pts, closed, fill } => {
                let v: Vec<P> = pts.iter().map(|q| map(*q)).collect();
                d.poly(&v, *closed, if *fill { 0.0 } else { w }, ink, *fill && *closed);
            }
            Prim2d::Text { pos, text, size } => {
                let ink = if ink == Ink::Line { Ink::Text } else { ink };
                if angle==0. {d.text(map(*pos), text, size * s, Anchor::Left, VAnchor::Middle, ink);}
                else {
                    let mut style=TextStyle::plain(size*s,Anchor::Left,VAnchor::Middle);style.angle=angle;
                    d.styled_text(map(*pos),text,&style,ink);
                }
            }
        }
    }
}

/// "Nice" drawing scale 1:N so that `real` fits into `space`.
pub fn drawing_scale(real: (f32, f32), space: (f32, f32)) -> f32 {
    let needed = (real.0 / space.0).max(real.1 / space.1);
    [1.0, 2.0, 2.5, 4.0, 5.0, 10.0, 15.0, 20.0, 25.0, 50.0, 100.0]
        .into_iter()
        .find(|n| *n >= needed)
        .unwrap_or(needed.ceil())
}

fn draw_mounting_page(d: &mut dyn Draw, prj: &Project, id: u64, frame: &crate::frame::PlotFrame) {
    let Some(e) = prj.layout.enclosures.iter().find(|e| e.id == id) else {
        let msg = tr("Enclosure not found - choose another one in the page properties");
        d.text(frame.area().min + p(15.0, 25.0), msg, 4.0, Anchor::Left, VAnchor::Middle, Ink::Error);
        return;
    };
    let a = frame.area();
    let (x0, y0, x1, y1) = (a.min.x + 25.0, a.min.y + 12.0, a.max.x - 15.0, a.max.y - 22.0);
    // The mounting plate, then every other face that carries items (door, walls, roof,
    // bottom) side by side at one scale.
    let faces: Vec<Face> = std::iter::once(Face::Plate).chain(Face::ALL.into_iter().filter(|f| *f != Face::Plate && e.items.iter().any(|i| i.face == *f))).collect();
    let gap = e.width * 0.25;
    let sizes: Vec<(f32, f32)> = faces.iter().map(|f| e.view_size(*f)).collect();
    let total_w = sizes.iter().map(|s| s.0).sum::<f32>() + gap * (faces.len() - 1) as f32;
    let total_h = sizes.iter().map(|s| s.1).fold(0.0, f32::max);
    let n = drawing_scale((total_w, total_h), (x1 - x0, y1 - y0));
    let k = 1.0 / n;
    let (w, h) = (e.width * k, e.height * k);
    let origin = p(x0 + ((x1 - x0) - total_w * k) / 2.0, y0 + ((y1 - y0) - total_h * k) / 2.0);
    let mut x = origin.x;
    for (f, (fw, _)) in faces.iter().zip(&sizes) {
        let o = p(x, origin.y);
        draw_enclosure(d, e, *f, false, o, k, &BTreeSet::new(), &BTreeSet::new(), prj);
        if faces.len() > 1 {
            let title = if *f == Face::Door { tr("Door (front view)") } else { f.title() };
            d.text(p(o.x + fw * k / 2.0, o.y - 3.0), title, 3.0, Anchor::Center, VAnchor::Bottom, Ink::Text);
        }
        x += (fw + gap) * k;
    }
    // Dimensions
    let bottom = origin.y + h + 6.0;
    d.line(p(origin.x, bottom), p(origin.x + w, bottom), 0.18, Ink::Frame, false);
    for x in [origin.x, origin.x + w] {
        d.line(p(x, bottom - 2.0), p(x, bottom + 2.0), 0.18, Ink::Frame, false);
    }
    d.text(p(origin.x + w / 2.0, bottom - 1.5), &format!("{:.0}", e.width), 2.5, Anchor::Center, VAnchor::Bottom, Ink::Text);
    let left = origin.x - 6.0;
    d.line(p(left, origin.y), p(left, origin.y + h), 0.18, Ink::Frame, false);
    for y in [origin.y, origin.y + h] {
        d.line(p(left - 2.0, y), p(left + 2.0, y), 0.18, Ink::Frame, false);
    }
    d.text(p(left - 1.5, origin.y + h / 2.0), &format!("{:.0}", e.height), 2.5, Anchor::Right, VAnchor::Middle, Ink::Text);
    let caption = format!("{}  {}  -  {:.0} x {:.0} x {:.0} mm  -  {}", e.name, e.part, e.width, e.height, e.depth, tr_fmt!("Scale 1:{}", n));
    d.text(p(x0, a.max.y - 8.0), &caption, 3.0, Anchor::Left, VAnchor::Middle, Ink::Text);
}

#[cfg(test)]
mod tests;
