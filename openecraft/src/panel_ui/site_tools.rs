//! Tools of the site plan: drawing the boundary of the plot corner by corner, placing
//! the reference point, measuring from one point to another and dragging the corners
//! of the boundary ([`openecraft_core::panel::plot`]). The points snap to the corners
//! of the boundary, the rooms and the shafts and to the reference point.

use super::site_plan::{self, Frame};
use super::{PanelUi, View};
use egui::{vec2, Key, Pos2, Rect, Response, Stroke};
use openecraft_core::model::Project;
use openecraft_core::panel::{plot, GeoReference};
use openecraft_i18n::tr_fmt;

/// What a click in the site plan does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SiteTool {
    /// A click on a room shows it; the corners of the boundary are dragged.
    #[default]
    Select,
    /// Draws the boundary of the plot corner by corner.
    Boundary,
    /// Places the reference point.
    Reference,
    /// Measures from one point to another.
    Measure,
}

/// A measure written out: distance, how far east and north, the direction.
pub(crate) fn measure_text(m: &plot::Measure) -> String {
    tr_fmt!("{} m: {} m east, {} m north, {}° from north", format!("{:.3}", m.distance / 1000.0), format!("{:.3}", m.east), format!("{:.3}", m.north), format!("{:.1}", m.bearing))
}

impl PanelUi {
    /// Chooses a tool of the site plan (Layout space › Site); the site plan is shown.
    /// Chosen again, it stops.
    pub fn choose_site_tool(&mut self, tool: SiteTool) {
        let again = self.site && self.view == View::Plan && self.site_tool == tool;
        if !self.site {
            self.show_site_plan(None);
        }
        self.view = View::Plan;
        self.site_points.clear();
        self.site_drag = None;
        self.site_tool = if again { SiteTool::Select } else { tool };
        self.status.clear();
    }

    /// A click with the tool of the site plan at `q` (the plant, mm); `close`: on the
    /// first corner of the boundary drawn.
    pub(super) fn site_click(&mut self, prj: &mut Project, q: [f32; 2], close: bool) {
        match self.site_tool {
            SiteTool::Select => {}
            SiteTool::Boundary if close && self.site_points.len() >= 3 => self.finish_boundary(prj),
            SiteTool::Boundary => self.site_points.push(q),
            SiteTool::Reference => {
                let r = prj.layout.plot.reference.clone().unwrap_or_default();
                match plot::set_reference(prj, Some(GeoReference { x: q[0], z: q[1], ..r })) {
                    Ok(()) => {
                        self.status = tr_fmt!("Reference point placed at x {} m, z {} m; enter its coordinates in the Buildings pane", format!("{:.3}", q[0] / 1000.0), format!("{:.3}", q[1] / 1000.0));
                        self.site_tool = SiteTool::Select;
                    }
                    Err(e) => self.status = e,
                }
            }
            SiteTool::Measure => {
                if self.site_points.len() >= 2 {
                    self.site_points.clear();
                }
                self.site_points.push(q);
                if let [a, b] = self.site_points[..] {
                    self.status = measure_text(&plot::measure(&prj.layout, a, b));
                }
            }
        }
    }

    /// Closes the boundary drawn: it becomes the plot's (or stays to be corrected).
    pub(super) fn finish_boundary(&mut self, prj: &mut Project) {
        let pts = std::mem::take(&mut self.site_points);
        match plot::set_outline(prj, pts.clone()) {
            Ok(()) => {
                let area = format!("{:.1}", plot::area(&prj.layout.plot.outline));
                let outside: Vec<String> = plot::outside(prj)
                    .into_iter()
                    .map(|s| match s {
                        plot::Standing::Room(i) | plot::Standing::Cabinet(i) => prj.layout.enclosures[i].name.clone(),
                        plot::Standing::Roof(bi, k) => format!("{} {}", prj.layout.buildings[bi].name, prj.layout.buildings[bi].roofs[k].name),
                    })
                    .collect();
                self.status = if outside.is_empty() { tr_fmt!("Plot boundary drawn: {} m²", area) } else { tr_fmt!("Plot boundary drawn: {} m²; outside it: {}", area, outside.join(", ")) };
                self.site_tool = SiteTool::Select;
            }
            Err(e) => {
                self.status = e;
                self.site_points = pts;
            }
        }
    }

    /// Keys of the site tools: Enter closes the boundary, Backspace takes back the last
    /// point, Escape drops the points or stops the tool.
    pub(super) fn site_key(&mut self, prj: &mut Project, key: Key) {
        match key {
            Key::Enter if self.site_tool == SiteTool::Boundary && self.site_points.len() >= 3 => self.finish_boundary(prj),
            Key::Backspace => {
                self.site_points.pop();
            }
            Key::Escape if !self.site_points.is_empty() => self.site_points.clear(),
            Key::Escape => self.site_tool = SiteTool::Select,
            _ => {}
        }
    }

    /// The corner of the plot boundary at `p`.
    fn corner_at(prj: &Project, frame: &Frame, p: Pos2) -> Option<usize> {
        prj.layout.plot.outline.iter().position(|&q| frame.at(q).distance(p) <= 6.0)
    }

    /// What the tools of the site plan do with the pointer in `rect`; gives whether a
    /// corner of the boundary is dragged (the view is then not moved).
    pub(super) fn site_tools(&mut self, ui: &egui::Ui, rect: Rect, resp: &Response, prj: &mut Project) -> bool {
        let frame = site_plan::frame(rect, prj, &self.site_plan);
        if self.site_tool == SiteTool::Select {
            if resp.drag_started() {
                if let Some(k) = ui.input(|i| i.pointer.press_origin()).and_then(|p| Self::corner_at(prj, &frame, p)) {
                    self.site_drag = Some((k, prj.layout.plot.outline.clone()));
                }
            }
            if let (Some((k, _)), Some(p)) = (&self.site_drag, resp.interact_pointer_pos()) {
                let (q, _) = site_plan::snap(&site_plan::snap_points(prj, Some(*k)), &frame, p);
                if let Some(c) = prj.layout.plot.outline.get_mut(*k) {
                    *c = q;
                }
            }
            if resp.drag_stopped() {
                if let Some((_, old)) = self.site_drag.take() {
                    let moved = prj.layout.plot.outline.clone();
                    if let Err(e) = plot::set_outline(prj, moved) {
                        prj.layout.plot.outline = old;
                        self.status = e;
                    }
                }
            }
            if self.site_drag.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if resp.hover_pos().is_some_and(|p| Self::corner_at(prj, &frame, p).is_some()) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            return self.site_drag.is_some();
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if let Some(p) = resp.hover_pos().filter(|_| resp.clicked()) {
            let (q, _) = site_plan::snap(&site_plan::snap_points(prj, None), &frame, p);
            let close = self.site_tool == SiteTool::Boundary && self.site_points.first().is_some_and(|&f| frame.at(f).distance(p) <= 8.0);
            self.site_click(prj, q, close);
        }
        if resp.double_clicked() && self.site_tool == SiteTool::Boundary && self.site_points.len() >= 3 {
            self.finish_boundary(prj);
        }
        if (resp.hovered() || !self.site_points.is_empty()) && !ui.ctx().egui_wants_keyboard_input() {
            for key in [Key::Enter, Key::Backspace, Key::Escape] {
                if ui.input(|i| i.key_pressed(key)) {
                    self.site_key(prj, key);
                }
            }
        }
        false
    }

    /// What the tools of the site plan show over it: the boundary being drawn, the
    /// measure, the point snapped to and the coordinates under the pointer.
    pub(super) fn paint_site_tools(&self, ui: &egui::Ui, rect: Rect, prj: &Project, pointer: Option<Pos2>) {
        let frame = site_plan::frame(rect, prj, &self.site_plan);
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        let accent = visuals.selection.stroke.color;
        let at = pointer.filter(|p| rect.contains(*p)).map(|p| site_plan::snap(&site_plan::snap_points(prj, self.site_drag.as_ref().map(|d| d.0)), &frame, p));
        let drawing = self.site_tool != SiteTool::Select;
        let mut pts: Vec<Pos2> = self.site_points.iter().map(|&q| frame.at(q)).collect();
        if drawing {
            if let Some((q, _)) = at.filter(|_| self.site_tool != SiteTool::Measure || self.site_points.len() == 1) {
                pts.push(frame.at(q));
            }
        }
        match self.site_tool {
            SiteTool::Boundary if !pts.is_empty() => {
                painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0, crate::style::ok())));
                for p in &pts {
                    painter.circle_filled(*p, 3.0, crate::style::ok());
                }
                // The first corner closes it.
                if self.site_points.len() >= 3 {
                    painter.circle_stroke(pts[0], 7.0, Stroke::new(1.5, crate::style::ok()));
                }
                if let [.., a, b] = self.site_points.iter().chain(at.iter().map(|x| &x.0)).copied().collect::<Vec<_>>()[..] {
                    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                    painter.text(frame.at(b) + vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.3} m", len / 1000.0), egui::FontId::proportional(11.0), accent);
                }
            }
            SiteTool::Measure if pts.len() >= 2 => {
                painter.line_segment([pts[0], pts[1]], Stroke::new(2.0, accent));
                for p in &pts[..2] {
                    painter.circle_stroke(*p, 4.0, Stroke::new(1.5, accent));
                }
                let q: Vec<[f32; 2]> = self.site_points.iter().copied().chain(at.iter().map(|x| x.0)).take(2).collect();
                let text = measure_text(&plot::measure(&prj.layout, q[0], q[1]));
                let galley = painter.layout_no_wrap(text, egui::FontId::proportional(11.0), visuals.text_color());
                let mid = pts[0] + (pts[1] - pts[0]) / 2.0 + vec2(8.0, -8.0);
                let back = Rect::from_min_size(mid - vec2(0.0, galley.size().y), galley.size()).expand(3.0);
                painter.rect_filled(back, 3.0, visuals.extreme_bg_color.gamma_multiply(0.9));
                painter.galley(back.min + vec2(3.0, 3.0), galley, visuals.text_color());
            }
            _ => {}
        }
        // The point snapped to, and its coordinates.
        if let Some((q, snapped)) = at {
            let p = frame.at(q);
            if snapped && (drawing || self.site_drag.is_some()) {
                painter.rect_stroke(Rect::from_center_size(p, vec2(10.0, 10.0)), 0.0, Stroke::new(1.5, accent), egui::StrokeKind::Middle);
            }
            let galley = painter.layout_no_wrap(site_plan::coordinates(prj, q), egui::FontId::monospace(11.0), visuals.weak_text_color());
            let back = Rect::from_min_size(rect.right_bottom() - galley.size() - vec2(10.0, 10.0), galley.size()).expand(3.0);
            painter.rect_filled(back, 3.0, visuals.extreme_bg_color);
            painter.galley(back.min + vec2(3.0, 3.0), galley, visuals.weak_text_color());
        }
    }
}

#[cfg(test)]
mod tests;
