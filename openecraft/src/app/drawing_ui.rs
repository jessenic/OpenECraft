//! Schematic editing beyond symbols and wires: drawing tools, layers, dimensions,
//! images, connection and cable definitions, property placement, and the
//! connection / cable / layer navigators.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use crate::editor::Tool;
use openecraft_core::graphics::{self, Layer};
use crate::io::Loaded;
use openecraft_core::model::*;
use openecraft_core::netlist::{junctions, pin_designation};
use openecraft_core::symbols::Anchor;
use openecraft_core::render::{self, Draw, Ink};
use openecraft_core::{cabledef, conndef, forms, ops, properties, props, structure, wirenum};
use egui::RichText;

/// A small T with the diagonal of a variant and the targets numbered as in the
/// connection symbols of EPLAN (the source port has no number).
fn junction_preview(ui: &mut egui::Ui, j: &Junction, selected: bool) -> egui::Response {
    use openecraft_core::geom::{p, P};
    let size = egui::vec2(44.0, 44.0);
    let (resp, painter) = ui.allocate_painter(size, egui::Sense::click());
    let rect = resp.rect;
    let stroke = if selected { ui.visuals().selection.stroke } else { ui.visuals().widgets.inactive.fg_stroke };
    painter.rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Inside);
    let c = rect.center();
    let k = 1.6;
    let at = |q: P| egui::pos2(c.x + q.x * k, c.y + q.y * k);
    struct Painter<'a>(&'a egui::Painter, &'a dyn Fn(P) -> egui::Pos2, egui::Color32);
    impl Draw for Painter<'_> {
        fn line(&mut self, a: P, b: P, width: f32, _ink: Ink, _dashed: bool) {
            self.0.line_segment([(self.1)(a), (self.1)(b)], egui::Stroke::new(width * 4.0, self.2));
        }
        fn poly(&mut self, _pts: &[P], _closed: bool, _width: f32, _ink: Ink, _fill: bool) {}
        fn circle(&mut self, c: P, r: f32, _width: f32, _ink: Ink, _fill: bool) {
            self.0.circle_filled((self.1)(c), r * 1.6, self.2);
        }
        fn text(&mut self, pos: P, text: &str, size: f32, _h: openecraft_core::symbols::Anchor, _v: openecraft_core::render::VAnchor, _ink: Ink) {
            self.0.text((self.1)(pos), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(size * 3.0), self.2);
        }
    }
    let color = stroke.color;
    let mut d = Painter(&painter, &at, color);
    let reach = render::JUNCTION_R * 4.0;
    for a in j.arms() {
        d.line(P::default(), a.vector() * reach, render::WIRE_W, Ink::Wire, false);
    }
    if j.is_corner() {
        return resp;
    }
    let flat = Junction { pos: p(0.0, 0.0), dot: false, ..j.clone() };
    render::junction_symbol(&mut d, &flat, Ink::Wire, true, false);
    // The targets numbered: of a T-junction and a bridge from their source; a crossing
    // has none, every end is a source.
    if j.kind != JunctionKind::Crossing {
        for (i, (_, target)) in j.links().into_iter().enumerate() {
            let v = target.vector();
            let side = p(-v.y, v.x) * 2.6;
            d.text(v * (reach - 2.0) + side, &(i + 1).to_string(), 2.2, openecraft_core::symbols::Anchor::Center, openecraft_core::render::VAnchor::Middle, Ink::Text);
        }
    }
    resp
}

/// Selection of the layer of an item ("" is the default layer).
fn layer_combo(ui: &mut egui::Ui, salt: &str, layer: &mut String, names: &[String]) {
    let shown = if layer.is_empty() { tr("(default)").to_string() } else { layer.clone() };
    egui::ComboBox::from_id_salt(salt).selected_text(shown).show_ui(ui, |ui| {
        ui.selectable_value(layer, String::new(), tr("(default)"));
        for n in names.iter().filter(|n| !n.is_empty()) {
            ui.selectable_value(layer, n.clone(), n);
        }
    });
}

/// Own colour of an item (none: the layer's or the normal colour).
pub(super) fn color_field(ui: &mut egui::Ui, color: &mut Option<[u8; 3]>) {
    ui.horizontal(|ui| {
        let mut own = color.is_some();
        if ui.checkbox(&mut own, tr("Own colour")).changed() {
            *color = own.then_some([200, 30, 30]);
        }
        if let Some(c) = color {
            ui.color_edit_button_srgb(c);
        }
    });
}

fn anchor_title(a: Anchor) -> &'static str {
    match a {
        Anchor::Left => tr("Left"),
        Anchor::Center => tr("Centred"),
        Anchor::Right => tr("Right"),
    }
}

fn valign_title(v: VAnchor) -> &'static str {
    match v {
        VAnchor::Top => tr("Top"),
        VAnchor::Middle => tr("Middle"),
        VAnchor::Bottom => tr("Bottom"),
    }
}

/// Properties of a free text: content with placeholders, height, rotation,
/// alignment, wrapping, style, frame, hyperlink, layer and path function text.
fn text_properties(ui: &mut egui::Ui, t: &mut TextItem, names: &[String]) {
    ui.label(RichText::new(if t.path_function { tr("Path function text") } else { tr("Text") }).strong());
    ui.add(egui::TextEdit::multiline(&mut t.text).desired_rows(3));
    egui::ComboBox::from_id_salt("tph")
        .selected_text(tr("Insert placeholder…"))
        .width(200.0)
        .show_ui(ui, |ui| {
            for (ph, meaning) in forms::PLACEHOLDERS.iter().filter(|(ph, _)| *ph != "{{ }}") {
                if ui.button(format!("{}  {}", tr(meaning), ph)).clicked() {
                    t.text.push_str(ph);
                }
            }
        })
        .response
        .on_hover_text(tr("Special text: project and page properties such as {page} or {prop:Name} are filled in when the page is drawn"));
    egui::Grid::new("txtprops").num_columns(2).show(ui, |ui| {
        ui.label(tr("Height (mm)"));
        ui.add(egui::DragValue::new(&mut t.size).range(1.0..=50.0).speed(0.1));
        ui.end_row();
        ui.label(tr("Angle"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut t.angle).range(0.0..=359.9).speed(1.0).suffix(" °"));
            for a in [0.0, 90.0, 180.0, 270.0] {
                ui.selectable_value(&mut t.angle, a, format!("{a:.0}°"));
            }
        });
        ui.end_row();
        ui.label(tr("Alignment"));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("talign").selected_text(anchor_title(t.align)).show_ui(ui, |ui| {
                for a in [Anchor::Left, Anchor::Center, Anchor::Right] {
                    ui.selectable_value(&mut t.align, a, anchor_title(a));
                }
            });
            egui::ComboBox::from_id_salt("tvalign").selected_text(valign_title(t.valign)).show_ui(ui, |ui| {
                for v in [VAnchor::Top, VAnchor::Middle, VAnchor::Bottom] {
                    ui.selectable_value(&mut t.valign, v, valign_title(v));
                }
            });
        });
        ui.end_row();
        ui.label(tr("Wrap width (mm)"));
        ui.add(egui::DragValue::new(&mut t.width).range(0.0..=400.0).speed(1.0)).on_hover_text(tr("0: no wrapping"));
        ui.end_row();
        ui.label(tr("Style"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut t.bold, tr("Bold"));
            ui.checkbox(&mut t.italic, tr("Italic"));
            ui.checkbox(&mut t.underline, tr("Underline"));
        });
        ui.end_row();
        ui.label(tr("Frame"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut t.frame, tr("Border"));
            ui.checkbox(&mut t.background, tr("Background")).on_hover_text(tr("Covers what is behind the text"));
        });
        ui.end_row();
        ui.label(tr("Link"));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut t.link).desired_width(150.0));
            if openecraft_parts::api::is_web_link(&t.link) {
                if ui.button(tr("Open link")).on_hover_text(tr("Ctrl + click on the text opens it as well")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(t.link.trim().to_string()));
                }
            } else if !t.link.trim().is_empty() {
                ui.colored_label(crate::style::warning(), tr("Only web and e-mail addresses (http, https, mailto) are links"));
            }
        });
        ui.end_row();
        ui.label(tr("Layer"));
        layer_combo(ui, "tlayer", &mut t.layer, names);
        ui.end_row();
    });
    ui.checkbox(&mut t.path_function, tr("Path function text"))
        .on_hover_text(tr("Function text of the functions in this column that have none of their own (device list, terminal, plug and PLC diagrams)"));
    color_field(ui, &mut t.color);
}

impl App {
    /// An image file was picked: it is inserted at the next click.
    pub(super) fn load_image(&mut self, ctx: &egui::Context, loaded: &Loaded) {
        let name = crate::io::short_name(&loaded.name).to_string();
        let Some(px) = graphics::image_pixels(&loaded.bytes) else {
            self.notify(ctx, tr_fmt!("{} is not a PNG or JPEG image", name));
            return;
        };
        let size = graphics::image_size(px, 60.0);
        let img = ImageItem {
            a: openecraft_core::geom::p(0.0, 0.0),
            b: size,
            file: openecraft_core::document::Document::new(&name, &loaded.bytes),
            px,
            layer: self.editor.active_layer.clone(),
            ..Default::default()
        };
        self.editor.set_tool(Tool::Insert);
        let mut clip = ops::Clip::new();
        clip.images.push(img);
        self.editor.insert = Some(clip);
        self.editor.insert_macro = None;
        self.notify(ctx, tr("Click into the page to place the image"));
    }

    /// Properties of texts, graphics, dimensions, images and definitions. Returns
    /// false if `id` is none of them.
    pub(super) fn graphic_properties(&mut self, ui: &mut egui::Ui, id: u64) -> bool {
        let pi = self.session.page;
        let names = graphics::layer_names(&self.session.prj);
        let cables: Vec<(String, Vec<String>)> = self.session.prj.cables.iter().map(|c| (c.tag.clone(), c.core_list())).collect();
        let pg = &mut self.session.prj.pages[pi];
        if let Some(t) = pg.texts.iter_mut().find(|t| t.id == id) {
            text_properties(ui, t, &names);
            return true;
        }
        if let Some(s) = pg.shapes.iter_mut().find(|s| s.id == id) {
            ui.label(RichText::new(shape_title(s.kind)).strong());
            egui::Grid::new("shprops").num_columns(2).show(ui, |ui| {
                ui.label(tr("Line width"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut s.width).range(0.0..=5.0).speed(0.05).suffix(" mm"));
                    if s.width == 0.0 {
                        ui.weak(tr("normal"));
                    }
                });
                ui.end_row();
                ui.label(tr("Dashed"));
                ui.checkbox(&mut s.dashed, "");
                ui.end_row();
                if graphics::closed(s.kind) {
                    ui.label(tr("Filled"));
                    ui.checkbox(&mut s.fill, "");
                    ui.end_row();
                }
                if s.kind == ShapeKind::Arc {
                    ui.label(tr("Angle"));
                    ui.add(egui::DragValue::new(&mut s.sweep).range(-360.0..=360.0).suffix(" °"));
                    ui.end_row();
                }
                if s.kind == ShapeKind::Circle {
                    ui.label(tr("Radius"));
                    let mut r = s.a.dist(s.b);
                    if ui.add(egui::DragValue::new(&mut r).range(0.5..=500.0).speed(0.5).suffix(" mm")).changed() {
                        s.b = s.a + openecraft_core::geom::p(r, 0.0);
                    }
                    ui.end_row();
                }
                ui.label(tr("Layer"));
                layer_combo(ui, "slayer", &mut s.layer, &names);
                ui.end_row();
            });
            color_field(ui, &mut s.color);
            if matches!(s.kind, ShapeKind::Polyline | ShapeKind::Polygon) {
                ui.weak(tr_fmt!("{} corner points", s.pts.len()));
            }
            return true;
        }
        if let Some(d) = pg.dimensions.iter_mut().find(|d| d.id == id) {
            ui.label(RichText::new(tr("Dimension")).strong());
            egui::Grid::new("dimprops").num_columns(2).show(ui, |ui| {
                ui.label(tr("Direction"));
                egui::ComboBox::from_id_salt("dimkind").selected_text(dim_title(d.kind)).show_ui(ui, |ui| {
                    for k in [DimKind::Aligned, DimKind::Horizontal, DimKind::Vertical] {
                        ui.selectable_value(&mut d.kind, k, dim_title(k));
                    }
                });
                ui.end_row();
                ui.label(tr("Measured"));
                ui.label(format!("{:.2} mm", graphics::dim_length(d).abs()));
                ui.end_row();
                ui.label(tr("Scale"));
                ui.add(egui::DragValue::new(&mut d.scale).range(0.001..=10000.0).speed(0.1).prefix("× ")).on_hover_text(tr("Real length per millimetre on the page, e.g. 10 for a 1:10 drawing"));
                ui.end_row();
                ui.label(tr("Decimals"));
                ui.add(egui::DragValue::new(&mut d.decimals).range(0..=4));
                ui.end_row();
                ui.label(tr("Unit"));
                ui.add(egui::TextEdit::singleline(&mut d.unit).hint_text("mm").desired_width(60.0));
                ui.end_row();
                ui.label(tr("Text"));
                let measured = graphics::dim_text(&Dimension { text: String::new(), ..d.clone() });
                ui.add(egui::TextEdit::singleline(&mut d.text).hint_text(measured).desired_width(120.0))
                    .on_hover_text(tr("Shown instead of the measured value"));
                ui.end_row();
                ui.label(tr("Distance"));
                ui.add(egui::DragValue::new(&mut d.offset).speed(0.5).suffix(" mm"));
                ui.end_row();
                ui.label(tr("Layer"));
                layer_combo(ui, "dlayer", &mut d.layer, &names);
                ui.end_row();
            });
            return true;
        }
        if let Some(img) = pg.images.iter_mut().find(|i| i.id == id) {
            ui.label(RichText::new(tr("Image")).strong());
            ui.label(RichText::new(&img.file.name).monospace());
            ui.weak(tr_fmt!("{} × {} pixels, {}", img.px[0], img.px[1], openecraft_core::document::size_text(img.file.size())));
            let (mut w, mut h) = (img.b.x - img.a.x, img.b.y - img.a.y);
            egui::Grid::new("imgprops").num_columns(2).show(ui, |ui| {
                ui.label(tr("Width"));
                let cw = ui.add(egui::DragValue::new(&mut w).range(2.0..=1000.0).speed(0.5).suffix(" mm")).changed();
                ui.end_row();
                ui.label(tr("Height"));
                let ch = ui.add(egui::DragValue::new(&mut h).range(2.0..=1000.0).speed(0.5).suffix(" mm")).changed();
                ui.end_row();
                if cw {
                    h = graphics::image_size(img.px, w).y;
                } else if ch && img.px[1] > 0 {
                    w = h * img.px[0] as f32 / img.px[1] as f32;
                }
                if cw || ch {
                    img.b = img.a + openecraft_core::geom::p(w, h);
                }
                ui.label(tr("Layer"));
                layer_combo(ui, "ilayer", &mut img.layer, &names);
                ui.end_row();
            });
            ui.weak(tr("The aspect ratio is kept."));
            return true;
        }
        if pg.conn_defs.iter().any(|c| c.id == id) {
            self.conn_def_properties(ui, id, &cables);
            return true;
        }
        if pg.cable_defs.iter().any(|c| c.id == id) {
            self.cable_def_properties(ui, id, &cables);
            return true;
        }
        if pg.junctions.iter().any(|j| j.id == id) {
            self.junction_properties(ui, id);
            return true;
        }
        false
    }

    /// Direction, variant (with a preview of each) and representation of a T-junction,
    /// the orientation of a corner, and the wires of their connection.
    fn junction_properties(&mut self, ui: &mut egui::Ui, id: u64) {
        let pi = self.session.page;
        let pg = &self.session.prj.pages[pi];
        let Some(before) = pg.junctions.iter().find(|j| j.id == id).cloned() else { return };
        ui.label(RichText::new(junctions::kind_name(before.kind)).strong());
        let fits = junctions::fits(&self.session.prj, pg, &before);
        let mut j = before.clone();
        // The orientations of a corner, crossing or bridge as previews to pick from.
        let choices: Vec<Junction> = match j.kind {
            JunctionKind::Corner => [(TeeDir::Down, TeeDir::Right), (TeeDir::Down, TeeDir::Left), (TeeDir::Up, TeeDir::Right), (TeeDir::Up, TeeDir::Left)].into_iter().map(|(v, h)| Junction::corner(j.pos, v, h)).collect(),
            JunctionKind::Crossing => [(TeeDir::Up, 1), (TeeDir::Up, 2), (TeeDir::Left, 1), (TeeDir::Left, 2)].into_iter().map(|(dir, variant)| Junction { dir, variant, ..j.clone() }).collect(),
            JunctionKind::Bridge => TeeDir::ALL.into_iter().map(|dir| Junction { dir, ..j.clone() }).collect(),
            JunctionKind::Tee => Vec::new(),
        };
        if !choices.is_empty() {
            ui.label(if j.is_corner() { tr("Arms") } else { tr("Routing") });
            ui.horizontal(|ui| {
                for preview in choices {
                    let same = preview.links() == j.links();
                    if junction_preview(ui, &preview, same).on_hover_text(junctions::describe(&preview)).clicked() {
                        j = Junction { id: j.id, pos: j.pos, dot: j.dot, ..preview };
                    }
                }
            });
            if j.connects() {
                ui.checkbox(&mut j.dot, tr("Dot")).on_hover_text(tr("Drawn as a connection dot; the routing stays"));
            }
            ui.weak(junctions::describe(&j));
            if !fits {
                ui.colored_label(
                    crate::style::warning(),
                    if j.is_corner() {
                        tr("Two wires must meet here at a right angle. With Auto-connect on, connection points facing its arms are wired to it.")
                    } else {
                        tr("Wires must leave here in all four directions. Until then the connection is chained in schematic order.")
                    },
                );
            }
        } else {
            egui::Grid::new("tjprops").num_columns(2).show(ui, |ui| {
                ui.label(tr("Direction"));
                egui::ComboBox::from_id_salt("tjdir").selected_text(junctions::side(j.dir)).show_ui(ui, |ui| {
                    for d in TeeDir::ALL {
                        ui.selectable_value(&mut j.dir, d, junctions::side(d));
                    }
                });
                ui.end_row();
                ui.label(tr("Representation"));
                ui.checkbox(&mut j.dot, tr("Dot")).on_hover_text(tr("Drawn as a connection dot; the routing stays"));
                ui.end_row();
            });
            ui.label(tr("Routing"));
            ui.horizontal(|ui| {
                for v in junctions::VARIANTS {
                    let preview = Junction { variant: v, ..j.clone() };
                    let r = junction_preview(ui, &preview, j.variant == v).on_hover_text(junctions::describe(&preview));
                    if r.clicked() {
                        j.variant = v;
                    }
                }
            });
            ui.weak(junctions::describe(&j));
            if !fits {
                ui.colored_label(crate::style::warning(), tr("Three wires must meet here: one in its direction and two across. Until then the connection is chained in schematic order."));
            }
        }
        if j != before {
            if let Some(x) = self.session.prj.pages[pi].junctions.iter_mut().find(|x| x.id == id) {
                *x = j.clone();
            }
        }
        let an = &self.session.analysis;
        let conns = junctions::connections_of(&self.session.prj, &an.open, &an.connections, pi, &before);
        if !conns.is_empty() {
            ui.separator();
            ui.label(tr("Wires of the connection"));
            for i in conns {
                let c = &an.connections[i];
                ui.label(RichText::new(format!("{} – {}", pin_designation(&self.session.prj, c.a), pin_designation(&self.session.prj, c.b))).monospace());
            }
        }
    }

    fn conn_def_properties(&mut self, ui: &mut egui::Ui, id: u64, cables: &[(String, Vec<String>)]) {
        let pi = self.session.page;
        ui.label(RichText::new(tr("Connection definition point")).strong());
        let pg = &self.session.prj.pages[pi];
        let Some(before) = pg.conn_defs.iter().find(|c| c.id == id).cloned() else { return };
        let on_wire = conndef::wire_at(pg, before.pos).map(|w| w.id);
        if let Some(net) = on_wire.and_then(|w| self.session.analysis.open.net_of_wire(w)) {
            let mut v: Vec<String> = net.pins.iter().map(|p| pin_designation(&self.session.prj, *p)).collect();
            v.sort_by(|a, b| natural_cmp(a, b));
            v.dedup();
            ui.label(tr("Connection:"));
            ui.label(RichText::new(v.join(" – ")).monospace());
        } else {
            ui.colored_label(crate::style::lock(), tr("Not on a wire"));
        }
        let auto = on_wire.map(|w| openecraft_core::potentials::props_of_wire(&self.session.prj, &self.session.analysis.netlist, &self.session.analysis.open, w)).unwrap_or_default();
        let mut cd = before.clone();
        egui::Grid::new("cdprops").num_columns(2).show(ui, |ui| {
            ui.label(tr("Wire number"));
            ui.add(egui::TextEdit::singleline(&mut cd.number).hint_text(tr("not numbered")).desired_width(90.0));
            ui.end_row();
            super::potentials_ui::wire_fields(ui, &mut cd.color, &mut cd.cross_section, &auto);
            ui.label(tr("Cable"));
            let shown = if cd.cable.is_empty() { "(none)".to_string() } else { cd.cable.clone() };
            egui::ComboBox::from_id_salt("cdcable").selected_text(shown).show_ui(ui, |ui| {
                if ui.selectable_label(cd.cable.is_empty(), "(none)").clicked() {
                    cd.cable.clear();
                    cd.core.clear();
                }
                for (tag, _) in cables {
                    if ui.selectable_label(cd.cable == *tag, tag).clicked() && cd.cable != *tag {
                        cd.cable = tag.clone();
                        cd.core.clear();
                    }
                }
            });
            ui.end_row();
            if let Some((_, cores)) = cables.iter().find(|(t, _)| *t == cd.cable) {
                ui.label(tr("Core"));
                egui::ComboBox::from_id_salt("cdcore").selected_text(cd.core.clone()).show_ui(ui, |ui| {
                    for c in cores {
                        ui.selectable_value(&mut cd.core, c.clone(), c);
                    }
                });
                ui.end_row();
            }
            ui.label(tr("Description"));
            ui.text_edit_singleline(&mut cd.description);
            ui.end_row();
        });
        let reapply = ui.button(tr("Apply to the connection")).on_hover_text(tr("The wires of the connection take these values again")).clicked();
        let changed = cd != before;
        if let Some(c) = self.session.prj.pages[pi].conn_defs.iter_mut().find(|c| c.id == id) {
            *c = cd;
        }
        if changed || reapply {
            conndef::apply(&mut self.session.prj, &self.session.analysis.open, pi, id);
        }
    }

    fn cable_def_properties(&mut self, ui: &mut egui::Ui, id: u64, cables: &[(String, Vec<String>)]) {
        let pi = self.session.page;
        ui.label(RichText::new(tr("Cable definition")).strong());
        let Some(mut def) = self.session.prj.pages[pi].cable_defs.iter().find(|c| c.id == id).cloned() else { return };
        let crossed = cabledef::crossed_wires(&self.session.prj.pages[pi], def.a, def.b);
        let mut apply = false;
        egui::Grid::new("cbdprops").num_columns(2).show(ui, |ui| {
            ui.label(tr("Cable"));
            egui::ComboBox::from_id_salt("cbdcable").selected_text(def.cable.clone()).show_ui(ui, |ui| {
                for (tag, _) in cables {
                    if ui.selectable_label(def.cable == *tag, tag).clicked() && def.cable != *tag {
                        def.cable = tag.clone();
                        apply = true;
                    }
                }
            });
            ui.end_row();
            ui.label(tr("Shield"));
            ui.checkbox(&mut def.shield, "");
            ui.end_row();
            ui.label(tr("Crossed wires"));
            ui.label(crossed.len().to_string());
            ui.end_row();
        });
        if let Some(c) = self.session.prj.cables.iter_mut().find(|c| c.tag == def.cable) {
            ui.separator();
            egui::Grid::new("cbdcable_data").num_columns(2).show(ui, |ui| {
                ui.label(tr("Type"));
                ui.text_edit_singleline(&mut c.cable_type);
                ui.end_row();
                ui.label(tr("Cores"));
                ui.text_edit_singleline(&mut c.cores).on_hover_text(tr("Core designations separated by blanks"));
                ui.end_row();
                ui.label(tr("Cross-section"));
                ui.text_edit_singleline(&mut c.cross_section);
                ui.end_row();
                ui.label(tr("Length"));
                ui.add(egui::DragValue::new(&mut c.length).range(0.0..=10000.0).speed(0.1).suffix(" m"));
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr("Number cores")).on_hover_text(tr("Cores 1, 2, 3 … for the wires crossed, plus GNYE")).clicked() {
                    c.cores = cabledef::numbered_cores(crossed.len().max(1), true);
                    apply = true;
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Assign crossed wires")).on_hover_text(tr("The crossed wires get free cores of the cable in order")).clicked() {
                apply = true;
            }
            if ui.button(tr("Draw another line for this cable")).clicked() {
                self.editor.cable_target = def.cable.clone();
                self.editor.set_tool(Tool::CableDef);
            }
        });
        if let Some(c) = self.session.prj.pages[pi].cable_defs.iter_mut().find(|c| c.id == id) {
            *c = def;
        }
        if apply {
            let n = cabledef::apply(&mut self.session.prj, pi, id);
            let ctx = ui.ctx().clone();
            self.notify(&ctx, tr_fmt!("{} wires are cores of the cable", n));
        }
    }

    /// Tag position, placed properties and placement scheme of a symbol.
    pub(super) fn placement_properties(&mut self, ui: &mut egui::Ui, pi: usize, si: usize) {
        let symbols = self.session.prj.symbol_table();
        let s = &self.session.prj.pages[pi].symbols[si];
        let values: Vec<String> = s.props.iter().map(|pp| props::text(&self.session.prj, pi, s, pp)).collect();
        let titles: Vec<String> = s.props.iter().map(|pp| properties::def(&self.session.prj, &pp.key).map(|d| d.title_of(&pp.key)).unwrap_or_else(|| pp.key.clone())).collect();
        let schemes: Vec<(String, String)> = props::all_schemes(&self.session.prj).map(|x| (x.name.clone(), x.title())).collect();
        let classes = properties::Object::Function(s.id).classes(&self.session.prj);
        let classes: Vec<properties::ObjectClass> =
            [properties::ObjectClass::Device, properties::ObjectClass::Part].into_iter().filter(|_| !s.tag.is_empty()).chain(classes).chain([properties::ObjectClass::Page, properties::ObjectClass::Project]).collect();
        let open = !s.props.is_empty() || s.tag_offset.is_some();
        egui::CollapsingHeader::new(tr("Property placement")).id_salt("placement").default_open(open).show(ui, |ui| {
            let added = super::properties_ui::key_picker(ui, &self.session.prj, &classes, false, tr("➕ Place property"));
            let mut save_scheme = None;
            let mut apply_scheme = None;
            let s = &mut self.session.prj.pages[pi].symbols[si];
            ui.horizontal(|ui| {
                if s.tag_offset.is_some() {
                    ui.weak(tr("Tag placed by hand"));
                    if ui.small_button(tr("Reset")).clicked() {
                        s.tag_offset = None;
                    }
                } else {
                    ui.weak(tr("Drag the tag of the selected symbol to place it."));
                }
            });
            let mut remove = None;
            egui::Grid::new("pprops").num_columns(4).show(ui, |ui| {
                for (i, pp) in s.props.iter_mut().enumerate() {
                    ui.label(&titles[i]).on_hover_text(&pp.key);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut pp.size).range(1.0..=10.0).speed(0.1).suffix(" mm"));
                        super::properties_ui::anchor_combo(ui, ("ppanchor", i), &mut pp.anchor);
                        ui.checkbox(&mut pp.name, "").on_hover_text(tr("Shown as \"Name: value\""));
                    });
                    let v = values.get(i).cloned().unwrap_or_default();
                    if v.is_empty() {
                        ui.weak(tr("(empty)"));
                    } else {
                        ui.add(egui::Label::new(v).truncate());
                    }
                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                s.props.remove(i);
            }
            if let Some(k) = added {
                props::add(&symbols, s, &k);
            }
            ui.horizontal(|ui| {
                ui.label(tr("Scheme"));
                let shown = schemes.iter().find(|x| x.0 == s.scheme).map(|x| x.1.clone()).unwrap_or_else(|| tr("(none)").into());
                egui::ComboBox::from_id_salt("ppscheme").selected_text(shown).show_ui(ui, |ui| {
                    for (name, title) in &schemes {
                        if ui.selectable_label(*name == s.scheme, title).clicked() {
                            apply_scheme = Some(name.clone());
                        }
                    }
                });
                let id = ui.scope_id().with("new_scheme");
                let mut name: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
                ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("Scheme name")).desired_width(90.0));
                if ui.add_enabled(!s.props.is_empty() && !name.trim().is_empty(), egui::Button::new(tr("Save as scheme"))).on_hover_text(tr("Saves the placed properties as a placement scheme of the project")).clicked() {
                    save_scheme = Some(props::capture(&symbols, s, &name));
                    name.clear();
                }
                ui.data_mut(|d| d.insert_temp(id, name));
            });
            if let Some(name) = apply_scheme {
                let ids = [self.session.prj.pages[pi].symbols[si].id];
                if let Err(e) = props::apply_to(&mut self.session.prj, &name, &ids) {
                    self.notify_error(&ui.ctx().clone(), e);
                }
            }
            if let Some(scheme) = save_scheme {
                let name = scheme.name.clone();
                match props::define_scheme(&mut self.session.prj, scheme) {
                    Ok(()) => self.session.prj.pages[pi].symbols[si].scheme = name,
                    Err(e) => self.notify_error(&ui.ctx().clone(), e),
                }
            }
        });
    }

    // ------------------------------------------------------------------ navigators

    pub(super) fn layers_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::Plus, tr("Layer")).clicked() {
                let mut n = 1;
                while graphics::layer(&self.session.prj, &tr_fmt!("Layer {}", n)).is_some() {
                    n += 1;
                }
                self.session.prj.layers.push(Layer::new(&tr_fmt!("Layer {}", n)));
            }
        });
        ui.add(egui::Label::new(RichText::new(tr("Layers of graphics, texts, dimensions and images")).weak()).wrap());
        // Layers only used by items get a definition so they can be switched.
        for name in graphics::layer_names(&self.session.prj) {
            if graphics::layer(&self.session.prj, &name).is_none() {
                self.session.prj.layers.push(Layer::new(&name));
            }
        }
        let counts: Vec<usize> = self.session.prj.layers.iter().map(|l| graphics::layer_count(&self.session.prj, &l.name)).collect();
        let default_count = graphics::layer_count(&self.session.prj, "");
        let names: Vec<String> = self.session.prj.layers.iter().map(|l| l.name.clone()).collect();
        let error = crate::style::colors(self.dark).error;
        let mut rename: Option<(String, String)> = None;
        let mut refused: Option<&'static str> = None;
        let mut remove: Option<String> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("layers").num_columns(6).striped(true).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new(tr("Name")).strong());
                ui.label(RichText::new(tr("Visible")).strong());
                ui.label(RichText::new(tr("Locked")).strong());
                ui.label(RichText::new(tr("Print")).strong());
                ui.label("");
                ui.end_row();
                ui.radio_value(&mut self.editor.active_layer, String::new(), "").on_hover_text(tr("New items go onto this layer"));
                ui.label(tr("(default)"));
                ui.weak(default_count.to_string());
                ui.end_row();
                for (i, l) in self.session.prj.layers.iter_mut().enumerate() {
                    // Hidden or locked layers cannot take new items.
                    let usable = l.visible && !l.locked;
                    ui.add_enabled_ui(usable, |ui| ui.radio_value(&mut self.editor.active_layer, l.name.clone(), ""))
                        .inner
                        .on_hover_text(tr("New items go onto this layer"))
                        .on_disabled_hover_text(tr("A hidden or locked layer cannot take new items"));
                    // The name is edited in a scratch copy and renamed once on Enter or
                    // when the field is left.
                    let buffer = ui.make_persistent_id(("layer_name", i));
                    let mut name = ui.data(|d| d.get_temp::<String>(buffer)).unwrap_or_else(|| l.name.clone());
                    let problem = graphics::layer_rename_problem(&names, &l.name, &name);
                    let mut edit = egui::TextEdit::singleline(&mut name).desired_width(90.0);
                    if problem.is_some() {
                        edit = edit.text_color(error);
                    }
                    let r = ui.add(edit);
                    let r = match problem {
                        Some(p) => r.on_hover_text(tr(p)),
                        None => r,
                    };
                    if r.lost_focus() {
                        ui.data_mut(|d| d.remove::<String>(buffer));
                        match problem {
                            None if name != l.name => rename = Some((l.name.clone(), name)),
                            None => {}
                            Some(p) => refused = Some(p),
                        }
                    } else if r.has_focus() {
                        ui.data_mut(|d| d.insert_temp(buffer, name));
                    }
                    ui.checkbox(&mut l.visible, "");
                    ui.checkbox(&mut l.locked, "");
                    ui.checkbox(&mut l.print, "");
                    ui.horizontal(|ui| {
                        let mut c = l.color.is_some();
                        if ui.checkbox(&mut c, "").on_hover_text(tr("Colour of the items without their own")).changed() {
                            l.color = c.then_some([0, 120, 200]);
                        }
                        if let Some(col) = &mut l.color {
                            ui.color_edit_button_srgb(col);
                        }
                        ui.weak(counts[i].to_string()).on_hover_text(tr("Items on the layer"));
                        if icons::button(ui, Icon::Delete, tr("Delete the layer; its items move to the default layer")).clicked() {
                            remove = Some(l.name.clone());
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if let Some((old, new)) = rename {
            graphics::rename_layer(&mut self.session.prj, &old, &new);
            if self.editor.active_layer == old {
                self.editor.active_layer = new;
            }
        }
        if let Some(p) = refused {
            let ctx = ui.ctx().clone();
            self.notify_error(&ctx, tr(p));
        }
        if let Some(name) = remove {
            graphics::remove_layer(&mut self.session.prj, &name);
            if self.editor.active_layer == name {
                self.editor.active_layer.clear();
            }
        }
        // Selected items on hidden or locked layers are deselected.
        let page = &self.session.prj.pages[self.session.page];
        let prj = &self.session.prj;
        self.editor.selection.retain(|id| graphics::item_layer(page, *id).is_none_or(|l| graphics::shown(prj, l, false) && !graphics::locked(prj, l)));
    }

    pub(super) fn connections_tab(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::TextEdit::singleline(&mut self.connection_filter).hint_text(tr("Filter connections…")));
        let f = self.connection_filter.to_lowercase();
        // Number, connected points, first wire and all wires (page, id).
        type Row = (String, String, (usize, u64), Vec<(usize, u64)>);
        let mut rows: Vec<Row> = Vec::new();
        for net in self.session.analysis.open.nets.iter().filter(|n| !n.wires.is_empty()) {
            let number = wirenum::number_of(&self.session.prj, net);
            let mut pins: Vec<String> = net.pins.iter().map(|p| pin_designation(&self.session.prj, *p)).collect();
            pins.sort_by(|a, b| natural_cmp(a, b));
            pins.dedup();
            let text = pins.join(" – ");
            if !f.is_empty() && !number.to_lowercase().contains(&f) && !text.to_lowercase().contains(&f) && !net.name.to_lowercase().contains(&f) {
                continue;
            }
            rows.push((number, text, net.wires[0], net.wires.clone()));
        }
        rows.sort_by(|a, b| natural_cmp(&a.0, &b.0).then_with(|| natural_cmp(&a.1, &b.1)));
        ui.weak(tr_fmt!("{} connections", rows.len()));
        let mut jump = None;
        let mut select: Option<Vec<(usize, u64)>> = None;
        let mut define: Option<(usize, u64)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 18.0, rows.len(), |ui, range| {
            for (number, text, first, wires) in &rows[range] {
                let title = if number.is_empty() { format!("–  {text}") } else { format!("{number}  {text}") };
                let r = ui.add(egui::Label::new(RichText::new(title).monospace()).sense(egui::Sense::click()).truncate());
                if r.clicked() {
                    jump = Some(*first);
                }
                r.context_menu(|ui| {
                    if ui.button(tr("Show")).clicked() {
                        jump = Some(*first);
                    }
                    if ui.button(tr("Select wires on this page")).clicked() {
                        select = Some(wires.clone());
                    }
                    if ui.button(tr("Place connection definition point")).on_hover_text(tr("In the middle of the first wire")).clicked() {
                        define = Some(*first);
                    }
                });
            }
        });
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
        if let Some(wires) = select {
            self.editor.selection = wires.iter().filter(|w| w.0 == self.session.page).map(|w| w.1).collect();
        }
        if let Some((pg, wid)) = define {
            if let Some(w) = self.session.prj.pages[pg].wires.iter().find(|w| w.id == wid) {
                let mid = openecraft_core::geom::p((w.a.x + w.b.x) / 2.0, (w.a.y + w.b.y) / 2.0);
                let mut cd = conndef::adopt(&self.session.prj, &self.session.analysis.open, pg, mid);
                cd.id = self.session.prj.new_id();
                let id = cd.id;
                self.session.prj.pages[pg].conn_defs.push(cd);
                self.goto(pg, id);
            }
        }
    }

    pub(super) fn cables_tab(&mut self, ui: &mut egui::Ui) {
        let structure = structure::page_structure(&self.session.prj.pages[self.session.page]);
        ui.horizontal_wrapped(|ui| {
            if icons::text_button(ui, Icon::Plus, tr("New cable")).on_hover_text(tr("In the structure of the current page")).clicked() {
                let tag = ops::add_cable(&mut self.session.prj, &structure);
                self.editor.cable_target = tag;
            }
            if ui.button(tr("Cables…")).clicked() {
                self.show_cables = true;
            }
        });
        // Cores in use: cable -> (core, page, wire id)
        let mut cores: std::collections::HashMap<&str, Vec<(String, usize, u64)>> = Default::default();
        for (pi, pg) in self.session.prj.pages.iter().enumerate() {
            for w in pg.wires.iter().filter(|w| !w.cable.is_empty()) {
                cores.entry(w.cable.as_str()).or_default().push((w.core.clone(), pi, w.id));
            }
        }
        let defs: Vec<(String, usize, u64)> = self
            .session.prj
            .pages
            .iter()
            .enumerate()
            .flat_map(|(pi, pg)| pg.cable_defs.iter().map(move |c| (c.cable.clone(), pi, c.id)))
            .collect();
        let mut jump = None;
        let mut draw = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for c in &self.session.prj.cables {
                let used = cores.get(c.tag.as_str()).cloned().unwrap_or_default();
                let total = c.core_list().len();
                let head = format!("{}   {}/{}  {}", c.tag, used.len(), total, c.cable_type);
                let target = self.editor.cable_target == c.tag;
                let header = egui::CollapsingHeader::new(RichText::new(head).monospace().strong()).id_salt(("cable", &c.tag)).show(ui, |ui| {
                    if ui.selectable_label(target, tr("Draw cable definition")).on_hover_text(tr("Draw a line across the wires that are cores of this cable")).clicked() {
                        draw = Some(c.tag.clone());
                    }
                    let mut used = used.clone();
                    used.sort_by(|a, b| natural_cmp(&a.0, &b.0));
                    for (core, pi, wid) in &used {
                        let page = structure::page_full_name(&self.session.prj.pages[*pi]);
                        if ui.link(tr_fmt!("{}  ›  page {}", core, page)).clicked() {
                            jump = Some((*pi, *wid));
                        }
                    }
                    for free in c.core_list().iter().filter(|k| !used.iter().any(|u| &u.0 == *k)) {
                        ui.weak(tr_fmt!("{}  (free)", free));
                    }
                    for (_, pi, id) in defs.iter().filter(|d| d.0 == c.tag) {
                        let page = structure::page_full_name(&self.session.prj.pages[*pi]);
                        if ui.link(tr_fmt!("Cable definition on page {}", page)).clicked() {
                            jump = Some((*pi, *id));
                        }
                    }
                });
                header.header_response.context_menu(|ui| {
                    if ui.button(tr("Draw cable definition")).clicked() {
                        draw = Some(c.tag.clone());
                    }
                });
            }
            if self.session.prj.cables.is_empty() {
                ui.weak(tr("No cables yet. Draw a cable definition across wires or add a cable."));
            }
        });
        if let Some(tag) = draw {
            self.editor.cable_target = tag;
            self.editor.set_tool(Tool::CableDef);
        }
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
    }

    pub(super) fn device_action(&mut self, tag: &str, action: DeviceAction) {
        match action {
            DeviceAction::Select => {
                self.editor.selection = self.session.prj.pages[self.session.page].symbols.iter().filter(|s| s.tag == tag).map(|s| s.id).collect();
            }
            DeviceAction::Place => self.editor.place_tag = Some(tag.to_string()),
            DeviceAction::StopPlacing => self.editor.place_tag = None,
            DeviceAction::Rename => self.rename_device = Some((tag.to_string(), tag.to_string())),
        }
    }

    /// Dialog to rename a device (all its functions).
    pub(super) fn rename_device_window(&mut self, ctx: &egui::Context) {
        let Some((old, mut new)) = self.rename_device.take() else { return };
        let (window, esc) = self.dialogs.window(ctx, tr("Rename device"), "rename_device", None);
        let mut open = !esc;
        let mut done = false;
        window.open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(tr_fmt!("New tag for {}", old));
            let r = ui.text_edit_singleline(&mut new);
            let problem = rename_problem(&self.session.prj, &old, new.trim());
            if let Some(p) = &problem {
                ui.colored_label(crate::style::error(), p);
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ok = problem.is_none();
            ui.horizontal(|ui| {
                if (ui.add_enabled(ok, egui::Button::new(tr("Rename"))).clicked() || (enter && ok)) && new.trim() != old {
                    ops::rename_device(&mut self.session.prj, &old, new.trim());
                    done = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    done = true;
                }
            });
        });
        if open && !done {
            self.rename_device = Some((old, new));
        }
    }
}

/// Why a device cannot be renamed to `new` (None: it can).
pub(super) fn rename_problem(prj: &openecraft_core::model::Project, old: &str, new: &str) -> Option<String> {
    ops::rename_problem(prj, old, new)
}

/// What the context menu of a device in the navigator asks for.
#[derive(Clone, Copy)]
pub(super) enum DeviceAction {
    Select,
    Place,
    StopPlacing,
    Rename,
}

/// Context menu of a device in the device navigator.
pub(super) fn device_menu(ui: &mut egui::Ui, placing: bool) -> Option<DeviceAction> {
    let mut a = None;
    if ui.button(tr("Select on this page")).clicked() {
        a = Some(DeviceAction::Select);
    }
    if ui.button(tr("Place functions of this device")).on_hover_text(tr("Symbols placed next get this device tag (contacts are numbered on)")).clicked() {
        a = Some(DeviceAction::Place);
    }
    if placing && ui.button(tr("Stop placing for this device")).clicked() {
        a = Some(DeviceAction::StopPlacing);
    }
    if ui.button(tr("Rename…")).clicked() {
        a = Some(DeviceAction::Rename);
    }
    if a.is_some() {
        ui.close();
    }
    a
}

fn shape_title(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Line => tr("Graphic line"),
        ShapeKind::Rect => tr("Graphic rectangle"),
        ShapeKind::Circle => tr("Circle"),
        ShapeKind::Ellipse => tr("Ellipse"),
        ShapeKind::Arc => tr("Arc"),
        ShapeKind::Polyline => tr("Polyline"),
        ShapeKind::Polygon => tr("Polygon"),
    }
}

fn dim_title(kind: DimKind) -> &'static str {
    match kind {
        DimKind::Aligned => tr("Aligned"),
        DimKind::Horizontal => tr("Horizontal"),
        DimKind::Vertical => tr("Vertical"),
    }
}
