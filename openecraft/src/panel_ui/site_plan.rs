//! The site plan: the rooms of the buildings of the plant seen from above with their
//! shafts, stairs, roofs (roof windows, PV modules) and the links between the
//! buildings, the boundary of the plot (what stands outside it in the colour of errors)
//! and the reference point with grid north; shown in the Buildings pane and as the
//! floor plan of the site in the Layout space.

use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke, Vec2};
use openecraft_core::model::Project;
use openecraft_core::panel::{self, buildings as bld, plot, roofs, solar, stairs};
use openecraft_i18n::{tr, tr_fmt};

/// Corners in the floor plan.
type Outline = Vec<[f32; 2]>;

/// Glass of roof windows.
const GLASS: Color32 = Color32::from_rgba_premultiplied(60, 100, 140, 120);
/// PV modules.
pub(crate) const PV_MODULE: Color32 = Color32::from_rgba_premultiplied(20, 30, 70, 200);

/// What the site plan shows and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SitePlan {
    /// The building highlighted (index), all of them for `None`.
    pub building: Option<usize>,
    /// Its level highlighted (id), 0 for all levels.
    pub level: u64,
    /// Zoom on top of fitting the plant into the view, and the shift of the view in
    /// points.
    pub zoom: f32,
    pub pan: Vec2,
    /// The part of the plant fitted into the view (x, z, mm), kept while the plan is
    /// edited; `None`: all of it ([`extent`]).
    pub extent: Option<([f32; 2], [f32; 2])>,
}

impl Default for SitePlan {
    fn default() -> Self {
        SitePlan { building: None, level: 0, zoom: 1.0, pan: Vec2::ZERO, extent: None }
    }
}

/// How the site plan maps the plant (x, z, mm) onto the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub origin: Pos2,
    /// Points per mm.
    pub scale: f32,
}

impl Frame {
    pub fn at(&self, q: [f32; 2]) -> Pos2 {
        self.origin + vec2(q[0], q[1]) * self.scale
    }

    /// The point of the plant under `p`.
    pub fn plant(&self, p: Pos2) -> [f32; 2] {
        let d = (p - self.origin) / self.scale;
        [d.x, d.y]
    }
}

/// Room outlines in the plant (x, z), by room.
fn room_outlines(prj: &Project, poses: &[panel::SitePose]) -> Vec<(usize, Outline)> {
    let layout = &prj.layout;
    panel::rooms(layout)
        .into_iter()
        .map(|ri| {
            let pts = panel::outline(&layout.enclosures[ri])
                .into_iter()
                .map(|[x, z]| {
                    let q = poses[ri].apply([x, 0.0, z]);
                    [q[0], q[2]]
                })
                .collect();
            (ri, pts)
        })
        .collect()
}

/// The part of the plant the site plan shows: the rooms, links, shafts, roofs, the plot
/// boundary and the reference point; 40 m round the origin when there is nothing.
pub(crate) fn extent(prj: &Project) -> ([f32; 2], [f32; 2]) {
    let layout = &prj.layout;
    let poses = panel::site_poses(prj);
    let mut all: Vec<[f32; 2]> = room_outlines(prj, &poses).into_iter().flat_map(|r| r.1).collect();
    all.extend(layout.site_links.iter().filter_map(|l| bld::site_link_path(prj, &poses, l)).flatten().map(|q| [q[0], q[2]]));
    for b in &layout.buildings {
        all.extend(b.shafts.iter().flat_map(|s| bld::shaft_corners(b, s)));
        let bp = bld::building_pose(b);
        all.extend(b.roofs.iter().flat_map(roofs::slopes).flatten().map(|q| {
            let v = bp.apply([q[0], 0.0, q[1]]);
            [v[0], v[2]]
        }));
    }
    all.extend(layout.plot.outline.iter().copied());
    all.extend(layout.plot.reference.iter().map(|r| [r.x, r.z]));
    if all.is_empty() {
        return ([-20_000.0; 2], [20_000.0; 2]);
    }
    panel::bounds(&all)
}

/// The frame of the site plan in `size`: its extent fitted in, zoomed and moved.
pub(crate) fn frame(size: Rect, prj: &Project, plan: &SitePlan) -> Frame {
    let (lo, hi) = plan.extent.unwrap_or_else(|| extent(prj));
    let span = [(hi[0] - lo[0]).max(1000.0), (hi[1] - lo[1]).max(1000.0)];
    let inner = size.shrink(16.0);
    let scale = (inner.width() / span[0]).min(inner.height() / span[1]) * plan.zoom;
    Frame { origin: inner.center() + plan.pan - vec2((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0) * scale, scale }
}

/// The points the tools of the site plan snap to: the corners of the plot boundary
/// (but `skip`), of the rooms with and without their walls, of the shafts and the
/// reference point.
pub(crate) fn snap_points(prj: &Project, skip: Option<usize>) -> Vec<[f32; 2]> {
    let layout = &prj.layout;
    let poses = panel::site_poses(prj);
    let mut v: Vec<[f32; 2]> = layout.plot.outline.iter().enumerate().filter(|(k, _)| Some(*k) != skip).map(|(_, q)| *q).collect();
    v.extend(room_outlines(prj, &poses).into_iter().flat_map(|r| r.1));
    v.extend(plot::footprints(prj).into_iter().filter(|f| matches!(f.0, plot::Standing::Room(_))).flat_map(|f| f.1));
    v.extend(layout.buildings.iter().flat_map(|b| b.shafts.iter().flat_map(move |s| bld::shaft_corners(b, s))));
    v.extend(layout.plot.reference.iter().map(|r| [r.x, r.z]));
    v
}

/// The point of the plant a tool takes at `p`: a point to snap to within 8 points, or
/// the point under it rounded to 10 mm; and whether it snapped.
pub(crate) fn snap(points: &[[f32; 2]], frame: &Frame, p: Pos2) -> ([f32; 2], bool) {
    let near = points.iter().map(|&q| (q, frame.at(q).distance(p))).filter(|(_, d)| *d <= 8.0).min_by(|a, b| a.1.total_cmp(&b.1));
    match near {
        Some((q, _)) => (q, true),
        None => (frame.plant(p).map(|v| (v / 10.0).round() * 10.0), false),
    }
}

/// Metres with the sign and three decimals.
pub(crate) fn metres(mm: f64) -> String {
    format!("{:.3} m", mm / 1000.0)
}

/// The coordinates of a point of the plant written out: x and z of the plant, and with
/// a reference point easting and northing (or east and north of the point without
/// coordinates).
pub(crate) fn coordinates(prj: &Project, q: [f32; 2]) -> String {
    let plant = format!("x {}  z {}", metres(f64::from(q[0])), metres(f64::from(q[1])));
    match &prj.layout.plot.reference {
        Some(r) if r.easting != 0.0 || r.northing != 0.0 => {
            let g = plot::to_geo(r, [q[0], r.y, q[1]]);
            format!("{plant}   E {:.3}  N {:.3}", g[0], g[1])
        }
        Some(r) => {
            let d = plot::from_reference(r, q);
            tr_fmt!("{}   {} m east, {} m north of the reference point", plant, format!("{:.3}", d[0]), format!("{:.3}", d[1]))
        }
        None => plant,
    }
}

/// Whether `p` lies inside the polygon `pts`.
fn inside(pts: &[Pos2], p: Pos2) -> bool {
    let mut odd = false;
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

/// Draws the site plan into `size`; returns the highlighted room under `pointer` (the
/// one on the highest level).
#[allow(clippy::too_many_lines)]
pub(crate) fn paint(ui: &egui::Ui, size: Rect, prj: &Project, plan: &SitePlan, pointer: Option<Pos2>) -> Option<usize> {
    let layout = &prj.layout;
    let ours = |bi: usize| plan.building.is_none_or(|b| b == bi);
    let poses = panel::site_poses(prj);
    // Every room's outline in the plant (x, z).
    let rooms = room_outlines(prj, &poses);
    let outside = plot::outside(prj);
    let links: Vec<Outline> = layout.site_links.iter().filter_map(|l| bld::site_link_path(prj, &poses, l)).map(|p| p.into_iter().map(|q| [q[0], q[2]]).collect()).collect();
    // The roofs (their slopes and windows), shafts and stairs of the buildings in the plant.
    let to_plant = |b: &panel::Building, q: [f32; 2]| {
        let v = bld::building_pose(b).apply([q[0], 0.0, q[1]]);
        [v[0], v[2]]
    };
    let shown: Vec<(usize, &panel::Building)> = layout.buildings.iter().enumerate().filter(|(bi, _)| ours(*bi)).collect();
    let shafts: Vec<Outline> = shown.iter().flat_map(|(_, b)| b.shafts.iter().map(|s| bld::shaft_corners(b, s).iter().map(|&q| to_plant(b, q)).collect())).collect();
    let roof_slopes: Vec<(usize, Outline)> = layout.buildings.iter().enumerate().flat_map(|(bi, b)| b.roofs.iter().flat_map(move |r| roofs::slopes(r).into_iter().map(move |p| (bi, p.into_iter().map(|q| to_plant(b, q)).collect())))).collect();
    let roof_windows: Vec<Outline> = layout
        .buildings
        .iter()
        .flat_map(|b| b.roofs.iter().flat_map(move |r| r.windows.iter().filter_map(move |w| roofs::window_corners(r, w)).map(move |(c, _)| c.iter().map(|v| to_plant(b, [v[0], v[2]])).collect())))
        .collect();
    let pv_modules: Vec<Outline> = layout
        .buildings
        .iter()
        .flat_map(|b| b.roofs.iter().flat_map(move |r| r.solar.iter().flat_map(move |a| solar::module_outlines(r, a)).map(move |m| m.iter().map(|&q| to_plant(b, q)).collect())))
        .collect();
    let stair_lines: Vec<(Vec<Outline>, Outline)> = shown
        .iter()
        .flat_map(|(_, b)| {
            b.stairs
                .iter()
                .filter_map(|s| stairs::geometry(b, s))
                .filter(|g| plan.level == 0 || b.levels.iter().any(|l| l.id == plan.level && g.from <= l.elevation + 0.5 && g.to >= l.elevation - 0.5))
                .map(|g| (g.pieces.iter().map(|p| p.outline.iter().map(|&q| to_plant(b, q)).collect()).collect(), g.walk.iter().map(|&q| to_plant(b, q)).collect()))
        })
        .collect();
    let painter = ui.painter_at(size);
    let visuals = ui.visuals();
    painter.rect_filled(size, 2.0, visuals.extreme_bg_color);
    let line = visuals.text_color();
    let weak = visuals.weak_text_color();
    let accent = visuals.selection.stroke.color;
    let frame = frame(size, prj, plan);
    let scale = frame.scale;
    let at = |q: [f32; 2]| -> Pos2 { frame.at(q) };
    let plot = &layout.plot;
    if rooms.is_empty() && plot.is_default() {
        painter.text(size.center_top() + vec2(0.0, 24.0), egui::Align2::CENTER_CENTER, tr("No rooms in the plant yet"), egui::FontId::proportional(13.0), weak);
    }
    // The plot boundary below everything, with its corners, name and area.
    if plot.outline.len() >= 2 {
        let green = crate::style::ok();
        let pts: Vec<Pos2> = plot.outline.iter().map(|&q| at(q)).collect();
        painter.add(egui::Shape::closed_line(pts.clone(), Stroke::new(2.0, green)));
        for p in &pts {
            painter.circle_filled(*p, 2.5, green);
        }
        let top = pts.iter().copied().min_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))).unwrap_or_default();
        let area = format!("{:.1} m²", plot::area(&plot.outline));
        let label = if plot.name.is_empty() { tr_fmt!("Plot, {}", area) } else { format!("{}, {}", plot.name, area) };
        painter.text(top + vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(11.0), green);
    }
    let highlighted = |ri: usize| {
        let e = &layout.enclosures[ri];
        let in_building = match plan.building {
            Some(b) => bld::building_of(layout, ri) == Some(b),
            None => true,
        };
        in_building && (plan.level == 0 || e.level == plan.level)
    };
    // The room under the pointer: of those there, the one on the highest level.
    let floor = |ri: usize| bld::level_by_id(layout, layout.enclosures[ri].level).map_or(0.0, |l| l.elevation);
    let hovered = pointer.filter(|p| size.contains(*p)).and_then(|p| {
        rooms
            .iter()
            .filter(|(ri, pts)| highlighted(*ri) && inside(&pts.iter().map(|&q| at(q)).collect::<Vec<_>>(), p))
            .map(|r| r.0)
            .max_by(|a, b| floor(*a).total_cmp(&floor(*b)))
    });
    // Names already written: those of rooms above each other are written one below the
    // other.
    let mut labels: Vec<Pos2> = Vec::new();
    for (ri, pts) in &rooms {
        let e = &layout.enclosures[*ri];
        let on_level = highlighted(*ri);
        let pts: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
        let (mut stroke, fill) = if hovered == Some(*ri) {
            (Stroke::new(2.0, accent), visuals.selection.bg_fill.gamma_multiply(0.6))
        } else if on_level {
            (Stroke::new(1.5, line), visuals.selection.bg_fill.gamma_multiply(0.35))
        } else {
            (Stroke::new(1.0, weak), Color32::TRANSPARENT)
        };
        // Outside the plot boundary.
        if outside.contains(&plot::Standing::Room(*ri)) {
            stroke = Stroke::new(2.0, crate::style::error());
        }
        if fill != Color32::TRANSPARENT {
            // Rooms at right angles: their cells are filled one by one.
            for (a, c) in panel::floor_cells(e) {
                let corners = [[a[0], a[2]], [c[0], a[2]], [c[0], c[2]], [a[0], c[2]]].map(|[x, z]| {
                    let q = poses[*ri].apply([x, 0.0, z]);
                    at([q[0], q[2]])
                });
                painter.add(egui::Shape::convex_polygon(corners.to_vec(), fill, Stroke::NONE));
            }
        }
        painter.add(egui::Shape::closed_line(pts.clone(), stroke));
        if on_level {
            let mut c = pts.iter().fold(Pos2::ZERO, |s, p| s + p.to_vec2()) / pts.len() as f32;
            while labels.iter().any(|l| (l.x - c.x).abs() < 40.0 && (l.y - c.y).abs() < 13.0) {
                c.y += 13.0;
            }
            labels.push(c);
            painter.text(c, egui::Align2::CENTER_CENTER, e.name.as_str(), egui::FontId::proportional(11.0), line);
        }
    }
    // The roofs: their eaves, ridges and hips dashed, the roof windows.
    for (bi, slope) in &roof_slopes {
        let pts: Vec<Pos2> = slope.iter().map(|&q| at(q)).collect();
        let over = outside.iter().any(|s| matches!(s, plot::Standing::Roof(b, _) if b == bi));
        let color = if over { crate::style::error() } else if ours(*bi) { line.gamma_multiply(0.7) } else { weak };
        for k in 0..pts.len() {
            painter.extend(egui::Shape::dashed_line(&[pts[k], pts[(k + 1) % pts.len()]], Stroke::new(1.0, color), 5.0, 3.0));
        }
    }
    for w in &roof_windows {
        let pts: Vec<Pos2> = w.iter().map(|&q| at(q)).collect();
        painter.add(egui::Shape::convex_polygon(pts, GLASS, Stroke::new(1.0, line)));
    }
    for m in &pv_modules {
        let pts: Vec<Pos2> = m.iter().map(|&q| at(q)).collect();
        painter.add(egui::Shape::convex_polygon(pts, PV_MODULE, Stroke::new(0.5, weak)));
    }
    // The stairs reaching the level shown: their steps and the walking line with its arrow.
    for (pieces, walk) in &stair_lines {
        for p in pieces {
            painter.add(egui::Shape::closed_line(p.iter().map(|&q| at(q)).collect(), Stroke::new(1.0, line)));
        }
        let pts: Vec<Pos2> = walk.iter().map(|&q| at(q)).collect();
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, accent)));
        if let [.., a, b] = pts.as_slice() {
            let d = (*b - *a).normalized() * 7.0;
            painter.add(egui::Shape::convex_polygon(vec![*b, *b - d + d.rot90() * 0.5, *b - d - d.rot90() * 0.5], accent, Stroke::NONE));
        }
    }
    for s in &shafts {
        let pts: Vec<Pos2> = s.iter().map(|&q| at(q)).collect();
        painter.add(egui::Shape::closed_line(pts.clone(), Stroke::new(1.5, accent)));
        painter.line_segment([pts[0], pts[2]], Stroke::new(1.0, accent));
        painter.line_segment([pts[1], pts[3]], Stroke::new(1.0, accent));
    }
    for (path, l) in links.iter().zip(&layout.site_links) {
        let pts: Vec<Pos2> = path.iter().map(|&q| at(q)).collect();
        let color = crate::style::lock();
        for w in pts.windows(2) {
            painter.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(2.0, color), 6.0, 4.0));
        }
        if let Some(m) = pts.get(pts.len() / 2) {
            painter.text(*m + vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, format!("{} ({})", l.name, l.kind.title()), egui::FontId::proportional(11.0), color);
        }
    }
    for (bi, b) in layout.buildings.iter().enumerate() {
        let corners: Vec<[f32; 2]> = rooms.iter().filter(|r| bld::building_of(layout, r.0) == Some(bi)).flat_map(|r| r.1.iter().copied()).collect();
        let place = if corners.is_empty() { [b.site.x, b.site.z] } else { panel::bounds(&corners).0 };
        let color = if plan.building == Some(bi) || plan.building.is_none() { accent } else { weak };
        painter.text(at(place) - vec2(0.0, 4.0), egui::Align2::LEFT_BOTTOM, b.name.as_str(), egui::FontId::proportional(13.0), color);
    }
    // The cabinets standing outside the plot boundary.
    for f in plot::footprints(prj).into_iter().filter(|f| matches!(f.0, plot::Standing::Cabinet(_)) && outside.contains(&f.0)) {
        painter.add(egui::Shape::closed_line(f.1.iter().map(|&q| at(q)).collect(), Stroke::new(1.5, crate::style::error())));
    }
    // The reference point: a circle with a cross, its name and coordinates.
    if let Some(r) = &plot.reference {
        let c = at([r.x, r.z]);
        painter.circle_stroke(c, 6.0, Stroke::new(1.5, accent));
        painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], Stroke::new(1.0, accent));
        painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], Stroke::new(1.0, accent));
        let name = if r.name.is_empty() { tr("Reference point").to_string() } else { r.name.clone() };
        let text = if r.crs.is_empty() { name } else { format!("{name} ({})", r.crs) };
        painter.text(c + vec2(10.0, 8.0), egui::Align2::LEFT_TOP, text, egui::FontId::proportional(11.0), accent);
    }
    // Grid north (the plant's rear without a reference point) at the top right.
    {
        let n = plot::axes(&plot::reference(layout)).1;
        let dir = vec2(n[0] as f32, n[1] as f32);
        let c = pos2(size.right() - 24.0, size.top() + 30.0);
        let (tip, tail) = (c + dir * 13.0, c - dir * 10.0);
        let side = dir.rot90() * 5.0;
        painter.add(egui::Shape::convex_polygon(vec![tip, c - dir * 4.0 + side, c - dir * 4.0 - side], weak, Stroke::NONE));
        painter.line_segment([c, tail], Stroke::new(1.5, weak));
        painter.text(tip + dir * 8.0, egui::Align2::CENTER_CENTER, "N", egui::FontId::proportional(11.0), weak);
    }
    // Scale bar: the longest round length up to a third of the view.
    let metres = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    if let Some(m) = metres.iter().rev().find(|&&m| m * 1000.0 * scale <= size.width() / 3.0).filter(|&&m| m * 1000.0 * scale > 20.0) {
        let bar = m * 1000.0 * scale;
        let p0 = pos2(size.left() + 12.0, size.bottom() - 12.0);
        painter.line_segment([p0, p0 + vec2(bar, 0.0)], Stroke::new(1.5, weak));
        painter.text(p0 + vec2(bar / 2.0, -3.0), egui::Align2::CENTER_BOTTOM, format!("{m:.0} m"), egui::FontId::proportional(10.0), weak);
    }
    hovered
}

#[cfg(test)]
mod tests;
