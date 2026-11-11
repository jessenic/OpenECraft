//! Floor plan of a room: walls drawn by clicking their corners, walls dragged to move
//! them, the cabinets and the parts standing in the room dragged to their places, the
//! pipe runs seen from above.

use super::routing_ui::PIPE_COLOR;
use super::{PanelUi, Tool, View};
use crate::editor::Palette;
use egui::{vec2, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Shape, Stroke};
use openecraft_core::geom::{p, P};
use openecraft_core::model::Project;
use openecraft_core::panel::{self, Face, InnerWall, ItemKind, Opening, PanelItem, SitePlace};
use openecraft_i18n::{tr, tr_fmt};

/// Grid the corners of drawn walls snap to, mm.
pub(super) const WALL_SNAP: f32 = 50.0;
/// Grid cabinets and dragged walls snap to, mm.
const MOVE_SNAP: f32 = 10.0;
/// Pick distance on screen, px.
const PICK: f32 = 8.0;

/// A gesture in the floor plan.
pub(super) enum PlanDrag {
    /// A wall moved along its normal: the outline before and where the drag started.
    Wall { wall: usize, outline: Vec<[f32; 2]>, start: P },
    /// A line-up moved: its first cabinet, its place before and where the drag started.
    Cabinet { first: usize, place: SitePlace, start: P },
    /// An inner wall moved across itself: how it was and where the drag started.
    Inner { wall: InnerWall, start: P },
    /// A part standing on the floor moved: its place before and where the drag started.
    Floor { id: u64, at: P, start: P },
}

/// A device or accessory standing on the floor of a room.
fn on_floor(it: &PanelItem) -> bool {
    it.face == Face::Bottom && matches!(it.kind, ItemKind::Device | ItemKind::Accessory)
}

/// The next corner of walls being drawn: on the grid, at a right angle to the last one.
pub(super) fn ortho_corner(last: Option<P>, q: P) -> P {
    let s = |v: f32| (v / WALL_SNAP).round() * WALL_SNAP;
    let q = p(s(q.x), s(q.y));
    match last {
        Some(l) if (q.x - l.x).abs() >= (q.y - l.y).abs() => p(q.x, l.y),
        Some(l) => p(l.x, q.y),
        None => q,
    }
}

/// The outline of drawn corners, closed at a right angle back to the first corner.
pub(super) fn closed_outline(points: &[P]) -> Result<Vec<[f32; 2]>, String> {
    let pts: Vec<[f32; 2]> = points.iter().map(|q| [q.x, q.y]).collect();
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else { return panel::normalize_outline(&pts) };
    if (first[0] - last[0]).abs() < 0.01 || (first[1] - last[1]).abs() < 0.01 {
        return panel::normalize_outline(&pts);
    }
    // Two ways round the missing corner: the first that gives a valid outline.
    let a = [pts.clone(), vec![[last[0], first[1]]]].concat();
    let b = [pts, vec![[first[0], last[1]]]].concat();
    panel::normalize_outline(&a).or_else(|_| panel::normalize_outline(&b))
}

/// An end of an inner wall: on the grid, at a right angle to the last one, and onto a
/// wall of the room when it is near one (an inner wall starts and ends at a wall).
pub(super) fn inner_corner(walls: &[panel::Wall], last: Option<P>, q: P) -> P {
    let mut c = ortho_corner(last, q);
    for w in walls {
        let n = w.normal;
        // Distance in front of the wall's inside surface, along its normal.
        let d = (c.x - w.a[0]) * n[0] + (c.y - w.a[1]) * n[1];
        let along = (c.x - w.a[0]) * w.dir()[0] + (c.y - w.a[1]) * w.dir()[1];
        if d.abs() <= WALL_SNAP && (-0.5..=w.length() + 0.5).contains(&along) {
            // Only across the wall, so that the right angle to the last end stays.
            let moved = p(c.x - n[0] * d, c.y - n[1] * d);
            if last.is_none_or(|l| (l.x - moved.x).abs() < 0.01 || (l.y - moved.y).abs() < 0.01) {
                c = moved;
            }
        }
    }
    c
}

/// Another room seen in the floor plan of a room: its index, its outline and floor
/// cells and the footprints of its cabinets, in plan coordinates of the room shown.
pub(super) struct Neighbour {
    pub room: usize,
    pub outline: Vec<[f32; 2]>,
    pub cells: Vec<[[f32; 2]; 4]>,
    pub cabinets: Vec<[[f32; 2]; 4]>,
}

/// The other rooms of the layout as seen in the floor plan of room `ei`, at their
/// places in the plant: those beside it, not on the levels above or below.
pub(super) fn neighbours(prj: &Project, ei: usize) -> Vec<Neighbour> {
    let poses = panel::site_poses(prj);
    let here = poses[ei];
    let plan = |k: usize, q: [f32; 2]| {
        let l = here.local(poses[k].apply([q[0], 0.0, q[1]]));
        [l[0], l[2]]
    };
    let height = |k: usize| prj.layout.enclosures[k].height;
    let beside = |k: usize| poses[k].at[1] < here.at[1] + height(ei) - 0.5 && here.at[1] < poses[k].at[1] + height(k) - 0.5;
    panel::rooms(&prj.layout)
        .into_iter()
        .filter(|&k| k != ei && beside(k))
        .map(|k| {
            let e = &prj.layout.enclosures[k];
            let cells = panel::floor_cells(e).into_iter().map(|(a, b)| [[a[0], a[2]], [b[0], a[2]], [b[0], b[2]], [a[0], b[2]]].map(|q| plan(k, q))).collect();
            let cabinets = panel::cabinets_in(&prj.layout, k).into_iter().map(|c| panel::footprint(prj, k, c).map(|q| plan(k, q))).collect();
            Neighbour { room: k, outline: panel::outline(e).into_iter().map(|q| plan(k, q)).collect(), cells, cabinets }
        })
        .collect()
}

/// The ways through the walls into and out of room `ei` from the plan of room `ei`:
/// both ends in its plan coordinates.
pub(super) fn links_in_plan(prj: &Project, ei: usize) -> Vec<([f32; 2], [f32; 2])> {
    let poses = panel::site_poses(prj);
    let here = poses[ei];
    let plan = |k: usize, q: [f32; 3]| {
        let l = here.local(poses[k].apply([q[0], 0.0, q[2]]));
        [l[0], l[2]]
    };
    panel::wall_links(prj).into_iter().filter(|l| (l.from == ei || l.to == ei) && l.via != panel::Via::Shaft).map(|l| (plan(l.from, l.a), plan(l.to, l.b))).collect()
}

/// The shafts of the building of room `ei` that pass its level: name and corners in its
/// plan coordinates.
pub(super) fn shafts_in_plan(prj: &Project, ei: usize) -> Vec<(String, [[f32; 2]; 4])> {
    let layout = &prj.layout;
    let Some((bi, li)) = panel::buildings::find_level(layout, layout.enclosures[ei].level) else { return Vec::new() };
    let b = &layout.buildings[bi];
    let here = panel::site_poses(prj)[ei];
    let at = |id: u64| b.levels.iter().position(|l| l.id == id);
    b.shafts
        .iter()
        .filter(|s| at(s.bottom).zip(at(s.top)).is_some_and(|(lo, hi)| lo.min(hi) <= li && li <= lo.max(hi)))
        .map(|s| {
            let corners = panel::buildings::shaft_corners(b, s).map(|[x, z]| {
                let l = here.local([x, 0.0, z]);
                [l[0], l[2]]
            });
            (s.name.clone(), corners)
        })
        .collect()
}

/// A stair reaching the level of a room: its steps and walking line in the room's plan
/// coordinates, and whether it comes up from below (drawn dashed: the hole in the floor).
pub(super) struct PlanStair {
    pub steps: Vec<Vec<[f32; 2]>>,
    pub walk: Vec<[f32; 2]>,
    pub from_below: bool,
}

/// The stairs of the building of room `ei` that start on, arrive at or pass its level.
pub(super) fn stairs_in_plan(prj: &Project, ei: usize) -> Vec<PlanStair> {
    let layout = &prj.layout;
    let e = &layout.enclosures[ei];
    let Some(level) = panel::buildings::level_by_id(layout, e.level) else { return Vec::new() };
    let here = panel::site_poses(prj)[ei];
    panel::stairs::stairs_at_level(prj, e.level)
        .into_iter()
        .map(|(bi, _, g)| {
            let bp = panel::buildings::building_pose(&layout.buildings[bi]);
            let local = |q: &[f32; 2]| {
                let v = here.local(bp.apply([q[0], 0.0, q[1]]));
                [v[0], v[2]]
            };
            PlanStair { steps: g.pieces.iter().map(|p| p.outline.iter().map(local).collect()).collect(), walk: g.walk.iter().map(local).collect(), from_below: g.from < level.elevation - 0.5 }
        })
        .collect()
}

fn snap(v: f32) -> f32 {
    (v / MOVE_SNAP).round() * MOVE_SNAP
}

fn segment_distance(q: P, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let len2 = (dx * dx + dz * dz).max(1e-6);
    let t = (((q.x - a[0]) * dx + (q.y - a[1]) * dz) / len2).clamp(0.0, 1.0);
    let (x, z) = (a[0] + t * dx - q.x, a[1] + t * dz - q.y);
    (x * x + z * z).sqrt()
}

impl PanelUi {
    /// Finishes the walls being drawn: the room gets them as its outline.
    pub(super) fn finish_walls(&mut self, prj: &mut Project, ei: usize) {
        match closed_outline(&self.wall_points).and_then(|o| panel::set_outline(&mut prj.layout.enclosures[ei], &o)) {
            Ok(()) => {
                let n = panel::walls(&prj.layout.enclosures[ei]).len();
                self.status = tr_fmt!("Floor plan with {} walls", n);
                self.wall_points.clear();
                self.set_tool(Tool::Select);
            }
            Err(e) => self.status = e,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn plan_view(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize) {
        let pal = Palette::new(ui.visuals().dark_mode);
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        crate::editor::take_keys(ui, &resp);
        let rect = resp.rect;
        self.keep_centre(rect);
        let outline = panel::outline(&prj.layout.enclosures[ei]);
        let (lo, hi) = panel::bounds(&outline);
        let margin = 600.0;
        if self.fit_pending && rect.width() > 100.0 && rect.height() > 100.0 {
            let (w, h) = (hi[0] - lo[0] + 2.0 * margin, hi[1] - lo[1] + 2.0 * margin);
            self.zoom = ((rect.width() - 40.0) / w).min((rect.height() - 40.0) / h).max(0.002);
            self.offset = vec2((rect.width() - w * self.zoom) / 2.0 - (lo[0] - margin) * self.zoom, (rect.height() - h * self.zoom) / 2.0 - (lo[1] - margin) * self.zoom);
            self.fit_pending = false;
        }
        let gesture = openecraft_ui::nav::read(ui, &resp);
        self.offset += gesture.pan;
        if let (true, Some(at)) = (gesture.zooms(), gesture.at) {
            let before = self.s2w(rect, at);
            self.zoom = (self.zoom * gesture.zoom).clamp(0.002, 5.0);
            self.offset = at - rect.min - vec2(before.x, before.y) * self.zoom;
        }
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
            self.offset += resp.drag_delta();
        }
        // Fingers navigating end a drag of the first one where it is.
        if gesture.touch {
            self.plan_drag = None;
        }
        let ptr = ui.input(|i| i.pointer.hover_pos()).filter(|q| rect.contains(*q));
        let plan = ptr.map(|s| self.s2w(rect, s));
        let pick = PICK / self.zoom;

        // Keyboard
        if self.keys_active && !ui.ctx().egui_wants_keyboard_input() {
            let (esc, back, enter, r) = ui.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Backspace), i.key_pressed(Key::Enter), i.key_pressed(Key::R) && i.modifiers.is_none()));
            if esc {
                if self.wall_points.is_empty() {
                    self.set_tool(Tool::Select);
                    self.plan_cabinet = None;
                    self.plan_inner = None;
                } else {
                    self.wall_points.clear();
                }
            }
            let del = ui.input(|i| i.key_pressed(Key::Delete));
            if del && self.plan_inner.is_none() {
                let selected = std::mem::take(&mut self.selection);
                prj.layout.enclosures[ei].items.retain(|i| !(on_floor(i) && selected.contains(&i.id)));
            }
            if let (true, Some(id)) = (del, self.plan_inner.take()) {
                panel::remove_inner_wall(&mut prj.layout.enclosures[ei], id);
                if matches!(self.face, Face::Inner(w, _) if w == id) {
                    self.face = Face::Wall(0);
                }
            }
            if back {
                self.wall_points.pop();
            }
            if enter && self.wall_points.len() >= 3 && self.tool == Tool::Walls {
                self.finish_walls(prj, ei);
            }
            if enter && self.tool == Tool::InnerWalls {
                self.wall_points.clear();
            }
            if let (true, Some(first)) = (r, self.plan_cabinet) {
                // About the centre of its footprint: it stays where it stood.
                if let Some(place) = panel::turned_place(prj, first, 90.0) {
                    prj.layout.enclosures[first].site = Some(place);
                }
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            if self.wall_points.is_empty() {
                self.set_tool(Tool::Select);
            } else {
                self.wall_points.pop();
            }
        }

        // What is under the pointer: a cabinet (its line-up's first one) or a wall.
        let cabinets = panel::cabinets_in(&prj.layout, ei);
        let footprints: Vec<(usize, [[f32; 2]; 4])> = cabinets.iter().map(|&c| (c, panel::footprint(prj, ei, c))).collect();
        let walls = panel::walls(&prj.layout.enclosures[ei]);
        let first_of = |prj: &Project, c: usize| panel::lineup_of(&prj.layout, c).first().copied().unwrap_or(c);
        let cabinet_at = |q: P| footprints.iter().find(|(_, fp)| panel::inside_outline(fp, [q.x, q.y])).map(|(c, _)| *c);
        let wall_at = |q: P| walls.iter().position(|w| segment_distance(q, w.a, w.b) <= pick);
        let inner_walls = prj.layout.enclosures[ei].inner_walls.clone();
        // An inner wall and the side of it the point is on.
        let inner_at = |q: P| {
            inner_walls.iter().find(|w| segment_distance(q, w.a, w.b) <= w.thickness / 2.0 + pick).map(|w| {
                let n = panel::inner_sides(w)[0].normal;
                (w.id, u8::from((q.x - w.a[0]) * n[0] + (q.y - w.a[1]) * n[1] < 0.0))
            })
        };
        // The wall surface at a point: an outer wall or a side of an inner wall.
        let face_at = |q: P| inner_at(q).map(|(id, s)| Face::Inner(id, s)).or_else(|| wall_at(q).map(|n| Face::Wall(n as u16)));
        // Parts standing on the floor come before what they stand on.
        let floor: Vec<(u64, [[f32; 2]; 4])> = prj.layout.enclosures[ei].items.iter().filter(|i| on_floor(i)).map(|i| (i.id, panel::floor_footprint(&prj.layout.enclosures[ei], i))).collect();
        let floor_at = |q: P| floor.iter().find(|(_, fp)| panel::inside_outline(fp, [q.x, q.y])).map(|(id, _)| *id);
        let hit_floor = plan.and_then(floor_at);
        let hit_cabinet = plan.filter(|_| hit_floor.is_none()).and_then(cabinet_at);
        let hit_wall = plan.filter(|_| hit_cabinet.is_none() && hit_floor.is_none()).and_then(wall_at);
        let hit_inner = plan.filter(|_| hit_cabinet.is_none() && hit_floor.is_none()).and_then(inner_at).map(|(id, _)| id);
        // Another room, outside this one.
        let others = neighbours(prj, ei);
        let hit_room = plan.filter(|q| hit_wall.is_none() && !panel::inside_outline(&outline, [q.x, q.y])).and_then(|q| others.iter().find(|n| panel::inside_outline(&n.outline, [q.x, q.y]))).map(|n| n.room);
        // A drag starts where the button went down and follows the pointer outside the view too.
        let pressed = ui.input(|i| i.pointer.press_origin()).map(|s| self.s2w(rect, s));
        let dragged_to = ui.input(|i| i.pointer.interact_pos()).map(|s| self.s2w(rect, s));

        // Mouse (not while fingers navigate)
        match self.tool.clone() {
            _ if gesture.touch => {}
            Tool::Walls => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plan) {
                    let corner = ortho_corner(self.wall_points.last().copied(), q);
                    let closes = self.wall_points.len() >= 3 && self.wall_points.first().is_some_and(|f| (f.x - corner.x).abs() + (f.y - corner.y).abs() <= pick.max(WALL_SNAP));
                    if closes || resp.double_clicked() {
                        self.finish_walls(prj, ei);
                    } else if self.wall_points.last().is_none_or(|l| !l.same(corner)) {
                        self.wall_points.push(corner);
                    }
                }
            }
            Tool::InnerWalls => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plan) {
                    // A chain of inner walls: every click ends one and starts the next.
                    let corner = inner_corner(&walls, self.wall_points.last().copied(), q);
                    match self.wall_points.last().copied() {
                        _ if resp.double_clicked() => self.wall_points.clear(),
                        Some(last) if !last.same(corner) => match panel::add_inner_wall(prj, ei, [last.x, last.y], [corner.x, corner.y], panel::INNER_WALL) {
                            Ok(id) => {
                                // The properties show the wall just drawn.
                                self.wall_points = vec![corner];
                                self.plan_inner = Some(id);
                                self.selection.clear();
                            }
                            Err(e) => self.status = e,
                        },
                        Some(_) => {}
                        None => self.wall_points.push(corner),
                    }
                }
            }
            Tool::Opening(opening) => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plan) {
                    // In the wall clicked (the side of an inner wall clicked), centred where it was clicked.
                    let e = &prj.layout.enclosures[ei];
                    match face_at(q).and_then(|f| panel::side(e, f).map(|w| (f, w))) {
                        Some((face, w)) => {
                            let (s, r) = (w.start(), w.right());
                            let along = (q.x - s[0]) * r[0] + (q.y - s[1]) * r[1];
                            let (width, _) = opening.size();
                            match panel::add_opening(prj, ei, opening, face, Some(along - width / 2.0), None, None) {
                                Ok(id) => {
                                    self.face = face;
                                    self.select_item(id);
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        None => self.status = tr("Click on a wall").to_string(),
                    }
                }
            }
            Tool::Select => {
                if resp.drag_started_by(PointerButton::Primary) {
                    if let Some(q) = pressed {
                        let standing = floor_at(q).and_then(|id| prj.layout.enclosures[ei].item(id).map(|it| (id, p(it.x, it.y))));
                        if let Some((id, at)) = standing {
                            self.select_item(id);
                            self.plan_drag = Some(PlanDrag::Floor { id, at, start: q });
                        } else if let Some(c) = cabinet_at(q) {
                            let first = first_of(prj, c);
                            if let Some(place) = panel::room_place(prj, first) {
                                self.plan_cabinet = Some(first);
                                self.plan_drag = Some(PlanDrag::Cabinet { first, place, start: q });
                            }
                        } else if let Some((id, _)) = inner_at(q) {
                            if let Some(wall) = inner_walls.iter().find(|w| w.id == id) {
                                self.plan_inner = Some(id);
                                self.plan_drag = Some(PlanDrag::Inner { wall: wall.clone(), start: q });
                            }
                        } else if let Some(wall) = wall_at(q) {
                            self.plan_drag = Some(PlanDrag::Wall { wall, outline: outline.clone(), start: q });
                        }
                    }
                }
                if let (Some(drag), Some(q)) = (&self.plan_drag, dragged_to) {
                    match drag {
                        PlanDrag::Wall { wall, outline, start } => {
                            let w = &panel::walls(&prj.layout.enclosures[ei])[*wall];
                            let offset = snap(-((q.x - start.x) * w.normal[0] + (q.y - start.y) * w.normal[1]));
                            match panel::moved_wall(outline, *wall, offset) {
                                Ok(o) => {
                                    let _ = panel::set_outline(&mut prj.layout.enclosures[ei], &o);
                                    self.status = tr_fmt!("Wall {}: moved by {} mm", wall + 1, format!("{offset:.0}"));
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        PlanDrag::Floor { id, at, start } => {
                            if let Some(it) = prj.layout.enclosures[ei].items.iter_mut().find(|i| i.id == *id) {
                                (it.x, it.y) = (snap(at.x + q.x - start.x), snap(at.y + q.y - start.y));
                            }
                        }
                        PlanDrag::Cabinet { first, place, start } => {
                            let moved = SitePlace { x: snap(place.x + q.x - start.x), z: snap(place.z + q.y - start.y), angle: place.angle };
                            prj.layout.enclosures[*first].site = Some(moved);
                        }
                        PlanDrag::Inner { wall, start } => {
                            // From where it was, across itself.
                            let n = panel::inner_sides(wall)[0].normal;
                            let offset = snap((q.x - start.x) * n[0] + (q.y - start.y) * n[1]);
                            let e = &mut prj.layout.enclosures[ei];
                            if let Some(w) = e.inner_walls.iter_mut().find(|w| w.id == wall.id) {
                                *w = wall.clone();
                            }
                            match panel::moved_inner_wall(e, wall.id, offset, None, None) {
                                Ok(moved) => {
                                    if let Some(w) = e.inner_walls.iter_mut().find(|w| w.id == wall.id) {
                                        *w = moved;
                                    }
                                }
                                Err(msg) => self.status = msg,
                            }
                        }
                    }
                }
                if resp.drag_stopped() {
                    self.plan_drag = None;
                }
                if resp.clicked_by(PointerButton::Primary) {
                    self.plan_cabinet = hit_cabinet.map(|c| first_of(prj, c));
                    self.plan_inner = hit_inner;
                    match hit_floor {
                        Some(id) => self.select_item(id),
                        None => self.selection.clear(),
                    }
                }
                if resp.double_clicked() {
                    if let Some(id) = hit_floor {
                        // The floor with the part selected.
                        self.front_face(Face::Bottom, false);
                        self.select_item(id);
                        return;
                    } else if let Some(c) = hit_cabinet {
                        // Open the cabinet.
                        self.enclosure = c;
                        self.view = View::Front;
                        self.selection.clear();
                        self.fit_pending = true;
                        self.cam = None;
                        return;
                    } else if let Some(face) = plan.and_then(face_at) {
                        self.front_face(face, false);
                        return;
                    } else if let Some(k) = hit_room {
                        // Open the other room in its floor plan.
                        self.enclosure = k;
                        self.face = Face::Wall(0);
                        self.selection.clear();
                        self.plan_cabinet = None;
                        self.plan_inner = None;
                        self.fit_pending = true;
                        self.cam = None;
                        return;
                    }
                }
                if let Some(w) = hit_inner.and_then(|id| inner_walls.iter().find(|w| w.id == id)) {
                    ui.ctx().set_cursor_icon(if (w.a[0] - w.b[0]).abs() < 0.5 { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
                } else if hit_wall.is_some() || matches!(self.plan_drag, Some(PlanDrag::Wall { .. })) {
                    let wall = match &self.plan_drag {
                        Some(PlanDrag::Wall { wall, .. }) => Some(*wall),
                        _ => hit_wall,
                    };
                    if let Some(w) = wall.and_then(|w| walls.get(w)) {
                        ui.ctx().set_cursor_icon(if w.normal[0].abs() > 0.5 { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
                    }
                } else if hit_cabinet.is_some() || hit_floor.is_some() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
            }
            _ => {}
        }

        // Draw
        let e = &prj.layout.enclosures[ei];
        let (off, zoom) = (self.offset, self.zoom);
        let to_screen = |x: f32, z: f32| rect.min + off + vec2(x, z) * zoom;
        painter.rect_filled(rect, 0.0, pal.canvas);
        // Grid of metres (or ten metres when zoomed out).
        let step = if zoom * 1000.0 < 12.0 { 10000.0 } else { 1000.0 };
        let (a, b) = (self.s2w(rect, rect.min), self.s2w(rect, rect.max));
        let grid = Stroke::new(1.0, pal.grid.gamma_multiply(0.35));
        let mut x = (a.x / step).floor() * step;
        while x <= b.x {
            painter.line_segment([to_screen(x, a.y), to_screen(x, b.y)], grid);
            x += step;
        }
        let mut z = (a.y / step).floor() * step;
        while z <= b.y {
            painter.line_segment([to_screen(a.x, z), to_screen(b.x, z)], grid);
            z += step;
        }
        // The other rooms around it, faint, with their names and cabinets.
        for n in &others {
            let hovered = hit_room == Some(n.room);
            let fill = pal.paper.gamma_multiply(if hovered { 0.75 } else { 0.45 });
            for cell in &n.cells {
                painter.add(Shape::convex_polygon(cell.iter().map(|q| to_screen(q[0], q[1])).collect(), fill, Stroke::NONE));
            }
            let mut ring: Vec<Pos2> = n.outline.iter().map(|q| to_screen(q[0], q[1])).collect();
            ring.extend(ring.first().copied());
            painter.add(Shape::line(ring, Stroke::new(if hovered { 2.0 } else { 1.5 }, pal.line.gamma_multiply(0.5))));
            for fp in &n.cabinets {
                painter.add(Shape::convex_polygon(fp.iter().map(|q| to_screen(q[0], q[1])).collect(), pal.plate.gamma_multiply(0.5), Stroke::new(1.0, pal.frame.gamma_multiply(0.5))));
            }
            let (lo, hi) = panel::bounds(&n.outline);
            let centre = to_screen((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
            painter.text(centre, egui::Align2::CENTER_CENTER, &prj.layout.enclosures[n.room].name, egui::FontId::proportional(14.0), pal.text.gamma_multiply(0.6));
        }
        if let Some(k) = hit_room.filter(|_| self.plan_drag.is_none()) {
            resp.clone().on_hover_text_at_pointer(tr_fmt!("{}: a double-click opens its floor plan", prj.layout.enclosures[k].name));
        }
        // Floor, walls and their numbers and lengths.
        for (a, b) in panel::floor_cells(e) {
            painter.rect_filled(Rect::from_two_pos(to_screen(a[0], a[2]), to_screen(b[0], b[2])), 0.0, pal.paper);
        }
        let dragged_wall = match &self.plan_drag {
            Some(PlanDrag::Wall { wall, .. }) => Some(*wall),
            _ => None,
        };
        let walls = panel::walls(e);
        for (i, w) in walls.iter().enumerate() {
            let (a, b) = panel::wall_box(e, Face::Wall(i as u16));
            let picking = matches!(self.tool, Tool::Select | Tool::Opening(_));
            let active = dragged_wall == Some(i) || (dragged_wall.is_none() && hit_wall == Some(i) && picking);
            let color = if active { pal.selected } else if self.face == Face::Wall(i as u16) { pal.highlight } else { pal.line };
            painter.rect_filled(Rect::from_two_pos(to_screen(a[0], a[2]), to_screen(b[0], b[2])).expand(1.0), 0.0, color);
            // Number and length inside the room, beside the wall.
            let mid = to_screen((w.a[0] + w.b[0]) / 2.0, (w.a[1] + w.b[1]) / 2.0) + vec2(w.normal[0], w.normal[1]) * 8.0;
            let align = match w.normal {
                [x, _] if x > 0.5 => egui::Align2::LEFT_CENTER,
                [x, _] if x < -0.5 => egui::Align2::RIGHT_CENTER,
                [_, z] if z > 0.5 => egui::Align2::CENTER_TOP,
                _ => egui::Align2::CENTER_BOTTOM,
            };
            let label = format!("{}  {:.2} m", i + 1, w.length() / 1000.0);
            painter.text(mid, align, label, egui::FontId::proportional(12.0), pal.text);
        }
        // Under the pointer: which wall it is, and that the highlighted one is the wall shown.
        let wall_tip = hit_wall.filter(|_| matches!(self.tool, Tool::Select) && self.plan_drag.is_none()).and_then(|i| walls.get(i).map(|w| (i, w.length())));
        let wall_tip = wall_tip.map(|(i, length)| {
            let length = format!("{:.2}", length / 1000.0);
            if self.face == Face::Wall(i as u16) {
                tr_fmt!("Wall {}, {} m: the wall shown in the wall view (highlighted); drag to move it", i + 1, length)
            } else {
                tr_fmt!("Wall {}, {} m: drag to move it, a double-click shows it to equip it", i + 1, length)
            }
        });
        // Inner walls with their numbers.
        for (k, w) in e.inner_walls.iter().enumerate() {
            let (a, b) = panel::wall_box(e, Face::Inner(w.id, 0));
            let active = self.plan_inner == Some(w.id) || hit_inner == Some(w.id);
            let color = if active { pal.selected } else if matches!(self.face, Face::Inner(id, _) if id == w.id) { pal.highlight } else { pal.line };
            let r = Rect::from_two_pos(to_screen(a[0], a[2]), to_screen(b[0], b[2])).expand(0.5);
            painter.rect_filled(r, 0.0, color.gamma_multiply(0.85));
            let length = (w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs();
            let align = if r.width() < r.height() { egui::Align2::LEFT_CENTER } else { egui::Align2::CENTER_TOP };
            let at = if r.width() < r.height() { r.right_center() + vec2(6.0, 0.0) } else { r.center_bottom() + vec2(0.0, 4.0) };
            painter.text(at, align, format!("I{}  {:.2} m", k + 1, length / 1000.0), egui::FontId::proportional(12.0), pal.text);
        }
        // Doors (gap, leaf and its swing) and windows (gap with the glass).
        for it in e.items.iter().filter(|i| !i.opening.is_cutout()) {
            let Some(w) = panel::side(e, it.face) else { continue };
            let w = &w;
            // Through an outer wall or the whole inner wall.
            let t = match it.face {
                Face::Inner(id, _) => e.inner_walls.iter().find(|x| x.id == id).map_or(panel::INNER_WALL, |x| x.thickness),
                _ => e.wall(),
            };
            let (s, r, nm) = (w.start(), w.right(), w.normal);
            let at = |along: f32, into: f32| to_screen(s[0] + r[0] * along + nm[0] * into, s[1] + r[1] * along + nm[1] * into);
            let selected = self.selection.contains(&it.id);
            let ink = if selected { pal.selected } else { pal.line };
            let gap = [at(it.x, 0.0), at(it.x + it.w, 0.0), at(it.x + it.w, -t), at(it.x, -t)];
            painter.add(Shape::convex_polygon(gap.to_vec(), pal.paper, Stroke::NONE));
            if it.opening == Opening::Penetration {
                // The hole through the wall, in the colour of the pipe runs.
                let ink = if selected { pal.selected } else { PIPE_COLOR };
                painter.add(Shape::convex_polygon(gap.to_vec(), ink.gamma_multiply(0.35), Stroke::new(if selected { 2.0 } else { 1.2 }, ink)));
                continue;
            }
            if it.opening == Opening::Entry {
                // The lead-in through the wall with an arrow from outside into the room.
                let ink = if selected { pal.selected } else { PIPE_COLOR };
                painter.add(Shape::convex_polygon(gap.to_vec(), ink.gamma_multiply(0.6), Stroke::new(if selected { 2.0 } else { 1.2 }, ink)));
                let mid = it.x + it.w / 2.0;
                let stroke = Stroke::new(1.5, ink);
                painter.line_segment([at(mid, -t - 150.0), at(mid, 120.0)], stroke);
                painter.line_segment([at(mid, 120.0), at(mid - 50.0, 60.0)], stroke);
                painter.line_segment([at(mid, 120.0), at(mid + 50.0, 60.0)], stroke);
                continue;
            }
            let slide = it.swing.slide();
            if it.opening == Opening::Door && slide != 0.0 {
                // A sliding leaf in front of the wall, its way beside the opening dashed.
                let leaf = Stroke::new(if selected { 3.0 } else { 2.0 }, ink);
                let y = panel::SLIDE_DEPTH / 2.0;
                painter.line_segment([at(it.x, y), at(it.x + it.w, y)], leaf);
                let (a, b) = if slide < 0.0 { (it.x - it.w, it.x) } else { (it.x + it.w, it.x + 2.0 * it.w) };
                painter.extend(Shape::dashed_line(&[at(a, y), at(b, y)], Stroke::new(1.0, ink.gamma_multiply(0.6)), 6.0, 4.0));
                let tip = at(it.x + it.w / 2.0 + slide * it.w * 0.3, y + 120.0);
                let tail = at(it.x + it.w / 2.0 - slide * it.w * 0.3, y + 120.0);
                painter.arrow(tail, tip - tail, Stroke::new(1.0, ink.gamma_multiply(0.8)));
            } else if it.opening == Opening::Door {
                // The leaf at its hinges, opened at a right angle, and its swing; into
                // the room or out of it from the outer surface of the wall.
                let leaf = Stroke::new(if selected { 2.0 } else { 1.2 }, ink);
                let (hinge, dir) = if it.swing.right() { (it.x + it.w, -1.0) } else { (it.x, 1.0) };
                let (base, out) = if it.swing.outward() { (-t, -1.0) } else { (0.0, 1.0) };
                painter.line_segment([at(hinge, base), at(hinge, base + out * it.w)], leaf);
                let arc: Vec<Pos2> = (0..=16)
                    .map(|k| {
                        let a = std::f32::consts::FRAC_PI_2 * k as f32 / 16.0;
                        at(hinge + dir * it.w * a.cos(), base + out * it.w * a.sin())
                    })
                    .collect();
                painter.add(Shape::line(arc, Stroke::new(1.0, ink.gamma_multiply(0.6))));
            } else if slide != 0.0 {
                // Two sashes side by side in the wall, the moving one with an arrow.
                let frame = Stroke::new(1.2, ink);
                painter.line_segment([at(it.x, 0.0), at(it.x + it.w, 0.0)], frame);
                painter.line_segment([at(it.x, -t), at(it.x + it.w, -t)], frame);
                let glass = Stroke::new(if selected { 4.0 } else { 3.0 }, pal.highlight);
                let mid = it.x + it.w / 2.0;
                painter.line_segment([at(it.x, -t * 0.65), at(mid + 40.0, -t * 0.65)], glass);
                painter.line_segment([at(mid - 40.0, -t * 0.35), at(it.x + it.w, -t * 0.35)], glass);
                let tip = at(mid + slide * it.w * 0.3, 160.0);
                let tail = at(mid - slide * it.w * 0.3, 160.0);
                painter.arrow(tail, tip - tail, Stroke::new(1.0, ink.gamma_multiply(0.8)));
            } else {
                // Frame on both faces of the wall and the glass between them.
                let frame = Stroke::new(1.2, ink);
                painter.line_segment([at(it.x, 0.0), at(it.x + it.w, 0.0)], frame);
                painter.line_segment([at(it.x, -t), at(it.x + it.w, -t)], frame);
                painter.line_segment([at(it.x, -t / 2.0), at(it.x + it.w, -t / 2.0)], Stroke::new(if selected { 4.0 } else { 3.0 }, pal.highlight));
            }
            for along in [it.x, it.x + it.w] {
                painter.line_segment([at(along, 0.0), at(along, -t)], Stroke::new(1.5, ink));
            }
        }
        // The shafts through the level: an open square with a cross.
        for (name, corners) in shafts_in_plan(prj, ei) {
            let pts: Vec<Pos2> = corners.iter().map(|q| to_screen(q[0], q[1])).collect();
            let stroke = Stroke::new(1.5, PIPE_COLOR);
            painter.add(Shape::convex_polygon(pts.clone(), PIPE_COLOR.gamma_multiply(0.15), stroke));
            painter.line_segment([pts[0], pts[2]], Stroke::new(1.0, PIPE_COLOR));
            painter.line_segment([pts[1], pts[3]], Stroke::new(1.0, PIPE_COLOR));
            painter.text(pts[0] + vec2(4.0, 2.0), egui::Align2::LEFT_TOP, name, egui::FontId::proportional(12.0), pal.text);
        }
        // The installation zone in the floor along the walls, see-through.
        if self.show_zones {
            for band in openecraft_core::routing3d::flush::bands(prj, ei, &prj.layout.routing) {
                let kind = prj.layout.routing.zones[band.zone].kind;
                if kind != openecraft_core::routing3d::ZoneKind::Floor {
                    continue;
                }
                let pts: Vec<Pos2> = band.corners.iter().map(|q| to_screen(q[0], q[2])).collect();
                painter.add(Shape::convex_polygon(pts, super::scene3d::zone_color(kind).gamma_multiply(0.25), Stroke::NONE));
            }
        }
        // The stairs on the level: their steps and the walking line up to its arrow;
        // dashed where they come up from below.
        for st in stairs_in_plan(prj, ei) {
            let stroke = Stroke::new(1.2, pal.text);
            for step in &st.steps {
                let pts: Vec<Pos2> = step.iter().map(|q| to_screen(q[0], q[1])).collect();
                if st.from_below {
                    for k in 0..pts.len() {
                        painter.extend(Shape::dashed_line(&[pts[k], pts[(k + 1) % pts.len()]], stroke, 4.0, 3.0));
                    }
                } else {
                    painter.add(Shape::closed_line(pts, stroke));
                }
            }
            let walk: Vec<Pos2> = st.walk.iter().map(|q| to_screen(q[0], q[1])).collect();
            painter.add(Shape::line(walk.clone(), Stroke::new(1.5, pal.highlight)));
            if let [.., a, b] = walk.as_slice() {
                let d = (*b - *a).normalized() * 9.0;
                painter.add(Shape::convex_polygon(vec![*b, *b - d + d.rot90() * 0.5, *b - d - d.rot90() * 0.5], pal.highlight, Stroke::NONE));
            }
        }
        // Under a roof: the parts of the floor below the sloped ceiling shaded, with the
        // lowest ceiling height there.
        if let Some(c) = panel::roofs::ceiling(prj, ei) {
            for cell in panel::roofs::floor_polys(e) {
                for (poly, lin) in c.pieces(&cell) {
                    if lin.is_flat() && (lin.c - c.flat).abs() < 0.5 {
                        continue;
                    }
                    let pts: Vec<Pos2> = poly.iter().map(|q| to_screen(q[0], q[1])).collect();
                    painter.add(Shape::convex_polygon(pts.clone(), pal.highlight.gamma_multiply(0.12), Stroke::NONE));
                    for k in 0..pts.len() {
                        painter.extend(Shape::dashed_line(&[pts[k], pts[(k + 1) % pts.len()]], Stroke::new(1.0, pal.highlight), 5.0, 4.0));
                    }
                    let low = poly.iter().map(|q| lin.at(*q)).fold(f32::INFINITY, f32::min);
                    let centre = pts.iter().fold(Pos2::ZERO, |s, p| s + p.to_vec2()) / pts.len() as f32;
                    painter.text(centre, egui::Align2::CENTER_CENTER, tr_fmt!("sloped ceiling from {} m", format!("{:.2}", low / 1000.0)), egui::FontId::proportional(11.0), pal.text);
                }
            }
        }
        // The ways through the wall penetrations into the rooms behind the walls.
        for (a, b) in links_in_plan(prj, ei) {
            let (a, b) = (to_screen(a[0], a[1]), to_screen(b[0], b[1]));
            painter.extend(Shape::dashed_line(&[a, b], Stroke::new(2.0, PIPE_COLOR), 6.0, 4.0));
            painter.circle_filled(b, 3.0, PIPE_COLOR);
        }
        // Cabinets: footprint, front edge, name.
        // A cabinet in a bad place (outside, across a wall, in a door swing, …) is drawn
        // in the warning colour and tells why under the pointer.
        let mut hover_tip = None;
        for (c, fp) in &footprints {
            let pts: Vec<Pos2> = fp.iter().map(|q| to_screen(q[0], q[1])).collect();
            let selected = self.plan_cabinet == Some(first_of(prj, *c));
            let problems = panel::cabinet_problems(prj, ei, *c);
            let warn = crate::style::warning();
            let frame = if selected { pal.selected } else if problems.is_empty() { pal.frame } else { warn };
            let stroke = Stroke::new(if selected || !problems.is_empty() { 2.5 } else { 1.0 }, frame);
            let fill = if problems.is_empty() { pal.plate } else { warn.gamma_multiply(0.25) };
            painter.add(Shape::convex_polygon(pts.clone(), fill, stroke));
            if hit_cabinet == Some(*c) && self.plan_drag.is_none() {
                let mut lines = vec![tr_fmt!("{}: drag to move it, R turns it, a double-click opens it", prj.layout.enclosures[*c].name)];
                lines.extend(problems.into_iter().map(|(_, _, msg)| msg));
                hover_tip = Some(lines.join("\n"));
            }
            painter.line_segment([pts[2], pts[3]], Stroke::new(3.0, stroke.color));
            let centre = Pos2::new(pts.iter().map(|q| q.x).sum::<f32>() / 4.0, pts.iter().map(|q| q.y).sum::<f32>() / 4.0);
            painter.text(centre, egui::Align2::CENTER_CENTER, &prj.layout.enclosures[*c].name, egui::FontId::proportional(12.0), pal.text);
        }
        // Parts standing on the floor: footprint and tag (or part number), in the warning
        // colour where they stand badly.
        for it in e.items.iter().filter(|i| on_floor(i)) {
            let pts: Vec<Pos2> = panel::floor_footprint(e, it).iter().map(|q| to_screen(q[0], q[1])).collect();
            let name = if it.tag.is_empty() { it.part.clone() } else { it.tag.clone() };
            let mut problems: Vec<String> = panel::floor_problems(prj, ei, it).into_iter().map(|(_, _, msg)| msg).collect();
            if !panel::mount_ok(prj, e, it) {
                problems.push(tr_fmt!("{}: {} does not stand on the floor, choose a floor-standing part", e.name, name));
            }
            let selected = self.selection.contains(&it.id);
            let warn = crate::style::warning();
            let frame = if selected { pal.selected } else if problems.is_empty() { pal.frame } else { warn };
            let fill = if problems.is_empty() { pal.plate.gamma_multiply(0.7) } else { warn.gamma_multiply(0.25) };
            painter.add(Shape::convex_polygon(pts.clone(), fill, Stroke::new(if selected || !problems.is_empty() { 2.5 } else { 1.0 }, frame)));
            if hit_floor == Some(it.id) && self.plan_drag.is_none() {
                let mut lines = vec![tr_fmt!("{}: drag to move it, a double-click shows the floor", name)];
                lines.extend(problems);
                hover_tip = Some(lines.join("\n"));
            }
            let centre = Pos2::new(pts.iter().map(|q| q.x).sum::<f32>() / 4.0, pts.iter().map(|q| q.y).sum::<f32>() / 4.0);
            painter.text(centre, egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(11.0), pal.text);
        }
        // Pipe runs seen from above (dashed: they run above the floor).
        for path in &e.paths {
            if path.face.is_none() || path.points.len() < 2 {
                continue;
            }
            let pts: Vec<Pos2> = path.points.iter().map(|q| to_screen(q[0], q[2])).collect();
            let selected = self.selected_object == Some(path.id);
            painter.extend(Shape::dashed_line(&pts, Stroke::new(if selected { 3.0 } else { 2.0 }, PIPE_COLOR), 10.0, 6.0));
            painter.text(pts[0] + vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, &path.name, egui::FontId::proportional(11.0), PIPE_COLOR);
        }
        // Walls being drawn.
        if !self.wall_points.is_empty() {
            let stroke = Stroke::new(2.0, pal.selected);
            let mut pts: Vec<Pos2> = self.wall_points.iter().map(|q| to_screen(q.x, q.y)).collect();
            let last = self.wall_points.last().copied();
            let next = plan.map(|q| if self.tool == Tool::InnerWalls { inner_corner(&walls, last, q) } else { ortho_corner(last, q) });
            if let Some(n) = next {
                pts.push(to_screen(n.x, n.y));
                let l = self.wall_points.last().copied().unwrap_or(n);
                let length = (n.x - l.x).abs() + (n.y - l.y).abs();
                painter.text(to_screen(n.x, n.y) + vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.2} m", length / 1000.0), egui::FontId::proportional(12.0), pal.selected);
            }
            painter.add(Shape::line(pts, stroke));
            let f = self.wall_points[0];
            painter.circle_stroke(to_screen(f.x, f.y), 6.0, stroke);
        } else if self.tool == Tool::Walls {
            if let Some(q) = plan {
                let c = ortho_corner(None, q);
                painter.circle_stroke(to_screen(c.x, c.y), 4.0, Stroke::new(1.5, pal.selected));
            }
        }
        // Hint and pointer position.
        let hint = match self.tool {
            Tool::Walls => tr("Click the corners of the walls; click the first corner again, double-click or press Enter to close the room. Backspace removes the last corner, Esc cancels."),
            Tool::InnerWalls => tr("Click the ends of the inner walls; every click starts the next one, ends near a wall snap onto it. Esc, Enter or a double-click ends the chain, Esc again the tool."),
            Tool::Opening(Opening::Door) => tr("Click on a wall where the door goes; it swings into the room. Size and position in the properties."),
            Tool::Opening(Opening::Penetration) => tr("Click on an outer wall where the wall penetration goes; cables, tubes and pipes pass through it into the room behind the wall. Size and height in the properties."),
            Tool::Opening(Opening::Entry) => tr("Click on an outer wall where the building entry goes; cables and pipes from outside the building come in through it. Size and height in the properties."),
            Tool::Opening(_) => tr("Click on a wall where the window goes. Size, position and sill height in the properties."),
            _ => tr("Drag a wall to move it, drag a cabinet to its place (R turns it); double-click a wall to equip it, a cabinet to open it. Del removes the selected inner wall."),
        };
        // Wrapped to the view, on a backing so that it stays readable over the plan.
        let galley = painter.layout(hint.to_string(), egui::FontId::proportional(12.0), pal.text, (rect.width() - 16.0).max(80.0));
        let at = rect.left_top() + vec2(8.0, 8.0);
        painter.rect_filled(Rect::from_min_size(at, galley.size()).expand(3.0), 3.0, pal.canvas.gamma_multiply(0.85));
        painter.galley(at, galley, pal.text);
        if let Some(tip) = hover_tip.or(wall_tip) {
            resp.clone().on_hover_text_at_pointer(tip);
        }
        if let Some(q) = plan {
            // Where a click puts the corner or end, not the raw pointer.
            let last = self.wall_points.last().copied();
            let q = match self.tool {
                Tool::Walls => ortho_corner(last, q),
                Tool::InnerWalls => inner_corner(&walls, last, q),
                _ => q,
            };
            let at = tr_fmt!("x {} mm, z {} mm", format!("{:.0}", q.x), format!("{:.0}", q.y));
            painter.text(rect.left_bottom() + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, at, egui::FontId::proportional(12.0), pal.text);
        }
    }
}
