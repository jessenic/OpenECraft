//! Basic 2D geometry in page coordinates (millimetres, y pointing down).

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Neg, Sub};

/// Drawing grid in millimetres (EPLAN IEC default is 4 mm).
pub const GRID: f32 = 4.0;

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

pub const fn p(x: f32, y: f32) -> P {
    P { x, y }
}

impl P {
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn dist(self, o: P) -> f32 {
        (self - o).len()
    }

    pub fn snap(self, grid: f32) -> P {
        p((self.x / grid).round() * grid, (self.y / grid).round() * grid)
    }

    /// Rotate by `quarter` quarter turns clockwise (as seen on screen, y down)
    /// after optionally mirroring along the vertical axis.
    pub fn transform(self, rot: u8, mirror: bool) -> P {
        let mut q = if mirror { p(-self.x, self.y) } else { self };
        for _ in 0..(rot % 4) {
            q = p(-q.y, q.x);
        }
        q
    }

    /// Integer key used for connectivity (0.01 mm resolution).
    pub fn key(self) -> (i64, i64) {
        ((self.x * 100.0).round() as i64, (self.y * 100.0).round() as i64)
    }

    pub fn same(self, o: P) -> bool {
        self.key() == o.key()
    }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
}

impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for P {
    type Output = P;
    fn mul(self, s: f32) -> P {
        p(self.x * s, self.y * s)
    }
}

impl Neg for P {
    type Output = P;
    fn neg(self) -> P {
        p(-self.x, -self.y)
    }
}

/// Axis aligned rectangle.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct R {
    pub min: P,
    pub max: P,
}

impl R {
    pub fn from_points(a: P, b: P) -> R {
        R {
            min: p(a.x.min(b.x), a.y.min(b.y)),
            max: p(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn nothing() -> R {
        R {
            min: p(f32::INFINITY, f32::INFINITY),
            max: p(f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y
    }

    pub fn extend(&mut self, q: P) {
        self.min.x = self.min.x.min(q.x);
        self.min.y = self.min.y.min(q.y);
        self.max.x = self.max.x.max(q.x);
        self.max.y = self.max.y.max(q.y);
    }

    pub fn union(&mut self, o: R) {
        if o.is_valid() {
            self.extend(o.min);
            self.extend(o.max);
        }
    }

    pub fn expand(self, m: f32) -> R {
        R {
            min: p(self.min.x - m, self.min.y - m),
            max: p(self.max.x + m, self.max.y + m),
        }
    }

    pub fn contains(&self, q: P) -> bool {
        q.x >= self.min.x && q.x <= self.max.x && q.y >= self.min.y && q.y <= self.max.y
    }

    pub fn contains_rect(&self, o: &R) -> bool {
        self.contains(o.min) && self.contains(o.max)
    }

    pub fn center(&self) -> P {
        p((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Do the two rectangles share more than an edge?
    pub fn overlaps(&self, o: &R) -> bool {
        self.min.x < o.max.x && o.min.x < self.max.x && self.min.y < o.max.y && o.min.y < self.max.y
    }

    /// Does the segment `a`-`b` run through the rectangle (Liang–Barsky clipping)?
    pub fn crosses(&self, a: P, b: P) -> bool {
        let d = b - a;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for (pp, q) in [(-d.x, a.x - self.min.x), (d.x, self.max.x - a.x), (-d.y, a.y - self.min.y), (d.y, self.max.y - a.y)] {
            if pp == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let t = q / pp;
                if pp < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    }
}

/// Distance from point `q` to segment `a`-`b`.
pub fn dist_to_segment(q: P, a: P, b: P) -> f32 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 <= f32::EPSILON {
        return q.dist(a);
    }
    let t = (((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / l2).clamp(0.0, 1.0);
    q.dist(a + ab * t)
}

/// True if `q` lies on the segment `a`-`b` strictly between its end points.
pub fn on_segment_interior(q: P, a: P, b: P) -> bool {
    if q.same(a) || q.same(b) {
        return false;
    }
    dist_to_segment(q, a, b) < 0.01
}

#[cfg(test)]
mod tests;
