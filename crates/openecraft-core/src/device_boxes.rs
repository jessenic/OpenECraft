//! Freely drawn device boxes (in the spirit of EPLAN's device boxes) and the device
//! connection points placed on them. A box is a symbol [`DEVICE_BOX`] whose outline is
//! [`SymbolInst::size`]; a connection point is a symbol with [`Func::Connection`] and
//! one connection point. A point belongs to the device of its tag: placed on a box it
//! takes the box's tag, and the points of a box (its device, on its outline or inside)
//! move, turn, copy and delete with it. Different points are different connections.

use openecraft_i18n::tr;
use crate::geom::{dist_to_segment, p, P, R, GRID};
use crate::model::{Page, Project, SymbolInst};
use crate::ops;
use crate::symbols::{Func, DEVICE_BOX, DEVICE_POINT};
use std::collections::BTreeSet;

/// Points closer than this lie on the outline.
const TOUCH: f32 = 0.01;

pub fn is_box(s: &SymbolInst) -> bool {
    s.sym == DEVICE_BOX
}

/// A device connection point (built-in or a user symbol with that function).
pub fn is_point(prj: &Project, s: &SymbolInst) -> bool {
    s.sym == DEVICE_POINT || prj.symbol(&s.sym).is_some_and(|d| d.func == Func::Connection)
}

/// Outline of a box on its page.
pub fn rect(prj: &Project, s: &SymbolInst) -> Option<R> {
    prj.symbol(&s.sym).map(|d| s.bounds(&d))
}

/// Does `q` lie on the outline of `r` (within `tol`)?
pub fn outline_hit(r: R, q: P, tol: f32) -> bool {
    let c = [r.min, p(r.max.x, r.min.y), r.max, p(r.min.x, r.max.y)];
    (0..4).any(|i| dist_to_segment(q, c[i], c[(i + 1) % 4]) <= tol)
}

/// The box at `q` (on its outline or inside); the smallest one where boxes nest.
pub fn box_at<'a>(prj: &Project, page: &'a Page, q: P) -> Option<&'a SymbolInst> {
    page.symbols
        .iter()
        .filter(|s| is_box(s))
        .filter_map(|s| rect(prj, s).filter(|r| r.expand(TOUCH).contains(q)).map(|r| (r.width() * r.height(), s)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, s)| s)
}

/// The connection points of a box: the device connection points of its device on the
/// page that lie on its outline or inside.
pub fn points_of(prj: &Project, page: &Page, box_id: u64) -> Vec<u64> {
    let Some(b) = page.symbols.iter().find(|s| s.id == box_id && is_box(s)) else { return Vec::new() };
    let Some(r) = rect(prj, b) else { return Vec::new() };
    if b.tag.is_empty() {
        return Vec::new();
    }
    page.symbols.iter().filter(|s| s.tag == b.tag && is_point(prj, s) && r.expand(TOUCH).contains(s.pos)).map(|s| s.id).collect()
}

/// The selection with the connection points of the boxes in it (move, turn, copy and
/// delete take a box with its points).
pub fn with_points(prj: &Project, page_i: usize, sel: &BTreeSet<u64>) -> BTreeSet<u64> {
    let page = &prj.pages[page_i];
    let mut all = sel.clone();
    for s in page.symbols.iter().filter(|s| sel.contains(&s.id) && is_box(s)) {
        all.extend(points_of(prj, page, s.id));
    }
    all
}

/// Is the point on (or inside) a box of its own device? Its tag is not shown then:
/// the box shows it.
pub fn on_own_box(prj: &Project, page: &Page, s: &SymbolInst) -> bool {
    !s.tag.is_empty()
        && page.symbols.iter().any(|b| is_box(b) && b.tag == s.tag && rect(prj, b).is_some_and(|r| r.expand(TOUCH).contains(s.pos)))
}

/// Increments the number a designation ends with (`13` → `14`, `X9` → `X10`,
/// `1.9` → `1.10`); None when it ends without one.
pub fn increment(designation: &str) -> Option<String> {
    let digits = designation.chars().rev().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let (head, num) = designation.split_at(designation.len() - digits);
    let n: u64 = num.parse().ok()?;
    Some(format!("{head}{}", n + 1))
}

/// Designation of the next connection point of a device: one more than the highest
/// number a designation of its points ends with (`1` for the first one).
pub fn next_designation(prj: &Project, tag: &str) -> String {
    let mut best: Option<(u64, String)> = None;
    for s in prj.pages.iter().flat_map(|pg| pg.symbols.iter()).filter(|s| s.tag == tag && is_point(prj, s)) {
        let Some(d) = s.pins.first() else { continue };
        let digits = d.chars().rev().take_while(char::is_ascii_digit).count();
        let Ok(n) = d[d.len() - digits..].parse::<u64>() else { continue };
        if best.as_ref().is_none_or(|(b, _)| n > *b) {
            best = Some((n, d.clone()));
        }
    }
    best.and_then(|(_, d)| increment(&d)).unwrap_or_else(|| "1".into())
}

/// The direction a connection point at `q` on the outline of a box leads away from it
/// (quarter turns clockwise from upwards: top edge 0, right 1, bottom 2, left 3);
/// None off the outline (corners count to the top and bottom edges).
pub fn direction_at(prj: &Project, page: &Page, q: P) -> Option<u8> {
    let r = rect(prj, box_at(prj, page, q)?)?;
    let on = |a: f32, b: f32| (a - b).abs() < TOUCH;
    if on(q.y, r.min.y) {
        Some(0)
    } else if on(q.y, r.max.y) {
        Some(2)
    } else if on(q.x, r.max.x) {
        Some(1)
    } else if on(q.x, r.min.x) {
        Some(3)
    } else {
        None
    }
}

/// The tag a new connection point at `q` takes: the device of the box there.
pub fn tag_at(prj: &Project, page_i: usize, q: P) -> Option<String> {
    box_at(prj, &prj.pages[page_i], q).map(|b| b.tag.clone()).filter(|t| !t.is_empty())
}

/// Draws a box with the corners `a` and `b` (on the grid) as a new device (`tag`
/// empty: the next free `-A` tag in the page structure). Returns its id.
pub fn add_box(prj: &mut Project, page_i: usize, a: P, b: P, tag: &str, function_text: &str) -> Result<u64, String> {
    let r = R::from_points(a.snap(GRID), b.snap(GRID));
    if r.width() < GRID || r.height() < GRID {
        return Err(tr("A device box needs a width and a height of at least one grid step (4 mm)").into());
    }
    let def = prj.symbol(DEVICE_BOX).ok_or("devbox")?;
    let mut s = ops::new_symbol(prj, page_i, &ops::PlaceCtx::default(), &def, r.min, 0, false);
    if !tag.trim().is_empty() {
        s.tag = crate::structure::Dt::of_page(&prj.pages[page_i]).complete(tag.trim());
    }
    s.function_text = function_text.into();
    s.size = Some(p(r.width(), r.height()));
    let id = s.id;
    prj.pages[page_i].symbols.push(s);
    Ok(id)
}

/// Places a device connection point at `pos` (on the grid), its connection leading
/// away `rot` quarter turns clockwise from upwards, for the device `tag` (empty: the
/// device of the box there) with `designation` (empty: the next number). Returns its
/// id.
pub fn place_point(prj: &mut Project, page_i: usize, pos: P, rot: u8, tag: &str, designation: &str) -> Result<u64, String> {
    let pos = pos.snap(GRID);
    let tag = match tag.trim() {
        "" => tag_at(prj, page_i, pos).ok_or_else(|| tr("There is no device box at this point: give the device tag").to_string())?,
        t => crate::structure::Dt::of_page(&prj.pages[page_i]).complete(t),
    };
    let def = prj.symbol(DEVICE_POINT).ok_or("devconn")?;
    let mut s = ops::new_symbol(prj, page_i, &ops::PlaceCtx::default(), &def, pos, rot % 4, false);
    s.tag = tag;
    s.pins = vec![match designation.trim() {
        "" => next_designation(prj, &s.tag),
        d => d.to_string(),
    }];
    let id = s.id;
    prj.pages[page_i].symbols.push(s);
    Ok(id)
}

/// Gives a box the outline `to` (corners on the grid). Connection points on an edge
/// that moves go with it, their wires follow; points inside stay.
pub fn resize(prj: &mut Project, page_i: usize, box_id: u64, to: R) -> Result<(), String> {
    let to = R::from_points(to.min.snap(GRID), to.max.snap(GRID));
    if to.width() < GRID || to.height() < GRID {
        return Err(tr("A device box needs a width and a height of at least one grid step (4 mm)").into());
    }
    let page = &prj.pages[page_i];
    let Some(b) = page.symbols.iter().find(|s| s.id == box_id && is_box(s)) else { return Err(tr("No device box with this id on the page").into()) };
    let from = rect(prj, b).ok_or("devbox")?;
    let points = points_of(prj, page, box_id);
    let edge = |v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32| {
        if (v - lo).abs() < TOUCH {
            nlo
        } else if (v - hi).abs() < TOUCH {
            nhi
        } else {
            v
        }
    };
    let symbols = prj.symbol_table();
    for id in points {
        let page = &mut prj.pages[page_i];
        let Some(s) = page.symbols.iter().find(|s| s.id == id) else { continue };
        let q = p(edge(s.pos.x, from.min.x, from.max.x, to.min.x, to.max.x), edge(s.pos.y, from.min.y, from.max.y, to.min.y, to.max.y));
        let d = q - s.pos;
        if d.x != 0.0 || d.y != 0.0 {
            let one = BTreeSet::from([id]);
            let attached = ops::attached_wire_ends(&symbols, page, &one);
            ops::translate(page, &one, &attached, d);
        }
    }
    if let Some(b) = prj.pages[page_i].symbols.iter_mut().find(|s| s.id == box_id) {
        b.pos = to.min;
        b.rot = 0;
        b.mirror = false;
        b.size = Some(p(to.width(), to.height()));
    }
    ops::cleanup_wires(&mut prj.pages[page_i]);
    Ok(())
}

/// Gives a box another device tag; its connection points go with it.
pub fn set_tag(prj: &mut Project, page_i: usize, box_id: u64, tag: &str) {
    let points = points_of(prj, &prj.pages[page_i], box_id);
    for s in prj.pages[page_i].symbols.iter_mut().filter(|s| s.id == box_id || points.contains(&s.id)) {
        s.tag = tag.into();
    }
}

/// Assigns a connection point to a box: it becomes a connection of the box's device.
pub fn assign(prj: &mut Project, page_i: usize, point_id: u64, box_id: u64) -> Result<(), String> {
    let page = &prj.pages[page_i];
    let Some(b) = page.symbols.iter().find(|s| s.id == box_id && is_box(s)) else { return Err(tr("No device box with this id on the page").into()) };
    if b.tag.is_empty() {
        return Err(tr("The device box has no device tag").into());
    }
    let tag = b.tag.clone();
    if !page.symbols.iter().any(|s| s.id == point_id && is_point(prj, s)) {
        return Err(tr("No device connection point with this id on the page").into());
    }
    if let Some(s) = prj.pages[page_i].symbols.iter_mut().find(|s| s.id == point_id) {
        s.tag = tag;
    }
    Ok(())
}

/// The boxes on a page (id, tag, function text) in the order they were drawn.
pub fn boxes(page: &Page) -> Vec<(u64, String, String)> {
    page.symbols.iter().filter(|s| is_box(s)).map(|s| (s.id, s.tag.clone(), s.function_text.clone())).collect()
}

/// The box a connection point sits on (its device's box there), if any.
pub fn box_of(prj: &Project, page: &Page, point: &SymbolInst) -> Option<u64> {
    page.symbols.iter().filter(|b| is_box(b) && b.tag == point.tag && !b.tag.is_empty()).find(|b| points_of(prj, page, b.id).contains(&point.id)).map(|b| b.id)
}

#[cfg(test)]
mod tests;
