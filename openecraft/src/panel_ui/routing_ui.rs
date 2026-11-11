//! 3D routing in the panel layout: routing paths and reservations drawn over the
//! mounting plate, the routed cables and tubes, the routing rules and the freezing of
//! routes for production.

use super::{PanelUi, PATH_COLOR, RESERVE_COLOR, WIRE_HIGHLIGHT};
use openecraft_core::geom::{p, P};
use openecraft_core::model::Project;
use openecraft_core::panel::{self, PLATE_MARGIN, PLATE_Z};
use openecraft_core::parts::vec3::{add, dist, norm, sub};
use openecraft_core::analysis::Analysis;
use openecraft_core::routing3d::{self, Class, Laying, PathKind, ReserveMode, Reservation, RouteKind, RoutingPath, ShieldEnds, ZoneKind};
use egui::{Color32, Pos2, Rect, RichText, Shape, Stroke};
use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::icons::{self, Icon};

/// Colour of a class in the drawings.
pub(super) fn class_color(c: Class) -> Color32 {
    match c {
        Class::Power => Color32::from_rgb(225, 80, 40),
        Class::Control => Color32::from_rgb(40, 110, 230),
        Class::Signal => Color32::from_rgb(150, 70, 210),
        Class::Fluid => Color32::from_rgb(30, 170, 200),
    }
}

/// What a line of the 3D routing belongs to (for picking).
#[derive(Clone)]
pub(super) enum RouteRef {
    Item(String),
    Wire(usize),
    None,
}

/// A line of the 3D routing in view coordinates.
pub(super) struct RouteLine {
    pub points: Vec<[f32; 3]>,
    pub color: Color32,
    /// Second colour of a two-colour wire, drawn as a stripe.
    pub stripe: Option<Color32>,
    /// Width in pixels; the least one where the line has a `diameter`.
    pub width: f32,
    /// Outer diameter (mm) of a wire, cable or tube, drawn in perspective; 0 for none.
    pub diameter: f32,
    /// Bend radius (mm) each corner is rounded with (one per point); none: sharp corners.
    pub bends: Vec<f32>,
    /// In a cabinet: the direction away from its mounting plate and how far the line
    /// may come nearer to it (when it is laid beside others); none in a room.
    pub rest: [f32; 3],
    pub below: f32,
    pub on_top: bool,
    pub target: RouteRef,
    /// Drawn as it is, not laid beside the others (the cores where a cable parts).
    pub as_is: bool,
    /// The direction the route leaves the first and the last point in, where they are
    /// its ends and it is known (view coordinates, as long as the cells of the routing
    /// there are wide; none: zero).
    pub leave: [[f32; 3]; 2],
    /// How a wire or core looks at its first and last point where it is connected there.
    pub ends: [super::scene3d::EndLook; 2],
}

/// How the end `k` of a routed wire looks at its connection point; dimmed like the wire.
fn route_end(prj: &Project, an: &Analysis, it: &openecraft_core::routing3d::RoutedItem, k: usize, dim: bool) -> super::scene3d::EndLook {
    let Some(w) = it.wire.and_then(|wi| an.wiring.get(wi)).filter(|_| it.kind == RouteKind::Wire) else { return Vec::new() };
    // The route may run from the wire's `to` end.
    let device = |at: &str| at.rsplit_once(':').map_or(at, |(d, _)| d).to_string();
    let swapped = it.ends[0].tag != device(&w.from) && it.ends[0].tag == device(&w.to);
    let (at, other) = if (k == 0) != swapped { (&w.from, &w.to) } else { (&w.to, &w.from) };
    super::scene3d::wire_end(prj, &an.ends, at, other, &w.cross_section, dim)
}

/// How the core of the cable `cable` looks at the connection point `at`.
pub(super) fn core_end(prj: &Project, an: &Analysis, cable: &str, at: &str) -> super::scene3d::EndLook {
    super::terminations::core_ends(prj, an, cable)
        .into_iter()
        .find(|(end, _, _)| end == at)
        .map(|(_, other, cs)| super::scene3d::wire_end(prj, &an.ends, at, &other, &cs, false))
        .unwrap_or_default()
}

/// Diameter of the cores of a cable of `diameter` as thick as it tells for their number
/// (see `routing3d::cable_diameter`).
fn core_diameter(it: &openecraft_core::routing3d::RoutedItem, diameter: f32) -> f32 {
    let cores_at = it.ends.iter().map(|e| e.cores.len()).max().unwrap_or(0).max(2) as f32;
    ((diameter - 2.0) / (1.2 * (1.0 + 0.5 * cores_at).sqrt())).clamp(0.2 * diameter, 0.5 * diameter)
}

/// What [`fan_out`] tells: the points of the cable's way it keeps, those it goes on with
/// to its end and the way of each core.
pub(super) type Fan = (usize, Vec<[f32; 3]>, Vec<Vec<[f32; 3]>>);

/// The device a cable's cores go to, for [`fan_out`]: its box (view coordinates) and
/// the direction out of it, to its front.
pub(super) type FanDevice = (([f32; 3], [f32; 3]), [f32; 3]);

/// Where the cores of a cable part to their connection points at a device: `way` is the
/// cable's route up to the device, its last point the middle of the connection points
/// `at`, each with the direction its core leaves it in (view coordinates). The cable
/// ends a little before the device: the points of `way` it keeps and those it goes on
/// with to its end, then the way of each core from there, a smooth curve leaving the
/// cable's end in its direction and coming into the connection point against the
/// direction the core leaves it in, within the box of its two ends. In a cabinet
/// (`device`: the box of the device and the direction out of it, to its front) the
/// cable ends clear of the device's box, and a core that passes other connection
/// points on its way (terminals one above the other on the bars) passes them in front
/// of the device. Where the cable ends closer behind a corner than
/// its bend there (`bends`: the bend radius of each point of `way`) reaches, it ends on
/// that bend: its last points lie along it, the cores go on round it.
pub(super) fn fan_out(way: &[[f32; 3]], bends: &[f32], at: &[([f32; 3], [f32; 3])], cable: f32, core: f32, device: Option<FanDevice>) -> Fan {
    use openecraft_core::parts::vec3::{cross, dot, length, scale};
    // Up to the device, without the middle of the connection points.
    let route = &way[..way.len().saturating_sub(1)];
    let Some(&approach) = route.last() else { return (0, way.first().copied().into_iter().collect(), Vec::new()) };
    let middle = way[way.len() - 1];
    let spread = at.iter().map(|(q, _)| dist(*q, middle)).fold(0.0, f32::max);
    // Back along the route to where it is as far from the connection points as the cores
    // need to turn to them (at most two fifths of it, the other end may part too) and
    // the cable is clear of the device.
    let reach = (1.5 * spread).max(3.0 * cable).max(FAN_LEAST);
    let keep_out = device.map(|((lo, hi), _)| (lo.map(|v| v - cable / 2.0 - 1.0), hi.map(|v| v + cable / 2.0 + 1.0)));
    let far = |p: [f32; 3]| dist(p, middle) >= reach && keep_out.is_none_or(|(lo, hi)| (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]));
    // The direction out of the device and how far out a core passing its other
    // connection points stays.
    let front = device.map(|((lo, hi), f)| (f, (0..8).map(|c| dot([[lo[0], hi[0]][c & 1], [lo[1], hi[1]][c >> 1 & 1], [lo[2], hi[2]][c >> 2]], f)).fold(f32::NEG_INFINITY, f32::max) + core / 2.0 + 1.0));
    let mut left = 0.4 * route.windows(2).map(|s| dist(s[0], s[1])).sum::<f32>();
    let (mut keep, mut split, mut dir) = (route.len(), approach, norm(sub(middle, approach)));
    for k in (1..route.len()).rev() {
        if far(route[k]) {
            break;
        }
        let step = dist(route[k - 1], route[k]);
        if step < 1e-3 {
            continue;
        }
        (keep, dir) = (k, scale(sub(route[k], route[k - 1]), 1.0 / step));
        let at = |t: f32| sub(route[k], scale(dir, t));
        if !far(route[k - 1]) && step < left {
            (left, split) = (left - step, route[k - 1]);
            continue;
        }
        // Where the segment reaches that far (halving), but no further than is left.
        let (mut near, mut far_t) = (0.0, step.min(left));
        if far(at(far_t)) {
            for _ in 0..24 {
                let mid = (near + far_t) / 2.0;
                if far(at(mid)) { far_t = mid } else { near = mid }
            }
        }
        split = at(far_t);
        break;
    }
    // On the bend of the corner before its end.
    let mut tail = vec![split];
    if keep >= 2 {
        let (corner, before) = (route[keep - 1], route[keep - 2]);
        let (u, behind) = (norm(sub(corner, before)), dist(corner, split));
        let turn = dot(u, dir).clamp(-1.0, 1.0).acos();
        let reach = (bends.get(keep - 1).copied().unwrap_or(0.0) * (turn / 2.0).tan()).min(dist(before, corner));
        if turn > 0.2 && turn < 3.0 && behind < reach {
            let r = reach / (turn / 2.0).tan();
            let across = norm(sub(dir, scale(u, turn.cos())));
            let start = sub(corner, scale(u, reach));
            // How far round the bend it is as far behind the corner (along the way on).
            let along = |phi: f32| -reach * turn.cos() + r * phi.sin() * turn.cos() + r * (1.0 - phi.cos()) * turn.sin();
            let (mut lo, mut hi) = (0.0, turn);
            for _ in 0..24 {
                let mid = (lo + hi) / 2.0;
                if along(mid) < behind { lo = mid } else { hi = mid }
            }
            let steps = ((hi / std::f32::consts::FRAC_PI_2 * 8.0).ceil() as usize).max(1);
            tail = (0..=steps).map(|i| hi * i as f32 / steps as f32).map(|phi| add(start, add(scale(u, r * phi.sin()), scale(across, r * (1.0 - phi.cos()))))).collect();
            (keep, split, dir) = (keep - 1, tail[tail.len() - 1], add(scale(u, hi.cos()), scale(across, hi.sin())));
        }
    }
    // Their places in the cable's cross-section: round its axis, in the order they lie
    // across it at the device.
    let aside = |q: [f32; 3]| {
        let d = sub(q, middle);
        sub(d, scale(dir, dot(d, dir)))
    };
    let across = at.iter().map(|(q, _)| aside(*q)).max_by(|a, b| length(*a).total_cmp(&length(*b))).filter(|v| length(*v) > 1e-3).map_or_else(|| any_across(dir), norm);
    let beside = cross(dir, across);
    let n = at.len();
    // Inside the cable, but no core in another.
    let ring = if n > 1 { ((cable - core) / 2.0).max(core / (2.0 * (std::f32::consts::PI / n as f32).sin())) } else { 0.0 };
    let mut slots: Vec<[f32; 3]> = (0..n).map(|k| {
        let a = std::f32::consts::TAU * (k as f32 + 0.5) / n as f32;
        add(scale(across, ring * a.cos()), scale(beside, ring * a.sin()))
    }).collect();
    slots.sort_by(|a, b| dot(*a, across).total_cmp(&dot(*b, across)));
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| dot(aside(at[a].0), across).total_cmp(&dot(aside(at[b].0), across)));
    let mut place = vec![[0.0; 3]; n];
    for (slot, &k) in slots.into_iter().zip(&order) {
        place[k] = slot;
    }
    let ways = at
        .iter()
        .zip(place)
        .map(|(&(q, exit), off)| {
            let start = add(split, off);
            let pull = 0.45 * dist(start, q);
            let (mut b, mut c) = (add(start, scale(dir, pull)), add(q, scale(norm(exit), pull)));
            // Past other connection points in line before its own it goes in front of the
            // device: its control points out there, and as much further as its end lies
            // behind that, so that it stays out until it comes down to its own.
            let mut lo = [0, 1, 2].map(|k| start[k].min(q[k]));
            let mut hi = [0, 1, 2].map(|k| start[k].max(q[k]));
            if let Some((f, out)) = front {
                let line = norm(sub(q, start));
                let passes = at.iter().any(|(o, _)| {
                    let (t, t_q) = (dot(sub(*o, start), line), dot(sub(q, start), line));
                    let aside = sub(sub(*o, start), scale(line, t));
                    let aside = sub(aside, scale(f, dot(aside, f)));
                    t > 0.0 && t < t_q - core && length(aside) < core
                });
                if passes {
                    let target = 2.0 * out - dot(q, f);
                    for p in [&mut b, &mut c] {
                        let lift = target - dot(*p, f);
                        if lift > 0.0 {
                            *p = add(*p, scale(f, lift));
                        }
                    }
                    for k in 0..3 {
                        lo[k] = lo[k].min(b[k]).min(c[k]);
                        hi[k] = hi[k].max(b[k]).max(c[k]);
                    }
                }
            }
            // Within the box of its two ends: a core never swings out past them (out of
            // a corner before the split, into the bars its terminal sits on).
            let within = |p: [f32; 3]| [0, 1, 2].map(|k| p[k].clamp(lo[k], hi[k]));
            let (b, c) = (within(b), within(c));
            (0..=CORE_STEPS)
                .map(|i| {
                    let t = i as f32 / CORE_STEPS as f32;
                    let u = 1.0 - t;
                    [0, 1, 2].map(|k| u * u * u * start[k] + 3.0 * u * u * t * b[k] + 3.0 * u * t * t * c[k] + t * t * t * q[k])
                })
                .collect()
        })
        .collect();
    (keep, tail, ways)
}

/// Lines passing the same cable gland or wall penetration (`through`: line and the
/// point before it, see [`PanelUi::routing_lines_3d`]) pass it side by side, in rows
/// across it as far apart as the thickest is thick, in the order they come from.
fn side_by_side(lines: &mut [RouteLine], through: &[(usize, usize)]) {
    use openecraft_core::parts::vec3::{cross, dot, scale};
    let mut at: std::collections::BTreeMap<[i32; 3], Vec<(usize, usize)>> = std::collections::BTreeMap::new();
    for &(l, i) in through {
        at.entry(lines[l].points[i].map(|v| (v / 10.0).round() as i32)).or_default().push((l, i));
    }
    for mut group in at.into_values().filter(|g| g.len() > 1) {
        let (l, i) = group[0];
        let (a, d) = (lines[l].points[i], norm(sub(lines[l].points[i + 1], lines[l].points[i])));
        let e1 = norm(cross(d, if d[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] }));
        let e2 = cross(d, e1);
        let gap = group.iter().map(|&(l, _)| lines[l].diameter).fold(0.0, f32::max) + 2.0;
        let from = |&(l, i): &(usize, usize)| lines[l].points.get(i.wrapping_sub(1)).map_or(0.0, |q| dot(sub(*q, a), e1));
        group.sort_by(|x, y| from(x).total_cmp(&from(y)).then(x.cmp(y)));
        let cols = (group.len() as f32).sqrt().ceil() as usize;
        let rows = group.len().div_ceil(cols);
        for (k, &(l, i)) in group.iter().enumerate() {
            let (col, row) = ((k % cols) as f32 - (cols - 1) as f32 / 2.0, (k / cols) as f32 - (rows - 1) as f32 / 2.0);
            let off = add(scale(e1, col * gap), scale(e2, row * gap));
            for q in &mut lines[l].points[i..=i + 1] {
                *q = add(*q, off);
            }
        }
    }
}

/// The corner `c` between the run from `p` and a short step to the gland `g` moved so
/// that the step runs on in the direction of the run (`c` where it is further than
/// [`JOIN_STEP`] from `g`).
fn in_line(p: [f32; 3], c: [f32; 3], g: [f32; 3]) -> [f32; 3] {
    use openecraft_core::parts::vec3::{dot, scale};
    if dist(c, g) >= JOIN_STEP || dist(p, c) < 1e-3 {
        return c;
    }
    let u = norm(sub(c, p));
    add(g, scale(u, dot(sub(c, g), u)))
}

/// The box of the item `id` of enclosure `k` in view coordinates (`to_view`: layout to
/// view), for [`fan_out`].
fn fan_device(prj: &Project, pos: &[[f32; 3]], k: usize, id: u64, to_view: &dyn Fn([f32; 3]) -> [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let e = &prj.layout.enclosures[k];
    let item = e.items.iter().find(|i| i.id == id)?;
    let (lo, hi) = panel::item_box(e, item);
    let corners: Vec<[f32; 3]> = (0..8).map(|c| to_view(add([[lo[0], hi[0]][c & 1], [lo[1], hi[1]][c >> 1 & 1], [lo[2], hi[2]][c >> 2]], pos[k]))).collect();
    Some(([0, 1, 2].map(|i| corners.iter().map(|q| q[i]).fold(f32::INFINITY, f32::min)), [0, 1, 2].map(|i| corners.iter().map(|q| q[i]).fold(f32::NEG_INFINITY, f32::max))))
}

/// A direction across `dir`.
fn any_across(dir: [f32; 3]) -> [f32; 3] {
    let other = if dir[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    norm(openecraft_core::parts::vec3::cross(dir, other))
}

/// Points along each of the curved cores where a cable parts ([`fan_out`]).
const CORE_STEPS: usize = 10;
/// How far at least from their connection points the cores of a cable part, mm.
const FAN_LEAST: f32 = 30.0;
/// Steps of a route shorter than this (mm) next to where two of its legs are joined are
/// put in line with the run before them (to the cells of the routing grid beside a
/// cable gland).
const JOIN_STEP: f32 = 30.0;

fn mode_choice(ui: &mut egui::Ui, salt: impl egui::AsIdSalt, mode: &mut ReserveMode) {
    egui::ComboBox::from_id_salt(salt).selected_text(mode.title()).show_ui(ui, |ui| {
        ui.selectable_value(mode, ReserveMode::Keepout, ReserveMode::Keepout.title());
        for c in Class::ALL {
            ui.selectable_value(mode, ReserveMode::For(c), ReserveMode::For(c).title());
        }
    });
}

fn kind_choice(ui: &mut egui::Ui, salt: &str, kind: &mut PathKind) {
    egui::ComboBox::from_id_salt(salt).selected_text(kind.title()).show_ui(ui, |ui| {
        for k in PathKind::ALL {
            ui.selectable_value(kind, k, k.title());
        }
    });
}

/// Room (mm) wires and cables laid side by side keep from the mounting plate (beyond
/// half their thickness): the base of the cable ducts and a little more.
pub(super) const PLATE_CLEAR: f32 = 3.0;

/// Colour of the pipe runs of rooms.
pub(super) const PIPE_COLOR: Color32 = Color32::from_rgb(40, 150, 200);

impl PanelUi {
    /// The pipe runs on the face of a room shown, with their names.
    pub(super) fn draw_pipe_runs(&self, painter: &egui::Painter, prj: &Project, ei: usize, to_screen: &dyn Fn(P) -> Pos2) {
        let e = &prj.layout.enclosures[ei];
        for path in &e.paths {
            let Some((face, pts, _)) = openecraft_core::panel::pipe_run_on_face(e, path) else { continue };
            if face != self.face {
                continue;
            }
            let pts: Vec<Pos2> = pts.into_iter().map(to_screen).collect();
            let selected = self.selected_object == Some(path.id);
            let width = (path.width * self.zoom).max(2.0);
            painter.add(Shape::line(pts.clone(), Stroke::new(width, PIPE_COLOR.gamma_multiply(if selected { 0.45 } else { 0.25 }))));
            painter.add(Shape::line(pts.clone(), Stroke::new(if selected { 2.0 } else { 1.0 }, PIPE_COLOR)));
            if let Some(first) = pts.first() {
                painter.text(*first, egui::Align2::LEFT_BOTTOM, &path.name, egui::FontId::proportional(11.0), PIPE_COLOR);
            }
        }
    }

    /// Makes a routing path of the corners drawn. Returns its id.
    pub(super) fn add_routing_path(&mut self, prj: &mut Project, ei: usize) -> u64 {
        let id = prj.new_id();
        let e = &mut prj.layout.enclosures[ei];
        let z = PLATE_Z + self.path_height;
        let points = self.path_points.drain(..).map(|q| [PLATE_MARGIN + q.x, PLATE_MARGIN + q.y, z]).collect();
        let name = format!("P{}", e.paths.len() + 1);
        e.paths.push(RoutingPath { id, name, kind: self.path_kind, points, width: self.path_width, classes: Vec::new(), reserve: 0.0, face: None });
        id
    }

    /// Makes a reservation over the plate between two corners, through the whole
    /// depth. Returns its id.
    pub(super) fn add_reservation(&mut self, prj: &mut Project, ei: usize, a: P, b: P) -> u64 {
        let id = prj.new_id();
        let e = &mut prj.layout.enclosures[ei];
        let (lo, hi) = (p(a.x.min(b.x), a.y.min(b.y)), p(a.x.max(b.x), a.y.max(b.y)));
        let name = format!("R{}", e.reservations.len() + 1);
        e.reservations.push(Reservation {
            id,
            name,
            min: [PLATE_MARGIN + lo.x, PLATE_MARGIN + lo.y, PLATE_Z],
            max: [PLATE_MARGIN + hi.x, PLATE_MARGIN + hi.y, e.depth],
            mode: self.reserve_mode,
        });
        id
    }

    /// Routing paths, reservations and the routed cables and tubes seen on the plate.
    pub(super) fn draw_routing_2d(&self, painter: &egui::Painter, prj: &Project, an: &Analysis, ei: usize, to_screen: &dyn Fn(P) -> Pos2) {
        let e = &prj.layout.enclosures[ei];
        let plate = |q: [f32; 3]| to_screen(p(q[0] - PLATE_MARGIN, q[1] - PLATE_MARGIN));
        for r in &e.reservations {
            let rect = Rect::from_two_pos(plate(r.min), plate(r.max));
            let color = match r.mode {
                ReserveMode::Keepout => RESERVE_COLOR,
                ReserveMode::For(c) => class_color(c),
            };
            let selected = self.selected_object == Some(r.id);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(if selected { 0.3 } else { 0.15 }));
            painter.rect_stroke(rect, 0.0, Stroke::new(if selected { 2.5 } else { 1.0 }, color), egui::StrokeKind::Inside);
            painter.text(rect.left_top() + egui::vec2(4.0, 4.0), egui::Align2::LEFT_TOP, &r.name, egui::FontId::proportional(11.0), color);
        }
        for path in &e.paths {
            let pts: Vec<Pos2> = path.points.iter().map(|q| plate(*q)).collect();
            let selected = self.selected_object == Some(path.id);
            let width = (path.width * self.zoom).max(2.0);
            painter.add(Shape::line(pts.clone(), Stroke::new(width, PATH_COLOR.gamma_multiply(if selected { 0.45 } else { 0.25 }))));
            painter.add(Shape::line(pts.clone(), Stroke::new(if selected { 2.0 } else { 1.0 }, PATH_COLOR)));
            if let Some(first) = pts.first() {
                painter.text(*first, egui::Align2::LEFT_BOTTOM, &path.name, egui::FontId::proportional(11.0), PATH_COLOR);
            }
        }
        if !self.show_wires && self.selected_route.is_none() {
            return;
        }
        let pos = openecraft_core::panel::positions(&prj.layout);
        for it in an.routing.items.iter().filter(|i| i.kind != RouteKind::Wire && i.enclosures.contains(&ei)) {
            let selected = self.selected_route.as_deref() == Some(it.key.as_str());
            if !self.show_wires && !selected {
                continue;
            }
            let (color, width) = if selected { (WIRE_HIGHLIGHT, 3.0) } else { (class_color(it.class), (it.diameter * self.zoom * 0.5).clamp(1.5, 6.0)) };
            // The legs in this line-up, not those through a room or another line-up.
            for points in it.stretches_in_lineup(&prj.layout, ei) {
                let pts: Vec<Pos2> = points.iter().map(|q| plate([q[0] - pos[ei][0], q[1] - pos[ei][1], q[2]])).collect();
                painter.add(Shape::line(pts, Stroke::new(width, color.gamma_multiply(0.8))));
            }
        }
    }

    /// Selects a cable or tube: the properties show it, the status its length.
    pub(super) fn select_route(&mut self, an: &Analysis, key: &str) {
        self.status = an.routing.item(key).map(|i| format!("{} {}  {:.0} mm", i.kind.title(), i.label, i.length)).unwrap_or_default();
        self.selected_route = Some(key.to_string());
        self.selected_wire = None;
        self.selection.clear();
    }

    /// The cables and tubes drawn over the plate of enclosure `ei` within `tol` of the
    /// plate point `q` (from their outline), nearest first (indices into the routing).
    pub(super) fn cables_at(&self, prj: &Project, an: &Analysis, ei: usize, q: P, tol: f32) -> Vec<usize> {
        let pos = panel::positions(&prj.layout);
        let plate = |v: &[f32; 3]| p(v[0] - pos[ei][0] - PLATE_MARGIN, v[1] - pos[ei][1] - PLATE_MARGIN);
        let mut near: Vec<(usize, f32)> = an
            .routing
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.kind != RouteKind::Wire && it.enclosures.contains(&ei))
            .filter_map(|(i, it)| {
                let d = it.stretches_in_lineup(&prj.layout, ei).into_iter().map(|pts| panel::route_distance(&pts.iter().map(plate).collect::<Vec<P>>(), q)).fold(f32::INFINITY, f32::min);
                let d = d - it.diameter / 2.0;
                (d <= tol).then_some((i, d))
            })
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        near.into_iter().map(|(i, _)| i).collect()
    }

    /// Lines of the 3D routing in view coordinates (`to_view`: layout to view
    /// coordinates, through a room for points of the room): routing paths and reservations of the enclosures shown, the routed
    /// cables and tubes, and the wires when they are routed in 3D.
    pub(super) fn routing_lines_3d(&self, prj: &Project, an: &Analysis, shown: &[usize], to_view: &dyn Fn(Option<usize>, [f32; 3]) -> [f32; 3], selected_wire: Option<usize>) -> Vec<RouteLine> {
        let pos = openecraft_core::panel::positions(&prj.layout);
        let mut out = Vec::new();
        for &k in shown {
            let e = &prj.layout.enclosures[k];
            let g = |q: [f32; 3]| to_view(Some(k), [q[0] + pos[k][0], q[1] + pos[k][1], q[2] + pos[k][2]]);
            for path in &e.paths {
                let selected = self.selected_object == Some(path.id);
                let color = if path.face.is_some() { PIPE_COLOR } else { PATH_COLOR };
                out.push(RouteLine { points: path.points.iter().map(|q| g(*q)).collect(), color, stripe: None, width: if selected { 3.0 } else { 1.5 }, diameter: 0.0, bends: Vec::new(), rest: [0.0; 3], below: f32::INFINITY, on_top: selected, target: RouteRef::None, as_is: false, leave: [[0.0; 3]; 2], ends: Default::default() });
            }
            for r in &e.reservations {
                let color = match r.mode {
                    ReserveMode::Keepout => RESERVE_COLOR,
                    ReserveMode::For(c) => class_color(c),
                };
                let (a, b) = (r.min, r.max);
                let c = |x: usize, y: usize, z: usize| g([[a[0], b[0]][x], [a[1], b[1]][y], [a[2], b[2]][z]]);
                // The twelve edges of the box.
                for (s, t) in [
                    ((0, 0, 0), (1, 0, 0)),
                    ((0, 1, 0), (1, 1, 0)),
                    ((0, 0, 1), (1, 0, 1)),
                    ((0, 1, 1), (1, 1, 1)),
                    ((0, 0, 0), (0, 1, 0)),
                    ((1, 0, 0), (1, 1, 0)),
                    ((0, 0, 1), (0, 1, 1)),
                    ((1, 0, 1), (1, 1, 1)),
                    ((0, 0, 0), (0, 0, 1)),
                    ((1, 0, 0), (1, 0, 1)),
                    ((0, 1, 0), (0, 1, 1)),
                    ((1, 1, 0), (1, 1, 1)),
                ] {
                    out.push(RouteLine { points: vec![c(s.0, s.1, s.2), c(t.0, t.1, t.2)], color, stripe: None, width: 1.0, diameter: 0.0, bends: Vec::new(), rest: [0.0; 3], below: f32::INFINITY, on_top: false, target: RouteRef::None, as_is: false, leave: [[0.0; 3]; 2], ends: Default::default() });
                }
            }
        }
        // Where lines pass a cable gland or wall penetration: line and point (the one
        // before it, the next one behind it).
        let mut through: Vec<(usize, usize)> = Vec::new();
        for it in an.routing.items.iter().filter(|i| i.routed() && i.enclosures.iter().any(|k| shown.contains(k))) {
            // Wires routed in 3D as wires; cables and tubes in the colour of their class,
            // as thick as they are.
            let (color, stripe, width, diameter, selected, target) = match (it.kind, it.wire) {
                (RouteKind::Wire, Some(wi)) if openecraft_core::routing3d::wire_in_3d(&prj.layout, &an.wiring[wi]) => {
                    let selected = selected_wire == Some(wi);
                    let done = self.done_wires.contains(&an.wiring[wi].key());
                    if !self.show_wires && !selected && !done {
                        continue;
                    }
                    let wire = &an.wiring[wi];
                    let (color, stripe, width) = super::wire_look(&wire.color, selected, done, selected_wire.is_some());
                    (color, stripe, width, openecraft_core::harness::outer_diameter(&wire.cross_section), selected, RouteRef::Wire(wi))
                }
                (RouteKind::Wire, _) => continue,
                _ => {
                    let selected = self.selected_route.as_deref() == Some(it.key.as_str());
                    if !self.show_wires && !selected {
                        continue;
                    }
                    let (color, width) = if selected { (WIRE_HIGHLIGHT, 4.0) } else { (class_color(it.class), 2.0) };
                    (color, None, width, it.diameter, selected, RouteRef::Item(it.key.clone()))
                }
            };
            // Leg by leg; legs through an enclosure not shown are left out, legs shown one
            // after the other are one line through the cable gland or wall penetration
            // between them, bent there with the route's bend radius.
            let stretches = it.stretches();
            let radii = it.stretch_radii();
            let radius = it.radii.iter().copied().fold(0.0, f32::max);
            let last = stretches.len().saturating_sub(1);
            // The last stretch drawn and its line.
            let mut joined: Option<(usize, usize)> = None;
            for (s, (leg, points)) in stretches.into_iter().enumerate() {
                let bends = radii.get(s).copied().unwrap_or_default();
                if leg.is_some_and(|k| !shown.contains(&k)) {
                    continue;
                }
                // In a cabinet the mounting plate is behind it: how far it lies in front.
                let in_room = leg.is_some_and(|k| prj.layout.enclosures[k].is_room());
                let cabinet = |q: &[f32; 3]| it.enclosures.iter().copied().find(|&k| !prj.layout.enclosures[k].is_room() && q[0] >= pos[k][0] && q[0] <= pos[k][0] + prj.layout.enclosures[k].width);
                let front = points.iter().filter_map(|q| cabinet(q).map(|k| q[2] - pos[k][2] - PLATE_Z)).fold(f32::INFINITY, f32::min);
                let (rest, below) = match points.first() {
                    Some(q) if !in_room && front.is_finite() => (norm(sub(to_view(leg, [q[0], q[1], q[2] + 1.0]), to_view(leg, *q))), front - it.diameter / 2.0 - PLATE_CLEAR),
                    _ => ([0.0; 3], f32::INFINITY),
                };
                // The way out of the ends of the route (not where the cores part there).
                let mut leave = [[0.0; 3]; 2];
                for (end, q) in [(0, points.first().filter(|_| s == 0)), (1, points.last().filter(|_| s == last))] {
                    if let (Some(q), Some(d)) = (q, it.ends[end].leave.filter(|_| it.ends[end].cores.is_empty())) {
                        let cell = prj.layout.routing.grid.max(if in_room { openecraft_core::routing3d::room::ROOM_GRID } else { 5.0 });
                        leave[end] = sub(to_view(leg, add(*q, openecraft_core::parts::vec3::scale(d, cell))), to_view(leg, *q));
                    }
                }
                // A wire's ends at the ends of the route (not into an enclosure not shown).
                let ends = [(0, s == 0), (1, s == last)].map(|(k, at_end)| if at_end && it.wire.is_some() && it.ends[k].item.is_some() && it.ends[k].cores.is_empty() { route_end(prj, an, it, k, selected_wire.is_some() && !selected) } else { Vec::new() });
                let mut points: Vec<[f32; 3]> = points.iter().map(|q| to_view(leg, *q)).collect();
                let mut bends = bends.to_vec();
                bends.resize(points.len(), 0.0);
                // The cores part a little before the devices at the ends of the first and the
                // last stretch, each into its connection point.
                let (mut cores, mut core_pins): (Vec<Vec<[f32; 3]>>, Vec<String>) = Default::default();
                for (end, at_end) in [(0, s == 0), (1, s == last)] {
                    let n = points.len();
                    if !at_end || it.ends[end].cores.is_empty() || n <= 2 {
                        continue;
                    }
                    // Seen from the far end of the stretch towards this one.
                    if end == 0 {
                        points.reverse();
                        bends.reverse();
                    }
                    let at: Vec<([f32; 3], [f32; 3])> = it.ends[end].cores.iter().zip(&it.ends[end].exits).map(|(q, x)| (to_view(leg, *q), sub(to_view(leg, add(*q, *x)), to_view(leg, *q)))).collect();
                    // In a cabinet the cable ends clear of the device and the cores pass its
                    // other terminals in front of it.
                    let device = it.ends[end].enclosure.zip(it.ends[end].item).filter(|_| rest != [0.0; 3]).and_then(|(k, id)| Some((fan_device(prj, &pos, k, id, &|q| to_view(leg, q))?, rest)));
                    let (keep, tail, fan) = fan_out(&points, &bends, &at, diameter, core_diameter(it, diameter), device);
                    core_pins.extend((0..fan.len()).map(|c| it.ends[end].pins.get(c).cloned().unwrap_or_default()));
                    cores.extend(fan);
                    points.truncate(keep);
                    bends.truncate(keep);
                    bends.extend(tail.iter().map(|_| 0.0));
                    points.extend(tail);
                    if end == 0 {
                        points.reverse();
                        bends.reverse();
                    }
                }
                for (c, pin) in cores.into_iter().zip(core_pins) {
                    out.push(RouteLine { bends: vec![0.0; c.len()], points: c, color, stripe: None, width: width * 0.5, diameter: core_diameter(it, diameter), rest: [0.0; 3], below: f32::INFINITY, on_top: selected, target: target.clone(), as_is: true, leave: [[0.0; 3]; 2], ends: [Vec::new(), core_end(prj, an, &it.key, &pin)] });
                }
                let index = match joined {
                    Some((prev, index)) if prev + 1 == s => {
                        let line = &mut out[index];
                        // Straight through: the short steps from the cells of the routing
                        // grid beside it are taken out by moving the corner before each
                        // in line with it (the run before leans over a little).
                        let n = line.points.len();
                        if n > 2 && points.len() > 1 {
                            line.points[n - 2] = in_line(line.points[n - 3], line.points[n - 2], line.points[n - 1]);
                        }
                        if points.len() > 2 {
                            points[1] = in_line(points[2], points[1], points[0]);
                        }
                        if let Some(b) = line.bends.last_mut() {
                            *b = radius;
                        }
                        if let Some(b) = bends.first_mut() {
                            *b = radius;
                        }
                        through.push((index, line.points.len() - 1));
                        line.points.extend(points);
                        line.bends.extend(bends);
                        if line.rest == [0.0; 3] {
                            line.rest = rest;
                        }
                        line.below = line.below.min(below);
                        line.leave[1] = leave[1];
                        line.ends[1] = ends[1].clone();
                        index
                    }
                    _ => {
                        out.push(RouteLine { points, color, stripe, width, diameter, bends, rest, below, on_top: selected, target: target.clone(), as_is: false, leave, ends: ends.clone() });
                        out.len() - 1
                    }
                };
                joined = Some((s, index));
            }
        }
        side_by_side(&mut out, &through);
        out
    }

    /// Properties of the 3D routing of an enclosure: what is routed, the selected
    /// route, routing paths, reservations, rules and freezing for production.
    pub(super) fn routing_section(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, ei: usize) {
        let r = &an.routing;
        let here: Vec<usize> = (0..r.items.len()).filter(|&i| r.items[i].enclosures.contains(&ei) || r.items[i].ends.iter().any(|e| e.enclosure == Some(ei))).collect();
        let open = self.selected_route.is_some() || self.selected_object.is_some();
        egui::CollapsingHeader::new(tr("3D routing")).id_salt("routing3d").default_open(open).show(ui, |ui| {
            let count = |k: RouteKind| here.iter().filter(|&&i| r.items[i].kind == k && r.items[i].routed()).count();
            ui.label(tr_fmt!("{} wires, {} cables and {} tubes routed", count(RouteKind::Wire), count(RouteKind::Cable), count(RouteKind::Tube)));
            let bundles = r.bundles.iter().filter(|b| b.items.iter().any(|i| here.contains(i))).count();
            ui.label(tr_fmt!("{} bundles, {} branch points", bundles, r.branches.len()));
            for &i in here.iter().filter(|&&i| !r.items[i].routed() && r.items[i].kind != RouteKind::Wire) {
                ui.colored_label(Color32::from_rgb(220, 120, 0), format!("{} {}: {}", r.items[i].kind.title(), r.items[i].label, r.items[i].status_text()));
            }
            ui.separator();
            // Cables and tubes of the enclosure.
            ui.label(RichText::new(tr("Cables and tubes")).strong());
            let mut pick = None;
            egui::Grid::new("rroutes").striped(true).num_columns(4).show(ui, |ui| {
                for &i in here.iter().filter(|&&i| r.items[i].kind != RouteKind::Wire) {
                    let it = &r.items[i];
                    let selected = self.selected_route.as_deref() == Some(it.key.as_str());
                    if ui.selectable_label(selected, RichText::new(&it.label).color(class_color(it.class))).clicked() {
                        pick = Some(if selected { None } else { Some(it.key.clone()) });
                    }
                    ui.label(it.class.title());
                    ui.label(if it.routed() { format!("{:.0} mm", it.length) } else { "–".into() });
                    ui.label(it.status_text());
                    ui.end_row();
                }
            });
            if let Some(key) = pick {
                self.selected_route = key;
                self.selected_wire = None;
            }
            ui.separator();
            self.paths_and_reservations(ui, prj, an, ei);
            ui.separator();
            self.production(ui, prj, an);
            ui.separator();
            rules(ui, prj);
        });
    }

    /// Properties of the selected cable or tube (as those of a selected wire): its route,
    /// shield and freezing, the devices at its ends.
    pub(super) fn route_properties(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis) {
        let r = &an.routing;
        let Some((idx, it)) = self.selected_route.as_ref().and_then(|k| r.items.iter().enumerate().find(|(_, i)| &i.key == k)) else { return };
        ui.label(RichText::new(format!("{} {}", it.kind.title(), it.label)).strong().color(WIRE_HIGHLIGHT));
        egui::Grid::new("rroute").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label(tr("From / to"));
            ui.label(format!("{} – {}", it.ends[0].tag, it.ends[1].tag));
            ui.end_row();
            ui.label(tr("Class"));
            ui.label(it.class.title());
            ui.end_row();
            ui.label(tr("Length"));
            ui.label(tr_fmt!("{} mm (cut {} mm)", format!("{:.0}", it.length), format!("{:.0}", it.cut)));
            ui.end_row();
            ui.label(tr("Free air"));
            ui.label(format!("{:.0} mm", it.free));
            ui.end_row();
            ui.label(tr("Diameter / bends"));
            ui.label(format!("{:.1} mm / {}", it.diameter, it.bends));
            ui.end_row();
            ui.label(tr("Min. bend radius"));
            ui.label(format!("{:.0} mm", it.bend_radius));
            ui.end_row();
            ui.label(tr("Bundles"));
            ui.label(r.bundles_of(idx).len().to_string());
            ui.end_row();
            ui.label(tr("Status"));
            ui.label(it.status_text());
            ui.end_row();
        });
        // How it is laid in rooms: as the rules say, or as set for it.
        ui.horizontal(|ui| {
            ui.label(tr("Laying in rooms"));
            let default = prj.layout.routing.laying;
            let own = prj.layout.laying.get(&it.key).copied();
            let shown = own.map_or_else(|| tr_fmt!("As the rules say ({})", tr(default.title())), |l| tr(l.title()).to_string());
            let mut choice = own;
            egui::ComboBox::from_id_salt("rlaying").selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut choice, None, tr_fmt!("As the rules say ({})", tr(default.title())));
                for l in Laying::ALL {
                    ui.selectable_value(&mut choice, Some(l), tr(l.title()));
                }
            });
            if choice != own {
                match choice {
                    Some(l) => prj.layout.laying.insert(it.key.clone(), l),
                    None => prj.layout.laying.remove(&it.key),
                };
            }
        })
        .response
        .on_hover_text(tr("Flush-mounted lines run in slots in the walls along the installation zones, or down into the floor and along its zone"));
        if it.kind == RouteKind::Cable {
            self.cable_cores(ui, prj, an, &it.key);
            ui.separator();
            self.cable_ends(ui, prj, an, &it.key, [&it.ends[0].tag, &it.ends[1].tag]);
        }
        if let Some(s) = r.shields.iter().find(|s| s.cable == it.key) {
            ui.label(RichText::new(tr("Shield")).strong());
            for e in &s.ends {
                ui.label(format!("{}: {}", e.tag, e.termination.text()));
            }
        }
        if it.kind == RouteKind::Cable {
            if let Some(c) = prj.cables.iter_mut().find(|c| c.tag == it.key) {
                ui.horizontal(|ui| {
                    let mut shielded = routing3d::is_shielded(c);
                    if ui.checkbox(&mut shielded, tr("Shielded")).changed() {
                        c.shielded = Some(shielded);
                    }
                    egui::ComboBox::from_id_salt("shieldends").selected_text(c.shield_ends.title()).show_ui(ui, |ui| {
                        for e in ShieldEnds::ALL {
                            ui.selectable_value(&mut c.shield_ends, e, e.title());
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Outer diameter"));
                    ui.add(egui::DragValue::new(&mut c.diameter).speed(0.1).range(0.0..=100.0).suffix(" mm")).on_hover_text(tr("0 = estimated from cores and cross-section"));
                });
            }
        }
        let frozen = prj.layout.frozen.iter().any(|f| f.key == it.key);
        ui.horizontal(|ui| {
            if frozen {
                if ui.button(tr("Release route")).clicked() {
                    routing3d::release(prj, std::slice::from_ref(&it.key));
                }
            } else if it.routed() && ui.button(tr("Freeze route")).on_hover_text(tr("Keep this route when recalculating, as long as it is still possible")).clicked() {
                routing3d::freeze(prj, r, std::slice::from_ref(&it.key));
            }
            if ui.small_button(tr("Deselect")).clicked() {
                self.selected_route = None;
            }
        });
        ui.separator();
        ui.label(RichText::new(tr("Connected devices")).strong());
        let mut select = None;
        for end in &it.ends {
            let (Some(ei), Some(id)) = (end.enclosure, end.item) else { continue };
            let Some(item) = prj.layout.enclosures.get(ei).and_then(|e| e.item(id)).filter(|i| !i.tag.is_empty()) else { continue };
            ui.horizontal(|ui| {
                if ui.link(RichText::new(&item.tag).monospace()).on_hover_text(tr("Select device")).clicked() {
                    select = Some((ei, id));
                }
                if ui.small_button(tr("Show in schematic")).clicked() {
                    self.goto_schematic = Some(item.tag.clone());
                }
            });
        }
        if let Some((ei, id)) = select {
            self.switch_enclosure(ei);
            self.select_item(id);
        }
    }

    /// The cores of a cable with the connection points each joins: a core selects its
    /// wire, a connection point its device.
    fn cable_cores(&mut self, ui: &mut egui::Ui, prj: &Project, an: &Analysis, cable: &str) {
        let cores: Vec<_> = an.connections.iter().filter(|c| c.cable == cable).collect();
        if cores.is_empty() {
            return;
        }
        ui.separator();
        ui.label(RichText::new(tr("Cores")).strong());
        let mut wire = None;
        let mut device = None;
        egui::Grid::new("rcores").striped(true).num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
            for title in [tr("Core"), tr("Cross-section"), tr("From"), tr("To")] {
                ui.label(RichText::new(title).weak());
            }
            ui.end_row();
            for c in cores {
                let (from, to) = (openecraft_core::netlist::pin_designation(prj, c.a), openecraft_core::netlist::pin_designation(prj, c.b));
                let name = match (c.core.is_empty(), c.color.is_empty()) {
                    (false, false) => format!("{} ({})", c.core, c.color),
                    (false, true) => c.core.clone(),
                    _ => c.color.clone(),
                };
                match an.wiring.iter().find(|w| (w.from == from && w.to == to) || (w.from == to && w.to == from)) {
                    Some(w) => {
                        if ui.link(name).on_hover_text(tr("Select wire")).clicked() {
                            wire = Some(w.key());
                        }
                    }
                    None => {
                        ui.label(name);
                    }
                }
                ui.label(&c.cross_section);
                for (pin, label) in [(c.a, from), (c.b, to)] {
                    let tag = &prj.pages[pin.page].symbols[pin.sym].tag;
                    match panel::locate(prj, tag) {
                        Some((ei, item)) => {
                            if ui.link(RichText::new(label).monospace()).on_hover_text(tr("Select device")).clicked() {
                                device = Some((ei, item.id));
                            }
                        }
                        None => {
                            ui.label(RichText::new(label).monospace());
                        }
                    }
                }
                ui.end_row();
            }
        });
        if let Some(key) = wire {
            self.select_wire(key);
        }
        if let Some((ei, id)) = device {
            self.switch_enclosure(ei);
            self.select_item(id);
        }
    }

    fn paths_and_reservations(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, ei: usize) {
        ui.label(RichText::new(tr("Routing paths")).strong());
        ui.horizontal(|ui| {
            ui.label(tr("New"));
            ui.add(egui::DragValue::new(&mut self.path_height).speed(1.0).range(0.0..=1000.0).prefix("z ").suffix(" mm")).on_hover_text(tr("Height above the mounting plate"));
            ui.add(egui::DragValue::new(&mut self.path_width).speed(1.0).range(5.0..=500.0).suffix(" mm")).on_hover_text(tr("Width"));
            kind_choice(ui, "newpathkind", &mut self.path_kind);
        });
        let e = &mut prj.layout.enclosures[ei];
        let mut remove = None;
        let mut select = None;
        for path in &mut e.paths {
            let selected = self.selected_object == Some(path.id);
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, RichText::new(&path.name).color(PATH_COLOR)).clicked() {
                    select = Some(if selected { None } else { Some(path.id) });
                }
                ui.label(path.kind.title());
                if let Some(u) = an.routing.paths.iter().find(|u| u.enclosure == ei && u.path == path.id) {
                    let full = if u.capacity > 0.0 { u.area / u.capacity * 100.0 } else { 0.0 };
                    let text = tr_fmt!("{} items, {} %", u.items, format!("{full:.0}"));
                    if full > 100.0 {
                        ui.colored_label(Color32::from_rgb(220, 60, 60), text);
                    } else {
                        ui.label(text);
                    }
                }
                if icons::button(ui, Icon::Delete, tr("Delete")).clicked() {
                    remove = Some(path.id);
                }
            });
            if selected {
                egui::Grid::new(("rpath", path.id)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label(tr("Name"));
                    ui.text_edit_singleline(&mut path.name);
                    ui.end_row();
                    ui.label(tr("For"));
                    kind_choice(ui, "pathkind", &mut path.kind);
                    ui.end_row();
                    ui.label(tr("Width"));
                    ui.add(egui::DragValue::new(&mut path.width).speed(1.0).range(5.0..=500.0).suffix(" mm"));
                    ui.end_row();
                    ui.label(tr("Height above the plate"));
                    let mut z = path.points.first().map(|q| q[2] - PLATE_Z).unwrap_or(0.0);
                    if ui.add(egui::DragValue::new(&mut z).speed(1.0).range(0.0..=2000.0).suffix(" mm")).changed() {
                        path.points.iter_mut().for_each(|q| q[2] = PLATE_Z + z);
                    }
                    ui.end_row();
                    ui.label(tr("Reserve"));
                    ui.add(egui::DragValue::new(&mut path.reserve).speed(1.0).range(0.0..=90.0).suffix(" %")).on_hover_text(tr("Share of the capacity kept free for later additions"));
                    ui.end_row();
                    ui.label(tr("Classes"));
                    ui.horizontal_wrapped(|ui| {
                        for c in Class::ALL {
                            let mut on = path.classes.is_empty() || path.classes.contains(&c);
                            if ui.checkbox(&mut on, c.title()).changed() {
                                let mut all: Vec<Class> = if path.classes.is_empty() { Class::ALL.to_vec() } else { path.classes.clone() };
                                all.retain(|x| *x != c);
                                if on {
                                    all.push(c);
                                    all.sort();
                                }
                                path.classes = if all.len() == Class::ALL.len() { Vec::new() } else { all };
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Length"));
                    ui.label(format!("{:.0} mm", path.length()));
                    ui.end_row();
                });
            }
        }
        ui.label(RichText::new(tr("Reservations")).strong());
        ui.horizontal(|ui| {
            ui.label(tr("New"));
            mode_choice(ui, "newreserve", &mut self.reserve_mode);
        });
        for res in &mut e.reservations {
            let selected = self.selected_object == Some(res.id);
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, &res.name).clicked() {
                    select = Some(if selected { None } else { Some(res.id) });
                }
                mode_choice(ui, ("reservemode", res.id), &mut res.mode);
                ui.label(format!("{:.0} × {:.0} mm", res.max[0] - res.min[0], res.max[1] - res.min[1]));
                if icons::button(ui, Icon::Delete, tr("Delete")).clicked() {
                    remove = Some(res.id);
                }
            });
            if selected {
                ui.horizontal(|ui| {
                    ui.label(tr("Name"));
                    ui.text_edit_singleline(&mut res.name);
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Depth from / to"));
                    ui.add(egui::DragValue::new(&mut res.min[2]).speed(1.0).range(0.0..=2000.0));
                    ui.add(egui::DragValue::new(&mut res.max[2]).speed(1.0).range(0.0..=2000.0).suffix(" mm"));
                });
            }
        }
        if let Some(id) = remove {
            e.paths.retain(|x| x.id != id);
            e.reservations.retain(|x| x.id != id);
            self.selected_object = None;
        }
        if let Some(s) = select {
            self.selected_object = s;
        }
    }

    /// Freezing routes for production and what changed against the frozen ones.
    fn production(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis) {
        ui.label(RichText::new(tr("Production")).strong());
        ui.label(tr_fmt!("{} routes frozen", prj.layout.frozen.len()));
        ui.horizontal(|ui| {
            let freeze = ui.add_enabled(!an.routes_pending, egui::Button::new(tr("Freeze all routes")));
            if freeze.on_hover_text(tr("The recalculation keeps frozen routes as long as they are still possible")).on_disabled_hover_text(tr("Cables and pipes are routed again in the background; until then the 3D view shows their earlier routes.")).clicked() {
                let n = routing3d::freeze(prj, &an.routing, &[]);
                self.status = tr_fmt!("{} routes frozen", n);
            }
            if !prj.layout.frozen.is_empty() && ui.button(tr("Release all")).clicked() {
                routing3d::release(prj, &[]);
            }
        });
        if !an.routing.changes.is_empty() {
            ui.colored_label(Color32::from_rgb(220, 120, 0), tr("Changed against the frozen routes:"));
            egui::Grid::new("rchanges").striped(true).num_columns(2).show(ui, |ui| {
                for c in &an.routing.changes {
                    ui.label(&c.label);
                    ui.label(match c.after {
                        Some(a) => tr_fmt!("{} mm, now {} mm", format!("{:.0}", c.before), format!("{a:.0}")),
                        None => tr_fmt!("{} mm, removed", format!("{:.0}", c.before)),
                    });
                    ui.end_row();
                }
            });
        }
    }
}

/// The installation zones flush-mounted lines run in, with a reset to DIN 18015-3.
fn installation_zones(ui: &mut egui::Ui, r: &mut routing3d::RoutingRules) {
    egui::CollapsingHeader::new(tr("Installation zones")).id_salt("rzones").show(ui, |ui| {
        let mut remove = None;
        egui::Grid::new("rzonesgrid").num_columns(5).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            for t in [tr("Name"), tr("Where"), tr("From – to"), "", ""] {
                ui.label(RichText::new(t).weak());
            }
            ui.end_row();
            for (k, z) in r.zones.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(&mut z.name).desired_width(50.0));
                egui::ComboBox::from_id_salt(("rzonekind", k)).selected_text(tr(z.kind.title())).width(190.0).show_ui(ui, |ui| {
                    for kind in ZoneKind::ALL {
                        ui.selectable_value(&mut z.kind, kind, tr(kind.title()));
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut z.from).speed(5.0).range(0.0..=10_000.0));
                    ui.add(egui::DragValue::new(&mut z.to).speed(5.0).range(0.0..=10_000.0).suffix(" mm"));
                });
                ui.checkbox(&mut z.on, "").on_hover_text(tr("Lines run in the zone"));
                if icons::button(ui, Icon::Delete, tr("Delete zone")).clicked() {
                    remove = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = remove {
            r.zones.remove(k);
        }
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::Plus, tr("New zone")).clicked() {
                r.zones.push(routing3d::InstallationZone { name: format!("Z{}", r.zones.len() + 1), kind: ZoneKind::AboveFloor, from: 600.0, to: 900.0, on: true });
            }
            if ui.button(tr("German zones (DIN 18015-3)")).on_hover_text(tr("Horizontal 15 - 45 cm above the floor and below the ceiling, the middle zone of kitchens 100 - 130 cm (off), vertical 10 - 30 cm beside doors, windows and corners, the ceiling and the floor 20 - 50 cm from the walls")).clicked() {
                r.zones = routing3d::german_zones();
            }
        });
    });
}

/// The routing rules of the project.
fn rules(ui: &mut egui::Ui, prj: &mut Project) {
    let r = &mut prj.layout.routing;
    egui::CollapsingHeader::new(tr("Routing rules")).id_salt("rrules").show(ui, |ui| {
        ui.checkbox(&mut r.wires_3d, tr("Route wires in 3D")).on_hover_text(tr("Wires take the obstacle-aware 3D routes too, instead of only the cable ducts on the plate"));
        egui::Grid::new("rrulesgrid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str, tip: &str| {
                ui.label(label);
                let resp = ui.add(egui::DragValue::new(v).speed(0.5).range(range).suffix(suffix));
                if !tip.is_empty() {
                    resp.on_hover_text(tip);
                }
                ui.end_row();
            };
            row(ui, tr("Grid"), &mut r.grid, 5.0..=200.0, " mm", tr("Edge length of the routing grid: smaller is finer and slower"));
            row(ui, tr("Clearance"), &mut r.clearance, 0.0..=100.0, " mm", tr("Distance from devices, rails and busbars"));
            row(ui, tr("Free air factor"), &mut r.free_factor, 1.0..=20.0, "", tr("A way through free air costs this many times the way in a duct or routing path"));
            row(ui, tr("Segregation"), &mut r.segregation, 0.0..=1000.0, " mm", tr("Distance of signals and fluids from power"));
            row(ui, tr("Segregation factor"), &mut r.segregation_factor, 1.0..=50.0, "", tr("Cost of running closer than the segregation distance"));
            row(ui, tr("Bend radius wires"), &mut r.bend_wire, 0.0..=30.0, " × d", tr("Of the outer diameter, as laid fixed"));
            row(ui, tr("Bend radius cables"), &mut r.bend_cable, 0.0..=30.0, " × d", tr("Of the outer diameter, as laid fixed"));
            row(ui, tr("Bend radius tubes"), &mut r.bend_tube, 0.0..=30.0, " × d", tr("Of the outer diameter; pipes are bent with elbows of one and a half diameters"));
            row(ui, tr("Cut length step wires"), &mut r.cut_wire, 1.0..=1000.0, " mm", "");
            row(ui, tr("Cut length step cables"), &mut r.cut_cable, 1.0..=1000.0, " mm", "");
            row(ui, tr("Cut length step tubes"), &mut r.cut_tube, 1.0..=1000.0, " mm", "");
            row(ui, tr("Cable end reserve"), &mut r.cable_reserve, 0.0..=5000.0, " mm", tr("Added to each cable end in the enclosure"));
            row(ui, tr("Shield pigtail"), &mut r.pigtail, 0.0..=500.0, " mm", tr("Length of a shield pigtail where there is no shield clamp"));
            ui.label(tr("Laying in rooms"));
            egui::ComboBox::from_id_salt("rruleslaying").selected_text(tr(r.laying.title())).show_ui(ui, |ui| {
                for l in Laying::ALL {
                    ui.selectable_value(&mut r.laying, l, tr(l.title()));
                }
            });
            ui.end_row();
            row(ui, tr("Slot depth"), &mut r.slot_depth, 5.0..=200.0, " mm", tr("Depth of the slots in the walls: flush-mounted lines lie this deep behind the surface"));
            row(ui, tr("Depth in the floor"), &mut r.floor_depth, 5.0..=300.0, " mm", tr("Depth of the lines in the floor below its surface (in the screed or on the raw floor)"));
        });
        installation_zones(ui, r);
        if ui.small_button(tr("Default rules")).clicked() {
            *r = Default::default();
        }
    });
}
