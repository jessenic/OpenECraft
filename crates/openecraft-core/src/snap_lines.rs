//! Invisible grid lines: where a page has connection points or wire ends off the
//! drawing grid (imported drawings, device boxes of any size), their coordinates
//! snap like grid lines, so wires can start at those points and things can be
//! connected to wires that are off the grid.

use crate::geom::{p, P};
use crate::model::{Page, Project};

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

impl SnapLines {
    /// The lines through the connection points and wire ends of `page` that are off
    /// `grid`.
    pub fn of_page(prj: &Project, page: &Page, grid: f32) -> SnapLines {
        let mut points: Vec<P> = page.wires.iter().flat_map(|w| [w.a, w.b]).collect();
        for s in &page.symbols {
            let Some(def) = prj.symbol(&s.sym) else { continue };
            points.extend((0..def.pins.len()).map(|i| def.pin_world(i, s.pos, s.rot, s.mirror)));
        }
        SnapLines { xs: lines(points.iter().map(|q| q.x), grid), ys: lines(points.iter().map(|q| q.y), grid) }
    }

    /// `q` snapped to the grid, each coordinate taking a line instead where one is
    /// nearer than the grid.
    pub fn snap(&self, q: P, grid: f32) -> P {
        p(nearest(&self.xs, q.x, grid), nearest(&self.ys, q.y, grid))
    }
}

#[cfg(test)]
mod tests;
