//! Symbol editor: user symbols of the project (graphics, connection points,
//! function), copies of built-in symbols and symbol library files.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use super::App;
use crate::editor::{EguiDraw, Palette, Tool};
use openecraft_core::geom::{p, P, GRID};
use crate::io::Purpose;
use openecraft_core::model::SymbolInst;
use openecraft_core::render::{self, Draw, Ink, VAnchor};
use openecraft_core::symbols::{self, Anchor, FieldKind, Func, Prim};
use openecraft_core::symlib::{self, Hit, SymbolLibrary, UserPin, UserSymbol, DRAW_SNAP};
use egui::{pos2, vec2, Key, PointerButton, RichText, Sense, Stroke};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymTool {
    #[default]
    Select,
    Line,
    Rect,
    Circle,
    Text,
    Pin,
    TagField,
    LabelField,
}

impl SymTool {
    const ALL: [(SymTool, &'static str, &'static str); 8] = [
        (SymTool::Select, tr_const!("⬉ Select"), tr_const!("Select and drag elements; Del deletes")),
        (SymTool::Line, tr_const!("— Line"), tr_const!("Click start and end point")),
        (SymTool::Rect, tr_const!("☐ Rect"), tr_const!("Click two corners")),
        (SymTool::Circle, tr_const!("○ Circle"), tr_const!("Click centre, then a point on the circle")),
        (SymTool::Text, tr_const!("T Text"), tr_const!("Click to place the text from the field below")),
        (SymTool::Pin, tr_const!("• Connection point"), tr_const!("Click on the 4 mm grid")),
        (SymTool::TagField, tr_const!("Tag field"), tr_const!("Position of the device tag (instead of the tag block)")),
        (SymTool::LabelField, tr_const!("Label field"), tr_const!("Position of the label (terminal number, address …)")),
    ];
}

pub struct SymbolEditor {
    pub open: bool,
    /// Key of the edited symbol.
    sel: Option<String>,
    tool: SymTool,
    start: Option<P>,
    hit: Option<Hit>,
    /// Pointer position of the running drag (symbol coordinates, snapped).
    drag: Option<P>,
    text: String,
    /// Pixels per millimetre.
    zoom: f32,
}

impl Default for SymbolEditor {
    fn default() -> Self {
        SymbolEditor { open: false, sel: None, tool: SymTool::Select, start: None, hit: None, drag: None, text: "Text".into(), zoom: 12.0 }
    }
}

impl SymbolEditor {
    /// True while a drag is running (the symbol is registered when it ends).
    pub fn busy(&self) -> bool {
        self.drag.is_some()
    }

    fn set_tool(&mut self, t: SymTool) {
        self.tool = t;
        self.start = None;
    }
}

enum Action {
    Import,
    Export,
    Place(String),
    Duplicate(usize),
    Delete(usize),
}

impl App {
    #[allow(clippy::too_many_lines)]
    pub(super) fn symbol_editor_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut action = None;
        let (prj, se) = (&mut self.session.prj, &mut self.symed);
        if se.sel.as_ref().map(|k| !prj.symbols.iter().any(|u| &u.key == k)).unwrap_or(true) {
            se.sel = prj.symbols.first().map(|u| u.key.clone());
            se.hit = None;
        }
        ui.scope(|ui| {
            ui.horizontal_top(|ui| {
                // ------------------------------------------------ symbol list
                ui.vertical(|ui| {
                    ui.set_width(180.0);
                    ui.horizontal_wrapped(|ui| {
                        if icons::text_button(ui, Icon::Plus, tr("New")).clicked() {
                            let mut u = UserSymbol::default();
                            u.key = symlib::new_key(&prj.symbols, &u.name);
                            u.prims.push(Prim::Poly { pts: vec![p(-4.0, -4.0), p(4.0, -4.0), p(4.0, 4.0), p(-4.0, 4.0)], closed: true, fill: false });
                            u.pins = vec![UserPin { pos: p(0.0, -8.0), label: "1".into(), internal: false }, UserPin { pos: p(0.0, 8.0), label: "2".into(), internal: false }];
                            u.prims.push(Prim::Line { a: p(0.0, -8.0), b: p(0.0, -4.0), dashed: false });
                            u.prims.push(Prim::Line { a: p(0.0, 4.0), b: p(0.0, 8.0), dashed: false });
                            se.sel = Some(u.key.clone());
                            prj.symbols.push(u);
                        }
                        ui.menu_button(tr("Copy built-in…"), |ui| {
                            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                                for cat in symbols::Category::ALL {
                                    ui.menu_button(cat.title(), |ui| {
                                        for def in symbols::library().iter().filter(|d| d.category == cat) {
                                            if ui.button(tr(def.name)).clicked() {
                                                let u = symlib::from_def(def, &prj.symbols);
                                                se.sel = Some(u.key.clone());
                                                prj.symbols.push(u);
                                                ui.close();
                                            }
                                        }
                                    });
                                }
                            });
                        });
                    });
                    ui.horizontal(|ui| {
                        if icons::text_button(ui, Icon::Upload, tr("Import…")).on_hover_text(tr("Merge a symbol library file")).clicked() {
                            action = Some(Action::Import);
                        }
                        if ui.add_enabled_ui(!prj.symbols.is_empty(), |ui| icons::text_button(ui, Icon::Download, tr("Export…"))).inner.on_hover_text(tr("All user symbols as library file")).clicked() {
                            action = Some(Action::Export);
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt("symlist").max_height(420.0).show(ui, |ui| {
                        let mut libs: Vec<&str> = prj.symbols.iter().map(|u| u.library.as_str()).collect();
                        libs.sort();
                        libs.dedup();
                        let mut pick = None;
                        for lib in libs {
                            ui.label(RichText::new(if lib.is_empty() { "(no library)" } else { lib }).strong());
                            for u in prj.symbols.iter().filter(|u| u.library == lib) {
                                let n = symlib::usage(prj, &u.key);
                                let text = if n > 0 { format!("{}  ({n}×)", u.name) } else { u.name.clone() };
                                if ui.selectable_label(se.sel.as_ref() == Some(&u.key), text).clicked() {
                                    pick = Some(u.key.clone());
                                }
                            }
                        }
                        if let Some(k) = pick {
                            se.sel = Some(k);
                            se.hit = None;
                            se.start = None;
                        }
                        if prj.symbols.is_empty() {
                            ui.weak(tr("No user symbols yet. Create one or copy a built-in symbol."));
                        }
                    });
                });
                ui.separator();
                let Some(si) = se.sel.as_ref().and_then(|k| prj.symbols.iter().position(|u| &u.key == k)) else {
                    return;
                };
                // The widgets take mutable values even when the user is only viewing the symbol.
                let mut edited = prj.symbols[si].clone();
                // ------------------------------------------------ canvas
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (t, label, tip) in SymTool::ALL {
                            if ui.selectable_label(se.tool == t, tr(label)).on_hover_text(tr(tip)).clicked() {
                                se.set_tool(t);
                            }
                        }
                    });
                    if se.tool == SymTool::Text {
                        ui.horizontal(|ui| {
                            ui.label(tr("Text"));
                            ui.text_edit_singleline(&mut se.text);
                        });
                    }
                    canvas(ui, se, &mut edited);
                    ui.weak(tr("Wheel: zoom · right click: cancel · graphics snap to 0.5 mm, connection points to the 4 mm grid"));
                });
                ui.separator();
                // ------------------------------------------------ properties
                ui.vertical(|ui| {
                    ui.set_width(270.0);
                    let usage = symlib::usage(prj, &prj.symbols[si].key);
                    let u = &mut edited;
                    egui::ScrollArea::vertical().id_salt("symprops").show(ui, |ui| {
                        symbol_properties(ui, u);
                        ui.separator();
                        element_properties(ui, se, u);
                        ui.separator();
                        for msg in symlib::problems(u) {
                            ui.colored_label(crate::style::lock(), format!("⚠ {msg}"));
                        }
                        ui.label(tr_fmt!("Placed {}× in the project", usage));
                        ui.horizontal(|ui| {
                            if ui.add_enabled_ui(symlib::problems(u).is_empty(), |ui| icons::text_button(ui, Icon::Markup, tr("Place in schematic"))).inner.clicked() {
                                action = Some(Action::Place(u.key.clone()));
                            }
                            if ui.button(tr("Duplicate")).clicked() {
                                action = Some(Action::Duplicate(si));
                            }
                            let del = ui.add_enabled_ui(usage == 0, |ui| icons::text_button(ui, Icon::Delete, tr("Delete"))).inner.on_disabled_hover_text(tr("The symbol is placed in the project"));
                            if del.clicked() {
                                action = Some(Action::Delete(si));
                            }
                        });
                    });
                });
                if edited != prj.symbols[si] {
                    prj.symbols[si] = edited;
                }
            });
        });
        match action {
            Some(Action::Import) => self.io.open(ctx, Purpose::Symbols),
            Some(Action::Export) => {
                let lib = SymbolLibrary { symbols: self.session.prj.symbols.to_vec() };
                let json = serde_json::to_string_pretty(&lib).unwrap_or_default();
                self.export(ctx, "symbols.json".into(), (tr("OpenECraft symbol library"), "json"), json);
            }
            Some(Action::Place(key)) => {
                if let Some(def) = self.session.prj.symbol(&key) {
                    self.workspace = super::Workspace::Schematic;
                    self.editor.set_tool(Tool::Place(def.key));
                    self.notify(ctx, tr_fmt!("Click into the page to place {}", tr(def.name)));
                }
            }
            Some(Action::Duplicate(si)) => {
                let mut copy = self.session.prj.symbols[si].clone();
                copy.name = format!("{} (copy)", copy.name);
                copy.key = symlib::new_key(&self.session.prj.symbols, &copy.name);
                self.symed.sel = Some(copy.key.clone());
                self.session.prj.symbols.push(copy);
            }
            Some(Action::Delete(si)) => {
                self.session.prj.symbols.remove(si);
                self.symed.sel = None;
                self.symed.hit = None;
            }
            None => {}
        }
    }
}

fn symbol_properties(ui: &mut egui::Ui, u: &mut UserSymbol) {
    ui.label(RichText::new(tr("Symbol")).strong());
    egui::Grid::new("symprops_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label(tr("Name"));
        ui.text_edit_singleline(&mut u.name);
        ui.end_row();
        ui.label(tr("Library"));
        ui.text_edit_singleline(&mut u.library).on_hover_text(tr("Group in the symbol list, e.g. your company"));
        ui.end_row();
        ui.label(tr("Function"));
        egui::ComboBox::from_id_salt("symfunc").selected_text(u.func.title()).width(200.0).show_ui(ui, |ui| {
            for f in Func::ALL {
                ui.selectable_value(&mut u.func, f, f.title());
            }
        });
        ui.end_row();
        ui.label(tr("Identifying letter"));
        ui.add(egui::TextEdit::singleline(&mut u.letter).desired_width(60.0).hint_text(tr("K, Q, MM …")));
        ui.end_row();
        ui.label(tr("Default label"));
        ui.add(egui::TextEdit::singleline(&mut u.default_label).desired_width(80.0)).on_hover_text(tr("Terminal number, PLC address, potential name …"));
        ui.end_row();
    });
    ui.checkbox(&mut u.show_tag, tr("Show device tag and cross references"));
    ui.checkbox(&mut u.show_pin_labels, tr("Show connection point designations"));
    if matches!(u.func, Func::Aux(_)) {
        ui.checkbox(&mut u.auto_number, tr("Number contacts automatically (13/14, 21/22 …)"));
    }
    if u.func.is_feedthrough() && u.pins.len() >= 2 {
        let mut bridged = u.bridges.contains(&(0, 1));
        if ui.checkbox(&mut bridged, tr("Connection points 1 and 2 are connected inside")).changed() {
            u.bridges.retain(|b| *b != (0, 1));
            if bridged {
                u.bridges.push((0, 1));
            }
        }
    }
    ui.weak(tr_fmt!("Key: {}", u.key));
}

fn point_edit(ui: &mut egui::Ui, label: &str, q: &mut P, speed: f32) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut q.x).speed(speed).prefix("x "));
        ui.add(egui::DragValue::new(&mut q.y).speed(speed).prefix("y "));
    });
    ui.end_row();
}

fn element_properties(ui: &mut egui::Ui, se: &mut SymbolEditor, u: &mut UserSymbol) {
    let mut delete = false;
    match se.hit {
        Some(Hit::Pin(i)) if i < u.pins.len() => {
            ui.label(RichText::new(tr_fmt!("Connection point {}", i + 1)).strong());
            egui::Grid::new("pinprops").num_columns(2).show(ui, |ui| {
                ui.label(tr("Designation"));
                ui.text_edit_singleline(&mut u.pins[i].label);
                ui.end_row();
                point_edit(ui, tr("Position"), &mut u.pins[i].pos, GRID);
            });
            u.pins[i].pos = symlib::snap(u.pins[i].pos, GRID);
            ui.checkbox(&mut u.pins[i].internal, tr("Internal bridge attachment"));
            delete = icons::text_button(ui, Icon::Delete, tr("Delete connection point")).clicked();
        }
        Some(Hit::Prim(i)) if i < u.prims.len() => {
            let prim = &mut u.prims[i];
            let kind = match prim {
                Prim::Image { .. } => tr("Image"),
                Prim::Line { .. } => tr("Line"),
                Prim::Poly { .. } | Prim::Path {..} => tr("Polyline / rectangle"),
                Prim::Circle { .. } => tr("Circle"),
                Prim::Text { .. } => tr("Text"),
                Prim::Field { .. } => tr("Field"),
            };
            ui.label(RichText::new(kind).strong());
            egui::Grid::new("primprops").num_columns(2).show(ui, |ui| match prim {
                Prim::Image { corners, .. } => {
                    for corner in corners {
                        point_edit(ui, tr("Position"), corner, 0.1);
                    }
                }
                Prim::Line { a, b, dashed } => {
                    point_edit(ui, tr("Start"), a, 0.1);
                    point_edit(ui, tr("End"), b, 0.1);
                    ui.label(tr("Dashed"));
                    ui.checkbox(dashed, "");
                    ui.end_row();
                }
                Prim::Poly { pts, closed, fill } | Prim::Path { pts, closed, fill, .. } => {
                    ui.label(tr("Points"));
                    ui.label(pts.len().to_string());
                    ui.end_row();
                    ui.label(tr("Closed / filled"));
                    ui.horizontal(|ui| {
                        ui.checkbox(closed, "");
                        ui.checkbox(fill, "");
                    });
                    ui.end_row();
                }
                Prim::Circle { c, r, fill, .. } => {
                    point_edit(ui, tr("Centre"), c, 0.1);
                    ui.label(tr("Radius"));
                    ui.add(egui::DragValue::new(r).speed(0.1).range(0.1..=100.0));
                    ui.end_row();
                    ui.label(tr("Filled"));
                    ui.checkbox(fill, "");
                    ui.end_row();
                }
                Prim::Text { pos, text, size, anchor, .. } => {
                    ui.label(tr("Text"));
                    ui.text_edit_singleline(text);
                    ui.end_row();
                    point_edit(ui, tr("Position"), pos, 0.1);
                    ui.label(tr("Size"));
                    ui.add(egui::DragValue::new(size).speed(0.1).range(0.5..=20.0));
                    ui.end_row();
                    anchor_edit(ui, anchor);
                }
                Prim::Field { kind, pos, size, anchor, angle } => {
                    ui.label(tr("Shows"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(kind, FieldKind::Tag, tr("Device tag"));
                        ui.selectable_value(kind, FieldKind::Label, tr("Label"));
                        ui.selectable_value(kind, FieldKind::Pin(0), tr("Connection point"));
                    });
                    ui.end_row();
                    if let FieldKind::Pin(index) = kind {
                        ui.label(tr("Connection point"));
                        ui.add(egui::DragValue::new(index).speed(1));
                        ui.end_row();
                    }
                    point_edit(ui, tr("Position"), pos, 0.1);
                    ui.label(tr("Size"));
                    ui.add(egui::DragValue::new(size).speed(0.1).range(0.5..=20.0));
                    ui.end_row();
                    anchor_edit(ui, anchor);
                    ui.label(tr("Angle"));
                    let mut degrees = angle.unwrap_or_default();
                    if ui.add(egui::DragValue::new(&mut degrees).range(-360.0..=360.)).changed() {
                        *angle = Some(degrees);
                    }
                    ui.end_row();
                }
            });
            match prim {
                Prim::Path { width, color, dashed, .. } => {
                    super::drawing_ui::color_field(ui,color);
                    ui.horizontal(|ui| {
                        ui.label(tr("Width"));
                        ui.add(egui::DragValue::new(width).speed(0.05).range(0.01..=10.));
                        ui.checkbox(dashed,tr("Dashed"));
                    });
                }
                Prim::Text { angle,color,.. } => {
                    super::drawing_ui::color_field(ui,color);
                    let mut degrees=angle.unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(tr("Angle"));
                        if ui.add(egui::DragValue::new(&mut degrees).range(-360.0..=360.)).changed() {
                            *angle=Some(degrees);
                        }
                    });
                }
                Prim::Circle { color,.. } => super::drawing_ui::color_field(ui,color),
                _ => (),
            }
            delete = icons::text_button(ui, Icon::Delete, tr("Delete element")).clicked();
        }
        _ => {
            ui.weak(tr("Click an element in the drawing to edit it."));
        }
    }
    if delete {
        delete_hit(se, u);
    }
}

fn anchor_edit(ui: &mut egui::Ui, anchor: &mut Anchor) {
    ui.label(tr("Alignment"));
    ui.horizontal(|ui| {
        ui.selectable_value(anchor, Anchor::Left, tr("Left"));
        ui.selectable_value(anchor, Anchor::Center, tr("Centre"));
        ui.selectable_value(anchor, Anchor::Right, tr("Right"));
    });
    ui.end_row();
}

fn delete_hit(se: &mut SymbolEditor, u: &mut UserSymbol) {
    match se.hit.take() {
        Some(Hit::Pin(i)) => symlib::remove_pin(u, i),
        Some(Hit::Prim(i)) if i < u.prims.len() => {
            u.prims.remove(i);
        }
        _ => {}
    }
}

fn canvas(ui: &mut egui::Ui, se: &mut SymbolEditor, u: &mut UserSymbol) {
    let pal = Palette::new(ui.visuals().dark_mode);
    let (resp, painter) = ui.allocate_painter(vec2(400.0, 340.0), Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 4.0, pal.paper);
    let origin = rect.center();
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            se.zoom = (se.zoom * (scroll * 0.002).exp()).clamp(3.0, 60.0);
        }
    }
    let zoom = se.zoom;
    let to_mm = move |s: egui::Pos2| p((s.x - origin.x) / zoom, (s.y - origin.y) / zoom);
    // Grid: millimetres and the 4 mm connection grid.
    let painter = painter.with_clip_rect(rect);
    let half = vec2(rect.width(), rect.height()) / 2.0 / se.zoom;
    let (nx, ny) = (half.x.ceil() as i32, half.y.ceil() as i32);
    for i in -nx..=nx {
        let x = origin.x + i as f32 * se.zoom;
        let major = i % GRID as i32 == 0;
        if major || se.zoom >= 8.0 {
            let c = if i == 0 { pal.frame } else if major { pal.grid } else { pal.grid.gamma_multiply(0.35) };
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(if i == 0 { 1.0 } else { 0.5 }, c));
        }
    }
    for j in -ny..=ny {
        let y = origin.y + j as f32 * se.zoom;
        let major = j % GRID as i32 == 0;
        if major || se.zoom >= 8.0 {
            let c = if j == 0 { pal.frame } else if major { pal.grid } else { pal.grid.gamma_multiply(0.35) };
            painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(if j == 0 { 1.0 } else { 0.5 }, c));
        }
    }
    let ptr = resp.hover_pos().map(to_mm);
    let step = if se.tool == SymTool::Pin || matches!(se.hit, Some(Hit::Pin(_))) && se.tool == SymTool::Select { GRID } else { DRAW_SNAP };
    let snapped = ptr.map(|q| symlib::snap(q, step));

    // Keyboard
    if resp.hovered() || resp.has_focus() {
        let (esc, del) = ui.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Delete)));
        if esc {
            if se.start.is_some() {
                se.start = None;
            } else {
                se.set_tool(SymTool::Select);
            }
        }
        if del && !ui.ctx().egui_wants_keyboard_input() {
            delete_hit(se, u);
        }
    }
    if resp.clicked_by(PointerButton::Secondary) {
        if se.start.is_some() {
            se.start = None;
        } else {
            se.set_tool(SymTool::Select);
        }
    }

    // Mouse
    let clicked = resp.clicked_by(PointerButton::Primary);
    match (se.tool, snapped) {
        (SymTool::Select, _) => {
            let tol = 6.0 / se.zoom;
            if resp.drag_started_by(PointerButton::Primary) {
                let press = ui.input(|i| i.pointer.press_origin()).map(to_mm);
                if let Some(q) = press {
                    se.hit = symlib::hit(u, q, tol);
                    let step = if matches!(se.hit, Some(Hit::Pin(_))) { GRID } else { DRAW_SNAP };
                    se.drag = se.hit.map(|_| symlib::snap(q, step));
                }
            }
            if let (Some(last), Some(q)) = (se.drag, resp.interact_pointer_pos().map(to_mm)) {
                let step = if matches!(se.hit, Some(Hit::Pin(_))) { GRID } else { DRAW_SNAP };
                let q = symlib::snap(q, step);
                let d = q - last;
                if d.x != 0.0 || d.y != 0.0 {
                    match se.hit {
                        Some(Hit::Pin(i)) if i < u.pins.len() => u.pins[i].pos = u.pins[i].pos + d,
                        Some(Hit::Prim(i)) if i < u.prims.len() => symlib::translate(&mut u.prims[i], d),
                        _ => {}
                    }
                    se.drag = Some(q);
                }
            }
            if resp.drag_stopped() {
                se.drag = None;
            }
            if clicked {
                se.hit = ptr.and_then(|q| symlib::hit(u, q, tol));
            }
        }
        (SymTool::Pin, Some(q)) if clicked => {
            add_pin(se, u, q);
        }
        (SymTool::Text, Some(q)) if clicked => {
            let text = if se.text.trim().is_empty() { "Text".to_string() } else { se.text.clone() };
            u.prims.push(Prim::Text { pos: q, text, size: 2.0, anchor: Anchor::Left, angle: None, color:None });
            se.hit = Some(Hit::Prim(u.prims.len() - 1));
        }
        (SymTool::TagField | SymTool::LabelField, Some(q)) if clicked => {
            let kind = if se.tool == SymTool::TagField { FieldKind::Tag } else { FieldKind::Label };
            u.prims.push(Prim::Field { kind, pos: q, size: 2.2, anchor: Anchor::Left, angle: None });
            se.hit = Some(Hit::Prim(u.prims.len() - 1));
        }
        (SymTool::Line | SymTool::Rect | SymTool::Circle, Some(q)) if clicked => match se.start.take() {
            None => se.start = Some(q),
            Some(a) if !a.same(q) => {
                let prim = match se.tool {
                    SymTool::Line => Prim::Line { a, b: q, dashed: false },
                    SymTool::Rect => Prim::Poly { pts: vec![a, p(q.x, a.y), q, p(a.x, q.y)], closed: true, fill: false },
                    _ => Prim::Circle { c: a, r: a.dist(q), fill: false, color:None },
                };
                u.prims.push(prim);
                se.hit = Some(Hit::Prim(u.prims.len() - 1));
            }
            Some(_) => {}
        },
        _ => {}
    }

    // Drawing
    let mut d = EguiDraw { painter: &painter, origin, zoom: se.zoom, pal: &pal, force: None, mirror: None, textures: None };
    let inst = SymbolInst {
        tag: "-K1".into(), label: if u.default_label.is_empty() { "1".into() } else { u.default_label.clone() },
        pins: u.pins.iter().map(|pin| pin.label.clone()).collect(), ..Default::default()
    };
    render::draw_prims(&mut d, &u.prims, &inst, Ink::Line);
    if let Some(Hit::Prim(i)) = se.hit {
        if let Some(prim) = u.prims.get(i) {
            render::draw_prims(&mut d, std::slice::from_ref(prim), &inst, Ink::Selected);
        }
    }
    for (i, pin) in u.pins.iter().enumerate() {
        let ink = if se.hit == Some(Hit::Pin(i)) { Ink::Selected } else { Ink::Pin };
        d.circle(pin.pos, 0.7, 0.2, ink, false);
        d.text(pin.pos + p(1.0, -0.9), &pin.label, 1.6, Anchor::Left, VAnchor::Middle, Ink::CrossRef);
    }
    // Previews
    let mut g = EguiDraw { painter: &painter, origin, zoom: se.zoom, pal: &pal, force: Some(Ink::Ghost), mirror: None, textures: None };
    if let (Some(a), Some(q)) = (se.start, snapped) {
        match se.tool {
            SymTool::Line => g.line(a, q, 0.25, Ink::Ghost, false),
            SymTool::Rect => g.rect(a, q, 0.25, Ink::Ghost, false),
            SymTool::Circle => g.circle(a, a.dist(q), 0.25, Ink::Ghost, false),
            _ => {}
        }
    }
    if let (Some(q), true) = (snapped, se.tool != SymTool::Select) {
        let c = pos2(origin.x + q.x * se.zoom, origin.y + q.y * se.zoom);
        painter.circle_stroke(c, 3.0, Stroke::new(1.0, pal.ghost));
    }
    if let Some(q) = ptr {
        painter.text(rect.left_top() + vec2(6.0, 6.0), egui::Align2::LEFT_TOP, format!("x {:.1}  y {:.1} mm", q.x, q.y), egui::FontId::monospace(11.0), pal.frame);
    }
}

fn add_pin(se: &mut SymbolEditor, u: &mut UserSymbol, pos: P) {
    if !u.pins.iter().any(|pin| pin.pos.same(pos)) {
        u.pins.push(UserPin { pos, label: symlib::next_pin_label(u), internal: false });
        se.hit = Some(Hit::Pin(u.pins.len() - 1));
    }
}
