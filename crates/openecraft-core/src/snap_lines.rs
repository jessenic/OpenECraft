//! Invisible grid lines: where a page has connection points or wire ends off the
//! drawing grid (imported drawings, device boxes of any size), their coordinates
//! snap like grid lines, so wires can start at those points and things can be
//! connected to wires that are off the grid. A symbol being placed or moved snaps
//! from its connection points, so they land on such lines and auto-connect finds them.

use std::collections::BTreeSet;

use crate::geom::{p, P};
use crate::model::{Page, Project, SymbolInst};
use crate::symbols::SymbolDef;

/// Coordinates closer than this are one line, and a coordinate this close to the grid
/// is on it.
const SAME: f32 = 0.01;

/// The invisible grid lines of a page: x of vertical lines, y of horizontal lines,
/// both sorted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SnapLines {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

fn off_grid(v: f32, grid: f32) -> bool {
    ((v / grid).round() * grid - v).abs() > SAME
}

fn lines(values: impl Iterator<Item = f32>, grid: f32) -> Vec<f32> {
    let mut v: Vec<f32> = values.filter(|v| off_grid(*v, grid)).collect();
    v.sort_by(f32::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() <= SAME);
    v
}

/// The grid point nearest to `v`, or a line if it is nearer.
fn nearest(lines: &[f32], v: f32, grid: f32) -> f32 {
    let on_grid = (v / grid).round() * grid;
    let i = lines.partition_point(|l| *l < v);
    [i.checked_sub(1), Some(i)]
        .into_iter()
        .flatten()
        .filter_map(|i| lines.get(i).copied())
        .filter(|l| (l - v).abs() < (on_grid - v).abs())
        .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
        .unwrap_or(on_grid)
}

/// `d` on the grid, or shifted so that one of `points` moved by `d` lands on a line
/// where that is a smaller shift.
fn nearest_from(lines: &[f32], d: f32, points: &[f32], grid: f32) -> f32 {
    let on_grid = (d / grid).round() * grid;
    let line = |q: f32| {
        let i = lines.partition_point(|l| *l < q);
        [i.checked_sub(1), Some(i)].into_iter().flatten().filter_map(|i| lines.get(i).copied())
    };
    points
        .iter()
        .flat_map(|q| line(q + d).map(move |l| l - q))
        .filter(|c| (c - d).abs() < (on_grid - d).abs())
        .min_by(|a, b| (a - d).abs().total_cmp(&(b - d).abs()))
        .unwrap_or(on_grid)
}

/// The connection points of a symbol: those of `def` placed at `s`.
fn pins_of(prj: &Project, s: &SymbolInst) -> Vec<P> {
    let Some(def) = prj.symbol(&s.sym) else { return Vec::new() };
    (0..def.pins.len()).map(|i| def.pin_world(i, s.pos, s.rot, s.mirror)).collect()
}

/// The connection points of the symbols and the ends of the wires among `sel`.
pub fn points_of(prj: &Project, page: &Page, sel: &BTreeSet<u64>) -> Vec<P> {
    let wires = page.wires.iter().filter(|w| sel.contains(&w.id)).flat_map(|w| [w.a, w.b]);
    let pins = page.symbols.iter().filter(|s| sel.contains(&s.id)).flat_map(|s| pins_of(prj, s));
    wires.chain(pins).collect()
}

/// The connection points of `def` placed at the origin as `inst`.
pub fn pins_at_origin(def: &SymbolDef, inst: &SymbolInst) -> Vec<P> {
    (0..def.pins.len()).map(|i| def.pin_world(i, p(0.0, 0.0), inst.rot, inst.mirror)).collect()
}

impl SnapLines {
    /// The lines through the connection points and wire ends of `page` that are off
    /// `grid`.
    pub fn of_page(prj: &Project, page: &Page, grid: f32) -> SnapLines {
        Self::without(prj, page, grid, &[])
    }

    /// The lines of `page` but those only through `moving`, points that move away.
    pub fn without(prj: &Project, page: &Page, grid: f32, moving: &[P]) -> SnapLines {
        let mut points: Vec<P> = page.wires.iter().flat_map(|w| [w.a, w.b]).collect();
        points.extend(page.symbols.iter().flat_map(|s| pins_of(prj, s)));
        points.retain(|q| !moving.iter().any(|m| m.same(*q)));
        SnapLines { xs: lines(points.iter().map(|q| q.x), grid), ys: lines(points.iter().map(|q| q.y), grid) }
    }

    /// `q` snapped to the grid, each coordinate taking a line instead where one is
    /// nearer than the grid.
    pub fn snap(&self, q: P, grid: f32) -> P {
        p(nearest(&self.xs, q.x, grid), nearest(&self.ys, q.y, grid))
    }

    /// The offset `d` for `points` (connection points of a symbol placed at `d`, or
    /// of a selection moved by `d`): on the grid, each coordinate shifted instead so
    /// that a point lands on a line where that is nearer.
    pub fn snap_from(&self, d: P, points: &[P], grid: f32) -> P {
        let (xs, ys): (Vec<f32>, Vec<f32>) = points.iter().map(|q| (q.x, q.y)).unzip();
        p(nearest_from(&self.xs, d.x, &xs, grid), nearest_from(&self.ys, d.y, &ys, grid))
    }
}

#[cfg(test)]
mod tests;
