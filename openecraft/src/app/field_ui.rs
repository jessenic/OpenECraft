//! Field wiring window: plant layout with the route network (drawn and edited on a
//! canvas), places of enclosures and devices, the planned field cables with their
//! route definitions, equipotential bonding, cable accessories and the rules.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use crate::editor::{EguiDraw, Palette};
use openecraft_core::field::{self, Accessory, AccessoryKind, Bond, BondKind, Node, Outline, Place, Segment, SegmentKind};
use openecraft_core::geom::p;
use openecraft_core::model::ReportKind;
use openecraft_core::routing3d::Class;
use egui::{pos2, vec2, Color32, Pos2, RichText};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Plant,
    Network,
    Cables,
    Bonding,
    Accessories,
    Rules,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum CanvasTool {
    #[default]
    Select,
    Node,
    Segment,
    Outline,
}

/// Drawing units of the plant drawing per metre ([`field::draw::draw`]).
const UNITS: f32 = 10.0;

pub struct FieldUi {
    pub open: bool,
    tab: Tab,
    tool: CanvasTool,
    kind: SegmentKind,
    /// First node of a segment being drawn.
    start: Option<u64>,
    /// Points of an outline being drawn.
    outline: Vec<[f32; 2]>,
    node: Option<u64>,
    dragging: Option<u64>,
    cable: Option<String>,
    /// Screen pixels per drawing unit and offset of the plant origin.
    zoom: f32,
    pan: egui::Vec2,
    fitted: bool,
    tag: String,
}

impl Default for FieldUi {
    fn default() -> Self {
        FieldUi {
            open: false,
            tab: Tab::Plant,
            tool: CanvasTool::Select,
            kind: SegmentKind::Tray,
            start: None,
            outline: Vec::new(),
            node: None,
            dragging: None,
            cable: None,
            zoom: 2.0,
            pan: vec2(40.0, 40.0),
            fitted: false,
            tag: String::new(),
        }
    }
}

/// Positions snap to 0.1 m.
fn snap(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn classes_ui(ui: &mut egui::Ui, bits: &mut u8) {
    for c in Class::ALL {
        let mut on = *bits == 0 || *bits & c.bit() != 0;
        if ui.checkbox(&mut on, c.title()).changed() {
            let all = Class::ALL.iter().fold(0, |m, c| m | c.bit());
            let cur = if *bits == 0 { all } else { *bits };
            let next = if on { cur | c.bit() } else { cur & !c.bit() };
            *bits = if next == all { 0 } else { next };
        }
    }
}

impl App {
    pub(super) fn field_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                for (t, name) in [
                    (Tab::Plant, tr("Plant layout")),
                    (Tab::Network, tr("Route network")),
                    (Tab::Cables, tr("Field cables")),
                    (Tab::Bonding, tr("Equipotential bonding")),
                    (Tab::Accessories, tr("Cable accessories")),
                    (Tab::Rules, tr("Rules")),
                ] {
                    ui.selectable_value(&mut self.field_ui.tab, t, name);
                }
            });
            let problems = self.session.analysis.field.issues.len();
            if problems > 0 {
                ui.colored_label(crate::style::lock(), tr_fmt!("{} field wiring problems (see Check)", problems));
            }
            ui.separator();
            match self.field_ui.tab {
                Tab::Plant => self.field_plant(ui, ctx),
                Tab::Network => self.field_network(ui),
                Tab::Cables => self.field_cables(ui, ctx),
                Tab::Bonding => self.field_bonding(ui, ctx),
                Tab::Accessories => self.field_accessories(ui),
                Tab::Rules => self.field_rules(ui),
            }
        });
    }

    #[allow(clippy::too_many_lines)]
    fn field_plant(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut fit = !self.field_ui.fitted;
        let mut svg = false;
        {
            let fu = &mut self.field_ui;
            ui.horizontal_wrapped(|ui| {
                for (t, name, hint) in [
                    (CanvasTool::Select, tr("Select"), tr("Select and drag nodes; drag the background to pan, scroll to zoom")),
                    (CanvasTool::Node, tr("Node"), tr("Click to add a node of the route network")),
                    (CanvasTool::Segment, tr("Segment"), tr("Click two nodes to connect them")),
                    (CanvasTool::Outline, tr("Outline"), tr("Click the corners of a machine or building, double-click to finish")),
                ] {
                    if ui.selectable_label(fu.tool == t, name).on_hover_text(hint).clicked() {
                        fu.tool = t;
                        fu.start = None;
                        fu.outline.clear();
                    }
                }
                egui::ComboBox::from_id_salt("fkind").selected_text(fu.kind.title()).show_ui(ui, |ui| {
                    for k in SegmentKind::ALL {
                        ui.selectable_value(&mut fu.kind, k, k.title());
                    }
                });
                ui.separator();
                let shown = fu.cable.clone().unwrap_or_else(|| tr("(no cable highlighted)").to_string());
                egui::ComboBox::from_id_salt("fcable").selected_text(shown).show_ui(ui, |ui| {
                    ui.selectable_value(&mut fu.cable, None, tr("(no cable highlighted)"));
                    for c in &self.session.analysis.field.cables {
                        ui.selectable_value(&mut fu.cable, Some(c.cable.clone()), &c.cable);
                    }
                });
                fit |= ui.button(tr("Fit")).clicked();
                svg = icons::text_button(ui, Icon::Download, tr("Export SVG…")).clicked();
            });
        }
        // Selected node: name, position, places, delete.
        if let Some(id) = self.field_ui.node.filter(|id| self.session.prj.field.node(*id).is_some()) {
            let mut remove = false;
            let mut place = None;
            let here: Vec<String> = self.session.prj.field.places.iter().filter(|p| p.node == id).map(|p| p.tag.clone()).collect();
            let n = self.session.prj.field.nodes.iter_mut().find(|n| n.id == id).expect("checked");
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Name"));
                ui.add(egui::TextEdit::singleline(&mut n.name).desired_width(80.0).hint_text(format!("N{id}")));
                ui.add(egui::DragValue::new(&mut n.x).speed(0.1).prefix("x ").suffix(" m"));
                ui.add(egui::DragValue::new(&mut n.y).speed(0.1).prefix("y ").suffix(" m"));
                ui.add(egui::DragValue::new(&mut n.z).speed(0.1).prefix("z ").suffix(" m")).on_hover_text(tr("Height above the floor"));
                if !here.is_empty() {
                    ui.label(tr_fmt!("Placed here: {}", here.join(", ")));
                }
                ui.add(egui::TextEdit::singleline(&mut self.field_ui.tag).desired_width(100.0).hint_text(tr("+CAB1 / -M1")));
                if ui.button(tr("Place")).on_hover_text(tr("Place an enclosure, device or machine location at this node")).clicked() && !self.field_ui.tag.trim().is_empty() {
                    place = Some(self.field_ui.tag.trim().to_string());
                }
                remove = icons::text_button(ui, Icon::Delete, tr("Delete node")).clicked();
            });
            if let Some(tag) = place {
                self.session.prj.field.places.retain(|p| p.tag != tag);
                self.session.prj.field.places.push(Place { tag, node: id, drop: 0.0 });
                self.field_ui.tag.clear();
            }
            if remove {
                self.session.prj.field.remove_node(id);
                self.field_ui.node = None;
            }
        }
        let size = ui.available_size().max(vec2(200.0, 200.0));
        let (resp, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let rect = resp.rect;
        let pal = Palette::new(ui.visuals().dark_mode);
        painter.rect_filled(rect, 2.0, pal.paper);
        let fu = &mut self.field_ui;
        if fit {
            if let Some((min, max)) = field::draw::bounds(&self.session.prj) {
                let (w, h) = (((max.x - min.x) * UNITS).max(10.0), ((max.y - min.y) * UNITS).max(10.0));
                fu.zoom = ((rect.width() - 80.0) / w).min((rect.height() - 80.0) / h).clamp(0.05, 50.0);
                fu.pan = vec2(40.0 - min.x * UNITS * fu.zoom, 40.0 - min.y * UNITS * fu.zoom);
            }
            fu.fitted = true;
        }
        let (zoom, origin) = (fu.zoom, rect.min + fu.pan);
        let to_world = |q: Pos2| ((q.x - origin.x) / (zoom * UNITS), (q.y - origin.y) / (zoom * UNITS));
        let to_screen = |x: f32, y: f32| pos2(origin.x + x * UNITS * zoom, origin.y + y * UNITS * zoom);
        // Grid every metre (every 10 m when zoomed out).
        let step = if zoom * UNITS < 8.0 { 10.0 } else { 1.0 };
        let (x0, y0) = to_world(rect.min);
        let (x1, y1) = to_world(rect.max);
        let mut gx = (x0 / step).floor() * step;
        while gx <= x1 {
            painter.line_segment([to_screen(gx, y0), to_screen(gx, y1)], (1.0, pal.grid));
            gx += step;
        }
        let mut gy = (y0 / step).floor() * step;
        while gy <= y1 {
            painter.line_segment([to_screen(x0, gy), to_screen(x1, gy)], (1.0, pal.grid));
            gy += step;
        }
        let mut d = EguiDraw { painter: &painter, origin, zoom, pal: &pal, force: None, mirror: None, textures: None };
        field::draw::draw(&mut d, &self.session.prj, &self.session.analysis.field, p(0.0, 0.0), UNITS, fu.cable.as_deref());
        let hl = Color32::from_rgb(230, 120, 20);
        if let Some(n) = fu.node.and_then(|id| self.session.prj.field.node(id)) {
            painter.circle_stroke(to_screen(n.x, n.y), 7.0, (2.0, hl));
        }
        if let Some(n) = fu.start.and_then(|id| self.session.prj.field.node(id)) {
            painter.circle_filled(to_screen(n.x, n.y), 5.0, hl);
        }
        if fu.outline.len() > 1 {
            let pts: Vec<Pos2> = fu.outline.iter().map(|q| to_screen(q[0], q[1])).collect();
            painter.add(egui::Shape::line(pts, (1.5, hl)));
        }

        // Interaction
        let nodes: Vec<(u64, Pos2)> = self.session.prj.field.nodes.iter().map(|n| (n.id, to_screen(n.x, n.y))).collect();
        let hit = |q: Pos2| nodes.iter().map(|(id, s)| (*id, s.distance(q))).filter(|(_, d)| *d < 9.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
        let gesture = openecraft_ui::nav::read(ui, &resp);
        if let (true, Some(at)) = (gesture.zooms(), gesture.at) {
            let before = to_world(at);
            fu.zoom = (zoom * gesture.zoom).clamp(0.05, 50.0);
            fu.pan = at - rect.min - vec2(before.0, before.1) * UNITS * fu.zoom;
        }
        fu.pan += gesture.pan;
        // Fingers navigating leave a dragged node where it is.
        if gesture.touch {
            fu.dragging = None;
        }
        if let Some(h) = resp.hover_pos() {
            let (x, y) = to_world(h);
            painter.text(rect.left_bottom() + vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("x {x:.1} m  y {y:.1} m"), egui::FontId::monospace(11.0), pal.line);
        }
        let pointer = resp.interact_pointer_pos();
        if resp.drag_started() && fu.tool == CanvasTool::Select && !gesture.touch {
            fu.dragging = pointer.and_then(hit);
            if fu.dragging.is_some() {
                fu.node = fu.dragging;
            }
        }
        if resp.dragged() && !gesture.touch {
            match (fu.dragging, pointer) {
                (Some(id), Some(q)) => {
                    let (x, y) = to_world(q);
                    if let Some(n) = self.session.prj.field.nodes.iter_mut().find(|n| n.id == id) {
                        (n.x, n.y) = (snap(x), snap(y));
                    }
                }
                _ => fu.pan += resp.drag_delta(),
            }
        }
        if resp.drag_stopped() {
            fu.dragging = None;
        }
        if resp.double_clicked() && fu.tool == CanvasTool::Outline && fu.outline.len() >= 3 {
            let id = self.session.prj.new_id();
            let name = format!("+M{}", self.session.prj.field.outlines.len() + 1);
            self.session.prj.field.outlines.push(Outline { id, name, points: std::mem::take(&mut fu.outline), conductive: true });
        } else if resp.clicked() {
            if let Some(q) = pointer {
                let (x, y) = to_world(q);
                match fu.tool {
                    CanvasTool::Select => fu.node = hit(q),
                    CanvasTool::Node => {
                        let id = self.session.prj.new_id();
                        self.session.prj.field.nodes.push(Node { id, name: String::new(), x: snap(x), y: snap(y), z: 0.0 });
                        fu.node = Some(id);
                    }
                    CanvasTool::Segment => match (fu.start, hit(q)) {
                        (None, h) => fu.start = h,
                        (Some(a), Some(b)) if a != b => {
                            let id = self.session.prj.new_id();
                            self.session.prj.field.segments.push(Segment { id, a, b, kind: fu.kind, ..Default::default() });
                            fu.start = Some(b);
                        }
                        _ => fu.start = None,
                    },
                    CanvasTool::Outline => fu.outline.push([snap(x), snap(y)]),
                }
            }
        }
        if resp.secondary_clicked() {
            fu.start = None;
            fu.outline.clear();
        }
        if svg {
            let s = field::draw::svg(&self.session.prj, &self.session.analysis.field, self.field_ui.cable.as_deref(), 400.0);
            self.export(ctx, self.suggested_name("svg"), (tr("SVG drawing"), "svg"), s);
        }
    }

    fn field_network(&mut self, ui: &mut egui::Ui) {
        let f = &mut self.session.prj.field;
        let names: Vec<(u64, String)> = f.nodes.iter().map(|n| (n.id, f.node_name(n.id))).collect();
        let node_combo = |ui: &mut egui::Ui, salt: (&str, usize), v: &mut u64| {
            let shown = names.iter().find(|n| n.0 == *v).map(|n| n.1.clone()).unwrap_or_else(|| "?".into());
            egui::ComboBox::from_id_salt(salt).width(70.0).selected_text(shown).show_ui(ui, |ui| {
                for (id, name) in &names {
                    ui.selectable_value(v, *id, name);
                }
            });
        };
        let fill = |id: u64| self.session.analysis.field.segment(id);
        egui::ScrollArea::vertical().id_salt("fnet").show(ui, |ui| {
            ui.label(RichText::new(tr("Segments")).strong());
            let mut remove = None;
            let lengths: Vec<f32> = f.segments.iter().map(|s| f.segment_length(s)).collect();
            egui::Grid::new("fsegs").striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
                for h in [tr("Name"), tr("From"), tr("To"), tr("Kind"), tr("Width × height (mm)"), tr("Classes"), tr("Divider"), tr("Reserve"), tr("Length"), tr("Bend radius"), tr("Part"), tr("Fill"), ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (i, s) in f.segments.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(50.0).hint_text(format!("S{}", s.id)));
                    node_combo(ui, ("fa", i), &mut s.a);
                    node_combo(ui, ("fb", i), &mut s.b);
                    egui::ComboBox::from_id_salt(("fk", i)).selected_text(s.kind.title()).show_ui(ui, |ui| {
                        for k in SegmentKind::ALL {
                            ui.selectable_value(&mut s.kind, k, k.title());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut s.width).speed(5.0).range(0.0..=2000.0));
                        ui.add(egui::DragValue::new(&mut s.height).speed(5.0).range(0.0..=500.0));
                    });
                    ui.menu_button(if s.classes == 0 { tr("all") } else { tr("some") }, |ui| classes_ui(ui, &mut s.classes));
                    ui.checkbox(&mut s.divider, "");
                    ui.add(egui::DragValue::new(&mut s.reserve).speed(1.0).range(0.0..=90.0).suffix(" %"));
                    ui.add(egui::DragValue::new(&mut s.length).speed(0.1).range(0.0..=10000.0).suffix(" m").custom_formatter(|v, _| if v == 0.0 { format!("{:.1} m", lengths[i]) } else { format!("{v:.1} m") }))
                        .on_hover_text(tr("0 = from the node positions"));
                    ui.add(egui::DragValue::new(&mut s.bend_radius).speed(5.0).range(0.0..=2000.0).suffix(" mm"));
                    ui.add(egui::TextEdit::singleline(&mut s.part).desired_width(90.0).hint_text(tr("part number")));
                    match fill(s.id) {
                        Some(u) => ui.label(match u.fill(s) {
                            Some(x) => tr_fmt!("{} cables, {} %", u.cables.len(), format!("{x:.0}")),
                            None => tr_fmt!("{} cables", u.cables.len()),
                        }),
                        None => ui.weak("–"),
                    };
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(s.id);
                    }
                    ui.end_row();
                }
            });
            if let Some(id) = remove {
                f.remove_segment(id);
            }
            if f.segments.is_empty() {
                ui.weak(tr("No segments yet: draw them in the plant layout."));
            }
            ui.separator();
            ui.label(RichText::new(tr("Places")).strong()).on_hover_text(tr("Enclosures (+CAB1), devices (-M1) and machine locations (+M1) at the nodes"));
            let mut remove = None;
            egui::Grid::new("fplaces").striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
                for (i, pl) in f.places.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut pl.tag).desired_width(110.0).font(egui::TextStyle::Monospace));
                    node_combo(ui, ("fp", i), &mut pl.node);
                    ui.add(egui::DragValue::new(&mut pl.drop).speed(0.1).range(0.0..=100.0).suffix(" m")).on_hover_text(tr("Length from the node to the device"));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                f.places.remove(i);
            }
            ui.separator();
            ui.label(RichText::new(tr("Outlines")).strong());
            let mut remove = None;
            for (i, o) in f.outlines.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut o.name).desired_width(110.0));
                    ui.checkbox(&mut o.conductive, tr("Conductive frame")).on_hover_text(tr("Must be connected to the equipotential bonding"));
                    ui.weak(tr_fmt!("{} corners", o.points.len()));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                f.outlines.remove(i);
            }
        });
    }

    fn field_cables(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let plan = &self.session.analysis.field;
        let f = &self.session.prj.field;
        let mut apply = false;
        let mut report = false;
        ui.horizontal(|ui| {
            apply = ui.add_enabled(plan.cables.iter().any(|c| c.routed()), egui::Button::new(tr("Take over cut lengths"))).on_hover_text(tr("Set the cable lengths to the planned cut lengths")).clicked();
            report = icons::text_button(ui, Icon::Report, tr("Report")).clicked();
            ui.weak(tr_fmt!("{} parts of the network", plan.islands));
        });
        if plan.cables.is_empty() {
            ui.weak(tr("No field cables: place the enclosures and field devices at nodes of the route network."));
        }
        let mut select = None;
        egui::ScrollArea::vertical().id_salt("fcab").max_height(260.0).show(ui, |ui| {
            egui::Grid::new("fcables").striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                for h in [tr("Cable"), tr("From"), tr("To"), tr("Class"), tr("Route"), tr("Length"), tr("Cut length"), tr("Status")] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for c in &plan.cables {
                    if ui.selectable_label(self.field_ui.cable.as_deref() == Some(c.cable.as_str()), RichText::new(&c.cable).monospace()).clicked() {
                        select = Some(c.cable.clone());
                    }
                    ui.label(&c.ends[0]);
                    ui.label(&c.ends[1]);
                    ui.label(c.class.title());
                    ui.label(c.segments.iter().map(|&s| f.segment_name(s)).collect::<Vec<_>>().join(" › "));
                    ui.label(if c.routed() { format!("{:.1} m", c.length) } else { "–".into() });
                    ui.label(if c.routed() { format!("{:.1} m", c.cut) } else { "–".into() });
                    if c.routed() {
                        ui.label(c.status_text());
                    } else {
                        ui.colored_label(Color32::from_rgb(220, 80, 60), c.status_text());
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(c) = select {
            self.field_ui.cable = Some(c);
        }
        // Route definition of the selected cable.
        if let Some(tag) = self.field_ui.cable.clone().filter(|t| self.session.prj.cable(t).is_some()) {
            ui.separator();
            ui.label(RichText::new(tr_fmt!("Route definition of {}", tag)).strong());
            let f = &mut self.session.prj.field;
            let names: Vec<(u64, String)> = f.nodes.iter().map(|n| (n.id, f.node_name(n.id))).collect();
            let segs: Vec<(u64, String)> = f.segments.iter().map(|s| (s.id, f.segment_name(s.id))).collect();
            let def = f.route_mut(&tag);
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Via:"));
                let mut remove = None;
                for (i, v) in def.via.iter().enumerate() {
                    let name = names.iter().find(|n| n.0 == *v).map(|n| n.1.as_str()).unwrap_or("?");
                    if ui.small_button(format!("{name} ✖")).on_hover_text(tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    def.via.remove(i);
                }
                ui.menu_button(tr("Add node"), |ui| {
                    for (id, name) in &names {
                        if ui.button(name).clicked() {
                            def.via.push(*id);
                            ui.close();
                        }
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Avoid:"));
                for (id, name) in &segs {
                    let mut on = def.avoid.contains(id);
                    if ui.checkbox(&mut on, name).changed() {
                        if on {
                            def.avoid.push(*id);
                        } else {
                            def.avoid.retain(|x| x != id);
                        }
                    }
                }
            });
            f.routes.retain(|r| !r.is_empty());
        }
        if apply {
            let n = field::apply_lengths(&mut self.session.prj, &self.session.analysis.field);
            self.notify(ctx, tr_fmt!("{} cable lengths taken over", n));
        }
        if report {
            self.report_kind = ReportKind::FieldRouting.into();
            self.show_reports = true;
        }
    }

    fn field_bonding(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let points = field::bonding::points(&self.session.prj);
        let names: Vec<String> = points.iter().map(|p| p.name.clone()).collect();
        let point_combo = |ui: &mut egui::Ui, salt: (&str, usize), v: &mut String| {
            egui::ComboBox::from_id_salt(salt).width(100.0).selected_text(v.clone()).show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(v, n.clone(), n);
                }
            });
        };
        let mut report = false;
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::Plus, tr("Bonding conductor")).clicked() {
                let id = self.session.prj.new_id();
                let a = self.session.prj.field.rules.main_earth.clone();
                let b = names.iter().find(|n| **n != a).cloned().unwrap_or_default();
                self.session.prj.field.bonds.push(Bond { id, a, b, kind: BondKind::Main, cross_section: "6".into(), ..Default::default() });
            }
            report = icons::text_button(ui, Icon::Report, tr("Report")).clicked();
        });
        let mut remove = None;
        egui::Grid::new("fbonds").striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
            for h in [tr("From"), tr("To"), tr("Kind"), tr("Cross-section"), tr("Length"), tr("Part"), ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (i, b) in self.session.prj.field.bonds.iter_mut().enumerate() {
                point_combo(ui, ("fba", i), &mut b.a);
                point_combo(ui, ("fbb", i), &mut b.b);
                egui::ComboBox::from_id_salt(("fbk", i)).selected_text(b.kind.title()).show_ui(ui, |ui| {
                    for k in BondKind::ALL {
                        ui.selectable_value(&mut b.kind, k, k.title());
                    }
                });
                ui.add(egui::TextEdit::singleline(&mut b.cross_section).desired_width(40.0).hint_text("mm²"));
                ui.add(egui::DragValue::new(&mut b.length).speed(0.1).range(0.0..=1000.0).suffix(" m"));
                ui.add(egui::TextEdit::singleline(&mut b.part).desired_width(90.0).hint_text(tr("part number")));
                if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = remove {
            self.session.prj.field.bonds.remove(i);
        }
        ui.separator();
        ui.label(RichText::new(tr("Bonding points")).strong());
        egui::Grid::new("fbpoints").striped(true).show(ui, |ui| {
            for p in &points {
                ui.monospace(&p.name);
                ui.label(p.kind.title());
                if p.bonded {
                    ui.colored_label(crate::style::ok(), tr("bonded"));
                } else {
                    ui.colored_label(crate::style::lock(), tr("not bonded"));
                }
                ui.end_row();
            }
        });
        if report {
            self.report_kind = ReportKind::Bonding.into();
            self.show_reports = true;
        }
        let _ = ctx;
    }

    fn field_accessories(&mut self, ui: &mut egui::Ui) {
        let cables: Vec<String> = self.session.prj.cables.iter().map(|c| c.tag.clone()).collect();
        let ends = |cable: &str| self.session.analysis.field.cable(cable).map(|c| c.ends.clone());
        if ui.add_enabled_ui(!cables.is_empty(), |ui| icons::text_button(ui, Icon::Plus, tr("Accessory"))).inner.clicked() {
            let id = self.session.prj.new_id();
            let cable = self.field_ui.cable.clone().filter(|c| cables.contains(c)).unwrap_or_else(|| cables[0].clone());
            self.session.prj.field.accessories.push(Accessory { id, cable, count: 1, ..Default::default() });
        }
        let mut remove = None;
        egui::ScrollArea::vertical().id_salt("facc").show(ui, |ui| {
            egui::Grid::new("faccs").striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
                for h in [tr("Cable"), tr("End"), tr("Kind"), tr("Part"), tr("Quantity"), ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (i, a) in self.session.prj.field.accessories.iter_mut().enumerate() {
                    egui::ComboBox::from_id_salt(("fac", i)).width(80.0).selected_text(a.cable.clone()).show_ui(ui, |ui| {
                        for c in &cables {
                            ui.selectable_value(&mut a.cable, c.clone(), c);
                        }
                    });
                    let shown = if a.at.is_empty() { tr("both ends").to_string() } else { a.at.clone() };
                    egui::ComboBox::from_id_salt(("fae", i)).width(90.0).selected_text(shown).show_ui(ui, |ui| {
                        ui.selectable_value(&mut a.at, String::new(), tr("both ends"));
                        for e in ends(&a.cable).into_iter().flatten() {
                            ui.selectable_value(&mut a.at, e.clone(), e);
                        }
                    });
                    egui::ComboBox::from_id_salt(("fak", i)).selected_text(a.kind.title()).show_ui(ui, |ui| {
                        for k in AccessoryKind::ALL {
                            ui.selectable_value(&mut a.kind, k, k.title());
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut a.part).desired_width(110.0).hint_text(tr("part number")));
                    ui.add(egui::DragValue::new(&mut a.count).range(1..=999));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(i) = remove {
            self.session.prj.field.accessories.remove(i);
        }
        if self.session.prj.field.accessories.is_empty() {
            ui.weak(tr("No accessories: glands, connectors, shield clamps … go into the bill of materials and are checked against the cable."));
        }
    }

    fn field_rules(&mut self, ui: &mut egui::Ui) {
        let r = &mut self.session.prj.field.rules;
        egui::Grid::new("frules").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tr("Reserve per cable end"));
            ui.add(egui::DragValue::new(&mut r.reserve).speed(0.1).range(0.0..=20.0).suffix(" m"));
            ui.end_row();
            ui.label(tr("Length inside an enclosure"));
            ui.add(egui::DragValue::new(&mut r.cabinet).speed(0.1).range(0.0..=20.0).suffix(" m"));
            ui.end_row();
            ui.label(tr("Cut length step"));
            ui.add(egui::DragValue::new(&mut r.cut).speed(0.1).range(0.0..=50.0).suffix(" m"));
            ui.end_row();
            ui.label(tr("Maximum fill of a carrier"));
            ui.add(egui::DragValue::new(&mut r.fill).speed(1.0).range(1.0..=100.0).suffix(" %"));
            ui.end_row();
            ui.label(tr("Bend radius in drag chains"));
            ui.add(egui::DragValue::new(&mut r.bend_factor).speed(0.5).range(1.0..=30.0).suffix(" × Ø"));
            ui.end_row();
            ui.label(tr("Main earthing terminal"));
            ui.text_edit_singleline(&mut r.main_earth);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut r.continuity, tr("Metal carriers meeting at a node are connected"));
            ui.end_row();
        });
    }
}
