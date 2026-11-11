//! Small 3D vector helpers and an orbit camera, shared by the 3D views.

pub type V3 = [f32; 3];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Euclidean distance.
pub fn dist(a: V3, b: V3) -> f32 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

/// Manhattan distance (sum of the axis distances).
pub fn manhattan(a: V3, b: V3) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

pub type Tri = [V3; 3];

/// Bounding box (min, max) of triangles.
pub fn tri_bounds(tris: &[Tri]) -> (V3, V3) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in tris.iter().flatten() {
        for k in 0..3 {
            min[k] = min[k].min(v[k]);
            max[k] = max[k].max(v[k]);
        }
    }
    (min, max)
}

fn boxes_overlap(a: (V3, V3), b: (V3, V3), eps: f32) -> bool {
    (0..3).all(|k| a.0[k] < b.1[k] - eps && b.0[k] < a.1[k] - eps)
}

/// True if the segment p0–p1 passes through the inside of the triangle (touching
/// edges, end points and segments lying in the triangle's plane do not count).
pub fn segment_crosses_triangle(p0: V3, p1: V3, t: &Tri) -> bool {
    const EPS: f32 = 1e-4;
    let dir = sub(p1, p0);
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(dir, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-9 {
        return false;
    }
    let f = 1.0 / a;
    let s = sub(p0, t[0]);
    let u = f * dot(s, h);
    let q = cross(s, e1);
    let v = f * dot(dir, q);
    let along = f * dot(e2, q);
    u > EPS && v > EPS && u + v < 1.0 - EPS && along > 1e-6 && along < 1.0 - 1e-6
}

/// Ray parity test: is `q` inside the closed surface(s) made of `tris`?
pub fn point_inside(q: V3, tris: &[Tri]) -> bool {
    // A slightly skewed ray avoids hitting edges of axis aligned models exactly.
    let far = [q[0] + 1.0e4, q[1] + 170.0, q[2] + 230.0];
    tris.iter().filter(|t| segment_crosses_triangle(q, far, t)).count() % 2 == 1
}

/// True if two triangle surfaces penetrate each other or one lies inside the
/// other. Surfaces that only touch do not count.
pub fn surfaces_intersect(a: &[Tri], b: &[Tri]) -> bool {
    let (ba, bb) = (tri_bounds(a), tri_bounds(b));
    if a.is_empty() || b.is_empty() || !boxes_overlap(ba, bb, 1e-4) {
        return false;
    }
    let edges_cross = |x: &[Tri], y: &[Tri], by: (V3, V3)| {
        x.iter().any(|t| {
            (0..3).any(|i| {
                let (p0, p1) = (t[i], t[(i + 1) % 3]);
                let eb = (
                    [p0[0].min(p1[0]), p0[1].min(p1[1]), p0[2].min(p1[2])],
                    [p0[0].max(p1[0]), p0[1].max(p1[1]), p0[2].max(p1[2])],
                );
                // Quick reject of edges outside the other surface.
                if (0..3).any(|k| eb.1[k] < by.0[k] || eb.0[k] > by.1[k]) {
                    return false;
                }
                y.iter().any(|u| segment_crosses_triangle(p0, p1, u))
            })
        })
    };
    if edges_cross(a, b, bb) || edges_cross(b, a, ba) {
        return true;
    }
    // No crossing edges: one surface may lie completely inside the other.
    let centroid = |t: &Tri| [(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0, (t[0][2] + t[1][2] + t[2][2]) / 3.0];
    point_inside(centroid(&a[0]), b) || point_inside(centroid(&b[0]), a)
}

/// The 12 triangles of a box (outward winding).
pub fn box_triangles(min: V3, max: V3) -> Vec<Tri> {
    let c = |x: usize, y: usize, z: usize| [if x == 0 { min[0] } else { max[0] }, if y == 0 { min[1] } else { max[1] }, if z == 0 { min[2] } else { max[2] }];
    let quads = [
        [c(0, 0, 0), c(0, 0, 1), c(0, 1, 1), c(0, 1, 0)],
        [c(1, 0, 0), c(1, 1, 0), c(1, 1, 1), c(1, 0, 1)],
        [c(0, 0, 0), c(1, 0, 0), c(1, 0, 1), c(0, 0, 1)],
        [c(0, 1, 0), c(0, 1, 1), c(1, 1, 1), c(1, 1, 0)],
        [c(0, 0, 0), c(0, 1, 0), c(1, 1, 0), c(1, 0, 0)],
        [c(0, 0, 1), c(1, 0, 1), c(1, 1, 1), c(0, 1, 1)],
    ];
    quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
}

/// The order in which to paint boxes (min, max) seen from `eye`, far to near: a box comes
/// after every box it may hide. Two boxes are ordered by a plane between them (the box on
/// the eye's side may hide the other); overlapping boxes, boxes without a constraint and
/// cycles go by the distance of their centres.
pub fn paint_order(boxes: &[(V3, V3)], eye: V3) -> Vec<usize> {
    let n = boxes.len();
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut before = vec![0usize; n];
    for i in 0..n {
        for j in i + 1..n {
            match (may_hide(&boxes[i], &boxes[j], eye), may_hide(&boxes[j], &boxes[i], eye)) {
                (true, false) => {
                    after[j].push(i);
                    before[i] += 1;
                }
                (false, true) => {
                    after[i].push(j);
                    before[j] += 1;
                }
                _ => {}
            }
        }
    }
    let dist = |b: &(V3, V3)| {
        let c = [0, 1, 2].map(|k| (b.0[k] + b.1[k]) / 2.0);
        dot(sub(c, eye), sub(c, eye))
    };
    let far: Vec<f32> = boxes.iter().map(dist).collect();
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for _ in 0..n {
        let farthest = |ready: bool| (0..n).filter(|&i| !done[i] && (!ready || before[i] == 0)).max_by(|&a, &b| far[a].total_cmp(&far[b]));
        let Some(i) = farthest(true).or_else(|| farthest(false)) else { break };
        done[i] = true;
        order.push(i);
        for &j in &after[i] {
            before[j] = before[j].saturating_sub(1);
        }
    }
    order
}

/// Whether box `a` may hide box `b` seen from `eye`: not when a plane between them has
/// the eye on the side of `b`.
pub fn may_hide(a: &(V3, V3), b: &(V3, V3), eye: V3) -> bool {
    !(0..3).any(|k| (a.1[k] <= b.0[k] && eye[k] >= a.1[k]) || (b.1[k] <= a.0[k] && eye[k] <= a.0[k]))
}

/// The part `t0 .. t1` (0 at `a`, 1 at `b`) of a segment inside a box, if any.
pub fn segment_in_box(a: V3, b: V3, (lo, hi): (V3, V3)) -> Option<(f32, f32)> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        let d = b[k] - a[k];
        if d.abs() < 1e-6 {
            if a[k] < lo[k] || a[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (u, v) = ((lo[k] - a[k]) / d, (hi[k] - a[k]) / d);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
    }
    (t1 - t0 > 1e-4).then_some((t0, t1))
}

/// A segment cut where it enters and leaves boxes: the pieces `t0 .. t1` (0 at `a`, 1 at
/// `b`) in order, each with the box it lies in (the first one where boxes overlap).
pub fn split_by_boxes(a: V3, b: V3, boxes: &[(V3, V3)]) -> Vec<(f32, f32, Option<usize>)> {
    let mut inside: Vec<(f32, f32, usize)> = boxes.iter().enumerate().filter_map(|(i, bx)| segment_in_box(a, b, *bx).map(|(t0, t1)| (t0, t1, i))).collect();
    inside.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut pieces = Vec::new();
    let mut done = 0.0f32;
    for (t0, t1, i) in inside {
        if t0 > done {
            pieces.push((done, t0, None));
        }
        if t1 > done {
            pieces.push((done.max(t0), t1, Some(i)));
            done = t1;
        }
    }
    if done < 1.0 {
        pieces.push((done, 1.0, None));
    }
    pieces
}

/// Where a wire goes in the paint order of boxes (`rank` per box), given the boxes that
/// may hide it and whether each is see-through for wires (a duct): after every
/// see-through box and before every solid one; the solid one wins where both cannot hold.
/// `(i, true)`: just after box `i`, `(i, false)`: just before it, `None`: no constraint.
pub fn wire_place(hiders: impl IntoIterator<Item = (usize, bool)>, rank: &[usize]) -> Option<(usize, bool)> {
    let (mut after, mut before): (Option<usize>, Option<usize>) = (None, None);
    for (i, see_through) in hiders {
        let slot = if see_through { &mut after } else { &mut before };
        let better = match *slot {
            Some(j) if see_through => rank[i] > rank[j],
            Some(j) => rank[i] < rank[j],
            None => true,
        };
        if better {
            *slot = Some(i);
        }
    }
    match (after, before) {
        (Some(a), Some(b)) if rank[a] < rank[b] => Some((a, true)),
        (_, Some(b)) => Some((b, false)),
        (Some(a), None) => Some((a, true)),
        (None, None) => None,
    }
}

pub fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

pub fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// Length of a polyline.
pub fn path_length(points: &[V3]) -> f32 {
    points.windows(2).map(|s| dist(s[0], s[1])).sum()
}

/// A polyline cut `at` along it from its start: the part before and the part after,
/// both with the point of the cut (the whole polyline before it when `at` is past
/// its end).
pub fn split_at(points: &[V3], at: f32) -> (Vec<V3>, Vec<V3>) {
    let mut gone = 0.0;
    for (i, s) in points.windows(2).enumerate() {
        let l = dist(s[0], s[1]);
        if gone + l >= at && l > 0.0 {
            let q = add(s[0], scale(sub(s[1], s[0]), ((at - gone) / l).max(0.0)));
            let mut head = points[..=i].to_vec();
            head.push(q);
            let mut tail = vec![q];
            tail.extend_from_slice(&points[i + 1..]);
            return (head, tail);
        }
        gone += l;
    }
    (points.to_vec(), points.last().into_iter().copied().collect())
}

/// The polyline with each corner rounded by an arc of `radius`, `steps` points per
/// quarter turn; the radius is the least the way may bend with. Where the arcs of two
/// corners do not both fit on the stretch between them, or the arc of a corner on an
/// end stretch, they are drawn with the largest radius that fits (the corners share
/// the stretch): the way never leaves the corners of the polyline. A jog too short for
/// the arcs of its two corners, which lead back into (nearly) the direction before them
/// (a lane change), is an S-curve of two arcs of the radius where there is room for it
/// before and after the jog. A corner turning by a few degrees only (a dog-leg onto the
/// grid, a lane shift) stays as it is.
pub fn fillet(points: &[V3], radius: f32, steps: usize) -> Vec<V3> {
    fillet_each(points, &vec![radius; points.len()], steps)
}

/// Corners turning less than this (radians) stay sharp ([`fillet`]).
const SHARP: f32 = 0.2;

/// [`fillet`] with a radius for each corner (`radii[i]` for `points[i]`; missing or
/// zero ones keep their corner).
pub fn fillet_each(points: &[V3], radii: &[f32], steps: usize) -> Vec<V3> {
    // Without repeated points.
    let mut points: Vec<V3> = points.to_vec();
    let mut radii: Vec<f32> = (0..points.len()).map(|i| radii.get(i).copied().unwrap_or(0.0)).collect();
    let mut k = 1;
    while k < points.len() {
        if length(sub(points[k], points[k - 1])) < 1e-4 {
            points.remove(k);
            radii.remove(k);
        } else {
            k += 1;
        }
    }
    let n = points.len();
    if n < 3 {
        return points;
    }
    let dir = |i: usize| norm(sub(points[i + 1], points[i]));
    let len = |i: usize| length(sub(points[i + 1], points[i]));
    // The turn at each corner (none at the ends and at corners that stay).
    let turn: Vec<f32> = (0..n)
        .map(|i| {
            if i == 0 || i + 1 == n || radii[i] <= 0.0 {
                return 0.0;
            }
            let t = dot(dir(i - 1), dir(i)).clamp(-1.0, 1.0).acos();
            if t < SHARP { 0.0 } else { t.min(3.0) }
        })
        .collect();
    // How far the arc of each corner reaches along the stretches on both sides of it:
    // r tan(turn / 2), less where two arcs do not both fit on the stretch between them
    // (both by the same share) or an arc does not fit on an end stretch.
    let want: Vec<f32> = (0..n).map(|i| if turn[i] > 0.0 { radii[i] * (turn[i] / 2.0).tan() } else { 0.0 }).collect();
    let mut tangent = want.clone();
    for j in 0..n - 1 {
        let need = want[j] + want[j + 1];
        if need > len(j) + 1e-6 {
            let f = len(j) / need;
            tangent[j] = tangent[j].min(want[j] * f);
            tangent[j + 1] = tangent[j + 1].min(want[j + 1] * f);
        }
    }
    let mut out = vec![points[0]];
    let mut i = 1;
    while i + 1 < n {
        if turn[i] <= 0.0 {
            out.push(points[i]);
            i += 1;
            continue;
        }
        // A lane change: two corners back into the direction before them, too close
        // for their arcs, with room for the S-curve on both sides.
        if i + 2 < n && turn[i + 1] > 0.0 && want[i] + want[i + 1] > len(i) + 1e-6 {
            let r = radii[i].min(radii[i + 1]);
            let (before, after) = (len(i - 1) - tangent[i - 1], len(i + 1) - tangent[i + 2]);
            if let Some(s) = s_curve(&points[i - 1..=i + 2], r, before, after, steps) {
                out.extend(s);
                i += 2;
                continue;
            }
        }
        let r = tangent[i] / (turn[i] / 2.0).tan();
        out.extend(arc(points[i], dir(i - 1), dir(i), r, tangent[i], steps));
        i += 1;
    }
    out.push(points[n - 1]);
    out.dedup_by(|x, y| length(sub(*x, *y)) < 1e-2);
    out
}

/// The arc of radius `r` round the corner at `p` from direction `u` into `w`, starting
/// `t` before the corner (its tangent length), `steps` points per quarter turn.
fn arc(p: V3, u: V3, w: V3, r: f32, t: f32, steps: usize) -> Vec<V3> {
    let turn = dot(u, w).clamp(-1.0, 1.0).acos();
    let s = sub(p, scale(u, t));
    let across = norm(sub(w, scale(u, dot(u, w))));
    let c = add(s, scale(across, r));
    let k = ((turn / std::f32::consts::FRAC_PI_2 * steps as f32).ceil() as usize).max(2);
    (0..=k).map(|j| turn * j as f32 / k as f32).map(|phi| add(c, add(scale(across, -r * phi.cos()), scale(u, r * phi.sin())))).collect()
}

/// A lane change (`points`: the corner before the jog, its two corners and the one
/// after; across, then on within some 20 degrees of the direction before) as two arcs of
/// radius `r` that touch each other: the first leaves the stretch before the jog, the
/// second comes into the stretch after it, each as far from the jog, where that is no
/// further than the room `before` the jog and `after` it.
fn s_curve(points: &[V3], r: f32, before: f32, after: f32, steps: usize) -> Option<Vec<V3>> {
    let [a, p, b, c] = [points[0], points[1], points[2], points[3]];
    let (u, w) = (norm(sub(p, a)), norm(sub(c, b)));
    let jog = sub(b, p);
    // Across each stretch towards the other one.
    let (to, back) = (sub(jog, scale(u, dot(jog, u))), sub(scale(w, dot(jog, w)), jog));
    if dot(u, w) < 0.94 || length(to) < 1e-3 || length(back) < 1e-3 {
        return None;
    }
    let (n1, n2) = (norm(to), norm(back));
    // The centres of the arcs that begin and end `x` from the jog, and how far they
    // are from touching.
    let centres = |x: f32| (add(sub(p, scale(u, x)), scale(n1, r)), add(add(b, scale(w, x)), scale(n2, r)));
    let gap = |x: f32| {
        let (c1, c2) = centres(x);
        length(sub(c2, c1)) - 2.0 * r
    };
    let most = before.min(after);
    if gap(0.0) >= 0.0 || most <= 0.0 || gap(most + 1e-3) < 0.0 {
        return None;
    }
    let (mut lo, mut hi) = (0.0, most + 1e-3);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if gap(mid) < 0.0 { lo = mid } else { hi = mid }
    }
    let (c1, c2) = centres(hi);
    let touch = scale(add(c1, c2), 0.5);
    let (t1, t2) = (sub(touch, c1), sub(touch, c2));
    let (first, second) = (dot(t1, u).atan2(-dot(t1, n1)), (-dot(t2, w)).atan2(-dot(t2, n2)));
    let count = |turn: f32| ((turn / std::f32::consts::FRAC_PI_2 * steps as f32).ceil() as usize).max(2);
    let mut out: Vec<V3> = (0..=count(first)).map(|j| first * j as f32 / count(first) as f32).map(|phi| add(c1, add(scale(n1, -r * phi.cos()), scale(u, r * phi.sin())))).collect();
    out.extend((0..count(second)).rev().map(|j| second * j as f32 / count(second) as f32).map(|phi| add(c2, add(scale(n2, -r * phi.cos()), scale(w, -r * phi.sin())))));
    Some(out)
}

/// The tightest curvature (1 / radius) along a polyline: the turn at each point over
/// the mean of the segments on both sides of it.
pub fn curvature(points: &[V3]) -> f32 {
    points
        .windows(3)
        .map(|s| {
            let (a, b) = (sub(s[1], s[0]), sub(s[2], s[1]));
            let (la, lb) = (length(a), length(b));
            if la < 1e-6 || lb < 1e-6 {
                return 0.0;
            }
            (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos() / ((la + lb) / 2.0)
        })
        .fold(0.0, f32::max)
}

/// A round tube of radius `r` with `sides` along `path`, its ends closed: triangles
/// facing outwards, each with the side it lies on (the caps on side 0). The rings keep
/// their orientation along the path (no twist).
pub fn tube(path: &[V3], r: f32, sides: usize) -> Vec<(Tri, usize)> {
    let mut path = path.to_vec();
    path.dedup_by(|a, b| length(sub(*a, *b)) < 1e-3);
    let path = path.as_slice();
    if path.len() < 2 || r <= 0.0 {
        return Vec::new();
    }
    let sides = sides.max(3);
    let n = path.len();
    let dir = |i: usize| norm(sub(path[(i + 1).min(n - 1)], path[i.min(n - 2)]));
    let mut normal = {
        let t = dir(0);
        let other = if t[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        norm(cross(t, other))
    };
    let mut rings: Vec<Vec<V3>> = Vec::with_capacity(n);
    for (i, &at) in path.iter().enumerate() {
        let (din, dout) = (dir(i.saturating_sub(1)), dir(i));
        let t = norm(add(din, dout));
        let t = if t == [0.0; 3] { dout } else { t };
        normal = norm(sub(normal, scale(t, dot(normal, t))));
        let side = cross(t, normal);
        // A corner keeps the tube's width (mitre), a sharp one only so far.
        let mitre = 1.0 / dot(t, dout).max(0.6);
        rings.push(
            (0..sides)
                .map(|k| {
                    let a = k as f32 / sides as f32 * std::f32::consts::TAU;
                    add(at, scale(add(scale(normal, a.cos()), scale(side, a.sin())), r * mitre))
                })
                .collect(),
        );
    }
    let mut out = Vec::new();
    let quad = |q: [V3; 4], out_dir: V3, k: usize, out: &mut Vec<(Tri, usize)>| {
        let facing = dot(cross(sub(q[1], q[0]), sub(q[2], q[0])), out_dir) >= 0.0;
        let q = if facing { q } else { [q[3], q[2], q[1], q[0]] };
        out.push(([q[0], q[1], q[2]], k));
        out.push(([q[0], q[2], q[3]], k));
    };
    for i in 0..n - 1 {
        let mid = scale(add(path[i], path[i + 1]), 0.5);
        for k in 0..sides {
            let k1 = (k + 1) % sides;
            let q = [rings[i][k], rings[i + 1][k], rings[i + 1][k1], rings[i][k1]];
            let centre = scale(add(add(q[0], q[1]), add(q[2], q[3])), 0.25);
            quad(q, sub(centre, mid), k, &mut out);
        }
    }
    for (ring, centre, out_dir) in [(&rings[0], path[0], scale(dir(0), -1.0)), (&rings[n - 1], path[n - 1], dir(n - 2))] {
        for k in 0..sides {
            let t = [centre, ring[k], ring[(k + 1) % sides]];
            let t = if dot(cross(sub(t[1], t[0]), sub(t[2], t[0])), out_dir) >= 0.0 { t } else { [t[0], t[2], t[1]] };
            out.push((t, 0));
        }
    }
    out
}

/// Camera orbiting around `target` (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target: V3,
}

impl Orbit {
    /// Eye position and camera axes (right, up, forward).
    pub fn basis(&self) -> (V3, V3, V3, V3) {
        let eye = [
            self.target[0] + self.dist * self.pitch.cos() * self.yaw.sin(),
            self.target[1] + self.dist * self.pitch.sin(),
            self.target[2] + self.dist * self.pitch.cos() * self.yaw.cos(),
        ];
        let fwd = norm(sub(self.target, eye));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        (eye, right, up, fwd)
    }
}

#[cfg(test)]
mod tests;
