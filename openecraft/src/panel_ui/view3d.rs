//! 3D view of the panel layout: the current enclosure or its whole line-up with walls,
//! door, mounting plate, rails, ducts, busbars, devices and the routed wiring; moving
//! the camera ([`super::nav3d`]) and picking. What it shows comes from the [`Scene3d`];
//! it is drawn by the graphics card ([`super::gpu3d`]) or projected here and painted far
//! to near.

use openecraft_i18n::{tr, tr_fmt};
use super::gpu3d::{Camera, Frame};
use super::nav3d::{self, Side};
use super::routing_ui::RouteRef;
use super::scene3d::{Scene3d, EQUIPMENT, ON_TOP};
use super::{wheel_delta, PanelUi, DOOR_OPEN_ANGLE, OUTER_WALLS};
use openecraft_core::geom::p;
use openecraft_core::model::Project;
use openecraft_core::panel::{self, ItemKind, SitePose};
use openecraft_core::parts::vec3::{self, dot, sub, Orbit, V3};
use openecraft_core::analysis::Analysis;
use openecraft_ui::icons::{self, Icon};
use egui::{vec2, Color32, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};

impl PanelUi {
    /// Opens or closes the door in the 3D view.
    pub fn door_button(&mut self, ui: &mut egui::Ui) {
        let text = if self.door_open { tr("Close door") } else { tr("Open door") };
        if ui.button(text).on_hover_text(tr("Swing the enclosure door open or shut (door devices and their wires move with it)")).clicked() {
            self.door_open = !self.door_open;
        }
    }

    /// 3D view of the current enclosure without the toolbar (wiring assistant).
    pub fn view_3d_current(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis) {
        if let Some(ei) = self.enclosure_index(prj) {
            self.view_3d(ui, prj, ei, an);
        }
    }

    /// Enclosures shown in the 3D view with their poses in view coordinates (x right,
    /// y up, z towards the door): all of them at their places in the room, a room with
    /// the cabinets standing in it, or the current one or its whole line-up from the
    /// left with the first one's bottom at y 0.
    /// The 3D view shows all of the plant: all enclosures, or the site is shown.
    pub(super) fn whole_site(&self) -> bool {
        self.show_site || self.site
    }

    /// The level whose rooms the 3D view shows side by side (its id): the level clicked
    /// in the navigator while the 3D view is shown.
    pub(super) fn level_3d(&self) -> Option<u64> {
        (self.site && self.site_plan.level != 0).then_some(self.site_plan.level)
    }

    pub(super) fn shown_3d(&self, prj: &Project, ei: usize) -> Vec<(usize, SitePose)> {
        if let Some(level) = self.level_3d() {
            // All rooms of the level at their places, with the cabinets standing in them.
            let poses = panel::site_poses(prj);
            return panel::buildings::rooms_on(&prj.layout, level).into_iter().flat_map(|r| std::iter::once(r).chain(panel::cabinets_in(&prj.layout, r))).map(|k| (k, poses[k])).collect();
        }
        if self.whole_site() {
            return panel::site_poses(prj).into_iter().enumerate().collect();
        }
        if prj.layout.enclosures[ei].is_room() {
            let poses = panel::site_poses(prj);
            let room = poses[ei];
            let local = |p: SitePose| SitePose { at: room.local(p.at), angle: p.angle - room.angle };
            // The whole building: its rooms with the cabinets in them.
            let rooms = match panel::buildings::building_of(&prj.layout, ei).filter(|_| self.show_building) {
                Some(bi) => {
                    let mut v = vec![ei];
                    v.extend(panel::buildings::building_rooms(&prj.layout, bi).into_iter().filter(|&r| r != ei));
                    v
                }
                None => vec![ei],
            };
            return rooms.into_iter().flat_map(|r| std::iter::once(r).chain(panel::cabinets_in(&prj.layout, r))).map(|k| (k, local(poses[k]))).collect();
        }
        let lu = if self.show_lineup { panel::lineup_of(&prj.layout, ei) } else { vec![ei] };
        let pos = panel::positions(&prj.layout);
        let first = lu.first().copied().unwrap_or(ei);
        let base = pos[first];
        let bottom = base[1] + prj.layout.enclosures[first].height;
        lu.into_iter()
            .map(|k| {
                let o = pos[k];
                (k, SitePose { at: [o[0] - base[0], bottom - o[1] - prj.layout.enclosures[k].height, o[2] - base[2]], angle: 0.0 })
            })
            .collect()
    }

    /// The pose of the room the 3D view is drawn in the coordinates of (the plant
    /// points `q` are at `local(q)`), `None` when it is drawn in the plant's.
    pub(super) fn view_frame(&self, prj: &Project, ei: usize) -> Option<SitePose> {
        (!self.whole_site() && prj.layout.enclosures.get(ei).is_some_and(|e| e.is_room())).then(|| panel::site_poses(prj)[ei])
    }

    /// The buildings whose floor slabs, stairs and roofs the 3D view shows: all of them
    /// with all enclosures, the room's own with its building.
    pub(super) fn buildings_3d(&self, prj: &Project, ei: usize) -> Vec<usize> {
        if let Some(level) = self.level_3d() {
            return panel::buildings::find_level(&prj.layout, level).map(|(bi, _)| bi).into_iter().collect();
        }
        if self.whole_site() {
            return (0..prj.layout.buildings.len()).collect();
        }
        panel::buildings::building_of(&prj.layout, ei).filter(|_| self.show_building).into_iter().collect()
    }

    pub(super) fn view_3d(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize, an: &Analysis) {
        let shown = self.shown_3d(prj, ei);
        let dark = ui.visuals().dark_mode;
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let target = if self.door_open { DOOR_OPEN_ANGLE } else { 0.0 };
        if (self.door_angle - target).abs() > 1e-3 {
            let step = dt * 2.5;
            self.door_angle += (target - self.door_angle).clamp(-step, step);
            ui.ctx().request_repaint();
        }
        // A camera set elsewhere (fit, focus) ends the glide to a view.
        if self.cam_goal.is_some_and(|(_, left)| self.cam != Some(left)) {
            self.cam_goal = None;
        }
        let fitted = default_cam(prj, &shown);
        let cam = self.cam.get_or_insert(fitted);
        let f = rect.height() * 1.3;
        let mut moved = false;
        // Orbit; pan with the right or middle button or Shift; zoom at the pointer
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) || (shift && resp.dragged_by(PointerButton::Primary)) {
            nav3d::pan(cam, resp.drag_delta(), f);
            moved = true;
        } else if resp.dragged_by(PointerButton::Primary) {
            nav3d::orbit(cam, resp.drag_delta());
            moved = true;
        }
        if resp.hovered() {
            let (wheel, zoom_delta, ptr) = ui.input(|i| (wheel_delta(i), i.zoom_delta(), i.pointer.hover_pos()));
            let factor = zoom_delta * (wheel * 0.0025).exp();
            if factor != 1.0 {
                nav3d::zoom_at(cam, factor, ptr.map_or(Vec2::ZERO, |q| q - rect.center()), f);
                moved = true;
            }
        }
        // Keyboard: moving through the view, Home fits it.
        let mut fit = false;
        if (self.keys_active || resp.hovered()) && !ui.ctx().egui_wants_keyboard_input() {
            let ((dir, fast), home) = ui.input(|i| (nav3d::held(i), i.key_pressed(Key::Home)));
            if dir.iter().any(|d| *d != 0.0) {
                nav3d::keys(cam, dir, fast, dt);
                moved = true;
                ui.ctx().request_repaint();
            }
            fit = home;
        }
        let mut goal = self.cam_goal.filter(|_| !moved).map(|g| g.0);
        if fit {
            goal = Some(Orbit { yaw: cam.yaw, pitch: cam.pitch, ..fitted });
        }
        if let Some(g) = goal {
            if nav3d::glide(cam, &g, dt) {
                goal = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        let cam = *cam;
        self.cam_goal = goal.map(|g| (g, cam));
        let (eye, right, up, fwd) = cam.basis();
        let camera = Camera { eye, right, up, fwd, f };
        let project_view = |q: V3| camera.project(rect.center(), q);
        let background = if dark { Color32::from_rgb(24, 26, 30) } else { Color32::from_rgb(214, 218, 224) };
        painter.rect_filled(rect, 0.0, background);

        let scene = self.scene_3d(ui.ctx(), prj, an, ei, &shown);
        let opacity = self.wall_opacity.clamp(0.0, 1.0);
        let hide_front = self.hide_front_walls;
        let picking = resp.clicked_by(PointerButton::Primary);
        let focusing = resp.double_clicked_by(PointerButton::Primary);
        let prims = if self.gpu_3d && !self.gpu.failed() {
            painter.add(self.gpu.shape(ui.ctx(), rect, scene.clone(), Frame { camera, opacity, hide_front, background }));
            // The faces under the pointer are only needed for a click.
            if picking || focusing { project(&scene, &camera, rect, opacity, hide_front) } else { Vec::new() }
        } else {
            let prims = project(&scene, &camera, rect, opacity, hide_front);
            paint(&painter, &prims);
            prims
        };
        // The front edges of the enclosures, the plot boundary and the reference point.
        for ((a, b), stroke) in scene.outlines.iter().map(|l| (l, Stroke::new(1.5, Color32::from_rgb(110, 115, 125)))).chain(scene.plot_lines.iter().map(|l| (l, Stroke::new(2.0, crate::style::ok())))) {
            if let (Some(pa), Some(pb)) = (project_view(*a), project_view(*b)) {
                painter.line_segment([pa.0, pb.0], stroke);
            }
        }
        for label in &scene.labels {
            if let Some((q, z)) = project_view(label.at) {
                let size = (label.size * 0.25 * camera.f / z).clamp(0.0, 18.0);
                if label.warning {
                    // Readable from anywhere: on a pale box, in the colour of errors.
                    let galley = painter.layout_no_wrap(label.tag.clone(), egui::FontId::proportional(size.max(11.0)), Color32::from_rgb(170, 30, 30));
                    let rect = egui::Rect::from_center_size(q, galley.size()).expand(4.0);
                    painter.rect(rect, 3.0, Color32::from_rgba_unmultiplied(255, 245, 240, 230), Stroke::new(1.0, Color32::from_rgb(200, 60, 60)), egui::StrokeKind::Outside);
                    painter.galley(rect.shrink(4.0).min, galley, Color32::from_rgb(170, 30, 30));
                } else if size >= 7.0 && dot(sub(eye, label.at), label.normal) > 0.0 {
                    painter.text(q, egui::Align2::CENTER_CENTER, &label.tag, egui::FontId::proportional(size), Color32::from_rgb(20, 30, 50));
                }
            }
        }
        for q in scene.branches.iter().filter_map(|q| project_view(*q)) {
            painter.circle(q.0, 3.5, Color32::WHITE, Stroke::new(1.5, Color32::from_rgb(60, 60, 60)));
        }
        // While the wires, cables and tubes are laid out on another thread: how far it is.
        if let Some(done) = self.layouts.progress() {
            let width = 300.0;
            let at = Rect::from_min_size(Pos2::new(rect.center().x - width / 2.0, rect.top() + 12.0), vec2(width, 60.0));
            let mut boxed = ui.new_child(egui::UiBuilder::new().max_rect(at));
            egui::Frame::popup(boxed.style()).show(&mut boxed, |ui| {
                ui.set_width(width - 20.0);
                ui.label(tr("Laying out the wires, cables and tubes…"));
                ui.add(egui::ProgressBar::new(done).show_percentage().animate(true));
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        // Standard views and fitting, from the direction looked in.
        let mut view = None;
        let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)).layout(egui::Layout::right_to_left(egui::Align::Min)));
        egui::Frame::popup(bar.style()).inner_margin(4.0).show(&mut bar, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if icons::button(ui, Icon::Fit, tr("Show everything (Home)")).clicked() {
                view = Some(Orbit { yaw: cam.yaw, pitch: cam.pitch, ..fitted });
            }
            for side in Side::ALL.into_iter().rev() {
                if ui.small_button(tr(side.title())).clicked() {
                    let (yaw, pitch) = side.angles();
                    view = Some(Orbit { yaw, pitch, ..fitted });
                }
            }
        });
        // Double-click: the camera turns to what is under the pointer and closes in.
        if let Some(ptr) = resp.interact_pointer_pos().filter(|_| focusing) {
            if let Some(z) = depth_under(&prims, ptr, opacity < 0.99) {
                view = Some(nav3d::look_at(&cam, camera.unproject(ptr - rect.center(), z)));
            }
        }
        if let Some(view) = view {
            self.cam_goal = Some((view, cam));
            ui.ctx().request_repaint();
        }
        // Picking: items of a neighbour in the line-up make it the current enclosure.
        if let Some(ptr) = resp.interact_pointer_pos().filter(|_| picking) {
            if self.pick_3d(prj, an, &scene, &prims, &project_view, ptr) {
                return;
            }
        }
        painter.text(
            rect.left_bottom() + vec2(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            tr("Drag: orbit · right/middle or Shift+drag: pan · wheel: zoom at the pointer · double-click: look there · W A S D / arrows, Q E: move · click: select"),
            egui::FontId::proportional(12.0),
            if dark { Color32::GRAY } else { Color32::DARK_GRAY },
        );
        if self.selected_wire.is_none() {
            let e = &prj.layout.enclosures[self.enclosure.min(prj.layout.enclosures.len() - 1)];
            // A room as in the list: width × depth × height; an enclosure width × height × depth.
            self.status = if e.is_room() {
                tr_fmt!("{}  room {} × {} × {}", e.name, format!("{:.0}", e.width), format!("{:.0}", e.depth), format!("{:.0}", e.height))
            } else {
                format!("{}  {:.0} × {:.0} × {:.0} mm", e.name, e.width, e.height, e.depth)
            };
        }
    }

    /// Selects what was clicked at `ptr`: a device, a wire, a cable or tube, another item.
    /// True when the status shows what was picked.
    fn pick_3d(&mut self, prj: &Project, an: &Analysis, scene: &Scene3d, prims: &[Prim], project_view: &dyn Fn(V3) -> Option<(Pos2, f32)>, ptr: Pos2) -> bool {
        // Faces are in drawing order: the last one under the pointer is visible.
        let hit = prims.iter().rev().find_map(|prim| match prim {
            Prim::Face { pts, z, n, item: Some(item), .. } if point_in_convex(ptr, &pts[..*n as usize]) => Some((*item, depth_at(ptr, &pts[..*n as usize], z))),
            _ => None,
        });
        let on_device = hit.and_then(|((k, id), _)| prj.layout.enclosures[k].item(id)).map(|i| i.kind == ItemKind::Device).unwrap_or(false);
        // Wires, cables and tubes in front of a device clicked are picked, those behind it not.
        let device_depth = hit.filter(|_| on_device).map(|(_, z)| z);
        let hit = hit.map(|(item, _)| item);
        let mut candidates: Vec<(usize, f32)> = Vec::new();
        let mut near_route: Option<(&str, f32)> = None;
        for (a, b, target) in &scene.picks {
            let (Some(pa), Some(pb)) = (project_view(*a), project_view(*b)) else { continue };
            let d = openecraft_core::geom::dist_to_segment(p(ptr.x, ptr.y), p(pa.0.x, pa.0.y), p(pb.0.x, pb.0.y));
            if d > 6.0 {
                continue;
            }
            let ab = pb.0 - pa.0;
            let t = if ab.length_sq() > 0.0 { ((ptr - pa.0).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            if device_depth.is_some_and(|dz| pa.1 + (pb.1 - pa.1) * t > dz - 1.0) {
                continue;
            }
            match target {
                RouteRef::Wire(wi) => match candidates.iter_mut().find(|c| c.0 == *wi) {
                    Some(c) => c.1 = c.1.min(d),
                    None => candidates.push((*wi, d)),
                },
                RouteRef::Item(key) if near_route.is_none_or(|(_, n)| d < n) => near_route = Some((key, d)),
                _ => {}
            }
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let candidates: Vec<usize> = candidates.into_iter().map(|c| c.0).collect();
        match (hit, near_route) {
            _ if !candidates.is_empty() => {
                self.pick_wire(an, &candidates);
                return true;
            }
            // Cables and tubes of the 3D routing.
            (_, Some((key, _))) => {
                self.select_route(an, key);
                return true;
            }
            (Some((k, id)), None) => {
                self.switch_enclosure(k);
                self.select_item(id);
            }
            (None, None) => self.deselect(),
        }
        false
    }
}

/// A projected face or line of one frame.
pub(super) enum Prim {
    /// `item`: enclosure and item id for picking; `z`: depth of the corners.
    Face { pts: [Pos2; 4], z: [f32; 4], n: u8, color: Color32, item: Option<(usize, u64)>, outline: bool },
    Line { a: Pos2, b: Pos2, color: Color32, width: f32 },
}

/// Width on the screen of a line of `diameter` mm at depth `z` (at least `least` px)
/// with the focal length `f`.
pub(super) fn line_px(diameter: f32, z: f32, least: f32, f: f32) -> f32 {
    if diameter > 0.0 { (diameter * f / z).clamp(least, 24.0) } else { least }
}

/// Cells per side of the [`ScreenGrid`].
const GRID: usize = 24;

/// The objects by the cells of a grid over the view that their rectangles on the
/// screen touch: the ones that may hide a wire there.
struct ScreenGrid {
    origin: Pos2,
    cell: Vec2,
    cells: Vec<Vec<usize>>,
    /// Objects with a corner behind the eye (no rectangle): candidates everywhere.
    everywhere: Vec<usize>,
}

impl ScreenGrid {
    fn new(view: Rect, rects: &[Option<Rect>]) -> ScreenGrid {
        let mut grid = ScreenGrid { origin: view.min, cell: view.size() / GRID as f32, cells: vec![Vec::new(); GRID * GRID], everywhere: Vec::new() };
        for (i, r) in rects.iter().enumerate() {
            match r {
                Some(r) => {
                    let ((x0, y0), (x1, y1)) = (grid.cell_of(r.min), grid.cell_of(r.max));
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            grid.cells[y * GRID + x].push(i);
                        }
                    }
                }
                None => grid.everywhere.push(i),
            }
        }
        grid
    }

    /// The cell of a point, the nearest one for a point outside the view.
    fn cell_of(&self, q: Pos2) -> (usize, usize) {
        let c = |v: f32, o: f32, s: f32| (((v - o) / s.max(1e-3)).floor().max(0.0) as usize).min(GRID - 1);
        (c(q.x, self.origin.x, self.cell.x), c(q.y, self.origin.y, self.cell.y))
    }

    /// The objects whose rectangles may touch `r` into `out`, each once: `seen` marks
    /// them with `stamp`, new for each call.
    fn candidates(&self, r: Rect, seen: &mut [u32], stamp: u32, out: &mut Vec<usize>) {
        out.clear();
        out.extend(self.everywhere.iter().copied());
        let ((x0, y0), (x1, y1)) = (self.cell_of(r.min), self.cell_of(r.max));
        for y in y0..=y1 {
            for x in x0..=x1 {
                for &i in &self.cells[y * GRID + x] {
                    if seen[i] != stamp {
                        seen[i] = stamp;
                        out.push(i);
                    }
                }
            }
        }
    }
}

/// The scene projected for the camera, in painting order: layer by layer, far to near,
/// the equipment object by object in the order of their boxes (walls and plate are
/// always behind the equipment when looking through the open door).
pub(super) fn project(scene: &Scene3d, camera: &Camera, rect: Rect, opacity: f32, hide_front: bool) -> Vec<Prim> {
    let eye = camera.eye;
    let project_view = |q: V3| camera.project(rect.center(), q);
    let order = vec3::paint_order(&scene.objects, eye);
    let mut rank = vec![0usize; scene.objects.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    // Sort key: the layer; the object and its part (none first; 0 wires it may hide,
    // 1 its faces, 2 the wires in a duct, 3 wires nothing hides, after the last object);
    // far to near.
    let key = |layer: u8, obj: Option<(usize, u8)>, z: f32| -> u64 {
        let obj = obj.map_or(0, |(i, part)| 1 + rank[i] as u64 * 4 + part as u64);
        (layer as u64) << 56 | obj << 32 | (!z.to_bits()) as u64
    };
    let mut prims: Vec<Prim> = Vec::new();
    let mut keys: Vec<(u64, u32)> = Vec::new();
    let mut push = |prims: &mut Vec<Prim>, k: u64, prim: Prim| {
        keys.push((k, prims.len() as u32));
        prims.push(prim);
    };
    for face in &scene.faces {
        let layer = match face.side {
            Some(s) => match scene.sides[s].layer(eye, hide_front) {
                Some(layer) => layer,
                None => continue,
            },
            None => face.layer,
        };
        if dot(face.normal, sub(eye, face.pts[0])) <= 0.0 {
            continue; // back face
        }
        let n = face.n as usize;
        let (mut pts, mut zs) = ([Pos2::ZERO; 4], [0.0; 4]);
        let mut seen = true;
        for (i, corner) in face.pts[..n].iter().enumerate() {
            match project_view(*corner) {
                Some((q, z)) => (pts[i], zs[i]) = (q, z),
                None => seen = false,
            }
        }
        let depth: f32 = zs[..n].iter().sum();
        if !seen {
            continue;
        }
        let glass = face.side.is_some_and(|s| scene.sides[s].glass);
        let color = if glass { face.color.gamma_multiply(super::scene3d::GLASS_ALPHA) } else if layer == OUTER_WALLS { face.color.gamma_multiply(opacity) } else { face.color };
        let k = key(layer, face.obj.map(|o| (o, 1)), depth / n as f32);
        push(&mut prims, k, Prim::Face { pts, z: zs, n: face.n, color, item: face.item, outline: face.outline });
    }
    for l in &scene.lines {
        if let (Some(pa), Some(pb)) = (project_view(l.a), project_view(l.b)) {
            let z = (pa.1 + pb.1) / 2.0;
            let width = line_px(l.diameter, z, l.least, camera.f);
            push(&mut prims, key(ON_TOP, None, z), Prim::Line { a: pa.0, b: pb.0, color: l.color, width });
        }
    }
    // Wires over the ducts that cover them on the screen (the one they run in and its
    // neighbours), under the devices. Screen rectangles of the objects; `None` when a
    // corner is behind the eye.
    let rects: Vec<Option<Rect>> = scene
        .objects
        .iter()
        .map(|(lo, hi)| {
            let corners = (0..8).map(|c| project_view([if c & 1 == 0 { lo[0] } else { hi[0] }, if c & 2 == 0 { lo[1] } else { hi[1] }, if c & 4 == 0 { lo[2] } else { hi[2] }]));
            corners.map(|p| p.map(|x| x.0)).collect::<Option<Vec<Pos2>>>().map(|pts| Rect::from_points(&pts))
        })
        .collect();
    let grid = ScreenGrid::new(rect, &rects);
    let mut seen = vec![0u32; scene.objects.len()];
    let mut near = Vec::new();
    for (stamp, w) in (1u32..).zip(&scene.wires) {
        let l = &w.line;
        let (Some(pa), Some(pb)) = (project_view(l.a), project_view(l.b)) else { continue };
        let piece = ([0, 1, 2].map(|k| l.a[k].min(l.b[k])), [0, 1, 2].map(|k| l.a[k].max(l.b[k])));
        let z = (pa.1 + pb.1) / 2.0;
        let width = line_px(l.diameter, z, l.least, camera.f);
        let on_screen = Rect::from_two_pos(pa.0, pb.0).expand(width);
        grid.candidates(on_screen, &mut seen, stamp, &mut near);
        let hiders = near
            .iter()
            .filter(|&&i| rects[i].is_none_or(|r| r.intersects(on_screen)) && vec3::may_hide(&scene.objects[i], &piece, eye))
            .map(|&i| (i, scene.ducts[i]))
            .chain(w.inside.map(|d| (d, true)));
        let obj = match vec3::wire_place(hiders, &rank) {
            Some((i, true)) => Some((i, 2)),
            Some((i, false)) => Some((i, 0)),
            None => order.last().map(|&i| (i, 3)),
        };
        push(&mut prims, key(EQUIPMENT, obj, z), Prim::Line { a: pa.0, b: pb.0, color: l.color, width });
        // A two-colour wire (green-yellow) with its second colour along the middle.
        if let Some(stripe) = l.stripe {
            push(&mut prims, key(EQUIPMENT, obj, z), Prim::Line { a: pa.0, b: pb.0, color: stripe, width: width * 0.45 });
        }
    }
    // Equal keys keep the order they were made in.
    keys.sort_unstable();
    let mut slots: Vec<Option<Prim>> = prims.into_iter().map(Some).collect();
    keys.into_iter().filter_map(|(_, i)| slots[i as usize].take()).collect()
}

/// Paints the projected primitives in their order.
fn paint(painter: &egui::Painter, prims: &[Prim]) {
    // Mesh triangles in a row go into one mesh: without feathering they show no seams.
    let mut mesh = egui::Mesh::default();
    let flush = |mesh: &mut egui::Mesh| {
        if !mesh.is_empty() {
            painter.add(egui::Shape::mesh(std::mem::take(mesh)));
        }
    };
    for prim in prims {
        match *prim {
            Prim::Face { pts, n, color, outline, .. } => {
                let pts = &pts[..n as usize];
                if outline {
                    flush(&mut mesh);
                    painter.add(openecraft_ui::faces::face(pts.to_vec(), color, Stroke::new(0.6, color.gamma_multiply(0.6))));
                } else {
                    let first = mesh.vertices.len() as u32;
                    for &q in pts {
                        mesh.colored_vertex(q, color);
                    }
                    for i in 1..pts.len() as u32 - 1 {
                        mesh.add_triangle(first, first + i, first + i + 1);
                    }
                }
            }
            Prim::Line { a, b, color, width } => {
                flush(&mut mesh);
                painter.line_segment([a, b], Stroke::new(width, color));
            }
        }
    }
    flush(&mut mesh);
}

/// Camera looking at the enclosures shown (see [`PanelUi::shown_3d`]).
pub(super) fn default_cam(prj: &Project, shown: &[(usize, SitePose)]) -> Orbit {
    let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for &(k, pose) in shown {
        let e = &prj.layout.enclosures[k];
        for c in 0..8 {
            let q = pose.apply([if c & 1 == 0 { 0.0 } else { e.width }, if c & 2 == 0 { 0.0 } else { e.height }, if c & 4 == 0 { 0.0 } else { e.depth }]);
            for i in 0..3 {
                min[i] = min[i].min(q[i]);
                max[i] = max[i].max(q[i]);
            }
        }
    }
    if shown.is_empty() {
        (min, max) = ([0.0; 3], [600.0, 800.0, 250.0]);
    }
    let target = [0, 1, 2].map(|i| (min[i] + max[i]) / 2.0);
    let size = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]);
    let (yaw, pitch) = Side::Iso.angles();
    Orbit { yaw, pitch, dist: size * 1.9, target }
}

/// Depth of what is seen at `q`: the nearest face, or with `items_first` (walls seen
/// through) the nearest face of an item if there is one.
fn depth_under(prims: &[Prim], q: Pos2, items_first: bool) -> Option<f32> {
    let faces = || {
        prims.iter().rev().filter_map(|prim| match prim {
            Prim::Face { pts, z, n, item, .. } if point_in_convex(q, &pts[..*n as usize]) => Some((item.is_some(), depth_at(q, &pts[..*n as usize], z))),
            _ => None,
        })
    };
    faces().find(|f| f.0 && items_first).or_else(|| faces().next()).map(|f| f.1)
}

/// Depth at `q` of a convex face with the corners `pts` at the depths `z`.
fn depth_at(q: Pos2, pts: &[Pos2], z: &[f32; 4]) -> f32 {
    for i in 1..pts.len().saturating_sub(1) {
        let (a, b, c) = (pts[0], pts[i], pts[i + 1]);
        let det = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if det.abs() < 1e-6 {
            continue;
        }
        let u = ((q.x - a.x) * (c.y - a.y) - (c.x - a.x) * (q.y - a.y)) / det;
        let v = ((b.x - a.x) * (q.y - a.y) - (q.x - a.x) * (b.y - a.y)) / det;
        if u >= -1e-3 && v >= -1e-3 && u + v <= 1.0 + 1e-3 {
            return z[0] + u * (z[i] - z[0]) + v * (z[i + 1] - z[0]);
        }
    }
    z[..pts.len()].iter().sum::<f32>() / pts.len().max(1) as f32
}

pub(super) fn point_in_convex(q: Pos2, pts: &[Pos2]) -> bool {
    let mut sign = 0.0f32;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let c = (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
        if c.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
