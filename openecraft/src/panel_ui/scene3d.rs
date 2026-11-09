//! What the 3D view of the panel layout shows, in view coordinates and independent of
//! the camera: faces with their lit colours, wires and routing lines, the boxes of the
//! equipment for the painting order, labels and outlines. It is built again only when
//! the analysis, the layout or what the view highlights changes; each frame projects it
//! ([`super::view3d`]) or hands it to the graphics card ([`super::gpu3d`]). The wires,
//! cables and tubes are laid out side by side on another thread ([`Layouts`]); until
//! that is done the scene shows them as they were routed.

use super::routing_ui::RouteRef;
use super::{PanelUi, OUTER_WALLS, WIRE_HIGHLIGHT};
use openecraft_core::analysis::Analysis;
use openecraft_core::model::Project;
use openecraft_core::panel::roofs::{self, Ceiling, Lin, RoofFaceKind};
use openecraft_core::panel::solar;
use openecraft_core::panel::{self, buildings as bld, stairs, Face, ItemKind, Opening, Role, SitePose, PLATE_MARGIN, PLATE_Z};
use openecraft_core::parts::vec3::{self, cross, dot, norm, sub, V3};
use openecraft_core::routing3d::bundles::{self, Channel, Way};
use openecraft_core::routing3d::RouteKind;
use egui::Color32;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Layer of the equipment (ducts, busbars, devices), painted in the order of its boxes.
pub(super) const EQUIPMENT: u8 = 5;
/// Layer of what is drawn over everything (the selected wire or route).
pub(super) const ON_TOP: u8 = OUTER_WALLS + 1;
/// Sides of the round tubes of cables and tubes, and of the many thin wires.
const TUBE_SIDES: usize = 10;
const WIRE_SIDES: usize = 6;
/// Grid (mm) the wires, cables and tubes running together are found on, to be laid
/// side by side.
const LANE_CELL: f32 = 8.0;
/// Wires without cable ducts shorter than this (mm on the plate) between two connection
/// points run at their height instead of along the plate.
const JUMPER: f32 = 200.0;
/// How long (ms) a scene waits for its layout before it is shown without it.
const LAYOUT_WAIT: u32 = 30;

/// A wire, cable or tube drawn as a round tube, before it is laid beside the others it
/// runs with: its corners and their bend radii, how it looks and what it is.
struct TubeWay {
    points: Vec<V3>,
    bends: Vec<f32>,
    diameter: f32,
    color: Color32,
    stripe: Option<Color32>,
    width: f32,
    on_top: bool,
    target: RouteRef,
    /// Sides of the tube and points per quarter turn of its bends.
    sides: usize,
    steps: usize,
    /// The direction away from the mounting plate it runs along and how far it may come
    /// nearer to it (see [`Way`]).
    rest: V3,
    below: f32,
    /// Drawn as it is, not laid beside the others.
    as_is: bool,
    /// The direction it leaves its ends in (see [`Way`]).
    leave: [V3; 2],
}

/// A flat face of the view: a triangle of a model or a side of a box.
pub(super) struct SceneFace {
    /// Corners; a triangle repeats its last one.
    pub pts: [V3; 4],
    /// Number of corners: 3 or 4.
    pub n: u8,
    /// Outward normal: a face seen from behind is left out.
    pub normal: V3,
    pub color: Color32,
    /// Painting layer (walls and plate behind the equipment, …).
    pub layer: u8,
    /// Index in [`Scene3d::sides`] for a face whose layer depends on the side it is seen from.
    pub side: Option<usize>,
    /// Index in [`Scene3d::objects`] of the equipment it belongs to.
    pub obj: Option<usize>,
    /// Enclosure and item, for picking.
    pub item: Option<(usize, u64)>,
    /// Outlined (sides of boxes, not triangles of models).
    pub outline: bool,
    /// Lies on another face (a cutout on the plate): drawn in front of it.
    pub decal: bool,
}

/// A wall, door or wall cutout that is painted over the equipment (translucent) when
/// seen from the outside and behind it when seen from the inside.
pub(super) struct SceneSide {
    pub normal: V3,
    pub at: V3,
    /// Layer seen from the inside.
    pub inside: u8,
    /// Left out when seen from the outside and the walls in front are hidden.
    pub hideable: bool,
    /// A cutout lying on its wall.
    pub decal: bool,
    /// Seen through from everywhere at [`GLASS_ALPHA`]: an installation zone, or a wall
    /// or floor opened to show the flush-mounted lines in it.
    pub glass: bool,
}

/// Opacity of what is drawn as glass ([`SceneSide::glass`]).
pub(super) const GLASS_ALPHA: f32 = 0.3;

impl SceneSide {
    pub fn outside(&self, eye: V3) -> bool {
        dot(self.normal, sub(eye, self.at)) > 0.0
    }

    /// Layer of its faces seen from `eye`; `None` when they are left out.
    pub fn layer(&self, eye: V3, hide_front: bool) -> Option<u8> {
        if self.glass {
            return Some(OUTER_WALLS);
        }
        match self.outside(eye) {
            true if hide_front && self.hideable => None,
            true => Some(OUTER_WALLS),
            false => Some(self.inside),
        }
    }
}

/// A line: a wire, cable, tube or routing path segment.
#[derive(Clone)]
pub(super) struct SceneLine {
    pub a: V3,
    pub b: V3,
    pub color: Color32,
    /// Second colour of a two-colour wire, along the middle.
    pub stripe: Option<Color32>,
    /// Least width (pixels).
    pub least: f32,
    /// Outer diameter (mm), drawn in perspective; 0 for none.
    pub diameter: f32,
    /// Drawn as a round tube by the graphics card ([`Scene3d::tubes`]).
    pub tube: bool,
}

/// A piece of a line among the equipment, cut where it enters and leaves the ducts.
pub(super) struct SceneWire {
    pub line: SceneLine,
    /// Duct (index in [`Scene3d::objects`]) it runs in.
    pub inside: Option<usize>,
}

/// Name of a device on its front face, or a warning at a point (a cable that is not
/// drawn: it could not be routed).
pub(super) struct SceneLabel {
    pub at: V3,
    /// Direction its front faces (none for a warning: seen from everywhere).
    pub normal: V3,
    /// Smaller side of the front (mm).
    pub size: f32,
    pub tag: String,
    pub warning: bool,
}

/// The content of the 3D view; see the module documentation.
#[derive(Default)]
pub(super) struct Scene3d {
    /// Tells this scene from every other one (the graphics card keeps its upload).
    pub id: u64,
    /// What it was built from.
    pub(super) analysis: u64,
    pub(super) layout: panel::Layout,
    pub(super) state: u64,
    /// The layout of its wires, cables and tubes it is shown without until it is ready
    /// (its key, see [`Layouts`]).
    pub(super) waiting: Option<u64>,
    pub faces: Vec<SceneFace>,
    pub sides: Vec<SceneSide>,
    /// Lines on top of everything.
    pub lines: Vec<SceneLine>,
    /// Lines among the equipment.
    pub wires: Vec<SceneWire>,
    /// The lines with a diameter as round tubes, bent round their corners, for the
    /// graphics card (the processor draws the lines).
    pub tubes: Vec<SceneFace>,
    /// Boxes (min, max) of the equipment, and which of them are ducts.
    pub objects: Vec<(V3, V3)>,
    pub ducts: Vec<bool>,
    /// Front edges of the enclosures.
    pub outlines: Vec<(V3, V3)>,
    /// The boundary of the plot on the ground and the reference point.
    pub plot_lines: Vec<(V3, V3)>,
    pub labels: Vec<SceneLabel>,
    /// Branch points of the bundles.
    pub branches: Vec<V3>,
    /// Segments of the wires and routes for picking.
    pub picks: Vec<(V3, V3, RouteRef)>,
    /// The tubes drawn: the way of each, its radius and what it is.
    pub tube_paths: Vec<(Vec<V3>, f32, RouteRef)>,
    /// Box around everything.
    pub bounds: (V3, V3),
}

/// What the wires, cables and tubes of a scene are laid out from ([`bundles::lay_out`]),
/// owned to go to another thread.
pub(super) struct LayoutInput {
    ways: Vec<LayoutWay>,
    obstacles: Vec<(V3, V3)>,
    channels: Vec<Channel>,
    insides: Vec<(V3, V3)>,
}

/// A [`Way`] of a [`LayoutInput`].
struct LayoutWay {
    points: Vec<V3>,
    bends: Vec<f32>,
    diameter: f32,
    rest: V3,
    below: f32,
    steps: usize,
    held: bool,
    leave: [V3; 2],
}

impl LayoutInput {
    /// Tells this input from every other one.
    fn key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for w in &self.ways {
            (w.points.len(), w.bends.len(), w.steps, w.held).hash(&mut h);
            w.points.iter().chain(&w.leave).flatten().chain(&w.bends).chain(&w.rest).chain([&w.diameter, &w.below]).for_each(|v| v.to_bits().hash(&mut h));
        }
        (self.obstacles.len(), self.insides.len(), self.channels.len()).hash(&mut h);
        let boxes = self.obstacles.iter().chain(&self.insides).chain(self.channels.iter().map(|c| &c.inside));
        boxes.flat_map(|(lo, hi)| lo.iter().chain(hi)).chain(self.channels.iter().flat_map(|c| &c.floor)).for_each(|v| v.to_bits().hash(&mut h));
        h.finish()
    }

    /// The ways laid out; `None` when `progress` (rounds done, of all) answers false.
    fn lay_out(&self, progress: &dyn Fn(usize, usize) -> bool) -> Option<Vec<Vec<V3>>> {
        let ways: Vec<Way> = self.ways.iter().map(|w| Way { points: &w.points, radii: &w.bends, diameter: w.diameter, rest: w.rest, below: w.below, steps: w.steps, held: w.held, leave: w.leave }).collect();
        bundles::lay_out(&ways, LANE_CELL, &self.obstacles, &self.channels, &self.insides, progress)
    }
}

/// The layouts of the 3D view: the last one worked out (by the key of its input), kept
/// while the input stays the same, and the one being worked out on another thread.
pub(super) struct Layouts {
    laid: Option<(u64, Arc<Vec<Vec<V3>>>)>,
    laying: Option<Laying>,
    /// The key of a layout that failed (is not tried again).
    failed: Option<u64>,
    /// How long (ms) a scene waits for its layout.
    pub(super) wait: u32,
}

/// A layout being worked out: its key, the rounds done and of all, whether it is to stop.
struct Laying {
    key: u64,
    done: Arc<(AtomicUsize, AtomicUsize)>,
    stop: Arc<AtomicBool>,
    job: crate::jobs::Job<Option<Vec<Vec<V3>>>>,
}

impl Default for Layouts {
    fn default() -> Self {
        // Tests wait for the layout of each scene, as without threads.
        Layouts { laid: None, laying: None, failed: None, wait: if cfg!(test) { u32::MAX } else { LAYOUT_WAIT } }
    }
}

impl Layouts {
    /// Takes the layout worked out on another thread once it is there.
    pub(super) fn collect(&mut self) {
        let Some(laying) = &self.laying else { return };
        match laying.job.take() {
            Some(Ok(Some(ways))) => self.laid = Some((laying.key, Arc::new(ways))),
            Some(Ok(None)) => {}
            Some(Err(_)) => self.failed = Some(laying.key),
            None => return,
        }
        self.laying = None;
    }

    /// The layout of `key` is being worked out.
    pub(super) fn running(&self, key: u64) -> bool {
        self.laying.as_ref().is_some_and(|l| l.key == key)
    }

    /// A scene waiting for the layout of `key` is to be built again: it is done, or it is
    /// neither being worked out nor failed.
    fn outdated(&self, key: u64) -> bool {
        self.laid.as_ref().is_some_and(|l| l.0 == key) || (!self.running(key) && self.failed != Some(key))
    }

    /// How far the layout being worked out is, 0 … 1; `None` without one.
    pub(super) fn progress(&self) -> Option<f32> {
        let l = self.laying.as_ref()?;
        let (done, total) = (l.done.0.load(Ordering::Relaxed), l.done.1.load(Ordering::Relaxed));
        Some(if total > 0 { done as f32 / total as f32 } else { 0.0 })
    }

    /// The ways of `input` laid out: kept, or worked out on another thread (`start`; one
    /// for another input is stopped), waiting for it a moment. Otherwise the key it is
    /// waited for.
    fn get(&mut self, ctx: &egui::Context, input: LayoutInput, start: bool) -> Result<Arc<Vec<Vec<V3>>>, u64> {
        let key = input.key();
        if let Some((k, ways)) = &self.laid {
            if *k == key {
                return Ok(ways.clone());
            }
        }
        if !self.running(key) && self.failed != Some(key) {
            if let Some(old) = self.laying.take() {
                old.stop.store(true, Ordering::Relaxed);
            }
            if !start {
                return Err(key);
            }
            let done = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
            let stop = Arc::new(AtomicBool::new(false));
            let (counter, halt) = (done.clone(), stop.clone());
            let job = crate::jobs::Job::start(String::new(), ctx, move || {
                input.lay_out(&|n, total| {
                    counter.0.store(n, Ordering::Relaxed);
                    counter.1.store(total, Ordering::Relaxed);
                    !halt.load(Ordering::Relaxed)
                })
            });
            self.laying = Some(Laying { key, done, stop, job });
        }
        let mut waited = 0;
        loop {
            self.collect();
            if !self.running(key) || waited >= self.wait {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            waited += 1;
        }
        match &self.laid {
            Some((k, ways)) if *k == key => Ok(ways.clone()),
            _ => Err(key),
        }
    }
}

static NEXT_SCENE: AtomicU64 = AtomicU64::new(1);

/// Hash of a set whatever the order of its elements.
fn unordered<T: Hash>(items: impl Iterator<Item = T>) -> u64 {
    items
        .map(|x| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            x.hash(&mut h);
            h.finish()
        })
        .fold(0u64, u64::wrapping_add)
}

/// A wire's way along the plate lifted over the boxes it would pass through: just
/// before one it rises in front of it (half its thickness and a little more), just
/// after it it goes down again.
fn over(points: &[V3], blockers: &[(V3, V3)], ducts: &[(V3, V3)], half: f32, bend: f32) -> Vec<V3> {
    let margin = half + 2.0;
    let mut out: Vec<V3> = points.first().copied().into_iter().collect();
    for s in points.windows(2) {
        let (a, b) = (s[0], s[1]);
        let at = |t: f32| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t);
        // Where the stretch runs inside the cable ducts: it goes up behind a duct and
        // comes down before one, through their walls, not their covers.
        let in_ducts: Vec<(f32, f32)> = ducts.iter().filter_map(|(lo, hi)| vec3::segment_in_box(a, b, (*lo, *hi))).collect();
        // The stretches inside the boxes (grown by the wire) and how far in front of the
        // plate their fronts are, joined where they overlap.
        // It rises and comes down a bend radius before and behind them, so that its
        // bends stay clear of their corners.
        let reach = bend / vec3::dist(a, b).max(1e-3);
        let mut hits: Vec<(f32, f32, f32)> = blockers
            .iter()
            .filter_map(|(lo, hi)| vec3::segment_in_box(a, b, (lo.map(|v| v - margin), hi.map(|v| v + margin))).map(|(t0, t1)| ((t0 - reach).max(0.0), (t1 + reach).min(1.0), hi[2] + margin)))
            .filter(|h| h.2 > a[2].max(b[2]))
            .collect();
        hits.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut joined: Vec<(f32, f32, f32)> = Vec::new();
        for h in hits {
            match joined.last_mut() {
                Some(last) if h.0 <= last.1 => *last = (last.0, last.1.max(h.1), last.2.max(h.2)),
                _ => joined.push(h),
            }
        }
        for (t0, t1, front) in joined {
            // Lifted outside the ducts only: inside them it stays under their covers.
            let mut pieces = vec![(t0, t1)];
            for &(d0, d1) in &in_ducts {
                pieces = pieces.into_iter().flat_map(|(a, b)| [(a, b.min(d0)), (a.max(d1), b)]).filter(|(a, b)| b - a > 1e-4).collect();
            }
            for (t0, t1) in pieces {
                let (p, q) = (at(t0), at(t1));
                out.extend([p, [p[0], p[1], front], [q[0], q[1], front], q]);
            }
        }
        out.push(b);
    }
    out.dedup_by(|x, y| vec3::dist(*x, *y) < 0.01);
    // No dip back down where a lifted stretch ends right where the way rises again
    // (the connection point straight above the last point along the plate).
    for i in 1..out.len().saturating_sub(1) {
        let (p, q, r) = (out[i - 1], out[i], out[i + 1]);
        if q[2] < p[2] && r[2] > q[2] && (r[0] - q[0]).abs() < 1e-3 && (r[1] - q[1]).abs() < 1e-3 {
            out[i][2] = p[2].min(r[2]);
        }
    }
    out
}

/// A wire's way along the plate from a connection point of its own device (box `own`)
/// at `height`: the points of the way inside the device (seen on the plate) left out, it
/// leaves at that height to above the first one outside, then goes down to it.
fn leave_device(along: &mut Vec<V3>, own: (V3, V3), height: f32) {
    let inside = |q: &V3| q[0] > own.0[0] && q[0] < own.1[0] && q[1] > own.0[1] && q[1] < own.1[1];
    let n = along.iter().take_while(|q| inside(q)).count();
    if n == 0 || n >= along.len() {
        return;
    }
    along.drain(..n);
    let q = along[0];
    along.insert(0, [q[0], q[1], height]);
}

/// A wire's way out of its device ([`leave_device`]) that comes down less than its
/// thickness beside the connection point comes down right under it: straight out of
/// the terminal, without a kink.
fn under_terminal(along: &mut [V3], terminal: V3, diameter: f32) {
    if along.len() < 2 {
        return;
    }
    let (top, q) = (along[0], along[1]);
    if top[0] != q[0] || top[1] != q[1] {
        return;
    }
    let off = ((q[0] - terminal[0]).powi(2) + (q[1] - terminal[1]).powi(2)).sqrt();
    if off > 1e-3 && off < diameter {
        along[0] = [terminal[0], terminal[1], top[2]];
        along[1] = [terminal[0], terminal[1], q[2]];
    }
}

/// A wire's way out of its device ([`leave_device`]) whose first point outside the
/// device lies in a cable duct: it comes down from the connection point's height to
/// the duct's depth just before the duct's wall (seen on the plate), not inside the
/// duct through its cover.
fn down_before_duct(along: &mut Vec<V3>, terminal: V3, ducts: &[(V3, V3)], half: f32) {
    if along.len() < 2 {
        return;
    }
    let (top, q) = (along[0], along[1]);
    if top[0] != q[0] || top[1] != q[1] {
        return;
    }
    let inside = |p: V3, (lo, hi): &(V3, V3)| p[0] > lo[0] && p[0] < hi[0] && p[1] > lo[1] && p[1] < hi[1];
    let Some(d) = ducts.iter().find(|d| inside(q, d) && !inside(terminal, d)) else { return };
    // Where the way from the terminal to the point in the duct enters the duct.
    let (a, b) = (terminal, q);
    let mut entering: f32 = 0.0;
    for k in 0..2 {
        if (b[k] - a[k]).abs() > 1e-6 {
            let (t1, t2) = ((d.0[k] - a[k]) / (b[k] - a[k]), (d.1[k] - a[k]) / (b[k] - a[k]));
            entering = entering.max(t1.min(t2));
        }
    }
    let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    if len < 1e-3 {
        return;
    }
    let t = (entering - (half + 2.0) / len).clamp(0.0, 1.0);
    let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    along.splice(0..1, [[c[0], c[1], top[2]], [c[0], c[1], q[2]]]);
}

/// The fingers (indices) a tube passes through: each of `ways` with its radius, the
/// fingers with the duct they belong to (index into `ducts`) and their box.
fn fingers_hit(fingers: &[(usize, std::ops::Range<usize>, (V3, V3))], ways: &[(Vec<V3>, f32)], ducts: &[(V3, V3)]) -> Vec<usize> {
    let grow = |(lo, hi): (V3, V3), r: f32| (lo.map(|v| v - r), hi.map(|v| v + r));
    let meet = |a: (V3, V3), b: (V3, V3)| (0..3).all(|k| a.0[k] <= b.1[k] && b.0[k] <= a.1[k]);
    let way_boxes: Vec<(V3, V3)> = ways.iter().map(|(pts, r)| grow(bounds(pts), *r)).collect();
    // The ways near each duct.
    let mut near: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    let mut hit = Vec::new();
    for (f, (duct, _, bx)) in fingers.iter().enumerate() {
        let close = near.entry(*duct).or_insert_with(|| (0..ways.len()).filter(|&w| meet(way_boxes[w], ducts[*duct])).collect());
        let through = close.iter().any(|&w| {
            let (pts, r) = &ways[w];
            let bx = grow(*bx, *r);
            meet(way_boxes[w], bx) && pts.windows(2).any(|s| vec3::segment_in_box(s[0], s[1], bx).is_some())
        });
        if through {
            hit.push(f);
        }
    }
    hit
}

/// Colours of the parts of a building.
const ROOF_TOP: Color32 = Color32::from_rgb(160, 82, 60);
const ROOF_UNDERSIDE: Color32 = Color32::from_rgb(205, 180, 140);
const ROOF_EDGE: Color32 = Color32::from_rgb(130, 70, 52);
const GABLE: Color32 = Color32::from_rgb(222, 216, 204);
const GLASS: Color32 = Color32::from_rgb(120, 170, 215);
const WINDOW_FRAME: Color32 = Color32::from_rgb(80, 84, 90);
const PV_CELLS: Color32 = Color32::from_rgb(28, 40, 78);
const PV_FRAME: Color32 = Color32::from_rgb(58, 60, 66);
const PV_MOUNTING: Color32 = Color32::from_rgb(176, 180, 186);
const STAIR: Color32 = Color32::from_rgb(196, 184, 160);
const SLAB: Color32 = Color32::from_rgb(168, 168, 172);

/// Colour of the installation zones of a kind: horizontal blue, vertical green, in the
/// floor and the ceiling orange.
pub(super) fn zone_color(kind: openecraft_core::routing3d::ZoneKind) -> Color32 {
    use openecraft_core::routing3d::ZoneKind as K;
    match kind {
        K::AboveFloor | K::BelowCeiling => Color32::from_rgb(70, 140, 230),
        K::BesideCorners | K::BesideDoors | K::BesideWindows => Color32::from_rgb(60, 180, 110),
        K::Floor | K::Ceiling => Color32::from_rgb(240, 150, 50),
    }
}

fn face_normal(pts: &[V3]) -> V3 {
    norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])))
}

/// A flat face of any number of corners (in the order of its normal) as faces of the
/// scene: itself, or the triangles of a fan.
fn add_poly(s: &mut Scene3d, pts: &[V3], normal: V3, color: Color32, layer: u8, side: Option<usize>, decal: bool) {
    let face = |q: [V3; 4], n: u8| SceneFace { pts: q, n, normal, color, layer, side, obj: None, item: None, outline: pts.len() <= 4, decal };
    match pts.len() {
        0..=2 => {}
        3 => s.faces.push(face([pts[0], pts[1], pts[2], pts[2]], 3)),
        4 => s.faces.push(face([pts[0], pts[1], pts[2], pts[3]], 4)),
        _ => {
            for k in 1..pts.len() - 1 {
                s.faces.push(face([pts[0], pts[k], pts[k + 1], pts[k + 1]], 3));
            }
        }
    }
}

/// A wall (or the ceiling: `slab`) of a room as a box `(min, max)` in the room's 3D
/// coordinates (y up from the floor) cut by the sloped ceiling: the faces of its pieces,
/// each up to the ceiling there (the ceiling `t` thick on it).
fn cut_room_box(c: &Ceiling, (min, max): (V3, V3), slab: bool, t: f32) -> Vec<(Vec<V3>, V3)> {
    let (lo, hi) = ([min[0], min[2]], [max[0], max[2]]);
    let rect = [[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]];
    // The sides of the pieces on the box's own sides.
    let on = |a: [f32; 2], b: [f32; 2]| (0..2).any(|k| ((a[k] - lo[k]).abs() < 0.5 && (b[k] - lo[k]).abs() < 0.5) || ((a[k] - hi[k]).abs() < 0.5 && (b[k] - hi[k]).abs() < 0.5));
    let mut out = Vec::new();
    for (poly, lin) in c.pieces(&rect) {
        let (bottom, top) = if slab { (lin, lin.plus(t)) } else { (Lin::constant(min[1]), lin.plus(t)) };
        // Not where the roof comes down below the bottom.
        let poly = roofs::clip(&poly, bottom.minus(top).plus(1.0));
        out.extend(roofs::prism(&poly, bottom, top, on));
    }
    out
}

impl PanelUi {
    /// The floor slabs of the rooms of the buildings shown, their stairs and roofs (with
    /// the gable walls and roof windows), seen from outside like the walls, and the PV
    /// arrays on the roofs.
    fn building_faces(&self, s: &mut Scene3d, prj: &Project, ei: usize, shown: &[(usize, SitePose)], lit: &dyn Fn(Color32, V3) -> Color32) {
        let frame = self.view_frame(prj, ei);
        let to_view = |q: V3| frame.map_or(q, |f| f.local(q));
        let turn = |n: V3| frame.map_or(n, |f| f.unturn(n));
        let layout = &prj.layout;
        for bi in self.buildings_3d(prj, ei) {
            let b = &layout.buildings[bi];
            let bp = bld::building_pose(b);
            // The level shown alone: the stairs reaching it, no roofs.
            let level = self.level_3d().and_then(|l| bld::level_by_id(layout, l));
            let geos: Vec<stairs::StairGeometry> = b
                .stairs
                .iter()
                .filter_map(|st| stairs::geometry(b, st))
                .filter(|g| level.is_none_or(|l| g.from <= l.elevation + 0.5 && g.to >= l.elevation - 0.5))
                .collect();
            // The floor slab of each room shown, open where a stair comes up through it.
            for &(k, pose) in shown.iter().filter(|(k, _)| bld::building_of(layout, *k) == Some(bi)) {
                let e = &layout.enclosures[k];
                let Some(level) = bld::level_by_id(layout, e.level) else { continue };
                let room = panel::site_poses(prj)[k];
                let holes: Vec<([f32; 2], [f32; 2])> = geos
                    .iter()
                    .filter(|g| g.from < level.elevation - 0.5 && g.to >= level.elevation - 0.5)
                    .map(|g| {
                        let pts: Vec<[f32; 2]> = g.pieces.iter().flat_map(|p| p.outline.iter()).map(|&[x, z]| {
                            let v = room.local(bp.apply([x, 0.0, z]));
                            [v[0], v[2]]
                        }).collect();
                        panel::bounds(&pts)
                    })
                    .collect();
                for cell in roofs::floor_polys(e) {
                    let mut parts = vec![cell];
                    for &(hlo, hhi) in &holes {
                        let hole = vec![[hlo[0], hlo[1]], [hhi[0], hlo[1]], [hhi[0], hhi[1]], [hlo[0], hhi[1]]];
                        parts = parts.into_iter().flat_map(|p| outside_of(&p, &hole)).collect();
                    }
                    // Opened to show the lines in the floor: seen through.
                    let side = self.open_walls.then(|| {
                        s.sides.push(SceneSide { normal: pose.turn([0.0, 1.0, 0.0]), at: pose.apply([0.0, 0.0, 0.0]), inside: 0, hideable: false, decal: false, glass: true });
                        s.sides.len() - 1
                    });
                    for p in parts {
                        for (pts, n) in roofs::prism(&p, Lin::constant(-bld::SLAB), Lin::constant(0.0), |_, _| true) {
                            let n = pose.turn(n);
                            add_poly(s, &pts.iter().map(|&q| pose.apply(q)).collect::<Vec<V3>>(), n, lit(SLAB, n), 1, side, false);
                        }
                    }
                }
            }
            // The stairs: steps, landings and newels.
            for g in &geos {
                for piece in &g.pieces {
                    for (pts, n) in roofs::prism(&piece.outline, Lin::constant(piece.bottom), Lin::constant(piece.top), |_, _| true) {
                        let n = turn(bp.turn(n));
                        add_poly(s, &pts.iter().map(|&q| to_view(bp.apply(q))).collect::<Vec<V3>>(), n, lit(STAIR, n), 1, None, false);
                    }
                }
            }
            if self.hide_ceiling || level.is_some() {
                continue;
            }
            // The roofs: translucent from outside like the walls, behind everything from
            // the attic.
            for r in &b.roofs {
                for f in roofs::roof_faces(b, r) {
                    let base = match f.kind {
                        RoofFaceKind::Top => ROOF_TOP,
                        RoofFaceKind::Underside => ROOF_UNDERSIDE,
                        RoofFaceKind::Edge => ROOF_EDGE,
                        RoofFaceKind::Gable => GABLE,
                        RoofFaceKind::Glass => GLASS,
                        RoofFaceKind::Frame => WINDOW_FRAME,
                        RoofFaceKind::Module => PV_CELLS,
                        RoofFaceKind::ModuleFrame => PV_FRAME,
                        RoofFaceKind::Mounting => PV_MOUNTING,
                    };
                    let pts: Vec<V3> = f.pts.iter().map(|&q| to_view(q)).collect();
                    let n = turn(f.normal);
                    let centre = [0, 1, 2].map(|i| pts.iter().map(|q| q[i]).sum::<f32>() / pts.len() as f32);
                    s.sides.push(SceneSide { normal: turn(f.outward), at: centre, inside: 0, hideable: true, decal: false, glass: false });
                    let side = Some(s.sides.len() - 1);
                    add_poly(s, &pts, n, lit(base, n), 0, side, matches!(f.kind, RoofFaceKind::Glass | RoofFaceKind::Frame));
                }
                // The PV arrays on it, solid.
                for f in solar::faces(b, r) {
                    let base = match f.kind {
                        RoofFaceKind::Module => PV_CELLS,
                        RoofFaceKind::ModuleFrame => PV_FRAME,
                        _ => PV_MOUNTING,
                    };
                    let pts: Vec<V3> = f.pts.iter().map(|&q| to_view(q)).collect();
                    let n = turn(f.normal);
                    add_poly(s, &pts, n, lit(base, n), 0, None, false);
                }
            }
        }
        // All of the plant: the plot boundary on the ground and the reference point as a
        // post with a cross at its foot.
        if self.whole_site() && self.level_3d().is_none() {
            let plot = &layout.plot;
            let n = plot.outline.len();
            s.plot_lines.extend((0..n).map(|i| {
                let (a, b) = (plot.outline[i], plot.outline[(i + 1) % n]);
                ([a[0], 0.0, a[1]], [b[0], 0.0, b[1]])
            }));
            if let Some(r) = &plot.reference {
                let c = 500.0;
                s.plot_lines.extend([([r.x - c, r.y, r.z], [r.x + c, r.y, r.z]), ([r.x, r.y, r.z - c], [r.x, r.y, r.z + c]), ([r.x, r.y, r.z], [r.x, r.y + 2000.0, r.z])]);
            }
        }
    }
}

/// The parts of convex polygon `p` outside the rectangle `hole` (convex pieces).
fn outside_of(p: &[[f32; 2]], hole: &[[f32; 2]]) -> Vec<Vec<[f32; 2]>> {
    let (lo, hi) = panel::bounds(hole);
    let lins = [Lin { a: 1.0, b: 0.0, c: -lo[0] }, Lin { a: -1.0, b: 0.0, c: hi[0] }, Lin { a: 0.0, b: 1.0, c: -lo[1] }, Lin { a: 0.0, b: -1.0, c: hi[1] }];
    // Outside: left of lo x, right of hi x, then above and below between them.
    let mut rest = p.to_vec();
    let mut out = Vec::new();
    for l in lins {
        let piece = roofs::clip(&rest, l);
        if piece.len() >= 3 && roofs::area(&piece).abs() > 1.0 {
            out.push(piece);
        }
        rest = roofs::clip(&rest, Lin { a: -l.a, b: -l.b, c: -l.c });
        if rest.len() < 3 {
            break;
        }
    }
    out
}

fn bounds(pts: &[V3]) -> (V3, V3) {
    let lo = [0, 1, 2].map(|k| pts.iter().map(|q| q[k]).fold(f32::INFINITY, f32::min));
    let hi = [0, 1, 2].map(|k| pts.iter().map(|q| q[k]).fold(f32::NEG_INFINITY, f32::max));
    (lo, hi)
}

impl PanelUi {
    /// The scene of the 3D view of enclosure `ei` showing `shown`: the one built before
    /// while nothing it shows changed and it does not wait for a layout that is done
    /// (always a new one during a gesture, whose layout checks are made live). Its wires,
    /// cables and tubes are laid out on another thread (not during a gesture or while
    /// the door turns); `ctx` repaints when that is done.
    pub(super) fn scene_3d(&mut self, ctx: &egui::Context, prj: &Project, an: &Analysis, ei: usize, shown: &[(usize, SitePose)]) -> Arc<Scene3d> {
        self.layouts.collect();
        let state = self.scene_state(an, ei, shown);
        let current = |s: &&Arc<Scene3d>| s.waiting.is_none_or(|k| !self.layouts.outdated(k));
        if let Some(s) = self.scene.as_ref().filter(|s| !self.busy() && s.analysis == an.id && s.state == state && s.layout == prj.layout).filter(current) {
            return s.clone();
        }
        let settled = !self.busy() && !self.door_turning();
        let mut layouts = std::mem::take(&mut self.layouts);
        let s = Arc::new(self.build_scene(prj, an, ei, shown, state, &mut |input| layouts.get(ctx, input, settled)));
        self.layouts = layouts;
        self.scene = Some(s.clone());
        s
    }

    /// Hash of the state of the view the scene depends on.
    fn scene_state(&self, an: &Analysis, ei: usize, shown: &[(usize, SitePose)]) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        ei.hash(&mut h);
        for (k, pose) in shown {
            k.hash(&mut h);
            pose.at.map(f32::to_bits).hash(&mut h);
            pose.angle.to_bits().hash(&mut h);
        }
        self.selection.hash(&mut h);
        self.wire_index(an).hash(&mut h);
        (self.show_wires, self.hide_ceiling, self.open_ducts, self.door_angle.to_bits()).hash(&mut h);
        (self.whole_site(), self.level_3d(), self.show_building, self.show_zones, self.open_walls).hash(&mut h);
        (&self.selected_route, self.selected_object).hash(&mut h);
        unordered(self.done_items.iter()).hash(&mut h);
        unordered(self.done_wires.iter()).hash(&mut h);
        h.finish()
    }

    #[allow(clippy::too_many_lines)]
    /// `lay_out`: the wires, cables and tubes laid out, or the key of the layout the scene
    /// waits for.
    fn build_scene(&self, prj: &Project, an: &Analysis, ei: usize, shown: &[(usize, SitePose)], state: u64, lay_out: &mut dyn FnMut(LayoutInput) -> Result<Arc<Vec<Vec<V3>>>, u64>) -> Scene3d {
        let mut s = Scene3d { id: NEXT_SCENE.fetch_add(1, Ordering::Relaxed), analysis: an.id, layout: prj.layout.clone(), state, ..Default::default() };
        let pos = panel::positions(&prj.layout);
        let (sin, cos) = self.door_angle.sin_cos();
        // The doors turn about their hinges on the left front edge (view coordinates).
        let swing_dir = |v: V3| [v[0] * cos - v[2] * sin, v[1], v[0] * sin + v[2] * cos];
        let light = norm([0.35, 0.75, 0.55]);
        let lit = |base: Color32, n: V3| {
            let shade = 0.55 + 0.45 * dot(n, light).max(0.0);
            Color32::from_rgb((base.r() as f32 * shade) as u8, (base.g() as f32 * shade) as u8, (base.b() as f32 * shade) as u8)
        };
        // Round tubes of a wire, cable or tube (a two-colour one with a stripe along it).
        let tube_faces = |tubes: &mut Vec<SceneFace>, path: &[V3], diameter: f32, color: Color32, stripe: Option<Color32>, sides: usize| {
            for (tri, side) in vec3::tube(path, diameter / 2.0, sides) {
                let normal = norm(cross(sub(tri[1], tri[0]), sub(tri[2], tri[0])));
                let base = match stripe {
                    Some(stripe) if side < sides / 3 => stripe,
                    _ => color,
                };
                tubes.push(SceneFace { pts: [tri[0], tri[1], tri[2], tri[2]], n: 3, normal, color: lit(base, normal), layer: EQUIPMENT, side: None, obj: None, item: None, outline: false, decal: false });
            }
        };
        // The fingers of the cable ducts (the duct, their faces, their box) and the
        // wires, cables and tubes drawn as tubes (their way and radius): fingers a tube
        // passes through are left out.
        let mut fingers: Vec<(usize, std::ops::Range<usize>, (V3, V3))> = Vec::new();
        let mut tube_ways: Vec<(Vec<V3>, f32)> = Vec::new();
        // The wires, cables and tubes drawn as tubes, laid side by side once all are known:
        // in the ducts of the cabinets (never over their covers), elsewhere inside their
        // walls.
        let mut pending: Vec<TubeWay> = Vec::new();
        // The pieces of the wires drawn as tubes in each enclosure (wire, stretch), joined
        // through the cable passages once all are known.
        let mut wire_pieces: Vec<(usize, usize, TubeWay)> = Vec::new();
        let mut channels: Vec<Channel> = Vec::new();
        let mut insides: Vec<(V3, V3)> = Vec::new();
        let mut lids: Vec<(V3, V3)> = Vec::new();
        // The solid parts of the cable ducts (base and closed rims): the wires are laid
        // out clear of them.
        let mut duct_parts: Vec<(V3, V3)> = Vec::new();
        // What stands on the mounting plates, down to the plates (no way runs behind it).
        let mut on_plates: Vec<(V3, V3)> = Vec::new();
        // Wire segments among the equipment, cut at the boxes once all of them are known.
        let mut equipment_wires: Vec<SceneLine> = Vec::new();
        // Whether a box is the object's own (not enlarged by a turn of the door or the
        // enclosure).
        let mut exact: Vec<bool> = Vec::new();
        let shown_wire = |i: usize| shown.iter().any(|(k, _)| an.wiring[i].in_enclosure(*k));
        let selected_wire = self.wire_index(an).filter(|i| shown_wire(*i));
        let wire_ends = selected_wire.map(|i| an.wiring[i].items);
        // Wires through one cable passage pass it side by side, each on a place of its
        // own in a square round the middle of the passage.
        let mut through_passages: std::collections::HashMap<[i32; 3], Vec<usize>> = std::collections::HashMap::new();
        for (wi, w) in an.wiring.iter().enumerate() {
            for (_, _, entry, exit) in w.stretch_passages() {
                for q in [entry, exit].into_iter().flatten() {
                    let list = through_passages.entry(q.map(|v| v.round() as i32)).or_default();
                    if !list.contains(&wi) {
                        list.push(wi);
                    }
                }
            }
        }
        let slot = |q: V3, wi: usize| -> V3 {
            let Some(list) = through_passages.get(&q.map(|v| v.round() as i32)) else { return [0.0; 3] };
            let Some(k) = list.iter().position(|&w| w == wi) else { return [0.0; 3] };
            let gap = list.iter().map(|&w| openecraft_core::harness::outer_diameter(&an.wiring[w].cross_section)).fold(0.0, f32::max) + 2.0;
            let cols = (list.len() as f32).sqrt().ceil() as usize;
            let rows = list.len().div_ceil(cols);
            [0.0, ((k % cols) as f32 - (cols - 1) as f32 / 2.0) * gap, ((k / cols) as f32 - (rows - 1) as f32 / 2.0) * gap]
        };
        let plant = panel::site_poses(prj);
        for &(k, pose) in shown {
            let e = &prj.layout.enclosures[k];
            // A room under a roof that slopes its ceiling: its walls and ceiling cut.
            let ceiling = if e.is_room() { roofs::ceiling_with(prj, &plant, k) } else { None };
            let swing = |q: V3| {
                let r = swing_dir([q[0], q[1], q[2] - e.depth]);
                [r[0], r[1], r[2] + e.depth]
            };
            let problems = panel::problems_of(&self.findings(prj, an, k));
            #[allow(clippy::too_many_arguments)]
            let add_box = |s: &mut Scene3d, layer: u8, side: Option<usize>, obj: Option<usize>, min: V3, max: V3, base: Color32, item: Option<u64>, door: bool, decal: bool| {
                let c = |x: usize, y: usize, z: usize| [if x == 0 { min[0] } else { max[0] }, if y == 0 { min[1] } else { max[1] }, if z == 0 { min[2] } else { max[2] }];
                let faces: [([usize; 4], V3); 6] = [
                    ([0b000, 0b010, 0b011, 0b001], [-1.0, 0.0, 0.0]),
                    ([0b100, 0b101, 0b111, 0b110], [1.0, 0.0, 0.0]),
                    ([0b000, 0b001, 0b101, 0b100], [0.0, -1.0, 0.0]),
                    ([0b010, 0b110, 0b111, 0b011], [0.0, 1.0, 0.0]),
                    ([0b000, 0b100, 0b110, 0b010], [0.0, 0.0, -1.0]),
                    ([0b001, 0b011, 0b111, 0b101], [0.0, 0.0, 1.0]),
                ];
                for (corners, n) in faces {
                    let mut pts = corners.map(|b| c(b >> 2 & 1, b >> 1 & 1, b & 1));
                    let n = if door {
                        pts = pts.map(swing);
                        swing_dir(n)
                    } else {
                        n
                    };
                    let normal = pose.turn(n);
                    s.faces.push(SceneFace { pts: pts.map(|q| pose.apply(q)), n: 4, normal, color: lit(base, normal), layer, side, obj, item: item.map(|id| (k, id)), outline: true, decal });
                }
            };
            // The enclosure's components: walls, side panels, frame, plinth, mounting
            // plate, door leaf.
            let (w, h, dpt) = (e.width, e.height, e.depth);
            let t = e.wall();
            // Enclosure coordinates (y down) to the 3D view (y up).
            let up = |q: V3| [q[0], h - q[1], q[2]];
            let up_box = |(a, b): (V3, V3)| (up([a[0], b[1], a[2]]), up([b[0], a[1], b[2]]));
            // A wall seen from the outside covers the equipment (drawn last, translucent);
            // seen from the inside it is behind everything.
            let wall_side = |s: &mut Scene3d, face: Face, b: (V3, V3), hideable: bool, decal: bool| {
                let n = panel::outward(e, face);
                let c = [0, 1, 2].map(|k| if n[k] > 0.0 { b.1[k] } else if n[k] < 0.0 { b.0[k] } else { (b.0[k] + b.1[k]) / 2.0 });
                // A room's wall opened to show the lines in it: seen through.
                let glass = self.open_walls && e.is_room() && hideable;
                s.sides.push(SceneSide { normal: pose.turn([n[0], -n[1], n[2]]), at: pose.apply(up(c)), inside: 0, hideable, decal, glass });
                Some(s.sides.len() - 1)
            };
            // Door leaf on its hinge: seen from the outside it covers the equipment
            // (translucent), seen from the inside it is behind the door devices.
            let mut door_side = None;
            for c in panel::components(e) {
                let [r, g, b] = openecraft_core::export3d::component_color(c.role);
                let color = Color32::from_rgb(r, g, b);
                for bx in c.boxes {
                    let (min, max) = up_box(bx);
                    match c.role {
                        Role::Door => {
                            let side = *door_side.get_or_insert_with(|| {
                                s.sides.push(SceneSide { normal: pose.turn(swing_dir([0.0, 0.0, 1.0])), at: pose.apply(swing([w / 2.0, h / 2.0, dpt + t])), inside: 3, hideable: false, decal: false, glass: false });
                                s.sides.len() - 1
                            });
                            add_box(&mut s, 3, Some(side), None, min, max, Color32::from_rgb(190, 194, 200), None, true, false);
                        }
                        // Left out on request: the ceiling (the walls in front go per frame).
                        Role::RoomWall(Face::Roof) if self.hide_ceiling => {}
                        Role::RoomWall(face) if ceiling.is_some() => {
                            let side = wall_side(&mut s, face, bx, true, false);
                            let c = ceiling.as_ref().unwrap_or_else(|| unreachable!());
                            for (pts, n) in cut_room_box(c, (min, max), face == Face::Roof, t) {
                                add_poly(&mut s, &pts.iter().map(|&q| pose.apply(q)).collect::<Vec<V3>>(), pose.turn(n), lit(color, pose.turn(n)), 0, side, false);
                            }
                        }
                        Role::Wall(face) | Role::RoomWall(face) => {
                            let side = wall_side(&mut s, face, bx, true, false);
                            add_box(&mut s, 0, side, None, min, max, color, None, false, false);
                        }
                        Role::GlandPlate => {
                            let side = wall_side(&mut s, Face::Bottom, bx, false, false);
                            add_box(&mut s, 0, side, None, min, max, color, None, false, false);
                        }
                        Role::MountingPlate | Role::Body | Role::RackRails => add_box(&mut s, 1, None, None, min, max, color, None, false, false),
                        Role::Plinth | Role::BayingKit => add_box(&mut s, 0, None, None, min, max, color, None, false, false),
                    }
                }
            }
            // The installation zones of a room, seen through.
            if self.show_zones && e.is_room() {
                for band in openecraft_core::routing3d::flush::bands(prj, k, &prj.layout.routing) {
                    let base = zone_color(prj.layout.routing.zones[band.zone].kind);
                    let pts: Vec<V3> = band.corners.iter().map(|&q| pose.apply(up(q))).collect();
                    let centre = [0, 1, 2].map(|i| pts.iter().map(|q| q[i]).sum::<f32>() / 4.0);
                    s.sides.push(SceneSide { normal: face_normal(&pts), at: centre, inside: 0, hideable: false, decal: true, glass: true });
                    let side = Some(s.sides.len() - 1);
                    // Both ways: seen from either side.
                    let n = face_normal(&pts);
                    add_poly(&mut s, &pts, n, base, 0, side, true);
                    let back: Vec<V3> = pts.iter().rev().copied().collect();
                    add_poly(&mut s, &back, [-n[0], -n[1], -n[2]], base, 0, side, true);
                }
            }
            // The roof windows over a sloped ceiling, seen from inside.
            if ceiling.is_some() && !self.hide_ceiling {
                for quad in roofs::windows_in_room(prj, &plant, k) {
                    let pts: Vec<V3> = quad.iter().map(|&q| pose.apply(q)).collect();
                    let n = face_normal(&pts);
                    let pts = if n[1] > 0.0 { pts.into_iter().rev().collect() } else { pts };
                    let n = face_normal(&pts);
                    add_poly(&mut s, &pts, n, lit(GLASS, n), 1, None, true);
                }
            }
            let to3 = |x: f32, y: f32, z: f32| [PLATE_MARGIN + x, h - PLATE_MARGIN - y, PLATE_Z + z];
            // A box (up coordinates) as seen in the view.
            let seen = |(lo, hi): (V3, V3)| bounds(&(0..8).map(|c| pose.apply([[lo[0], hi[0]][c & 1], [lo[1], hi[1]][c >> 1 & 1], [lo[2], hi[2]][c >> 2]])).collect::<Vec<V3>>());
            if !e.is_room() {
                // The inside of the cabinet: within its walls, roof and bottom.
                insides.push(seen(up_box(([t, t, PLATE_Z], [w - t, h - t, dpt - t]))));
            } else {
                // The inside of the room: its floor plan up to its ceiling.
                let (lo, hi) = panel::bounds(&panel::outline(e));
                insides.push(seen(up_box(([lo[0] + t, t, lo[1] + t], [hi[0] - t, h - t, hi[1] - t]))));
            }
            // (not the 19-inch parts of a rack: the cables run behind their panels).
            for it in e.items.iter().filter(|i| i.face == Face::Plate && matches!(i.kind, ItemKind::Device | ItemKind::Busbar | ItemKind::Accessory | ItemKind::Rail) && !panel::rack::is_rack_part(i)) {
                let (lo, hi) = panel::item_box(e, it);
                on_plates.push(seen(up_box(([lo[0], lo[1], lo[2].min(PLATE_Z)], hi))));
            }
            for it in &e.items {
                // Selected, at the end of the selected wire, with a problem or mounted:
                // the whole item in that colour, its model's own colours otherwise.
                let highlight = if self.selection.contains(&it.id) && k == ei {
                    Some(Color32::from_rgb(255, 150, 40))
                } else if wire_ends.map(|(a, b)| a == it.id || b == it.id).unwrap_or(false) {
                    Some(WIRE_HIGHLIGHT)
                } else if problems.contains(&it.id) {
                    Some(Color32::from_rgb(230, 70, 70))
                } else if self.done_items.contains(&it.id) {
                    Some(Color32::from_rgb(110, 185, 125))
                } else {
                    None
                };
                let base = match highlight {
                    Some(c) => c,
                    None => match it.kind {
                        ItemKind::Rail => Color32::from_rgb(205, 208, 212),
                        ItemKind::Duct => Color32::from_rgb(150, 154, 160),
                        ItemKind::Busbar => Color32::from_rgb(200, 120, 60),
                        ItemKind::Cutout => match it.opening {
                            Opening::Door => Color32::from_rgb(150, 110, 70),
                            Opening::Window => Color32::from_rgb(160, 205, 235),
                            Opening::Cutout | Opening::Penetration | Opening::Entry => Color32::from_rgb(40, 40, 44),
                        },
                        ItemKind::Device => Color32::from_rgb(170, 186, 210),
                        ItemKind::Accessory => Color32::from_rgb(95, 100, 110),
                        ItemKind::MountingPlate => Color32::from_rgb(185,189,195),
                    },
                };
                let on_door = it.face == Face::Door;
                let (b0, b1) = panel::item_box(e, it);
                let (min, max) = if it.kind == ItemKind::Cutout && it.face == Face::Plate {
                    // Dark patch just in front of the plate.
                    (to3(it.x, it.y + it.h, 0.0), to3(it.x + it.w, it.y, 0.3))
                } else if it.kind == ItemKind::Cutout && it.face.is_wall() {
                    // Through the wall, a little proud of both of its surfaces.
                    let n = panel::outward(e, it.face);
                    let (a, b) = ([0, 1, 2].map(|k| b0[k] - n[k].abs() * 0.5), [0, 1, 2].map(|k| b1[k] + n[k].abs() * 0.5));
                    (up([a[0], b[1], a[2]]), up([b[0], a[1], b[2]]))
                } else {
                    (up([b0[0], b1[1], b0[2]]), up([b1[0], b0[1], b1[2]]))
                };
                let obj = (!matches!(it.kind, ItemKind::Cutout | ItemKind::Rail)).then(|| {
                    let corners: Vec<V3> = (0..8)
                        .map(|c| {
                            let q = [if c & 1 == 0 { min[0] } else { max[0] }, if c & 2 == 0 { min[1] } else { max[1] }, if c & 4 == 0 { min[2] } else { max[2] }];
                            pose.apply(if on_door { swing(q) } else { q })
                        })
                        .collect();
                    s.objects.push(bounds(&corners));
                    s.ducts.push(it.kind == ItemKind::Duct);
                    exact.push(!on_door && (pose.angle.sin() * pose.angle.cos()).abs() < 1e-4);
                    s.objects.len() - 1
                });
                // Devices with a 3D model in their part are drawn with it instead of a box.
                if let Some(painted) = panel::device_mesh_painted(prj, it) {
                    let (tris, colors): (Vec<_>, Vec<_>) = painted.into_iter().unzip();
                    for (tri, own) in panel::place_triangles(e, it, tris).into_iter().zip(colors) {
                        let base = match (highlight, own) {
                            (None, Some([r, g, b])) => Color32::from_rgb(r, g, b),
                            _ => base,
                        };
                        let w = tri.map(up).map(|q| if on_door { swing(q) } else { q });
                        let n = norm(cross(sub(w[1], w[0]), sub(w[2], w[0])));
                        if n == [0.0; 3] {
                            continue; // no area
                        }
                        let normal = pose.turn(n);
                        let w = w.map(|q| pose.apply(q));
                        s.faces.push(SceneFace { pts: [w[0], w[1], w[2], w[2]], n: 3, normal, color: lit(base, normal), layer: EQUIPMENT, side: None, obj, item: Some((k, it.id)), outline: false, decal: false });
                    }
                    continue;
                }
                // Cable ducts of a cabinet: slotted walls and the cover.
                if it.kind == ItemKind::Duct && e.is_room() {
                    // A cable tray: its inside is a channel, open towards the room.
                    let (b0, b1) = panel::item_box(e, it);
                    let (width, _) = panel::duct_inside(it);
                    let side = (it.w.min(it.h) - width) / 2.0;
                    let n = vec3::sub(panel::face_point(e, it.face, [0.0, 0.0, 1.0]), panel::face_point(e, it.face, [0.0; 3]));
                    let up_axis = (0..3).max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs())).unwrap_or(1);
                    let along = (0..3).filter(|&k| k != up_axis).max_by(|&i, &j| (b1[i] - b0[i]).total_cmp(&(b1[j] - b0[j]))).unwrap_or(0);
                    let across = 3 - up_axis - along;
                    let (mut lo, mut hi) = (b0, b1);
                    (lo[across], hi[across]) = (b0[across] + side, b1[across] - side);
                    if n[up_axis] > 0.0 {
                        lo[up_axis] += side;
                    } else {
                        hi[up_axis] -= side;
                    }
                    channels.push(Channel { inside: seen(up_box((lo, hi))), floor: pose.turn([n[0], -n[1], n[2]]), rim: 0.0, wall: 0.0 });
                }
                if it.kind == ItemKind::Duct && !e.is_room() {
                    if it.face == Face::Plate {
                        // Its inside, between the walls and from the base to the cover.
                        let (b0, b1) = panel::item_box(e, it);
                        let (width, depth) = panel::duct_inside(it);
                        let side = (it.w.min(it.h) - width) / 2.0;
                        let (mut lo, mut hi) = (b0, b1);
                        let across = if it.w >= it.h { 1 } else { 0 };
                        (lo[across], hi[across]) = (b0[across] + side, b1[across] - side);
                        (lo[2], hi[2]) = (b0[2] + side, b0[2] + side + depth);
                        channels.push(Channel { inside: seen(up_box((lo, hi))), floor: pose.turn([0.0, 0.0, 1.0]), rim: panel::duct_walls(it).0 - side, wall: side });
                        lids.push(seen(up_box(([b0[0], b0[1], b1[2]], [b1[0], b1[1], b1[2] + dpt]))));
                        duct_parts.extend(panel::duct_boxes(e, it, false).into_iter().filter(|(_, _, finger)| !finger).map(|(bx, ..)| seen(up_box(bx))));
                    }
                    for (bx, [r, g, b], finger) in panel::duct_boxes(e, it, !self.open_ducts) {
                        let (min, max) = up_box(bx);
                        let first = s.faces.len();
                        add_box(&mut s, EQUIPMENT, None, obj, min, max, highlight.unwrap_or(Color32::from_rgb(r, g, b)), Some(it.id), on_door, false);
                        if let (true, false, Some(duct)) = (finger, on_door, obj) {
                            let corners: Vec<V3> = (0..8).map(|c| pose.apply([[min[0], max[0]][c & 1], [min[1], max[1]][c >> 1 & 1], [min[2], max[2]][c >> 2]])).collect();
                            fingers.push((duct, first..s.faces.len(), bounds(&corners)));
                        }
                    }
                    continue;
                }
                let (layer, side) = match it.kind {
                    ItemKind::Cutout if it.face.is_wall() => (0, wall_side(&mut s, it.face, (b0, b1), false, true)),
                    ItemKind::Cutout => (1, None),
                    ItemKind::Rail => (2, None),
                    ItemKind::MountingPlate => (1,None),
                    ItemKind::Duct | ItemKind::Busbar | ItemKind::Device | ItemKind::Accessory => (EQUIPMENT, None),
                };
                let decal = it.kind == ItemKind::Cutout && !it.face.is_wall();
                add_box(&mut s, layer, side, obj, min, max, base, Some(it.id), on_door, decal);
            }
            // The drives on the floor: shafts and couplings, pulleys and belts, gear
            // cases; picked as their machine.
            for d in panel::drives(prj, e).into_iter().filter(|d| !d.triangles.is_empty()) {
                let selected = k == ei && self.selection.contains(&d.machine);
                let pts: Vec<V3> = d.triangles.iter().flat_map(|(t, _)| t.map(|q| pose.apply(up(q)))).collect();
                s.objects.push(bounds(&pts));
                s.ducts.push(false);
                exact.push(false);
                let obj = Some(s.objects.len() - 1);
                for (tri, [r, g, b]) in d.triangles {
                    let w = tri.map(up);
                    let n = norm(cross(sub(w[1], w[0]), sub(w[2], w[0])));
                    if n == [0.0; 3] {
                        continue;
                    }
                    let normal = pose.turn(n);
                    let base = if selected { Color32::from_rgb(255, 150, 40) } else { Color32::from_rgb(r, g, b) };
                    let w = w.map(|q| pose.apply(q));
                    s.faces.push(SceneFace { pts: [w[0], w[1], w[2], w[2]], n: 3, normal, color: lit(base, normal), layer: EQUIPMENT, side: None, obj, item: Some((k, d.machine)), outline: false, decal: false });
                }
            }
            // Wires inside the ducts; the selected one is drawn on top of everything.
            // Wires routed in 3D are drawn with the routing below, cores of a cable routed
            // in 3D with their cable.
            let routed_3d = |w: &panel::RoutedWire| openecraft_core::routing3d::wire_in_3d(&prj.layout, w);
            // What stands on the plate in the way of the wires along it (by item), down to
            // the plate: no wire passes behind a device on its rail.
            let blockers: Vec<(u64, (V3, V3))> = e
                .items
                .iter()
                .filter(|i| i.face == Face::Plate && matches!(i.kind, ItemKind::Device | ItemKind::Busbar | ItemKind::Accessory | ItemKind::Rail))
                .map(|i| {
                    let (lo, hi) = up_box(panel::item_box(e, i));
                    (i.id, ([lo[0], lo[1], lo[2].min(PLATE_Z)], hi))
                })
                .collect();
            let in_cable = |w: &panel::RoutedWire| !w.cable.is_empty() && an.routing.item(&w.cable).is_some_and(|c| c.routed());
            let duct_boxes: Vec<(V3, V3)> = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Plate).map(|i| up_box(panel::item_box(e, i))).collect();
            for (wi, wire) in an.wiring.iter().enumerate().filter(|(_, wire)| wire.in_enclosure(k) && !routed_3d(wire)) {
                let selected = Some(wi) == selected_wire;
                if in_cable(wire) && !selected {
                    continue;
                }
                let done = self.done_wires.contains(&wire.key());
                if !self.show_wires && !selected && !done {
                    continue;
                }
                let (color, stripe, width) = super::wire_look(&wire.color, selected, done, selected_wire.is_some());
                let diameter = openecraft_core::harness::outer_diameter(&wire.cross_section);
                // Door ends run along the door and over the hinge loop; the part on the
                // door swings with it.
                let door_end = |at: V3, route: &Option<panel::Route>, plate: Option<&openecraft_core::geom::P>, z: f32| -> Vec<V3> {
                    let (Some(q), Some(route)) = (plate, route) else { return Vec::new() };
                    let (path, n) = panel::door_path(e, at, route, [PLATE_MARGIN + q.x, PLATE_MARGIN + q.y, PLATE_Z + z]);
                    path.into_iter().enumerate().map(|(i, v)| if i < n { swing(up(v)) } else { up(v) }).collect()
                };
                // Cable passages (line-up coordinates) in this enclosure's view: through
                // one straight on, for the bend radius on this side of its wall.
                let passage = |q: V3| vec3::add(up([q[0] - pos[k][0], q[1] - pos[k][1], q[2] - pos[k][2]]), slot(q, wi));
                let lead = prj.layout.routing.bend_radius(RouteKind::Wire, diameter) + diameter / 2.0 + 1.0;
                let through = |q: V3, inward: f32| {
                    let at = passage(q);
                    [at, [at[0] + inward * lead, at[1], at[2]]]
                };
                let paths: Vec<(usize, Vec<V3>)> = match wire.door {
                    (Some(a), Some(b)) if wire.on_door() => match &wire.door_routes.0 {
                        Some(r) => vec![(0, panel::door_jumper_path(e, a, b, r).into_iter().map(|v| swing(up(v))).collect())],
                        None => Vec::new(),
                    },
                    (a, b) => wire
                        .stretch_passages()
                        .into_iter()
                        .enumerate()
                        .filter(|(_, s)| s.0 == k)
                        .map(|(i, (_, route, entry, exit))| {
                            // In the ducts at half their depth; without them on the plate, a short
                            // jumper between two connection points at their height.
                            let jumper = match (entry, exit, a, b, wire.terminals) {
                                (None, None, None, None, (Some(s), Some(t))) if !route.via_ducts && route.plate_length < JUMPER => Some(s[2].min(t[2]) - PLATE_Z),
                                _ => None,
                            };
                            // A jumper on the plate runs over the rails.
                            let z = if route.via_ducts { panel::DUCT_PROFILE.1 * 0.5 } else { jumper.unwrap_or(5.0).max(panel::RAIL_PROFILE.1 + diameter) };
                            // Out of its own devices at the height of its connection points,
                            // over what stands in its way along the plate (but its own devices);
                            // from a passage down to the plate before the first duct.
                            let mut along: Vec<V3> = route.points.iter().map(|q| to3(q.x, q.y, z)).collect();
                            if let (Some(q), Some(first)) = (entry, along.first().copied()) {
                                let at = through(q, 1.0)[1];
                                along.insert(0, [at[0], at[1], first[2]]);
                            }
                            if let (Some(q), Some(last)) = (exit, along.last().copied()) {
                                let at = through(q, -1.0)[1];
                                along.push([at[0], at[1], last[2]]);
                            }
                            // The ways out of the connection points belong to it (they run
                            // along the fronts of the devices beside, over which they are
                            // lifted too).
                            let own = |id: u64| blockers.iter().find(|b| b.0 == id).map(|b| b.1);
                            let (mut from_terminal, mut to_terminal) = (false, false);
                            if let (None, None, Some(t), Some(b)) = (entry, a, wire.terminals.0.map(up), own(wire.items.0)) {
                                leave_device(&mut along, b, t[2]);
                                under_terminal(&mut along, t, diameter);
                                down_before_duct(&mut along, t, &duct_boxes, diameter / 2.0);
                                along.insert(0, t);
                                from_terminal = true;
                            }
                            if let (None, None, Some(t), Some(b)) = (exit, b, wire.terminals.1.map(up), own(wire.items.1)) {
                                along.reverse();
                                leave_device(&mut along, b, t[2]);
                                under_terminal(&mut along, t, diameter);
                                down_before_duct(&mut along, t, &duct_boxes, diameter / 2.0);
                                along.insert(0, t);
                                along.reverse();
                                to_terminal = true;
                            }
                            // From the connection points down to the plate.
                            let mut v = match (entry, a) {
                                (None, Some(a)) => door_end(a, &wire.door_routes.0, route.points.first(), z),
                                (None, None) if !from_terminal => wire.terminals.0.map(up).into_iter().collect(),
                                _ => Vec::new(),
                            };
                            v.extend(entry.into_iter().flat_map(|q| through(q, 1.0)));
                            let around: Vec<(V3, V3)> = blockers.iter().filter(|(id, _)| *id != wire.items.0 && *id != wire.items.1).map(|(_, b)| *b).collect();
                            v.extend(over(&along, &around, &duct_boxes, diameter / 2.0, prj.layout.routing.bend_radius(RouteKind::Wire, diameter)));
                            v.extend(exit.into_iter().flat_map(|q| through(q, -1.0).into_iter().rev()));
                            match (exit, b) {
                                (None, Some(b)) => v.extend(door_end(b, &wire.door_routes.1, route.points.last(), z).into_iter().rev()),
                                (None, None) if !to_terminal => v.extend(wire.terminals.1.map(up)),
                                _ => {}
                            }
                            (i, v)
                        })
                        .collect(),
                };
                // Bent round their corners as round tubes, laid beside the others in front of
                // the mounting plate.
                let bend = prj.layout.routing.bend_radius(RouteKind::Wire, diameter);
                let on_plate = !e.is_room() && wire.door == (None, None);
                for (i, path) in &paths {
                    let (rest, below) = match on_plate {
                        true => (pose.turn([0.0, 0.0, 1.0]), path.iter().map(|q| q[2] - PLATE_Z).fold(f32::INFINITY, f32::min) - diameter / 2.0 - super::routing_ui::PLATE_CLEAR),
                        false => ([0.0; 3], f32::INFINITY),
                    };
                    let path: Vec<V3> = path.iter().map(|q| pose.apply(*q)).collect();
                    if diameter > 0.0 {
                        let bends = vec![bend; path.len()];
                        wire_pieces.push((wi, *i, TubeWay { points: path, bends, diameter, color, stripe, width, on_top: selected, target: RouteRef::Wire(wi), sides: WIRE_SIDES, steps: 2, rest, below, as_is: false, leave: [[0.0; 3]; 2] }));
                        continue;
                    }
                    for seg in path.windows(2) {
                        let (a, b) = (seg[0], seg[1]);
                        if selected {
                            s.lines.push(SceneLine { a, b, color, stripe: None, least: width, diameter, tube: false });
                        } else {
                            equipment_wires.push(SceneLine { a, b, color, stripe, least: width, diameter, tube: false });
                        }
                        s.picks.push((a, b, RouteRef::Wire(wi)));
                    }
                }
            }
            // Enclosure outline (the front of a cabinet; rooms show their walls)
            if !e.is_room() {
                let corners = [[0.0, 0.0, dpt], [w, 0.0, dpt], [w, h, dpt], [0.0, h, dpt]];
                s.outlines.extend((0..4).map(|i| (pose.apply(corners[i]), pose.apply(corners[(i + 1) % 4]))));
            }
            // Device labels on the front faces
            for it in e.items.iter().filter(|i| i.kind == ItemKind::Device) {
                let (b0, b1) = panel::item_box(e, it);
                // Door devices show their front on the outside of the door leaf.
                let (front, normal) = if it.face == Face::Door {
                    (swing(up([(b0[0] + b1[0]) / 2.0, (b0[1] + b1[1]) / 2.0, b1[2] + 2.5])), swing_dir([0.0, 0.0, 1.0]))
                } else {
                    // The side facing into the enclosure.
                    let n = panel::face_frame(e, it.face)[3];
                    let q = panel::face_point(e, it.face, [it.x + it.w / 2.0, it.y + it.h / 2.0, it.z + it.d]);
                    (up(q), [n[0], -n[1], n[2]])
                };
                s.labels.push(SceneLabel { at: pose.apply(front), normal: pose.turn(normal), size: it.w.min(it.h), tag: it.tag.clone(), warning: false });
            }
        }
        // A wire through the cable passages between the cabinets is one tube, bent there.
        wire_pieces.sort_by_key(|p| (p.0, p.1));
        let mut last: Option<(usize, usize)> = None;
        for (wi, i, t) in wire_pieces {
            let next = last.is_some_and(|(w, j)| w == wi && j + 1 == i);
            match pending.last_mut() {
                Some(prev) if next && prev.points.last().zip(t.points.first()).is_some_and(|(a, b)| vec3::dist(*a, *b) < 0.5) => {
                    prev.points.extend(t.points.into_iter().skip(1));
                    prev.bends.extend(t.bends.into_iter().skip(1));
                    prev.below = prev.below.min(t.below);
                    prev.leave[1] = t.leave[1];
                }
                _ => pending.push(t),
            }
            last = Some((wi, i));
        }
        // 3D routing across the enclosures shown: routing paths, reservations, cables and
        // tubes (and the wires routed in 3D), from layout to view coordinates through the
        // enclosure a point lies in (the nearest one along the line-up), points of a room
        // through the room.
        let to_view = |room: Option<usize>, q: V3| {
            let gap = |k: usize| (pos[k][0] - q[0]).max(q[0] - pos[k][0] - prj.layout.enclosures[k].width).max(0.0);
            let in_room = room.filter(|r| prj.layout.enclosures[*r].is_room()).and_then(|r| shown.iter().find(|s| s.0 == r));
            let (k, pose) = in_room.or_else(|| shown.iter().min_by(|a, b| gap(a.0).total_cmp(&gap(b.0)))).copied().unwrap_or((ei, SitePose::default()));
            pose.apply([q[0] - pos[k][0], prj.layout.enclosures[k].height - (q[1] - pos[k][1]), q[2] - pos[k][2]])
        };
        let keys: Vec<usize> = shown.iter().map(|s| s.0).collect();
        // What could not be routed is not drawn: a warning at each of its ends says so.
        for it in an.routing.items.iter().filter(|i| !i.routed()) {
            for end in it.ends.iter().filter(|e| e.enclosure.is_some_and(|k| keys.contains(&k))) {
                if let Some(q) = end.point {
                    let tag = format!("{} {}\n{}", it.kind.title(), it.label, it.status_text());
                    s.labels.push(SceneLabel { at: to_view(end.enclosure, q), normal: [0.0; 3], size: 60.0, tag, warning: true });
                }
            }
        }
        for line in self.routing_lines_3d(prj, an, &keys, &to_view, selected_wire) {
            if line.diameter > 0.0 {
                let (sides, steps) = if matches!(line.target, RouteRef::Wire(_)) { (WIRE_SIDES, 2) } else { (TUBE_SIDES, 4) };
                pending.push(TubeWay { points: line.points, bends: line.bends, diameter: line.diameter, color: line.color, stripe: line.stripe, width: line.width, on_top: line.on_top, target: line.target, sides, steps, rest: line.rest, below: line.below, as_is: line.as_is, leave: line.leave });
                continue;
            }
            for seg in line.points.windows(2) {
                let l = SceneLine { a: seg[0], b: seg[1], color: line.color, stripe: line.stripe, least: line.width, diameter: 0.0, tube: false };
                if line.on_top {
                    s.lines.push(SceneLine { stripe: None, ..l });
                } else {
                    equipment_wires.push(l);
                }
                if !matches!(line.target, RouteRef::None) {
                    s.picks.push((seg[0], seg[1], line.target.clone()));
                }
            }
        }
        // Wires, cables and tubes that run together side by side, each on a lane of its own,
        // bent round their corners with their bend radius and pushed apart where they still
        // run inside or through each other, as round tubes (a two-colour one with a stripe
        // along it); kept out of the devices, busbars and accessories (their own boxes).
        // Until that is worked out, as they were routed; the cores where a cable parts
        // stay as they are, the others keep off them.
        let mut obstacles: Vec<(V3, V3)> = (0..s.objects.len()).filter(|&i| exact[i] && !s.ducts[i]).map(|i| s.objects[i]).collect();
        obstacles.extend(lids);
        obstacles.extend(duct_parts);
        obstacles.extend(on_plates);
        // The cable passages between bayed cabinets: what runs through one stays in it
        // (free to move within it).
        for &(k, pose) in shown.iter().filter(|s| !prj.layout.enclosures[s.0].is_room()) {
            let e = &prj.layout.enclosures[k];
            for p in panel::passages(&prj.layout, &pos, k) {
                let c = [p.at[0] - pos[k][0], p.at[1] - pos[k][1], p.at[2] - pos[k][2]];
                let t = e.wall();
                let corners: Vec<V3> = (0..8).map(|i| pose.apply([c[0] + if i & 1 == 0 { -t } else { t }, e.height - (c[1] + if i & 2 == 0 { -p.half[0] } else { p.half[0] }), c[2] + if i & 4 == 0 { -p.half[1] } else { p.half[1] }])).collect();
                insides.push(([0, 1, 2].map(|i| corners.iter().map(|q| q[i]).fold(f32::INFINITY, f32::min)), [0, 1, 2].map(|i| corners.iter().map(|q| q[i]).fold(f32::NEG_INFINITY, f32::max))));
            }
        }
        let ways = pending.iter().map(|t| LayoutWay { points: t.points.clone(), bends: t.bends.clone(), diameter: t.diameter, rest: t.rest, below: t.below, steps: t.steps, held: t.as_is, leave: t.leave }).collect();
        let laid = match lay_out(LayoutInput { ways, obstacles, channels, insides }) {
            Ok(ways) => ways,
            Err(key) => {
                s.waiting = Some(key);
                Arc::new(pending.iter().map(|t| vec3::fillet_each(&t.points, &t.bends, t.steps)).collect())
            }
        };
        let mut laid = laid.iter();
        for t in &pending {
            let points = laid.next().unwrap_or(&t.points);
            tube_faces(&mut s.tubes, points, t.diameter, t.color, t.stripe, t.sides);
            tube_ways.push((points.clone(), t.diameter / 2.0));
            s.tube_paths.push((points.clone(), t.diameter / 2.0, t.target.clone()));
            for seg in points.windows(2) {
                let l = SceneLine { a: seg[0], b: seg[1], color: t.color, stripe: t.stripe, least: t.width, diameter: t.diameter, tube: true };
                if t.on_top {
                    s.lines.push(SceneLine { stripe: None, ..l });
                } else {
                    equipment_wires.push(l);
                }
                if !matches!(t.target, RouteRef::None) {
                    s.picks.push((seg[0], seg[1], t.target.clone()));
                }
            }
        }
        let in_view = |q: V3| keys.iter().any(|&k| q[0] >= pos[k][0] && q[0] <= pos[k][0] + prj.layout.enclosures[k].width);
        if self.show_wires {
            s.branches = an.routing.branches.iter().filter(|b| in_view(b.at)).map(|b| to_view(None, b.at)).collect();
        }
        // Wires, cut where they enter and leave the ducts and the devices: the parts
        // inside a device are not seen.
        let cut: Vec<usize> = (0..s.objects.len()).filter(|&i| s.ducts[i] || exact[i]).collect();
        let cut_boxes: Vec<(V3, V3)> = cut.iter().map(|&i| if s.ducts[i] { (s.objects[i].0.map(|v| v - 0.5), s.objects[i].1.map(|v| v + 0.5)) } else { s.objects[i] }).collect();
        for l in equipment_wires {
            let at = |t: f32| [0, 1, 2].map(|k| l.a[k] + (l.b[k] - l.a[k]) * t);
            for (t0, t1, inside) in vec3::split_by_boxes(l.a, l.b, &cut_boxes) {
                let inside = inside.map(|c| cut[c]);
                if inside.is_some_and(|i| !s.ducts[i]) {
                    continue;
                }
                s.wires.push(SceneWire { line: SceneLine { a: at(t0), b: at(t1), ..l.clone() }, inside });
            }
        }
        self.building_faces(&mut s, prj, ei, shown, &lit);
        let hit = fingers_hit(&fingers, &tube_ways, &s.objects);
        if !hit.is_empty() {
            let mut keep = vec![true; s.faces.len()];
            for i in hit.into_iter().flat_map(|f| fingers[f].1.clone()) {
                keep[i] = false;
            }
            let mut i = 0;
            s.faces.retain(|_| {
                i += 1;
                keep[i - 1]
            });
        }
        let pts: Vec<V3> = s.faces.iter().flat_map(|f| f.pts).chain(s.wires.iter().flat_map(|w| [w.line.a, w.line.b])).chain(s.lines.iter().flat_map(|l| [l.a, l.b])).collect();
        s.bounds = if pts.is_empty() { ([0.0; 3], [0.0; 3]) } else { bounds(&pts) };
        s
    }
}

#[cfg(test)]
mod tests;


