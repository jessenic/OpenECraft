//! Part widgets: product group tree, tabbed part editor, read only summary with the
//! product picture and previews of the 2D front view and the 3D model.

use openecraft_i18n::{tr, tr_fmt};
use crate::icons::{self, Icon};
use crate::io::{short_name, FileKind};
use egui::{vec2, Color32, Pos2, RichText, Sense, Shape, Stroke};
use openecraft_parts::vec3::{cross, dot, norm, sub, Orbit};
use openecraft_parts::{
    default_connection_points, distinct_values, parse_mesh, parse_svg, natural_cmp, ConnectionPoint, Exit, FnKind, FunctionTemplate, Grouping,
    Component, Cutout, Mesh3d, Model2d, MountingData, Part, PartFilter, PlacedCutout, Prim2d, TopGroup, MOUNTING_KEYS, PRODUCT_GROUPS,
};
use openecraft_parts::portal::{Attachment, Lifecycle};
use openecraft_parts::{date_from_unix, productgroups};
use std::collections::BTreeMap;

/// A part dragged from a part list (parts management, data portal) to be dropped
/// where it is used, e.g. onto a schematic page.
#[derive(Clone, Debug, PartialEq)]
pub struct PartDrag {
    pub number: String,
}

/// A row of a part list that can be dragged as a [`PartDrag`] (when `drag` is on) and
/// still be clicked; the response covers both.
pub fn drag_row(ui: &mut egui::Ui, drag: bool, number: &str, add: impl FnOnce(&mut egui::Ui) -> egui::Response) -> egui::Response {
    if !drag {
        return add(ui);
    }
    let id = ui.make_persistent_id(("part_drag", number));
    let r = ui.dnd_drag_source(id, PartDrag { number: number.to_string() }, add);
    // egui shows a grab cursor over drag sources; the lists keep the normal pointer
    // like the other ones of the editor.
    if r.response.hovered() && !ui.ctx().is_being_dragged(id) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
    r.inner.union(r.response.interact(Sense::click())).on_hover_text(tr("Drag it onto a schematic page to place it"))
}

/// Graphics file of a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    /// 2D front view (SVG).
    Front2d,
    /// 3D model (STL, OBJ, STEP, IGES, Parasolid, IFC, JT).
    Mesh3d,
}

impl FileKind for ModelKind {
    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            ModelKind::Front2d => (tr("2D front view (SVG)"), &["svg"]),
            ModelKind::Mesh3d => (tr("3D model (STL, OBJ, STEP, IGES, Parasolid, IFC, JT)"), &["stl", "obj", "step", "stp", "igs", "iges", "x_t", "x_b", "xmt_txt", "xmt_bin", "ifc", "jt"]),
        }
    }
}

/// Reads a 3D model by file name extension.
pub fn parse_model_3d(file: &str, bytes: &[u8]) -> Result<Mesh3d, String> {
    let lower = file.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or_default();
    match ext {
        "step" | "stp" => openecraft_step::parse_step(bytes),
        "igs" | "iges" => openecraft_step::parse_iges(bytes),
        "x_t" | "x_b" | "xmt_txt" | "xmt_bin" => openecraft_step::parse_parasolid(bytes),
        "jt" => openecraft_jt::parse_jt(bytes),
        _ => parse_mesh(file, bytes),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartTab {
    General,
    Commercial,
    Technical,
    Mounting,
    Functions,
    Connections,
    Accessories,
    Graphics,
    Portal,
}

/// Editor settings kept between frames.
pub struct EditorState {
    pub tab: PartTab,
    /// Factor of the "Scale" button for imported 3D models.
    pub model_scale: f32,
    /// Connection point placed by clicking into the front view.
    pub point: Option<usize>,
}

impl Default for EditorState {
    fn default() -> Self {
        EditorState { tab: PartTab::General, model_scale: 25.4, point: None }
    }
}

fn choice_row(ui: &mut egui::Ui, id: (&str, &str), label: &str, value: &mut Option<String>, options: &[String]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id).selected_text(value.as_deref().unwrap_or(tr("(all)"))).width(180.0).show_ui(ui, |ui| {
        ui.selectable_value(value, None, tr("(all)"));
        for o in options {
            ui.selectable_value(value, Some(o.clone()), o);
        }
    });
    ui.end_row();
}

/// Filter of a part tree: text, manufacturer, supplier, product group and grouping,
/// followed by the number of shown parts.
pub fn part_filter(ui: &mut egui::Ui, id: &str, parts: &[&Part], f: &mut PartFilter) {
    ui.horizontal(|ui| {
        let reset = ui.add_enabled_ui(f.is_active(), |ui| icons::button(ui, Icon::Close, tr("Reset filter"))).inner;
        if reset.clicked() {
            f.reset();
        }
        ui.add(egui::TextEdit::singleline(&mut f.text).hint_text(tr("Filter (number, type, manufacturer, text …)")).desired_width(f32::INFINITY));
    });
    egui::CollapsingHeader::new(tr("Filter criteria")).id_salt((id, "criteria")).show(ui, |ui| {
        egui::Grid::new((id, "filter")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            choice_row(ui, (id, "manufacturer"), tr("Manufacturer"), &mut f.manufacturer, &distinct_values(parts.iter().copied(), |p| &p.manufacturer));
            choice_row(ui, (id, "supplier"), tr("Supplier"), &mut f.supplier, &distinct_values(parts.iter().copied(), |p| &p.supplier));
            ui.label(tr("Top group"));
            let title = f.top_group.map_or(tr("(all)"), |t| t.title());
            egui::ComboBox::from_id_salt((id, "top")).selected_text(title).width(180.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut f.top_group, None, tr("(all)"));
                for t in TopGroup::ALL {
                    ui.selectable_value(&mut f.top_group, Some(t), t.title());
                }
            });
            ui.end_row();
            let top = f.top_group;
            let groups = distinct_values(parts.iter().copied().filter(|p| top.is_none_or(|t| t == p.top_group)), |p| &p.group);
            choice_row(ui, (id, "group"), tr("Product group"), &mut f.group, &groups);
            ui.label(tr("Wall mounting"));
            ui.checkbox(&mut f.wall_mount, tr("Only parts for direct wall mounting"));
            ui.end_row();
            ui.label(tr("Floor mounting"));
            ui.checkbox(&mut f.floor_mount, tr("Only floor-standing parts"));
            ui.end_row();
            ui.label(tr("Group by"));
            egui::ComboBox::from_id_salt((id, "grouping")).selected_text(tr(f.grouping.title())).width(180.0).show_ui(ui, |ui| {
                for g in Grouping::ALL {
                    ui.selectable_value(&mut f.grouping, g, tr(g.title()));
                }
            });
            ui.end_row();
        });
    });
    let shown = parts.iter().filter(|p| f.matches(p)).count();
    let count = if f.is_active() { tr_fmt!("{} of {} parts", shown, parts.len()) } else { tr_fmt!("{} parts", parts.len()) };
    ui.label(RichText::new(count).weak());
}

/// Part tree, grouped as the filter says (product group, manufacturer or supplier).
/// Returns the part number that was double clicked.
pub fn part_tree(ui: &mut egui::Ui, id: &str, parts: &[&Part], filter: &PartFilter, selected: &mut Option<String>) -> Option<String> {
    part_tree_with(ui, id, parts, filter, selected, false)
}

/// [`part_tree`] whose parts can be dragged out of it as [`PartDrag`] (`drag`).
pub fn part_tree_with(ui: &mut egui::Ui, id: &str, parts: &[&Part], filter: &PartFilter, selected: &mut Option<String>, drag: bool) -> Option<String> {
    let (no_group, no_manufacturer, no_supplier) = (tr("(no product group)"), tr("(no manufacturer)"), tr("(no supplier)"));
    let or = |s: &str, empty: &str| if s.is_empty() { empty } else { s }.to_string();
    // First level: (sort key, title); product groups keep the top group order.
    type Groups<'a> = BTreeMap<String, BTreeMap<&'a str, Vec<&'a Part>>>;
    let mut tree: BTreeMap<(usize, String), Groups> = BTreeMap::new();
    for p in parts.iter().filter(|p| filter.matches(p)) {
        // Sorted as shown: product groups in the user's language.
        let group = crate::portal::group_text(&or(&p.group, no_group));
        let (first, sub) = match filter.grouping {
            Grouping::ProductGroup => ((p.top_group as usize, tr(p.top_group.title()).to_string()), p.subgroup.as_str()),
            Grouping::Manufacturer => ((0, or(&p.manufacturer, no_manufacturer)), ""),
            Grouping::Supplier => ((0, or(&p.supplier, no_supplier)), ""),
        };
        tree.entry(first).or_default().entry(group).or_default().entry(sub).or_default().push(p);
    }
    let open = if filter.is_active() { Some(true) } else { None };
    let mut activated = None;
    let mut leaf = |ui: &mut egui::Ui, list: &mut Vec<&Part>| {
        list.sort_by(|a, b| natural_cmp(&a.number, &b.number));
        for p in list.iter() {
            let is_sel = selected.as_deref() == Some(p.number.as_str());
            let r = ui.horizontal(|ui| {
                let r = drag_row(ui, drag, &p.number, |ui| ui.selectable_label(is_sel, &p.number)).on_hover_text(&p.description);
                if p.wall_mountable() {
                    wall_badge(ui);
                }
                if p.floor_standing() {
                    floor_badge(ui);
                }
                r
            });
            let r = r.inner;
            if r.clicked() {
                *selected = Some(p.number.clone());
            }
            if r.double_clicked() {
                activated = Some(p.number.clone());
            }
        }
    };
    let grouping = filter.grouping.title();
    for ((_, top), groups) in tree.iter_mut() {
        let top = top.as_str();
        egui::CollapsingHeader::new(RichText::new(top).strong())
            .id_salt((id, grouping, top))
            .default_open(true)
            .open(open)
            .show(ui, |ui| {
                for (group, subs) in groups.iter_mut() {
                    egui::CollapsingHeader::new(group.as_str()).id_salt((id, grouping, top, group.as_str())).open(open).show(ui, |ui| {
                        for (sub, list) in subs.iter_mut() {
                            if sub.is_empty() {
                                leaf(ui, list);
                            } else {
                                egui::CollapsingHeader::new(crate::portal::group_text(sub))
                                    .id_salt((id, grouping, top, group.as_str(), *sub))
                                    .open(open)
                                    .show(ui, |ui| leaf(ui, list));
                            }
                        }
                    });
                }
            });
    }
    if tree.is_empty() {
        ui.weak(tr("No parts found."));
    }
    activated
}

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    ui.end_row();
}

/// A typed value edited as text (holes, cutouts): kept as typed while it cannot be
/// read, shown in red.
fn typed_text_row(ui: &mut egui::Ui, part: &str, label: &str, data: &mut MountingData, key: &str, hint: &str) {
    ui.label(label);
    let id = ui.make_persistent_id(("mounting_data", part, key));
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| data.get(key).unwrap_or_default());
    let ok = text.trim().is_empty() || MountingData::default().set(key, &text);
    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY).hint_text(hint).text_color_opt((!ok).then_some(Color32::from_rgb(220, 60, 60))));
    let ok = text.trim().is_empty() || MountingData::default().set(key, &text);
    if r.changed() && ok {
        data.set(key, &text);
    }
    if r.has_focus() || !ok {
        ui.data_mut(|d| d.insert_temp(id, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    ui.end_row();
}

const HOLES_HINT: &str = "M4: 6,8; 42,8";
const CUTOUT_HINT: &str = "Ø22.5; 92 x 92 @ 0/-6.5 R3 15°";

/// Typed mounting data: clearances, holes, cutouts, power loss, fastening.
fn mounting_data(ui: &mut egui::Ui, part: &mut Part) {
    let number = part.number.clone();
    let moved = part.tech_data.lines().filter(|l| l.split_once(':').is_some_and(|(k, _)| MOUNTING_KEYS.iter().any(|m| m.eq_ignore_ascii_case(k.trim())) || k.trim().eq_ignore_ascii_case("Clearance"))).count();
    if moved > 0 && ui.button(tr_fmt!("Take {} values from the technical data", moved)).on_hover_text(tr("Moves lines such as Power loss: 3 W into the typed fields")).clicked() {
        part.migrate_tech_data();
    }
    let m = &mut part.mounting_data;
    egui::Grid::new("pmd").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        num_row(ui, tr("Clearance top"), &mut m.clearance_top, " mm");
        num_row(ui, tr("Clearance bottom"), &mut m.clearance_bottom, " mm");
        num_row(ui, tr("Clearance left"), &mut m.clearance_left, " mm");
        num_row(ui, tr("Clearance right"), &mut m.clearance_right, " mm");
        num_row(ui, tr("Clearance front"), &mut m.clearance_front, " mm");
        num_row(ui, tr("EMC clearance"), &mut m.emc_clearance, " mm");
        num_row(ui, tr("Service space"), &mut m.service_space, " mm");
        num_row(ui, tr("Outside the face"), &mut m.outside, " mm");
        num_row(ui, tr("Power loss"), &mut m.power_loss, " W");
        typed_text_row(ui, &number, tr("Mounting holes"), m, "Mounting holes", HOLES_HINT);
        typed_text_row(ui, &number, tr("Door cutout"), m, "Door cutout", CUTOUT_HINT);
        typed_text_row(ui, &number, tr("Cutout"), m, "Cutout", CUTOUT_HINT);
        text_row(ui, tr("Fastening"), &mut m.fastening);
        text_row(ui, tr("Faces"), &mut m.faces);
        text_row(ui, tr("Collision class"), &mut m.collision_class);
        text_row(ui, tr("Terminal type"), &mut m.terminal_type);
        ui.label(tr("Wall mounting"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut m.wall_mount, tr("Mounted directly on the wall or ceiling of a room"));
            if m.wall_mount {
                wall_badge(ui);
            }
        });
        ui.end_row();
        ui.label(tr("Floor mounting"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut m.floor_mount, tr("Stands on the floor of a room"));
            if m.floor_mount {
                floor_badge(ui);
            }
        });
        ui.end_row();
    });
    // The preview above the tables: side by side they are wider than the window.
    ui.add_space(6.0);
    mounting_preview(ui, part, vec2(240.0, 200.0));
    ui.weak(tr("Where the cutouts are: the centre from the middle of the front view (x to the right, y down), the turn counter-clockwise."));
    ui.label(RichText::new(tr("Cutouts")).strong());
    cutout_table(ui, "pcut", &mut part.mounting_data.cutouts);
    ui.label(RichText::new(tr("Door cutouts")).strong().color(DOOR_CUTOUT)).on_hover_text(tr("In the door, when they differ from the cutouts"));
    cutout_table(ui, "pdcut", &mut part.mounting_data.door_cutouts);
}

fn cutout_size(v: &mut f32) -> egui::DragValue<'_> {
    egui::DragValue::new(v).speed(0.5).range(0.1..=5000.0).suffix(" mm")
}

/// The cutouts of a list with their shape, size, place, turn and corner radius.
fn cutout_table(ui: &mut egui::Ui, id: &str, list: &mut Vec<PlacedCutout>) {
    let mut remove = None;
    if !list.is_empty() {
        egui::Grid::new(id).num_columns(8).striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
            for h in [tr("Shape"), tr("Width / Ø"), tr("Height"), "X", "Y", tr("Angle"), tr("Corner radius"), ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (i, c) in list.iter_mut().enumerate() {
                let mut round = matches!(c.shape, Cutout::Round { .. });
                let was = round;
                egui::ComboBox::from_id_salt((id, "shape", i)).selected_text(if round { tr("Round") } else { tr("Rectangular") }).width(90.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut round, true, tr("Round"));
                    ui.selectable_value(&mut round, false, tr("Rectangular"));
                });
                if round != was {
                    let [w, h] = c.size();
                    c.shape = if round { Cutout::Round { d: w.min(h) } } else { Cutout::Rect { w, h } };
                }
                match &mut c.shape {
                    Cutout::Round { d } => {
                        ui.add(cutout_size(d));
                        ui.label("");
                    }
                    Cutout::Rect { w, h } => {
                        ui.add(cutout_size(w));
                        ui.add(cutout_size(h));
                    }
                }
                for v in &mut c.offset {
                    ui.add(egui::DragValue::new(v).speed(0.5).range(-5000.0..=5000.0).suffix(" mm"));
                }
                if round {
                    ui.label("");
                    ui.label("");
                } else {
                    ui.add(egui::DragValue::new(&mut c.angle).speed(1.0).range(-360.0..=360.0).suffix("°"));
                    ui.add(egui::DragValue::new(&mut c.radius).speed(0.25).range(0.0..=1000.0).suffix(" mm"));
                }
                if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if icons::text_button(ui, Icon::Plus, tr("Add cutout")).clicked() {
        list.push(PlacedCutout::centered(Cutout::Rect { w: 50.0, h: 50.0 }));
    }
}

/// Colour of the door cutouts in the previews.
const DOOR_CUTOUT: Color32 = Color32::from_rgb(220, 120, 30);

/// The cutouts of a part (dashed) and its own door cutouts (dotted, orange) in a front
/// view drawn with `map` (part mm to screen).
fn paint_cutouts(painter: &egui::Painter, part: &Part, map: &dyn Fn([f32; 2]) -> Pos2, color: Color32) {
    let (w, h) = (part.width, part.height);
    let lists = [(part.cutouts(false), color, 5.0), (part.mounting_data.door_cutouts.clone(), DOOR_CUTOUT, 2.0)];
    for (list, color, dash) in lists {
        for c in list {
            let mut pts: Vec<Pos2> = c.outline(w, h).into_iter().map(map).collect();
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.2, color), dash, 3.0));
        }
    }
}

/// Front view of a part with its mounting holes and cutouts.
fn mounting_preview(ui: &mut egui::Ui, part: &Part, size: egui::Vec2) {
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let visuals = ui.visuals();
    painter.rect_filled(resp.rect, 4.0, visuals.extreme_bg_color);
    let Some([w, h, _]) = part.size() else {
        painter.text(resp.rect.center(), egui::Align2::CENTER_CENTER, tr("no mounting data"), egui::FontId::proportional(12.0), visuals.weak_text_color());
        return;
    };
    // Holes and cutouts may lie beyond the outline (lugs of rail devices).
    let mut lo = [0.0f32, 0.0f32];
    let mut hi = [w, h];
    let mut include = |q: [f32; 2]| {
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    };
    part.mounting_data.holes.iter().for_each(|q| include(*q));
    part.cutouts(false).iter().chain(&part.mounting_data.door_cutouts).flat_map(|c| c.outline(w, h)).for_each(&mut include);
    let zoom = ((size.x - 24.0) / (hi[0] - lo[0])).min((size.y - 24.0) / (hi[1] - lo[1]));
    let origin = resp.rect.center() - vec2(lo[0] + hi[0], lo[1] + hi[1]) * zoom / 2.0;
    let map = |q: [f32; 2]| origin + vec2(q[0], q[1]) * zoom;
    let ink = visuals.text_color();
    painter.rect_filled(egui::Rect::from_two_pos(map([0.0, 0.0]), map([w, h])), 0.0, visuals.widgets.inactive.bg_fill);
    painter.rect_stroke(egui::Rect::from_two_pos(map([0.0, 0.0]), map([w, h])), 0.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
    let accent = Color32::from_rgb(40, 110, 230);
    paint_cutouts(&painter, part, &map, accent);
    let hole: f32 = part.mounting_data.hole_thread.trim_start_matches(['Ø', 'M']).replace(',', ".").parse().unwrap_or(4.0);
    for q in &part.mounting_data.holes {
        painter.circle_stroke(map(*q), (hole / 2.0 * zoom).max(2.0), Stroke::new(1.2, ink));
    }
}

/// Marking of parts that stand on the floor of a room (part trees, editor, summary).
pub fn floor_badge(ui: &mut egui::Ui) {
    let color = Color32::from_rgb(120, 100, 40);
    ui.label(RichText::new(tr("FLOOR")).small().strong().color(Color32::WHITE).background_color(color))
        .on_hover_text(tr("Stands on the floor of a room, like pumps, tanks and compressors"));
}

/// Marking of parts that are mounted directly on a room wall (part trees, editor,
/// summary).
pub fn wall_badge(ui: &mut egui::Ui) {
    let color = Color32::from_rgb(40, 140, 120);
    ui.label(RichText::new(tr("WALL")).small().strong().color(Color32::WHITE).background_color(color))
        .on_hover_text(tr("Can be mounted directly on the wall or ceiling of a room, without a rail"));
}

fn num_row(ui: &mut egui::Ui, label: &str, value: &mut f32, suffix: &str) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).speed(0.1).range(0.0..=1.0e7).suffix(suffix));
    ui.end_row();
}

/// Tabbed editor for one part. `all_numbers` are offered as accessories, `duplicate`
/// marks the part number as already used. Returns a requested model import.
pub fn part_editor(ui: &mut egui::Ui, part: &mut Part, state: &mut EditorState, all_numbers: &[String], duplicate: bool) -> Option<ModelKind> {
    let mut request = None;
    ui.horizontal_wrapped(|ui| {
        for (t, name) in [
            (PartTab::General, tr("General")),
            (PartTab::Commercial, tr("Prices / other")),
            (PartTab::Technical, tr("Technical data")),
            (PartTab::Mounting, tr("Mounting data")),
            (PartTab::Functions, tr("Function template")),
            (PartTab::Connections, tr("Connection points")),
            (PartTab::Accessories, tr("Accessories")),
            (PartTab::Graphics, tr("Graphics")),
            (PartTab::Portal, tr("Data portal")),
        ] {
            ui.selectable_value(&mut state.tab, t, name);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("part_editor").auto_shrink([false, false]).show(ui, |ui| match state.tab {
        PartTab::General => {
            egui::Grid::new("pg").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Part number"));
                ui.vertical(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut part.number).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                    if duplicate {
                        ui.colored_label(Color32::from_rgb(220, 60, 60), tr("Part number already exists"));
                    }
                });
                ui.end_row();
                text_row(ui, tr("Type number"), &mut part.type_number);
                text_row(ui, tr("Order number"), &mut part.order_number);
                text_row(ui, tr("Manufacturer"), &mut part.manufacturer);
                text_row(ui, tr("Supplier"), &mut part.supplier);
                ui.label(tr("Description"));
                ui.add(egui::TextEdit::multiline(&mut part.description).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label(tr("Product top group"));
                egui::ComboBox::from_id_salt("top").selected_text(tr(part.top_group.title())).show_ui(ui, |ui| {
                    for t in TopGroup::ALL {
                        ui.selectable_value(&mut part.top_group, t, tr(t.title()));
                    }
                });
                ui.end_row();
                ui.label(tr("Product group"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut part.group).desired_width(200.0));
                    let own: Vec<&'static str> = PRODUCT_GROUPS.iter().map(|(g, _)| *g).collect();
                    group_choice(ui, "grp", &mut part.group, &own, productgroups::GROUPS);
                });
                ui.end_row();
                ui.label(tr("Product subgroup"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut part.subgroup).desired_width(200.0));
                    let own = PRODUCT_GROUPS.iter().find(|(g, _)| *g == part.group).map(|(_, s)| *s).unwrap_or(&[]);
                    group_choice(ui, "sub", &mut part.subgroup, own, productgroups::SUBGROUPS);
                });
                ui.end_row();
                ui.label(tr("Identifier"));
                ui.add(egui::TextEdit::singleline(&mut part.identifier).desired_width(50.0))
                    .on_hover_text(tr("Identifying letter of devices using this part (K, Q, S …)"));
                ui.end_row();
                ui.label(tr("Note"));
                ui.add(egui::TextEdit::multiline(&mut part.note).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
                if !part.picture.is_empty() {
                    ui.label(tr("Picture"));
                    ui.horizontal(|ui| {
                        part_picture(ui, part, 120.0);
                        if icons::button(ui, Icon::Delete, tr("Remove picture")).clicked() {
                            part.picture.clear();
                        }
                    });
                    ui.end_row();
                }
            });
        }
        PartTab::Commercial => {
            egui::Grid::new("pc").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Price"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut part.price).speed(0.1).range(0.0..=1.0e7).fixed_decimals(2));
                    ui.add(egui::TextEdit::singleline(&mut part.currency).desired_width(40.0));
                });
                ui.end_row();
                ui.label(tr("Price unit"));
                ui.add(egui::DragValue::new(&mut part.price_unit).range(1..=100000)).on_hover_text(tr("The price refers to this many units"));
                ui.end_row();
                ui.label(tr("Packaging quantity"));
                ui.add(egui::DragValue::new(&mut part.pack_quantity).range(1..=100000));
                ui.end_row();
                ui.label(tr("Quantity unit"));
                ui.add(egui::TextEdit::singleline(&mut part.quantity_unit).desired_width(60.0));
                ui.end_row();
                text_row(ui, tr("Certificates"), &mut part.certificates);
                text_row(ui, tr("Datasheet (URL)"), &mut part.datasheet);
            });
            if openecraft_parts::api::is_web_link(&part.datasheet) {
                ui.hyperlink_to(tr("Open datasheet"), &part.datasheet);
            }
        }
        PartTab::Technical => {
            egui::Grid::new("pt").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                text_row(ui, tr("Voltage"), &mut part.voltage);
                text_row(ui, tr("Current"), &mut part.current);
                text_row(ui, tr("Power"), &mut part.power);
                ui.label(tr("Technical data"));
                ui.add(egui::TextEdit::multiline(&mut part.tech_data).desired_rows(5).desired_width(f32::INFINITY));
                ui.end_row();
            });
        }
        PartTab::Mounting => {
            egui::Grid::new("pm").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                num_row(ui, tr("Width"), &mut part.width, " mm");
                num_row(ui, tr("Height"), &mut part.height, " mm");
                num_row(ui, tr("Depth"), &mut part.depth, " mm");
                num_row(ui, tr("Weight"), &mut part.weight, " kg");
                text_row(ui, tr("Mounting"), &mut part.mounting);
            });
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Mounting data")).strong());
            mounting_data(ui, part);
        }
        PartTab::Functions => {
            ui.weak(tr("Functions the part provides. Used for device selection, the contact image (reserve contacts), the project check and to assign connection designations. Designations are listed in symbol connection order, separated by blanks."));
            ui.add_space(4.0);
            let mut remove = None;
            egui::Grid::new("pf").num_columns(4).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label(RichText::new(tr("Function")).strong());
                ui.label(RichText::new(tr("Connection designations")).strong());
                ui.label(RichText::new(tr("Description")).strong());
                ui.label("");
                ui.end_row();
                for (i, f) in part.functions.iter_mut().enumerate() {
                    egui::ComboBox::from_id_salt(("fk", i)).selected_text(tr(f.kind.title())).width(170.0).show_ui(ui, |ui| {
                        for k in FnKind::ALL {
                            ui.selectable_value(&mut f.kind, k, tr(k.title()));
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut f.pins).desired_width(140.0).font(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut f.description).desired_width(160.0));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                part.functions.remove(i);
            }
            if icons::text_button(ui, Icon::Plus, tr("Add function")).clicked() {
                part.functions.push(FunctionTemplate::new(FnKind::No, ""));
            }
        }
        PartTab::Accessories => {
            ui.weak(tr("Accessories only work together with the parts they fit. A part lists its accessories, an accessory the parts it fits; either side is enough."));
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Accessories of this part")).strong());
            let mut remove = None;
            for (i, a) in part.accessories.iter().enumerate() {
                ui.horizontal(|ui| {
                    let known = all_numbers.contains(a);
                    ui.label(RichText::new(a).monospace().color(if known { ui.visuals().text_color() } else { Color32::from_rgb(220, 60, 60) }));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                part.accessories.remove(i);
            }
            let mut add = None;
            egui::ComboBox::from_id_salt("acc").selected_text(tr("➕ Add accessory")).width(240.0).show_ui(ui, |ui| {
                for n in all_numbers {
                    if *n != part.number && !part.accessories.contains(n) && ui.selectable_label(false, n).clicked() {
                        add = Some(n.clone());
                    }
                }
            });
            if let Some(n) = add {
                part.accessories.push(n);
            }
            ui.separator();
            ui.checkbox(&mut part.accessory, tr("This part is an accessory")).on_hover_text(tr("It only works attached to one of the parts it fits; the project check reports it on a device without such a part"));
            if part.accessory {
                ui.horizontal(|ui| {
                    ui.label(tr("Mounted"));
                    egui::ComboBox::from_id_salt("accmount").selected_text(tr(part.accessory_mount.title())).show_ui(ui, |ui| {
                        for m in openecraft_parts::AccessoryMount::ALL {
                            ui.selectable_value(&mut part.accessory_mount, m, tr(m.title()));
                        }
                    });
                })
                .response
                .on_hover_text(tr("Where the accessory sits on the part it fits, in the panel layout and the 3D view"));
                ui.label(RichText::new(tr("Fits these parts")).strong());
                let mut remove = None;
                for (i, n) in part.fits.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let known = all_numbers.contains(n);
                        ui.label(RichText::new(n).monospace().color(if known { ui.visuals().text_color() } else { Color32::from_rgb(220, 60, 60) }));
                        if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    part.fits.remove(i);
                }
                let mut add = None;
                egui::ComboBox::from_id_salt("accfits").selected_text(tr("➕ Add part")).width(240.0).show_ui(ui, |ui| {
                    for n in all_numbers {
                        if *n != part.number && !part.fits.contains(n) && ui.selectable_label(false, n).clicked() {
                            add = Some(n.clone());
                        }
                    }
                });
                if let Some(n) = add {
                    part.fits.push(n);
                }
            }
            ui.separator();
            ui.label(RichText::new(tr("Components (assembly)")).strong()).on_hover_text(tr("Parts this part is assembled from; the bill of materials lists them under it"));
            let mut remove = None;
            egui::Grid::new("pcomp").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                for (i, c) in part.components.iter_mut().enumerate() {
                    let known = all_numbers.contains(&c.number);
                    ui.label(RichText::new(&c.number).monospace().color(if known { ui.visuals().text_color() } else { Color32::from_rgb(220, 60, 60) }));
                    ui.add(egui::DragValue::new(&mut c.quantity).speed(0.1).range(0.0..=100000.0).prefix("× "));
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                part.components.remove(i);
            }
            let mut add = None;
            egui::ComboBox::from_id_salt("comp").selected_text(tr("➕ Add component")).width(240.0).show_ui(ui, |ui| {
                for n in all_numbers {
                    if *n != part.number && !part.components.iter().any(|c| &c.number == n) && ui.selectable_label(false, n).clicked() {
                        add = Some(n.clone());
                    }
                }
            });
            if let Some(n) = add {
                part.components.push(Component { number: n, quantity: 1.0 });
            }
        }
        PartTab::Connections => connections_tab(ui, part, &mut state.point),
        PartTab::Graphics => request = graphics_tab(ui, part, &mut state.model_scale),
        PartTab::Portal => portal_tab(ui, part, all_numbers),
    });
    request
}

/// Menu next to a group field: OpenECraft's suggestions, then the EPLAN product
/// groups with their numbers. Names are stored in English and shown translated.
fn group_choice(ui: &mut egui::Ui, id: &str, value: &mut String, own: &[&'static str], eplan: &[(u16, &'static str)]) {
    egui::ComboBox::from_id_salt(id).selected_text("…").width(24.0).height(420.0).show_ui(ui, |ui| {
        for &g in own {
            if ui.selectable_label(value == g, tr(g)).clicked() {
                *value = g.to_string();
            }
        }
        ui.menu_button(tr("EPLAN product groups"), |ui| {
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                for &(n, g) in eplan.iter().filter(|(n, _)| *n != 0) {
                    if ui.selectable_label(value == g, format!("{n:>3}  {}", tr(g))).clicked() {
                        *value = g.to_string();
                        ui.close();
                    }
                }
            });
        });
    });
}

/// Data portal: catalog, lifecycle and successor; server bookkeeping and files are
/// shown read only (files are managed with the parts manager).
fn portal_tab(ui: &mut egui::Ui, part: &mut Part, all_numbers: &[String]) {
    egui::Grid::new("pp").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        text_row(ui, tr("Catalog"), &mut part.catalog);
        ui.label(tr("Lifecycle"));
        egui::ComboBox::from_id_salt("life").selected_text(tr(part.lifecycle.title())).show_ui(ui, |ui| {
            for l in Lifecycle::ALL {
                ui.selectable_value(&mut part.lifecycle, l, tr(l.title()));
            }
        });
        ui.end_row();
        ui.label(tr("Successor"));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut part.successor).desired_width(200.0).font(egui::TextStyle::Monospace));
            egui::ComboBox::from_id_salt("succ").selected_text("…").width(24.0).height(320.0).show_ui(ui, |ui| {
                for n in all_numbers.iter().filter(|n| **n != part.number) {
                    if ui.selectable_label(part.successor == *n, n).clicked() {
                        part.successor = n.clone();
                    }
                }
            });
        });
        ui.end_row();
        let c = productgroups::classify(part);
        ui.label(tr("EPLAN numbers"));
        ui.label(tr_fmt!("Top group {}, product group {}, subgroup {}", c.top_group, c.group, c.subgroup))
            .on_hover_text(tr("Internal values of the EPLAN part exchange (0: undefined)"));
        ui.end_row();
        if part.revision > 0 {
            ui.label(tr("Server revision"));
            ui.label(tr_fmt!("{} (changed {})", part.revision, date_from_unix(part.modified)));
            ui.end_row();
        }
    });
    if !part.successor.is_empty() && !all_numbers.contains(&part.successor) {
        ui.colored_label(Color32::from_rgb(220, 140, 40), tr("The successor is not in this parts list"));
    }
    if !part.attachments.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Files")).strong());
        attachment_list(ui, &part.attachments);
    }
}

/// Files of a part: kind, name and size.
pub fn attachment_list(ui: &mut egui::Ui, attachments: &[Attachment]) {
    egui::Grid::new(ui.next_auto_id()).num_columns(3).striped(true).show(ui, |ui| {
        for a in attachments {
            ui.label(tr(a.kind.title()));
            ui.label(&a.name);
            ui.label(RichText::new(size_text(a.size)).weak());
            ui.end_row();
        }
    });
}

/// File size as "12 kB" / "3.4 MB".
pub fn size_text(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} kB", bytes.div_ceil(1000).max(1))
    }
}

fn exit_title(exit: Exit) -> &'static str {
    match exit {
        Exit::Up => tr("up"),
        Exit::Down => tr("down"),
        Exit::Left => tr("left"),
        Exit::Right => tr("right"),
        Exit::Front => tr("to the front"),
    }
}

/// Where the wires are connected in the panel layout.
fn connections_tab(ui: &mut egui::Ui, part: &mut Part, selected: &mut Option<usize>) {
    ui.weak(tr("Where the wires are connected in the panel layout: the designation as in the function template, the position in the front view (x from the left, y from the top, z above the mounting surface) and the direction the wire leaves in. Parts without own connection points get them from the function template."));
    ui.add_space(4.0);
    let own = !part.connection_points.is_empty();
    if selected.is_some_and(|i| i >= part.connection_points.len()) {
        *selected = None;
    }
    let shown = if own { part.connection_points.clone() } else { default_connection_points(part) };
    ui.horizontal(|ui| {
        let clicked = connection_preview(ui, part, &shown, if own { *selected } else { None }, vec2(240.0, 220.0));
        if let (Some([x, y]), Some(c)) = (clicked, selected.and_then(|i| part.connection_points.get_mut(i))) {
            c.pos[0] = x;
            c.pos[1] = y;
        }
        ui.vertical(|ui| {
            if own {
                ui.label(tr_fmt!("{} connection points", part.connection_points.len()));
                ui.weak(tr("Select a row, then click into the view to place it."));
            } else {
                ui.weak(tr("No own connection points – the generated ones are shown."));
            }
            if ui
                .button(tr("Generate from function template"))
                .on_hover_text(tr("Replaces the connection points by the IEC convention: odd designations at the top, even ones at the bottom"))
                .clicked()
            {
                part.connection_points = default_connection_points(part);
                *selected = None;
            }
            if own && icons::text_button(ui, Icon::Delete, tr("Remove all")).on_hover_text(tr("Use generated connection points again")).clicked() {
                part.connection_points.clear();
                *selected = None;
            }
        });
    });
    if own {
        let mut remove = None;
        egui::Grid::new("pcp").num_columns(10).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("");
            for h in [tr("Designation"), "X", "Y", "Z", tr("Wire leaves")] {
                ui.label(RichText::new(h).strong());
            }
            ui.label(RichText::new(tr("Min. mm²")).strong()).on_hover_text(tr("Smallest conductor the clamp takes (0: not known)"));
            ui.label(RichText::new(tr("Max. mm²")).strong()).on_hover_text(tr("Largest conductor the clamp takes (0: not known)"));
            ui.label(RichText::new(tr("Wires")).strong()).on_hover_text(tr("Most wires the clamp takes (0: not known)"));
            ui.label("");
            ui.end_row();
            for (i, c) in part.connection_points.iter_mut().enumerate() {
                if ui.radio(*selected == Some(i), "").on_hover_text(tr("Place by clicking into the view")).clicked() {
                    *selected = Some(i);
                }
                ui.add(egui::TextEdit::singleline(&mut c.pin).desired_width(60.0).font(egui::TextStyle::Monospace));
                for v in &mut c.pos {
                    ui.add(egui::DragValue::new(v).speed(0.5).range(0.0..=5000.0).suffix(" mm"));
                }
                egui::ComboBox::from_id_salt(("cpx", i)).selected_text(exit_title(c.exit)).width(110.0).show_ui(ui, |ui| {
                    for e in Exit::ALL {
                        ui.selectable_value(&mut c.exit, e, exit_title(e));
                    }
                });
                ui.add(egui::DragValue::new(&mut c.min_cross_section).speed(0.05).range(0.0..=1000.0).max_decimals(2));
                ui.add(egui::DragValue::new(&mut c.max_cross_section).speed(0.05).range(0.0..=1000.0).max_decimals(2));
                ui.add(egui::DragValue::new(&mut c.max_wires).range(0..=99));
                if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = remove {
            part.connection_points.remove(i);
            *selected = None;
        }
    }
    if icons::text_button(ui, Icon::Plus, tr("Add connection point")).clicked() {
        let [w, _, d] = part.size().unwrap_or_default();
        part.connection_points.push(ConnectionPoint { pin: String::new(), pos: [w / 2.0, 0.0, d * 0.75], exit: Exit::Up, ..Default::default() });
        *selected = Some(part.connection_points.len() - 1);
    }
}

/// Front view of the part with its connection points. Returns the clicked position
/// in part coordinates (mm, rounded to 0.5 mm).
fn connection_preview(ui: &mut egui::Ui, part: &Part, points: &[ConnectionPoint], selected: Option<usize>, size: egui::Vec2) -> Option<[f32; 2]> {
    let (resp, painter) = ui.allocate_painter(size, Sense::click());
    let visuals = ui.visuals();
    painter.rect_filled(resp.rect, 4.0, visuals.extreme_bg_color);
    let Some([w, h, _]) = part.size() else {
        painter.text(resp.rect.center(), egui::Align2::CENTER_CENTER, tr("no mounting data"), egui::FontId::proportional(12.0), visuals.weak_text_color());
        return None;
    };
    let zoom = ((size.x - 24.0) / w).min((size.y - 24.0) / h);
    let origin = resp.rect.center() - vec2(w, h) * zoom / 2.0;
    let map = |q: [f32; 2]| origin + vec2(q[0], q[1]) * zoom;
    let ink = visuals.text_color();
    painter.rect_filled(egui::Rect::from_two_pos(map([0.0, 0.0]), map([w, h])), 0.0, visuals.widgets.inactive.bg_fill);
    if let Some(m) = part.model_2d.as_ref().filter(|m| m.width > 0.0 && m.height > 0.0) {
        // The front view is fitted to the mounting data, like in the panel layout.
        let (sx, sy) = (w / m.width, h / m.height);
        paint_model_2d(&painter, m, &|q| map([q[0] * sx, q[1] * sy]), zoom * sx.min(sy), Stroke::new(1.0, ink.gamma_multiply(0.6)));
    }
    painter.rect_stroke(egui::Rect::from_two_pos(map([0.0, 0.0]), map([w, h])), 0.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
    paint_cutouts(&painter, part, &map, ink.gamma_multiply(0.5));
    let accent = Color32::from_rgb(40, 110, 230);
    let highlight = Color32::from_rgb(230, 40, 200);
    for (i, c) in points.iter().enumerate() {
        let color = if Some(i) == selected { highlight } else { accent };
        let at = map([c.pos[0], c.pos[1]]);
        let dir = match c.exit {
            Exit::Up => vec2(0.0, -1.0),
            Exit::Down => vec2(0.0, 1.0),
            Exit::Left => vec2(-1.0, 0.0),
            Exit::Right => vec2(1.0, 0.0),
            Exit::Front => vec2(0.0, 0.0),
        };
        painter.line_segment([at, at + dir * 10.0], Stroke::new(1.5, color));
        painter.circle_filled(at, if Some(i) == selected { 4.5 } else { 3.0 }, color);
        let label_at = if c.exit == Exit::Front { at + vec2(0.0, -9.0) } else { at - dir * 9.0 };
        painter.text(label_at, egui::Align2::CENTER_CENTER, &c.pin, egui::FontId::monospace(10.0), ink);
    }
    let pos = resp.interact_pointer_pos().filter(|_| resp.clicked())?;
    let q = (pos - origin) / zoom;
    let round = |v: f32, max: f32| ((v * 2.0).round() / 2.0).clamp(0.0, max);
    Some([round(q.x, w), round(q.y, h)])
}

/// 2D front view and 3D model of a part.
fn graphics_tab(ui: &mut egui::Ui, part: &mut Part, model_scale: &mut f32) -> Option<ModelKind> {
    let mut request = None;
    ui.weak(tr("The 2D front view is used in the panel front view and on mounting layout pages, the 3D model in the 3D panel view. Both are fitted to the mounting data (width × height × depth)."));
    ui.add_space(4.0);
    ui.label(RichText::new(tr("2D front view")).strong());
    ui.horizontal(|ui| {
        model_2d_preview(ui, part.model_2d.as_deref(), vec2(180.0, 140.0));
        ui.vertical(|ui| {
            match &part.model_2d {
                Some(m) => ui.label(tr_fmt!("{} × {} mm, {} elements", format!("{:.1}", m.width), format!("{:.1}", m.height), m.prims.len())),
                None => ui.weak(tr("none – a plain rectangle is drawn")),
            };
            if ui.button(tr("Import SVG…")).clicked() {
                request = Some(ModelKind::Front2d);
            }
            if part.model_2d.is_some() && icons::text_button(ui, Icon::Delete, tr("Remove")).clicked() {
                part.model_2d = None;
            }
        });
    });
    ui.separator();
    ui.label(RichText::new(tr("3D model")).strong());
    ui.horizontal(|ui| {
        match &part.model_3d {
            Some(m) => mesh_preview(ui, m, vec2(180.0, 160.0)),
            None => {
                ui.allocate_space(vec2(180.0, 40.0));
            }
        }
        ui.vertical(|ui| {
            match &part.model_3d {
                Some(m) => {
                    let s = m.size();
                    ui.label(tr_fmt!("{} × {} × {} mm, {} triangles", format!("{:.1}", s[0]), format!("{:.1}", s[1]), format!("{:.1}", s[2]), m.triangles.len()));
                }
                None => {
                    ui.weak(tr("none – a box of the mounting dimensions is drawn"));
                }
            }
            if ui.button(tr("Import STL / OBJ / STEP / IGES / IFC / JT…")).clicked() {
                request = Some(ModelKind::Mesh3d);
            }
            if part.model_3d.is_some() && icons::text_button(ui, Icon::Delete, tr("Remove")).clicked() {
                part.model_3d = None;
            }
        });
    });
    if let Some(m) = &mut part.model_3d {
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Rotate 90°:"));
            for (axis, label) in [(0, "X"), (1, "Y"), (2, "Z")] {
                if ui.button(label).on_hover_text(tr("Turn the model so that its front faces the viewer (+Z) and its top is +Y")).clicked() {
                    m.make_mut().rotate90(axis);
                }
            }
            ui.separator();
            ui.label(tr("Scale ×"));
            ui.add(egui::DragValue::new(model_scale).speed(0.01).range(0.001..=10000.0));
            if ui.button(tr("Apply")).on_hover_text(tr("e.g. 25.4 for models in inch, 1000 for models in metres")).clicked() {
                m.make_mut().scale(*model_scale);
            }
        });
        let s = m.size();
        let differs = [(s[0], part.width), (s[1], part.height), (s[2], part.depth)].iter().any(|(a, b)| (a - b).abs() > 0.5);
        if differs {
            ui.colored_label(
                Color32::from_rgb(220, 140, 0),
                tr_fmt!(
                    "The model ({} × {} × {}) differs from the mounting data ({} × {} × {}); it is stretched to the mounting data.",
                    format!("{:.1}", s[0]),
                    format!("{:.1}", s[1]),
                    format!("{:.1}", s[2]),
                    format!("{:.1}", part.width),
                    format!("{:.1}", part.height),
                    format!("{:.1}", part.depth)
                ),
            );
            if ui.button(tr("Use model size as mounting data")).clicked() {
                part.width = (s[0] * 10.0).round() / 10.0;
                part.height = (s[1] * 10.0).round() / 10.0;
                part.depth = (s[2] * 10.0).round() / 10.0;
            }
        }
    }
    request
}

/// Parses a model file and assigns it to the part. Returns a status message.
pub fn apply_model(part: &mut Part, kind: ModelKind, name: &str, bytes: &[u8]) -> Result<String, String> {
    Ok(assign_model(part, name, read_model(kind, name, bytes)?))
}

/// A model read from a file: a 2D front view or a 3D model.
pub enum Model {
    Front2d(Model2d),
    Mesh3d(Mesh3d),
}

/// Parses the model file `name` (STEP, IGES and Parasolid files are tessellated, which
/// can take long: the editor does it on another thread).
pub fn read_model(kind: ModelKind, name: &str, bytes: &[u8]) -> Result<Model, String> {
    match kind {
        ModelKind::Front2d => Ok(Model::Front2d(parse_svg(&String::from_utf8_lossy(bytes))?)),
        ModelKind::Mesh3d => Ok(Model::Mesh3d(parse_model_3d(short_name(name), bytes)?)),
    }
}

/// Assigns a model read from the file `name` to the part. Returns a status message.
pub fn assign_model(part: &mut Part, name: &str, model: Model) -> String {
    let file = short_name(name);
    let number = &part.number;
    match model {
        Model::Front2d(m) => {
            let msg = tr_fmt!("{}: 2D front view with {} elements assigned to {}", file, m.prims.len(), number);
            part.model_2d = Some(m.into());
            msg
        }
        Model::Mesh3d(m) => {
            let s = m.size();
            let msg = tr_fmt!("{}: 3D model with {} triangles ({} × {} × {} mm) assigned to {}", file, m.triangles.len(), format!("{:.1}", s[0]), format!("{:.1}", s[1]), format!("{:.1}", s[2]), number);
            // Parts without mounting data take the model size.
            if part.width <= 0.0 || part.height <= 0.0 {
                part.width = s[0];
                part.height = s[1];
                part.depth = s[2];
            }
            part.model_3d = Some(m.into());
            msg
        }
    }
}

/// Scaled drawing of a 2D front view.
pub fn model_2d_preview(ui: &mut egui::Ui, model: Option<&Model2d>, size: egui::Vec2) {
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let visuals = ui.visuals();
    painter.rect_filled(resp.rect, 4.0, visuals.extreme_bg_color);
    let Some(m) = model.filter(|m| m.width > 0.0 && m.height > 0.0) else {
        painter.text(resp.rect.center(), egui::Align2::CENTER_CENTER, tr("no 2D model"), egui::FontId::proportional(12.0), visuals.weak_text_color());
        return;
    };
    let zoom = ((size.x - 12.0) / m.width).min((size.y - 12.0) / m.height);
    let origin = resp.rect.center() - vec2(m.width, m.height) * zoom / 2.0;
    let map = |q: [f32; 2]| origin + vec2(q[0], q[1]) * zoom;
    painter.rect_filled(egui::Rect::from_two_pos(map([0.0, 0.0]), map([m.width, m.height])), 0.0, visuals.widgets.inactive.bg_fill);
    paint_model_2d(&painter, m, &map, zoom, Stroke::new(1.0, visuals.text_color()));
}

/// Draws the elements of a 2D front view; `map` turns model mm into screen positions.
fn paint_model_2d(painter: &egui::Painter, m: &Model2d, map: &dyn Fn([f32; 2]) -> Pos2, zoom: f32, line: Stroke) {
    let ink = line.color;
    for prim in &m.prims {
        match prim {
            Prim2d::Line { a, b } => {
                painter.line_segment([map(*a), map(*b)], line);
            }
            Prim2d::Rect { min, max, fill } => {
                let r = egui::Rect::from_two_pos(map(*min), map(*max));
                if *fill {
                    painter.rect_filled(r, 0.0, ink);
                } else {
                    painter.rect_stroke(r, 0.0, line, egui::StrokeKind::Middle);
                }
            }
            Prim2d::Circle { c, r, fill } => {
                let r = (r * zoom).max(1.0);
                if *fill {
                    painter.circle_filled(map(*c), r, ink);
                } else {
                    painter.circle_stroke(map(*c), r, line);
                }
            }
            Prim2d::Polyline { pts, closed, fill } => {
                let v: Vec<Pos2> = pts.iter().map(|q| map(*q)).collect();
                painter.add(match (*closed, *fill) {
                    (true, true) => Shape::convex_polygon(v, ink, Stroke::NONE),
                    (true, false) => Shape::closed_line(v, line),
                    _ => Shape::line(v, line),
                });
            }
            Prim2d::Text { pos, text, size } => {
                let px = size * zoom * 1.25;
                if px >= 3.0 {
                    painter.text(map(*pos), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(px), ink);
                }
            }
        }
    }
}

/// Small shaded view of a mesh; drag to rotate.
pub fn mesh_preview(ui: &mut egui::Ui, mesh: &Mesh3d, size: egui::Vec2) {
    let (resp, painter) = ui.allocate_painter(size, Sense::drag());
    let id = resp.id.with("angles");
    let (mut yaw, mut pitch): (f32, f32) = ui.data(|d| d.get_temp(id)).unwrap_or((0.6, 0.35));
    if resp.dragged() {
        yaw -= resp.drag_delta().x * 0.01;
        pitch = (pitch + resp.drag_delta().y * 0.01).clamp(-1.5, 1.5);
        ui.data_mut(|d| d.insert_temp(id, (yaw, pitch)));
    }
    let dark = ui.visuals().dark_mode;
    painter.rect_filled(resp.rect, 4.0, if dark { Color32::from_rgb(30, 32, 36) } else { Color32::from_rgb(232, 235, 240) });
    let s = mesh.size();
    let (min, _) = mesh.bounds();
    let c = [min[0] + s[0] / 2.0, min[1] + s[1] / 2.0, min[2] + s[2] / 2.0];
    let (eye, right, up, fwd) = Orbit { yaw, pitch, dist: 1.0, target: c }.basis();
    let view = sub(eye, c); // direction towards the viewer
    let radius = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt().max(1e-3) / 2.0;
    let scale = size.x.min(size.y) * 0.45 / radius;
    let center = resp.rect.center();
    let light = norm([0.35, 0.75, 0.55]);
    // Painted back to front (depth: distance along `fwd`, larger is farther): small
    // pieces keep large faces from covering what is in front of them, nearer faces
    // are a little lighter, and the sharp edges are drawn over their faces.
    let limit = radius * 0.12;
    let depth_of = |q: [f32; 3]| dot(sub(q, c), fwd);
    let screen = |q: [f32; 3]| {
        let d = sub(q, c);
        center + vec2(dot(d, right), -dot(d, up)) * scale
    };
    let normals: Vec<[f32; 3]> = mesh.triangles.iter().map(|t| {
        let v = t.map(|i| mesh.vertices[i as usize]);
        norm(cross(sub(v[1], v[0]), sub(v[2], v[0])))
    }).collect();
    let facing: Vec<bool> = normals.iter().map(|n| dot(*n, view) > 1e-6).collect();
    let mut items: Vec<(f32, [Pos2; 3], Option<Color32>)> = Vec::new();
    let mut pieces = Vec::new();
    for (k, t) in mesh.triangles.iter().enumerate() {
        if !facing[k] {
            continue;
        }
        let shade = 0.5 + 0.5 * dot(normals[k], light).max(0.0);
        pieces.clear();
        split_triangle(t.map(|i| mesh.vertices[i as usize]), limit, &mut pieces);
        for v in &pieces {
            let depth = v.iter().map(|q| depth_of(*q)).sum::<f32>() / 3.0;
            let near = 14.0 * (0.5 - depth / (2.0 * radius));
            let color = match mesh.color(k) {
                // A painted mesh in its colours, shaded the same way.
                Some(rgb) => {
                    let [r, g, b] = rgb.map(|v| (v as f32 * (0.35 + 0.75 * shade) + near).clamp(0.0, 255.0) as u8);
                    Color32::from_rgb(r, g, b)
                }
                None => {
                    let g = (180.0 * shade + near).clamp(0.0, 240.0) as u8;
                    Color32::from_rgb(g, g.saturating_add(6), g.saturating_add(14))
                }
            };
            items.push((depth, v.map(screen), Some(color)));
        }
    }
    for (a, b, faces) in sharp_edges(mesh, &normals) {
        if !faces.iter().any(|&f| facing[f]) {
            continue;
        }
        let n = ((dot(sub(a, b), sub(a, b)).sqrt() / limit).ceil() as usize).clamp(1, 64);
        for i in 0..n {
            let at = |t: f32| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t);
            let (p, q) = (at(i as f32 / n as f32), at((i + 1) as f32 / n as f32));
            let (sp, sq) = (screen(p), screen(q));
            items.push(((depth_of(p) + depth_of(q)) / 2.0 - limit * 0.6, [sp, sq, sq], None));
        }
    }
    items.sort_by(|a, b| b.0.total_cmp(&a.0));
    let ink = if dark { Color32::from_rgba_unmultiplied(10, 12, 16, 220) } else { Color32::from_rgba_unmultiplied(40, 46, 58, 200) };
    // One mesh without feathering: the pieces of a face meet without seams; edges are
    // thin quads in the same order.
    let mut out = egui::Mesh::default();
    for (_, pts, color) in items {
        let base = out.vertices.len() as u32;
        match color {
            Some(color) => {
                for q in pts {
                    out.colored_vertex(q, color);
                }
                out.add_triangle(base, base + 1, base + 2);
            }
            None if (pts[1] - pts[0]).length() > 1e-3 => {
                let d = pts[1] - pts[0];
                let n = vec2(-d.y, d.x).normalized() * 0.6;
                for q in [pts[0] - n, pts[0] + n, pts[1] + n, pts[1] - n] {
                    out.colored_vertex(q, ink);
                }
                out.add_triangle(base, base + 1, base + 2);
                out.add_triangle(base, base + 2, base + 3);
            }
            None => {}
        }
    }
    painter.add(Shape::mesh(out));
    if resp.hovered() {
        painter.text(resp.rect.left_bottom() + vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, tr("Drag to turn the model"), egui::FontId::proportional(10.0), Color32::GRAY);
    }
}

/// An edge of a mesh: both ends and the triangles it belongs to.
type Edge = ([f32; 3], [f32; 3], Vec<usize>);

/// Sharp edges of a mesh with the triangles they belong to: at the border or between
/// faces at more than 20°, found by position so that meshes without shared vertices
/// (STL) work too; none for very large meshes.
fn sharp_edges(mesh: &Mesh3d, normals: &[[f32; 3]]) -> Vec<Edge> {
    if mesh.triangles.len() > 20_000 {
        return Vec::new();
    }
    let key = |v: [f32; 3]| v.map(|x| (x * 100.0).round() as i64);
    let mut edges: std::collections::HashMap<([i64; 3], [i64; 3]), Edge> = std::collections::HashMap::new();
    for (k, t) in mesh.triangles.iter().enumerate() {
        for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (mesh.vertices[i as usize], mesh.vertices[j as usize]);
            let (ka, kb) = (key(a), key(b));
            let id = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(id).or_insert_with(|| (a, b, Vec::new())).2.push(k);
        }
    }
    edges.into_values().filter(|(_, _, f)| f.len() != 2 || dot(normals[f[0]], normals[f[1]]) < 0.94).collect()
}

/// Splits a triangle at the middle of its longest edge until no edge is longer than
/// `limit` (at most 64 pieces).
fn split_triangle(v: [[f32; 3]; 3], limit: f32, out: &mut Vec<[[f32; 3]; 3]>) {
    fn go(v: [[f32; 3]; 3], limit: f32, depth: u32, out: &mut Vec<[[f32; 3]; 3]>) {
        let len = |a: [f32; 3], b: [f32; 3]| dot(sub(a, b), sub(a, b));
        let edges = [len(v[0], v[1]), len(v[1], v[2]), len(v[2], v[0])];
        let k = (0..3).max_by(|&a, &b| edges[a].total_cmp(&edges[b])).unwrap_or(0);
        if depth >= 6 || edges[k] <= limit * limit {
            out.push(v);
            return;
        }
        let (a, b, c) = (v[k], v[(k + 1) % 3], v[(k + 2) % 3]);
        let m = [0, 1, 2].map(|i| (a[i] + b[i]) / 2.0);
        go([a, m, c], limit, depth + 1, out);
        go([m, b, c], limit, depth + 1, out);
    }
    go(v, limit, 0, out);
}

/// The quantity unit as shown: the default unit (pieces) in the user's language.
pub fn quantity_unit(unit: &str) -> &str {
    if unit == "pcs" { tr("pcs") } else { unit }
}

/// Most product pictures kept as textures; the ones shown longest ago are freed.
const PICTURES_KEPT: usize = 16;

/// The decoded product picture of a part as a texture, kept while it is among the last
/// [`PICTURES_KEPT`] shown.
fn picture_texture(ctx: &egui::Context, picture: &str) -> Option<egui::TextureHandle> {
    use std::hash::{Hash, Hasher};
    type Kept = Vec<(u64, Option<egui::TextureHandle>)>;
    let mut h = std::hash::DefaultHasher::new();
    picture.hash(&mut h);
    let key = h.finish();
    let id = egui::Id::unique("openecraft_part_pictures");
    let mut kept: Kept = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let texture = match kept.iter().position(|(k, _)| *k == key) {
        Some(i) => kept.remove(i).1,
        None => {
            let image = openecraft_parts::base64::decode(picture).and_then(|bytes| image::load_from_memory(&bytes).ok()).map(|img| {
                let rgba = img.to_rgba8();
                egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
            });
            image.map(|img| ctx.load_texture("part_picture", img, egui::TextureOptions::LINEAR))
        }
    };
    kept.push((key, texture.clone()));
    if kept.len() > PICTURES_KEPT {
        kept.remove(0);
    }
    ctx.data_mut(|d| d.insert_temp(id, kept));
    texture
}

/// The product picture of a part ([`Part::picture`]), at most `max` points wide and
/// high; nothing for parts without one.
pub fn part_picture(ui: &mut egui::Ui, part: &Part, max: f32) {
    if part.picture.is_empty() {
        return;
    }
    if let Some(texture) = picture_texture(ui.ctx(), &part.picture) {
        let size = texture.size_vec2();
        let scale = (max / size.x.max(size.y)).min(1.0);
        ui.add(egui::Image::new((texture.id(), size * scale)));
    }
}

/// The wires the connection points of a part take, e.g. `0.2 – 1.5 mm², 1 wire per
/// connection point`; empty without such data.
pub fn wire_summary(p: &Part) -> String {
    let limited: Vec<&ConnectionPoint> = p.connection_points.iter().filter(|c| c.has_wire_limits()).collect();
    if limited.is_empty() {
        return String::new();
    }
    let smallest = limited.iter().map(|c| c.min_cross_section).filter(|v| *v > 0.0).fold(f32::INFINITY, f32::min);
    let largest = limited.iter().map(|c| c.max_cross_section).fold(0.0, f32::max);
    let wires = limited.iter().map(|c| c.max_wires).max().unwrap_or(0);
    let mm2 = |v: f32| openecraft_i18n::decimal(v.into(), 2).trim_end_matches('0').trim_end_matches(['.', ',']).to_string();
    let range = match (smallest.is_finite(), largest > 0.0) {
        (true, true) => tr_fmt!("{} – {} mm²", mm2(smallest), mm2(largest)),
        (true, false) => tr_fmt!("from {} mm²", mm2(smallest)),
        (false, true) => tr_fmt!("up to {} mm²", mm2(largest)),
        (false, false) => String::new(),
    };
    match (range.is_empty(), wires) {
        (_, 0) => range,
        (true, n) => tr_fmt!("{} wires per connection point", n),
        (false, n) => tr_fmt!("{}, {} wires per connection point", range, n),
    }
}

/// The clearances of a part, e.g. `25 mm above, 25 mm below`; empty without any.
fn clearance_summary(md: &MountingData) -> String {
    let n = |v: f32| openecraft_i18n::decimal(v.into(), 1).trim_end_matches('0').trim_end_matches(['.', ',']).to_string();
    let mut sides = Vec::new();
    if md.clearance_top > 0.0 {
        sides.push(tr_fmt!("{} mm above", n(md.clearance_top)));
    }
    if md.clearance_bottom > 0.0 {
        sides.push(tr_fmt!("{} mm below", n(md.clearance_bottom)));
    }
    if md.clearance_left > 0.0 {
        sides.push(tr_fmt!("{} mm left", n(md.clearance_left)));
    }
    if md.clearance_right > 0.0 {
        sides.push(tr_fmt!("{} mm right", n(md.clearance_right)));
    }
    if md.clearance_front > 0.0 {
        sides.push(tr_fmt!("{} mm in front", n(md.clearance_front)));
    }
    sides.join(", ")
}

/// Read only summary of a part.
pub fn part_summary(ui: &mut egui::Ui, p: &Part) {
    ui.label(RichText::new(&p.number).monospace().strong());
    ui.label(&p.description);
    part_picture(ui, p, 200.0);
    egui::Grid::new("sum").num_columns(2).show(ui, |ui| {
        for (k, v) in [
            (tr("Manufacturer"), p.manufacturer.clone()),
            (tr("Type number"), p.type_number.clone()),
            (tr("Order number"), p.order_number.clone()),
            (tr("Product group"), format!("{} / {}", crate::portal::group_text(&p.group), crate::portal::group_text(&p.subgroup))),
            (tr("Voltage / current"), format!("{} {}", p.voltage, p.current)),
            (tr("Price"), if p.price > 0.0 { format!("{} {} / {} {}", openecraft_i18n::decimal(p.price.into(), 2), p.currency, p.price_unit, quantity_unit(&p.quantity_unit)) } else { String::new() }),
            (tr("Dimensions"), if p.width > 0.0 || p.height > 0.0 || p.depth > 0.0 { tr_fmt!("{} × {} × {} mm", p.width, p.height, p.depth) } else { String::new() }),
            (tr("Functions"), p.template_summary()),
            (tr("Accessories"), p.accessories.join(", ")),
            (tr("Fits"), p.fits.join(", ")),
            (tr("Lifecycle"), if p.lifecycle.is_active() { String::new() } else { tr(p.lifecycle.title()).to_string() }),
            (tr("Successor"), p.successor.clone()),
            (tr("Wall mounting"), if p.wall_mountable() { tr("yes").to_string() } else { String::new() }),
            (tr("Floor mounting"), if p.floor_standing() { tr("yes").to_string() } else { String::new() }),
            (tr("Mounting"), p.mounting.clone()),
            (tr("Mounting holes"), p.data_value("Mounting holes").unwrap_or_default()),
            (tr("Cutout"), p.data_value("Cutout").unwrap_or_default()),
            (tr("Door cutout"), p.data_value("Door cutout").unwrap_or_default()),
            (tr("Clearances"), clearance_summary(&p.mounting_data)),
            (tr("Wire connection"), wire_summary(p)),
        ] {
            if !v.trim().is_empty() && v.trim() != "/" {
                ui.label(RichText::new(k).weak());
                ui.label(v);
                ui.end_row();
            }
        }
    });
}

#[cfg(test)]
mod tests;
