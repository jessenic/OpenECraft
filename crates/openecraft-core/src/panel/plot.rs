//! The plot of the plant ([`Plot`]): its boundary, drawn in the site plan, which the
//! buildings stay inside (a room with its outer walls, a roof or a cabinet standing
//! outside it is a problem), and the georeferenced point ([`GeoReference`]) every point
//! of the plant is measured from: plant coordinates (x, z in mm, y up from the ground)
//! to easting, northing and height in its coordinate reference system and back
//! ([`to_geo`], [`from_geo`]).

use super::{inside_outline, outline, roofs, site_poses, Layout, SitePose};
use super::buildings::{building_of, building_pose};
use crate::issue::{Issue, Severity};
use crate::model::panel::GeoReference;
use crate::model::Project;
use crate::parts::vec3::V3;
use openecraft_i18n::tr;

/// Corners of the boundary closer than this are one, mm.
const MIN_EDGE: f32 = 10.0;
/// A point this close to the boundary lies on it, mm.
const ON_LINE: f64 = 1.0;

/// Sets the boundary of the plot (corners in the plant, x and z in mm); an empty list
/// removes it.
pub fn set_outline(prj: &mut Project, pts: Vec<[f32; 2]>) -> Result<(), String> {
    let pts = cleaned(pts);
    if !pts.is_empty() {
        check_outline(&pts)?;
    }
    prj.layout.plot.outline = pts;
    Ok(())
}

/// The corners without repeated ones, the closing corner equal to the first, and
/// corners on a straight line between their neighbours.
pub fn cleaned(mut pts: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < MIN_EDGE;
    pts.dedup_by(|a, b| near(*a, *b));
    while pts.len() > 1 && near(pts[0], pts[pts.len() - 1]) {
        pts.pop();
    }
    let mut k = 0;
    while pts.len() > 3 && k < pts.len() {
        let n = pts.len();
        let (a, b, c) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
        let cross = f64::from(b[0] - a[0]) * f64::from(c[1] - b[1]) - f64::from(b[1] - a[1]) * f64::from(c[0] - b[0]);
        let len = f64::from((c[0] - a[0]).hypot(c[1] - a[1])).max(1.0);
        if (cross / len).abs() < ON_LINE {
            pts.remove(k);
        } else {
            k += 1;
        }
    }
    pts
}

/// The boundary is a simple polygon: at least three corners, an area, no edge crossing
/// another one.
pub fn check_outline(pts: &[[f32; 2]]) -> Result<(), String> {
    if pts.iter().flatten().any(|v| !v.is_finite()) {
        return Err(tr("The corners of the plot boundary must be numbers").to_string());
    }
    if pts.len() < 3 {
        return Err(tr("A plot boundary has at least three corners").to_string());
    }
    if area(pts) < 1.0 {
        return Err(tr("The plot boundary encloses no area").to_string());
    }
    if self_crossing(pts) {
        return Err(tr("The plot boundary crosses itself").to_string());
    }
    Ok(())
}

fn edges(pts: &[[f32; 2]]) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    let n = pts.len();
    (0..n).map(move |i| (wide(pts[i]), wide(pts[(i + 1) % n])))
}

fn wide(q: [f32; 2]) -> [f64; 2] {
    [f64::from(q[0]), f64::from(q[1])]
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Distance of `q` from the segment `a`–`b`.
fn segment_distance(a: [f64; 2], b: [f64; 2], q: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 { (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (q[0] - a[0] - t * d[0]).hypot(q[1] - a[1] - t * d[1])
}

/// The segments cross each other, not only touch (at a corner or along a line).
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let tol = |p: [f64; 2], q: [f64; 2]| ON_LINE * (q[0] - p[0]).hypot(q[1] - p[1]).max(1.0);
    let (o1, o2) = (orient(a, b, c), orient(a, b, d));
    let (o3, o4) = (orient(c, d, a), orient(c, d, b));
    let (t1, t2) = (tol(a, b), tol(c, d));
    ((o1 > t1 && o2 < -t1) || (o1 < -t1 && o2 > t1)) && ((o3 > t2 && o4 < -t2) || (o3 < -t2 && o4 > t2))
}

/// Two edges that are not neighbours cross or touch.
fn self_crossing(pts: &[[f32; 2]]) -> bool {
    let e: Vec<_> = edges(pts).collect();
    let n = e.len();
    (0..n).any(|i| {
        (i + 1..n).filter(|&j| j != i + 1 && !(i == 0 && j == n - 1)).any(|j| {
            let ((a, b), (c, d)) = (e[i], e[j]);
            cross(a, b, c, d) || [segment_distance(a, b, c), segment_distance(a, b, d), segment_distance(c, d, a), segment_distance(c, d, b)].iter().any(|&x| x < ON_LINE)
        })
    })
}

/// Area of a polygon, m².
pub fn area(pts: &[[f32; 2]]) -> f64 {
    edges(pts).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f64>().abs() / 2.0 / 1e6
}

/// Length of the boundary, m.
pub fn perimeter(pts: &[[f32; 2]]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    edges(pts).map(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1])).sum::<f64>() / 1000.0
}

/// The point lies inside the boundary or on it.
pub fn inside_or_on(pts: &[[f32; 2]], q: [f32; 2]) -> bool {
    inside_outline(pts, q) || edges(pts).any(|(a, b)| segment_distance(a, b, wide(q)) < ON_LINE)
}

/// Polygon `inner` lies inside the boundary `pts` (touching it from inside counts).
pub fn contains(pts: &[[f32; 2]], inner: &[[f32; 2]]) -> bool {
    let n = inner.len();
    let mids = (0..n).map(|i| {
        let (a, b) = (inner[i], inner[(i + 1) % n]);
        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
    });
    inner.iter().copied().chain(mids).all(|q| inside_or_on(pts, q)) && !edges(inner).any(|(a, b)| edges(pts).any(|(c, d)| cross(a, b, c, d)))
}

/// A polygon grown outwards by `d` (corners moved along their bisectors).
pub fn grown(pts: &[[f32; 2]], d: f32) -> Vec<[f32; 2]> {
    let n = pts.len();
    if n < 3 || d == 0.0 {
        return pts.to_vec();
    }
    // Counter-clockwise (in x, z) the outside is right of the edges.
    let sign = if edges(pts).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f64>() > 0.0 { 1.0 } else { -1.0 };
    let normal = |a: [f32; 2], b: [f32; 2]| {
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-6);
        [sign * (b[1] - a[1]) / l, -sign * (b[0] - a[0]) / l]
    };
    (0..n)
        .map(|i| {
            let (p, q, r) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (n1, n2) = (normal(p, q), normal(q, r));
            let k = 1.0 + n1[0] * n2[0] + n1[1] * n2[1];
            let m = if k > 1e-3 { [(n1[0] + n2[0]) / k, (n1[1] + n2[1]) / k] } else { n1 };
            [q[0] + m[0] * d, q[1] + m[1] * d]
        })
        .collect()
}

fn in_plant(pose: SitePose, pts: &[[f32; 2]]) -> Vec<[f32; 2]> {
    pts.iter()
        .map(|&[x, z]| {
            let q = pose.apply([x, 0.0, z]);
            [q[0], q[2]]
        })
        .collect()
}

/// What stands in the plant seen from above (x, z): every room with its outer walls,
/// every cabinet outside the rooms and every roof with its overhang.
pub fn footprints(prj: &Project) -> Vec<(Standing, Vec<[f32; 2]>)> {
    let layout = &prj.layout;
    let poses = site_poses(prj);
    let mut out = Vec::new();
    for (i, e) in layout.enclosures.iter().enumerate() {
        if e.is_room() {
            out.push((Standing::Room(i), in_plant(poses[i], &grown(&outline(e), e.wall()))));
        } else if e.room == 0 {
            out.push((Standing::Cabinet(i), in_plant(poses[i], &[[0.0, 0.0], [e.width, 0.0], [e.width, e.depth], [0.0, e.depth]])));
        }
    }
    for (bi, b) in layout.buildings.iter().enumerate() {
        for (k, r) in b.roofs.iter().enumerate() {
            for slope in roofs::slopes(r) {
                out.push((Standing::Roof(bi, k), in_plant(building_pose(b), &slope)));
            }
        }
    }
    out
}

/// Something standing in the plant: a room, a cabinet (indices of enclosures) or a
/// roof (building, roof).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Room(usize),
    Cabinet(usize),
    Roof(usize, usize),
}

/// What stands outside the boundary of the plot (each once); nothing without one.
pub fn outside(prj: &Project) -> Vec<Standing> {
    let pts = &prj.layout.plot.outline;
    if check_outline(pts).is_err() {
        return Vec::new();
    }
    let mut out: Vec<Standing> = Vec::new();
    for (s, f) in footprints(prj) {
        if !out.contains(&s) && !contains(pts, &f) {
            out.push(s);
        }
    }
    out
}

/// The problems of the plot: a boundary that is no polygon, rooms and cabinets
/// standing outside it, roofs reaching over it, a reference point with coordinates
/// that are no numbers.
pub fn problems(prj: &Project) -> Vec<Issue> {
    let layout = &prj.layout;
    let plot = &layout.plot;
    let mut v = Vec::new();
    if let Some(r) = &plot.reference {
        if [r.easting, r.northing, r.height].iter().any(|x| !x.is_finite()) || [r.x, r.z, r.y, r.north].iter().any(|x| !x.is_finite()) {
            v.push(crate::issue!(Severity::Warning, None, "The coordinates of the reference point {} are not valid", r.name));
        }
    }
    if plot.outline.is_empty() {
        return v;
    }
    if check_outline(&plot.outline).is_err() {
        v.push(crate::issue!(Severity::Warning, None, "The plot boundary is no polygon: it crosses itself or has fewer than three corners"));
        return v;
    }
    for s in outside(prj) {
        v.push(match s {
            Standing::Room(ri) => {
                let e = &layout.enclosures[ri];
                match building_of(layout, ri) {
                    Some(bi) => crate::issue!(Severity::Error, None, "{}: room {} stands outside the plot boundary", layout.buildings[bi].name, e.name),
                    None => crate::issue!(Severity::Error, None, "Room {} stands outside the plot boundary", e.name),
                }
            }
            Standing::Cabinet(ci) => crate::issue!(Severity::Warning, None, "{} stands outside the plot boundary", layout.enclosures[ci].name),
            Standing::Roof(bi, k) => {
                let b = &layout.buildings[bi];
                crate::issue!(Severity::Warning, None, "{}: roof {} reaches over the plot boundary", b.name, b.roofs[k].name)
            }
        });
    }
    v
}

/// Unit vectors of east and of north in the plant (x, z).
pub fn axes(r: &GeoReference) -> ([f64; 2], [f64; 2]) {
    let (s, c) = f64::from(r.north).to_radians().sin_cos();
    ([c, -s], [-s, -c])
}

/// Easting, northing and height (m) of a point of the plant (mm, y up from the ground).
pub fn to_geo(r: &GeoReference, q: V3) -> [f64; 3] {
    let d = [f64::from(q[0] - r.x) / 1000.0, f64::from(q[2] - r.z) / 1000.0];
    let (e, n) = axes(r);
    [r.easting + d[0] * e[0] + d[1] * e[1], r.northing + d[0] * n[0] + d[1] * n[1], r.height + f64::from(q[1] - r.y) / 1000.0]
}

/// The point of the plant (mm) at an easting, northing and height (m).
pub fn from_geo(r: &GeoReference, g: [f64; 3]) -> V3 {
    let (de, dn) = (g[0] - r.easting, g[1] - r.northing);
    let (e, n) = axes(r);
    let (dx, dz) = (de * e[0] + dn * n[0], de * e[1] + dn * n[1]);
    [r.x + (dx * 1000.0) as f32, r.y + ((g[2] - r.height) * 1000.0) as f32, r.z + (dz * 1000.0) as f32]
}

/// How far a point of the plant (x, z in mm) is east and north of the reference point, m.
pub fn from_reference(r: &GeoReference, q: [f32; 2]) -> [f64; 2] {
    let g = to_geo(r, [q[0], r.y, q[1]]);
    [g[0] - r.easting, g[1] - r.northing]
}

/// The reference point of the plot, or the origin of the plant with north towards the
/// rear when there is none.
pub fn reference(layout: &Layout) -> GeoReference {
    layout.plot.reference.clone().unwrap_or_default()
}

/// A measure between two points of the plant (x, z in mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Distance, mm.
    pub distance: f32,
    /// How far `b` is east and north of `a`, m.
    pub east: f64,
    pub north: f64,
    /// Direction from `a` to `b`, degrees clockwise from grid north.
    pub bearing: f64,
}

pub fn measure(layout: &Layout, a: [f32; 2], b: [f32; 2]) -> Measure {
    let r = reference(layout);
    let (pa, pb) = (from_reference(&r, a), from_reference(&r, b));
    let (east, north) = (pb[0] - pa[0], pb[1] - pa[1]);
    Measure { distance: (b[0] - a[0]).hypot(b[1] - a[1]), east, north, bearing: east.atan2(north).to_degrees().rem_euclid(360.0) }
}

/// Sets or removes the reference point; the direction of north is kept in 0 – 360°.
pub fn set_reference(prj: &mut Project, r: Option<GeoReference>) -> Result<(), String> {
    if let Some(r) = &r {
        if [r.easting, r.northing, r.height].iter().any(|x| !x.is_finite()) || [r.x, r.z, r.y, r.north].iter().any(|x| !x.is_finite()) {
            return Err(tr("The coordinates of the reference point must be numbers").to_string());
        }
        if r.latitude.is_some_and(|l| !(-90.0..=90.0).contains(&l)) || r.longitude.is_some_and(|l| !(-180.0..=180.0).contains(&l)) {
            return Err(tr("Latitude is within ±90° and longitude within ±180°").to_string());
        }
    }
    prj.layout.plot.reference = r.map(|r| GeoReference { north: r.north.rem_euclid(360.0), ..r });
    Ok(())
}

#[cfg(test)]
mod tests;
