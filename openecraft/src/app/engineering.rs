//! Engineering windows: macros, terminal strips, PLC overview and marking.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use crate::editor::{EguiDraw, Palette, Tool};
use crate::io::Purpose;
use openecraft_core::macro_boxes;
use openecraft_core::macros::{self, Macro, MacroKind};
use openecraft_core::model::*;
use openecraft_core::ops::Clip;
use openecraft_core::render::{self, Draw, Ink};
use openecraft_core::marking::{self, Category, Column, Scheme, WireText};
use openecraft_core::{parts, plc, strips, symbols};
use egui::{vec2, RichText};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct EngineeringUi {
    pub show_macros: bool,
    macro_filter: String,
    /// Name of the selected macro.
    macro_sel: Option<String>,
    /// Placeholder values for the next insertion.
    macro_values: BTreeMap<String, String>,
    /// Macro waiting for its name (save dialog) and the variant to save it as.
    saving: Option<Macro>,
    saving_variant: String,
    /// Variant of the selected macro.
    macro_variant: String,
    pub show_strips: bool,
    pub show_potentials: bool,
    strip: String,
    pub show_plc: bool,
    card: String,
    pub show_revisions: bool,
    pub show_marking: bool,
}

enum MacroAction {
    Insert(usize),
    Delete(usize),
    DeleteVariant(usize, String),
    Export,
    Import,
    UpdateBoxes,
    GotoBox(usize, u64),
    SaveBox(usize, u64),
    RemoveBox(usize, u64),
}

/// Scaled drawing of a macro's items.
fn clip_preview(ui: &mut egui::Ui, prj: &Project, clip: &Clip, size: egui::Vec2) {
    let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
    let pal = Palette::new(ui.visuals().dark_mode);
    painter.rect_filled(resp.rect, 4.0, pal.paper);
    let b = clip.drawing_bounds().expand(4.0);
    if !b.is_valid() {
        return;
    }
    let zoom = ((size.x - 8.0) / b.width()).min((size.y - 8.0) / b.height()).min(4.0);
    let origin = resp.rect.center() - vec2(b.center().x, b.center().y) * zoom;
    let mut d = EguiDraw { painter: &painter, origin, zoom, pal: &pal, force: None, mirror: None, textures: None };
    for w in &clip.wires {
        d.line(w.a, w.b, render::WIRE_W, Ink::Wire, false);
    }
    for s in &clip.shapes {
        render::draw_shape(&mut d, s, Ink::Line);
    }
    for s in &clip.symbols {
        if let Some(def) = clip.symbol(prj,&s.sym) {
            render::draw_symbol(&mut d, &def, s, Ink::Line);
        }
    }
    for t in &clip.texts {
        d.text(t.pos, t.text.lines().next().unwrap_or(""), t.size, symbols::Anchor::Left, render::VAnchor::Middle, Ink::Text);
    }
}

impl App {
    /// Opens the save dialog for the selection (window macro) or the page (page macro).
    pub(super) fn save_as_macro(&mut self, whole_page: bool) {
        let page = &self.session.prj.pages[self.session.page];
        let mut m = if whole_page {
            macros::from_page(page, &page.description)
        } else {
            macros::from_selection(page, &self.editor.selection, "")
        };
        m.clip.include_project_data(&self.session.prj);
        self.eng.saving = Some(m);
        self.eng.saving_variant.clear();
    }

    pub(super) fn save_macro_dialog(&mut self, ctx: &egui::Context) {
        let Some(m) = &mut self.eng.saving else { return };
        let (mut save, mut cancel) = (false, false);
        let (window, esc) = self.dialogs.window(ctx, tr("Save as macro"), "save_macro", None);
        window.resizable(false).show(ctx, |ui| {
            ui.label(if m.kind == MacroKind::Page { tr_fmt!("Page macro: {}", m.summary()) } else { tr_fmt!("Window macro: {}", m.summary()) });
            egui::Grid::new("savemacro").num_columns(2).show(ui, |ui| {
                ui.label(tr("Name"));
                ui.text_edit_singleline(&mut m.name);
                ui.end_row();
                ui.label(tr("Category"));
                ui.add(egui::TextEdit::singleline(&mut m.category).hint_text(tr("e.g. Motors")));
                ui.end_row();
                ui.label(tr("Variant"));
                ui.add(egui::TextEdit::singleline(&mut self.eng.saving_variant).hint_text(macros::FIRST_VARIANT).desired_width(60.0))
                    .on_hover_text(tr("Another variant of an existing macro with this name (e.g. B), chosen when inserting"));
                ui.end_row();
                ui.label(tr("Description"));
                ui.text_edit_multiline(&mut m.description);
                ui.end_row();
            });
            ui.weak(tr("Placeholders like {MOTOR} in tags, texts and function texts are filled in when inserting."));
            ui.horizontal(|ui| {
                save = ui.add_enabled_ui(!m.name.trim().is_empty(), |ui| icons::text_button(ui, Icon::Save, tr("Save"))).inner.clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if save {
            let m = self.eng.saving.take().unwrap();
            let variant = self.eng.saving_variant.trim().to_string();
            let name = m.name.clone();
            let existed = self.macro_lib.get(&name).is_some();
            let replaced = self.macro_lib.upsert_variant(m, &variant);
            self.eng.macro_sel = Some(name.clone());
            self.eng.macro_variant = variant.clone();
            let msg = match (existed, replaced) {
                (false, _) => tr_fmt!("Macro {} saved", name),
                (true, true) => tr_fmt!("Variant {} of macro {} replaced", if variant.is_empty() { macros::FIRST_VARIANT } else { &variant }, name),
                (true, false) => tr_fmt!("Variant {} of macro {} saved", variant, name),
            };
            self.notify(ctx, msg);
            self.eng.show_macros = true;
        }
        if cancel || esc {
            self.eng.saving = None;
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn macros_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut action = None;
        let can_save_selection = !self.editor.selection.is_empty() && self.session.prj.pages[self.session.page].kind == PageKind::Schematic;
        let mut save = None;
        ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.eng.macro_filter).hint_text(tr("Filter…")).desired_width(160.0));
                if ui.add_enabled_ui(can_save_selection, |ui| icons::text_button(ui, Icon::Save, tr("Save selection as macro…"))).inner.clicked() {
                    save = Some(false);
                }
                if ui.button(tr("Save page as macro…")).clicked() {
                    save = Some(true);
                }
                ui.menu_button(tr("Library"), |ui| {
                    if ui.button(tr("Export macros (JSON)…")).clicked() {
                        action = Some(MacroAction::Export);
                    }
                    if ui.button(tr("Import macros…")).clicked() {
                        action = Some(MacroAction::Import);
                    }
                });
                let boxes: Vec<(usize, u64, String, macro_boxes::State)> = macro_boxes::all(&self.session.prj)
                    .into_iter()
                    .map(|(pi, b)| (pi, b.id, b.name.clone(), macro_boxes::state(&self.macro_lib, b)))
                    .collect();
                let outdated = boxes.iter().filter(|b| b.3 == macro_boxes::State::Outdated).count();
                ui.menu_button(tr_fmt!("Macro boxes ({})", boxes.len()), |ui| {
                    if ui.add_enabled(outdated > 0, egui::Button::new(tr_fmt!("Update {} changed macros", outdated))).on_hover_text(tr("Replaces the content of the macro boxes whose macro changed in the library")).clicked() {
                        action = Some(MacroAction::UpdateBoxes);
                        ui.close();
                    }
                    ui.separator();
                    if boxes.is_empty() {
                        ui.weak(tr("Window macros inserted from now on are recorded as macro boxes."));
                    }
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (pi, id, name, st) in &boxes {
                            let page = &self.session.prj.pages[*pi].name;
                            let (state, color) = match st {
                                macro_boxes::State::Current => (tr("current"), ui.visuals().weak_text_color()),
                                macro_boxes::State::Outdated => (tr("changed in the library"), crate::style::warning()),
                                macro_boxes::State::Missing => (tr("not in the library"), crate::style::error()),
                            };
                            ui.horizontal(|ui| {
                                if icons::button(ui, Icon::Next, tr("Show")).clicked() {
                                    action = Some(MacroAction::GotoBox(*pi, *id));
                                }
                                ui.label(tr_fmt!("Page {}: {}", page, name));
                                ui.colored_label(color, state);
                                if icons::button(ui, Icon::Save, tr("Save the box content as the macro in the library")).clicked() {
                                    action = Some(MacroAction::SaveBox(*pi, *id));
                                }
                                if icons::button(ui, Icon::Close, tr("Remove the macro box (its items stay)")).clicked() {
                                    action = Some(MacroAction::RemoveBox(*pi, *id));
                                }
                            });
                        }
                    });
                });
            });
            ui.separator();
            let h = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(240.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("macrolist").auto_shrink([false, false]).show(ui, |ui| {
                        let f = self.eng.macro_filter.to_lowercase();
                        let mut categories: Vec<&str> = self.macro_lib.macros.iter().map(|m| m.category.as_str()).collect();
                        categories.dedup();
                        for cat in categories {
                            let title = if cat.is_empty() { "(no category)" } else { cat };
                            egui::CollapsingHeader::new(RichText::new(title).strong()).id_salt(("mcat", cat)).default_open(true).show(ui, |ui| {
                                for m in self.macro_lib.macros.iter().filter(|m| m.category == cat) {
                                    if !f.is_empty() && !m.name.to_lowercase().contains(&f) && !m.description.to_lowercase().contains(&f) {
                                        continue;
                                    }
                                    let icon = if m.kind == MacroKind::Page { "🗋" } else { "☐" };
                                    let sel = self.eng.macro_sel.as_deref() == Some(m.name.as_str());
                                    if ui.selectable_label(sel, format!("{icon} {}", m.name)).on_hover_text(&m.description).clicked() {
                                        self.eng.macro_sel = Some(m.name.clone());
                                        self.eng.macro_values.clear();
                                        self.eng.macro_variant.clear();
                                    }
                                }
                            });
                        }
                        if self.macro_lib.macros.is_empty() {
                            ui.weak(tr("No macros yet. Select items in a schematic and save them as a macro."));
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    let Some(i) = self.eng.macro_sel.as_ref().and_then(|n| self.macro_lib.macros.iter().position(|m| &m.name == n)) else {
                        ui.weak(tr("Select a macro."));
                        return;
                    };
                    let values = &mut self.eng.macro_values;
                    let variant = &mut self.eng.macro_variant;
                    let m = &mut self.macro_lib.macros[i];
                    if m.with_variant(variant).is_none() {
                        variant.clear();
                    }
                    let shown = m.with_variant(variant).unwrap_or_else(|| m.clone());
                    egui::ScrollArea::vertical().id_salt("macrodetail").auto_shrink([false, false]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(&m.name);
                            ui.weak(if m.kind == MacroKind::Page { "page macro" } else { "window macro" });
                        });
                        egui::Grid::new("macroprops").num_columns(2).show(ui, |ui| {
                            ui.label(tr("Category"));
                            ui.text_edit_singleline(&mut m.category);
                            ui.end_row();
                            ui.label(tr("Description"));
                            ui.text_edit_multiline(&mut m.description);
                            ui.end_row();
                            if m.kind == MacroKind::Page {
                                ui.label(tr("Page description"));
                                ui.text_edit_singleline(&mut m.page_description);
                                ui.end_row();
                            }
                        });
                        let names = m.variant_names();
                        ui.horizontal(|ui| {
                            ui.label(tr("Variant"));
                            egui::ComboBox::from_id_salt("mvariant").selected_text(shown.main_variant().to_string()).show_ui(ui, |ui| {
                                for n in &names {
                                    if ui.selectable_label(shown.main_variant() == n, n).clicked() {
                                        *variant = n.clone();
                                    }
                                }
                            });
                            let own = shown.main_variant() == m.main_variant();
                            if !own && icons::text_button(ui, Icon::Delete, tr("Delete variant")).clicked() {
                                action = Some(MacroAction::DeleteVariant(i, shown.main_variant().to_string()));
                            }
                            if names.len() == 1 {
                                ui.weak(tr("Save a selection under this name with another variant to add one."));
                            }
                        });
                        ui.weak(shown.summary());
                        clip_preview(ui, &self.session.prj, &shown.clip, vec2(420.0, 200.0));
                        let placeholders = shown.placeholders();
                        if !placeholders.is_empty() {
                            ui.separator();
                            ui.label(RichText::new(tr("Value sets")).strong());
                            let mut remove = None;
                            egui::Grid::new("valuesets").striped(true).num_columns(placeholders.len() + 2).show(ui, |ui| {
                                ui.label(RichText::new(tr("Name")).strong());
                                for ph in &placeholders {
                                    ui.label(RichText::new(format!("{{{ph}}}")).strong());
                                }
                                ui.label("");
                                ui.end_row();
                                for (vi, vs) in m.value_sets.iter_mut().enumerate() {
                                    ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut vs.name));
                                    for ph in &placeholders {
                                        ui.add_sized([100.0, 20.0], egui::TextEdit::singleline(vs.values.entry(ph.clone()).or_default()));
                                    }
                                    if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                                        remove = Some(vi);
                                    }
                                    ui.end_row();
                                }
                            });
                            if let Some(vi) = remove {
                                m.value_sets.remove(vi);
                            }
                            if icons::text_button(ui, Icon::Plus, tr("Value set")).clicked() {
                                let name = format!("Set {}", m.value_sets.len() + 1);
                                m.value_sets.push(macros::ValueSet { name, values: values.clone() });
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(tr("Insert with")).strong());
                                egui::ComboBox::from_id_salt("useset").selected_text(tr("value set…")).show_ui(ui, |ui| {
                                    for vs in &m.value_sets {
                                        if ui.button(&vs.name).clicked() {
                                            *values = vs.values.clone();
                                        }
                                    }
                                });
                            });
                            egui::Grid::new("insertvalues").num_columns(2).show(ui, |ui| {
                                for ph in &placeholders {
                                    ui.label(format!("{{{ph}}}"));
                                    ui.text_edit_singleline(values.entry(ph.clone()).or_default());
                                    ui.end_row();
                                }
                            });
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let label = if m.kind == MacroKind::Page { tr("➕ Insert as new page") } else { tr("➕ Insert (click into the page)") };
                            if ui.button(label).clicked() {
                                action = Some(MacroAction::Insert(i));
                            }
                            if icons::text_button(ui, Icon::Delete, tr("Delete macro")).clicked() {
                                action = Some(MacroAction::Delete(i));
                            }
                        });
                    });
                });
            });
        });
        if let Some(whole_page) = save {
            self.save_as_macro(whole_page);
        }
        match action {
            Some(MacroAction::Insert(i)) => {
                let m = self.macro_lib.macros[i].with_variant(&self.eng.macro_variant).unwrap_or_else(|| self.macro_lib.macros[i].clone());
                if m.kind == MacroKind::Page {
                    let at = macros::insert_page(&mut self.session.prj, self.session.page, &m, &self.eng.macro_values);
                    self.goto(at, 0);
                    self.notify(ctx, tr_fmt!("Page macro {} inserted as page {}", m.name, self.session.prj.pages[at].name));
                } else {
                    self.editor.insert = Some(m.resolve(&self.eng.macro_values));
                    self.editor.insert_macro = Some((m.clone(), self.eng.macro_values.clone()));
                    self.editor.set_tool(Tool::Insert);
                    self.workspace = super::Workspace::Schematic;
                }
            }
            Some(MacroAction::Delete(i)) => {
                self.macro_lib.macros.remove(i);
                self.eng.macro_sel = None;
            }
            Some(MacroAction::UpdateBoxes) => {
                let r = macro_boxes::update(&mut self.session.prj, &self.macro_lib);
                let mut msg = tr_fmt!("{} macro boxes updated", r.updated);
                if r.locked > 0 {
                    msg = format!("{msg}\n{}", tr_fmt!("{} on released pages were left as they are", r.locked));
                }
                self.notify(ctx, msg);
            }
            Some(MacroAction::GotoBox(pi, id)) => {
                if pi != self.session.page {
                    self.goto(pi, 0);
                }
                if let Some(b) = self.session.prj.pages[pi].macro_boxes.iter().find(|b| b.id == id) {
                    self.editor.selection = macro_boxes::live_items(&self.session.prj.pages[pi], b);
                }
            }
            Some(MacroAction::SaveBox(pi, id)) => match macro_boxes::save_to_library(&mut self.session.prj, pi, id, &mut self.macro_lib) {
                Ok(()) => self.notify(ctx, tr("Macro saved to the library")),
                Err(e) => self.notify_error(ctx, e),
            },
            Some(MacroAction::RemoveBox(pi, id)) => {
                macro_boxes::remove(&mut self.session.prj, pi, id);
            }
            Some(MacroAction::DeleteVariant(i, name)) => {
                self.macro_lib.macros[i].remove_variant(&name);
                self.eng.macro_variant.clear();
            }
            Some(MacroAction::Export) => {
                let json = serde_json::to_string_pretty(&self.macro_lib).unwrap_or_default();
                self.export(ctx, "macros.json".into(), ("JSON", "json"), json);
            }
            Some(MacroAction::Import) => self.io.open(ctx, Purpose::Macros),
            None => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn strips_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let tags = strips::strip_tags(&self.session.prj);
        if !tags.contains(&self.eng.strip) {
            self.eng.strip = tags.first().cloned().unwrap_or_default();
        }
        let strip = self.eng.strip.clone();
        let rows = strips::rows(&self.session.prj, &strip, &self.session.analysis.netlist, &self.session.analysis.open);
        let layout = strips::strip_layout(&self.session.prj, &strip);
        let jumpers = strips::strip_jumpers(&self.session.prj, &strip);
        let level_problems = strips::level_problems(&self.session.prj, &strip);
        let mut data = self.session.prj.strips.get(&strip).cloned().unwrap_or_default();
        let before = data.clone();
        let (mut renumber, mut sort) = (false, false);
        let mut moved = None;
        let mut relabel: Vec<(usize, usize, String, String)> = Vec::new();
        let mut jump = None;
        ui.scope(|ui| {
            if tags.is_empty() {
                ui.weak(tr("No terminal strips: place terminals in a schematic first."));
                return;
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Strip"));
                egui::ComboBox::from_id_salt("strip").selected_text(&strip).show_ui(ui, |ui| {
                    for t in &tags {
                        ui.selectable_value(&mut self.eng.strip, t.clone(), t);
                    }
                });
                ui.separator();
                ui.label(tr("Spare terminals"));
                ui.add(egui::DragValue::new(&mut data.spares).range(0..=500));
                ui.separator();
                renumber = ui.button(tr("Renumber 1 … n")).on_hover_text(tr("In strip order; PE and other non-numeric terminals keep their designation")).clicked();
                sort = ui.button(tr("Sort by designation")).clicked();
                ui.separator();
                let mut auto = !data.manual_accessories;
                if ui
                    .checkbox(&mut auto, tr("Automatic accessories"))
                    .on_hover_text(tr("End brackets and end plates from the tech data of the terminal parts (End bracket, End plate)"))
                    .changed()
                {
                    data.manual_accessories = !auto;
                }
            });
            egui::CollapsingHeader::new(tr("Strip on the rail")).id_salt("strip_layout").default_open(true).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for e in &layout {
                        let text = RichText::new(e.kind.title()).monospace();
                        let accessory = matches!(e.kind, strips::ElementKind::EndBracket | strips::ElementKind::EndPlate);
                        let label = if accessory { ui.label(text.weak()) } else { ui.label(text) };
                        if !e.part.is_empty() {
                            label.on_hover_text(format!("{} – {:.1} mm", e.part, e.width));
                        }
                    }
                    ui.separator();
                    ui.label(tr_fmt!("{} mm", format!("{:.1}", layout.iter().map(|e| e.width).sum::<f32>())));
                });
                let accessories = layout.iter().any(|e| matches!(e.kind, strips::ElementKind::EndBracket | strips::ElementKind::EndPlate));
                if !data.manual_accessories && !accessories && !layout.is_empty() {
                    ui.weak(tr("The terminal parts name no end brackets or end plates (tech data End bracket, End plate)."));
                }
                for j in &jumpers {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr_fmt!("Jumper {}", j.number));
                        let mut kind = j.kind;
                        egui::ComboBox::from_id_salt(("jkind", j.number)).selected_text(strips::jumper_kind_title(kind)).show_ui(ui, |ui| {
                            for k in [JumperKind::PlugIn, JumperKind::Saddle, JumperKind::Wire] {
                                ui.selectable_value(&mut kind, k, strips::jumper_kind_title(k));
                            }
                        });
                        if kind != j.kind {
                            if kind == JumperKind::PlugIn {
                                data.jumper_kinds.remove(&j.number);
                            } else {
                                data.jumper_kinds.insert(j.number, kind);
                            }
                        }
                        ui.label(RichText::new(if j.part.is_empty() { "–" } else { &j.part }).monospace());
                        ui.label(j.length_text());
                        ui.label(j.span_text());
                        if !j.broken.is_empty() {
                            ui.label(j.broken_text());
                        }
                        if let Some(problem) = j.problem_text(&strip) {
                            ui.colored_label(crate::style::lock(), problem);
                        }
                    });
                }
            });
            for problem in &level_problems {
                ui.colored_label(crate::style::lock(), problem);
            }
            ui.weak(tr("Terminals with the same jumper number are bridged (same potential). Spare terminals are counted in the bill of materials."));
            ui.separator();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("strip").striped(true).num_columns(9).spacing([10.0, 4.0]).show(ui, |ui| {
                    for h in ["", tr("Terminal"), tr("Level"), tr("Jumper"), tr("External (top)"), tr("Internal (bottom)"), tr("Potential"), tr("Function text"), tr("Location")] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    let placed = rows.iter().filter(|r| r.terminal.is_some()).count();
                    for (i, r) in rows.iter().enumerate() {
                        let Some(t) = &r.terminal else {
                            ui.label("");
                            ui.weak(&r.label);
                            ui.label("");
                            ui.label("");
                            ui.weak(tr("spare"));
                            ui.end_row();
                            continue;
                        };
                        ui.horizontal(|ui| {
                            if ui.add_enabled_ui(i > 0, |ui| icons::button(ui, Icon::ChevronUp, tr("Move up"))).inner.clicked() {
                                moved = Some((i, true));
                            }
                            if ui.add_enabled_ui(i + 1 < placed, |ui| icons::button(ui, Icon::ChevronDown, tr("Move down"))).inner.clicked() {
                                moved = Some((i, false));
                            }
                        });
                        let mut label = r.label.clone();
                        ui.add_sized([50.0, 20.0], egui::TextEdit::singleline(&mut label));
                        if label != r.label {
                            relabel.push((t.page, t.sym, r.label.clone(), label));
                        }
                        let mut level = r.level;
                        if ui.add(egui::DragValue::new(&mut level).range(1..=4)).on_hover_text(tr("Level of a multi-level terminal: upper levels sit on the block of the terminal before")).changed() {
                            if level <= 1 {
                                data.levels.remove(&r.label);
                            } else {
                                data.levels.insert(r.label.clone(), level);
                            }
                        }
                        let mut j = data.jumpers.get(&r.label).copied().unwrap_or(0);
                        if ui.add(egui::DragValue::new(&mut j).range(0..=99)).on_hover_text(tr("0 = no jumper")).changed() {
                            if j == 0 {
                                data.jumpers.remove(&r.label);
                            } else {
                                data.jumpers.insert(r.label.clone(), j);
                            }
                        }
                        ui.label(&r.external);
                        ui.label(&r.internal);
                        ui.label(&r.potential);
                        ui.label(&r.function_text);
                        if ui.link(&r.location).clicked() {
                            jump = Some((t.page, t.id));
                        }
                        ui.end_row();
                    }
                });
            });
        });
        if data != before {
            self.session.prj.strips.insert(strip.clone(), data);
        }
        for (page, sym, old, new) in relabel {
            self.session.prj.pages[page].symbols[sym].label = new.clone();
            if let Some(d) = self.session.prj.strips.get_mut(&strip) {
                if let Some(j) = d.jumpers.remove(&old) {
                    d.jumpers.insert(new.clone(), j);
                }
                if let Some(l) = d.levels.remove(&old) {
                    d.levels.insert(new.clone(), l);
                }
                for o in d.order.iter_mut().filter(|o| **o == old) {
                    *o = new.clone();
                }
            }
        }
        if let Some((i, up)) = moved {
            strips::move_terminal(&mut self.session.prj, &strip, i, up);
        }
        if sort {
            if let Some(d) = self.session.prj.strips.get_mut(&strip) {
                d.order.clear();
            }
        }
        if renumber {
            let n = strips::renumber(&mut self.session.prj, &strip);
            self.notify(ctx, tr_fmt!("{}: {} terminals renumbered", strip, n));
        }
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
    }

    pub(super) fn marking_body(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let markers = marking::markers(&self.session.prj, &self.session.analysis.connections);
        let material = marking::material(&markers);
        let mut choices: Vec<(String, String)> = self
            .parts_db
            .parts
            .iter()
            .chain(parts::catalog())
            .filter(|p| p.group == "Marking")
            .map(|p| (p.number.clone(), p.description.clone()))
            .collect();
        choices.sort_by(|a, b| a.0.cmp(&b.0));
        choices.dedup_by(|a, b| a.0 == b.0);
        let mut export: Option<bool> = None;
        let mut picked = Vec::new();
        let s = &mut self.session.prj.marking;
        ui.scope(|ui| {
            ui.label(RichText::new(tr("Wire markers")).strong());
            ui.radio_value(&mut s.wire_text, WireText::Number, tr("Wire number on both ends"));
            ui.radio_value(&mut s.wire_text, WireText::Destination, tr("Destination: each end shows the connection point at the other end"));
            ui.separator();
            ui.label(RichText::new(tr("Marker material")).strong());
            egui::Grid::new("marking").num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
                let fields = [
                    (tr("Device markers"), &mut s.device_material),
                    (tr("Legend plates (door devices)"), &mut s.door_material),
                    (tr("Terminal markers"), &mut s.terminal_material),
                    (tr("Wire markers"), &mut s.wire_material),
                    (tr("Cable markers"), &mut s.cable_material),
                ];
                for (title, field) in fields {
                    ui.label(title);
                    ui.add_sized([160.0, 20.0], egui::TextEdit::singleline(field).font(egui::TextStyle::Monospace));
                    ui.menu_button("…", |ui| {
                        for (number, description) in &choices {
                            if ui.button(format!("{number}  {description}")).clicked() {
                                *field = number.clone();
                                picked.push(number.clone());
                            }
                        }
                        if choices.is_empty() {
                            ui.weak(tr("No parts of the product group Marking in the parts database."));
                        }
                    });
                    let pieces = material.iter().find(|m| m.0 == *field).map(|m| m.1).unwrap_or(0);
                    ui.label(tr_fmt!("{} pieces", pieces));
                    ui.end_row();
                }
            });
            ui.separator();
            ui.label(RichText::new(tr("Labeling scheme")).strong());
            scheme_editor(ui, s);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr_fmt!("{} markers", markers.iter().map(|m| m.count).sum::<u32>()));
                if ui.button(tr("Export marking (CSV)…")).on_hover_text(tr("Device, terminal, wire and cable markers for marking software")).clicked() {
                    export = Some(false);
                }
                if ui.button(tr("Export marking (Excel)…")).on_hover_text(tr("Device, terminal, wire and cable markers for marking software")).clicked() {
                    export = Some(true);
                }
            });
        });
        for number in picked {
            parts::ensure_in_project(&mut self.session.prj, &self.parts_db, &number);
        }
        if let Some(excel) = export {
            let scheme = marking::scheme(&self.session.prj.marking, None).unwrap_or_default();
            let markers = marking::markers(&self.session.prj, &self.session.analysis.connections);
            if excel {
                self.export_bytes(ctx, "marking.xlsx".into(), ("Excel", "xlsx"), &marking::xlsx(&markers, &scheme));
            } else {
                let ext = if scheme.separator == "\t" { "txt" } else { "csv" };
                self.export(ctx, format!("marking.{ext}"), ("CSV", ext), marking::text(&markers, &scheme));
            }
        }
    }

    pub(super) fn plc_overview_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let cards = plc::cards(&self.session.prj);
        if !self.eng.card.is_empty() && !cards.contains(&self.eng.card) {
            self.eng.card.clear();
        }
        let card = self.eng.card.clone();
        let points: Vec<plc::Point> =
            plc::points(&self.session.prj, &self.session.analysis.netlist).into_iter().filter(|p| card.is_empty() || p.card == card).collect();
        let mut data = self.session.prj.plc_cards.get(&card).cloned().unwrap_or_default();
        let before = data.clone();
        let (mut readdress, mut export, mut import) = (false, false, false);
        let mut edits: Vec<(plc::Point, String, String, String)> = Vec::new();
        let mut jump = None;
        ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Card"));
                egui::ComboBox::from_id_salt("plccard").selected_text(if card.is_empty() { "all cards" } else { &card }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.eng.card, String::new(), tr("all cards"));
                    for c in &cards {
                        ui.selectable_value(&mut self.eng.card, c.clone(), c);
                    }
                });
                ui.separator();
                export = icons::text_button(ui, Icon::Download, tr("Export symbol table (CSV)…")).on_hover_text(tr("Name; Data Type; Logical Address; Comment – for PLC programming tools")).clicked();
                import = icons::text_button(ui, Icon::Upload, tr("Import symbol table…")).on_hover_text(tr("Takes the comments as function texts (matched by address)")).clicked();
            });
            if !card.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr("Start byte"));
                    ui.add(egui::DragValue::new(&mut data.start_byte).range(0..=65535));
                    ui.label(tr("Rack"));
                    ui.add(egui::TextEdit::singleline(&mut data.rack).desired_width(40.0));
                    ui.label(tr("Slot"));
                    ui.add(egui::TextEdit::singleline(&mut data.slot).desired_width(40.0));
                    ui.label(tr("Description"));
                    ui.add(egui::TextEdit::singleline(&mut data.description).desired_width(180.0));
                    readdress = ui.button(tr("Readdress card")).on_hover_text(tr("Consecutive addresses from the start byte, in schematic order")).clicked();
                });
            }
            ui.separator();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("plcpoints").striped(true).num_columns(9).spacing([10.0, 4.0]).show(ui, |ui| {
                    for h in [tr("Card"), tr("Channel"), tr("Terminal"), tr("Address"), tr("Type"), tr("Symbolic address"), tr("Function text"), tr("Connected to"), tr("Location")] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for p in &points {
                        ui.push_id((p.id,p.channel_index),|ui| {
                        ui.label(&p.card);
                        ui.label(&p.channel);
                        ui.label(&p.terminal);
                        let mut addr = p.address.clone();
                        ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(&mut addr));
                        ui.label(p.io_kind());
                        let mut symbolic=p.symbolic.clone();
                        ui.add_sized([160.0,20.0],egui::TextEdit::singleline(&mut symbolic));
                        let mut text = p.function_text.clone();
                        ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut text));
                        if addr != p.address || text != p.function_text || symbolic!=p.symbolic {
                            edits.push((p.clone(),addr,symbolic,text));
                        }
                        ui.label(&p.connected);
                        if ui.link(&p.location).clicked() {
                            jump = Some((p.page, p.id));
                        }
                        ui.end_row();
                        });
                    }
                });
                if points.is_empty() {
                    ui.weak(tr("No PLC inputs / outputs placed."));
                }
            });
        });
        if data != before {
            self.session.prj.plc_cards.insert(card.clone(), data);
        }
        for (point,address,symbolic,comment) in edits {
            if let Err(problem)=plc::edit_point(&mut self.session.prj,&point,&address,&symbolic,&comment) {
                self.notify(ctx,problem);
            }
        }
        if readdress {
            let n = plc::readdress(&mut self.session.prj, &card);
            self.notify(ctx, tr_fmt!("{}: {} addresses changed", card, n));
        }
        if export {
            let csv = plc::symbol_table(&self.session.prj, &self.session.analysis.netlist);
            self.export(ctx, "plc_symbols.csv".into(), ("CSV", "csv"), csv);
        }
        if import {
            self.io.open(ctx, Purpose::PlcTable);
        }
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
    }
}

/// Choice of the labeling scheme and editing of the project's own schemes.
fn scheme_editor(ui: &mut egui::Ui, s: &mut marking::Settings) {
    let all = marking::schemes(s);
    let current = marking::scheme(s, None).unwrap_or_default();
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("marking_scheme").width(260.0).selected_text(&current.name).show_ui(ui, |ui| {
            for x in &all {
                if ui.selectable_label(x.name == current.name, &x.name).clicked() {
                    s.scheme = x.name.clone();
                }
            }
        });
        if ui.button(tr("Copy")).on_hover_text(tr("New project scheme from the selected one")).clicked() {
            let mut name = tr_fmt!("{} (copy)", current.name);
            while all.iter().any(|x| x.name == name) {
                name.push('+');
            }
            s.schemes.push(Scheme { name: name.clone(), ..current.clone() });
            s.scheme = name;
        }
        let own = s.schemes.iter().any(|x| x.name == current.name);
        if own && ui.button(tr("Remove")).clicked() {
            s.schemes.retain(|x| x.name != current.name);
            s.scheme.clear();
        }
    });
    let Some(x) = s.schemes.iter_mut().find(|x| x.name == current.name) else {
        let cols: Vec<&str> = current.columns.iter().map(|c| c.title()).collect();
        ui.weak(tr_fmt!("Columns: {}", cols.join(", ")));
        ui.weak(tr("Built-in scheme: copy it to change it."));
        return;
    };
    let old_name = x.name.clone();
    egui::Grid::new("marking_scheme_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label(tr("Name"));
        ui.text_edit_singleline(&mut x.name);
        ui.end_row();
        ui.label(tr("Separator"));
        ui.horizontal(|ui| {
            for (sep, title) in [(";", tr("Semicolon")), (",", tr("Comma")), ("\t", tr("Tab"))] {
                if ui.selectable_label(x.separator == sep, title).clicked() {
                    x.separator = sep.into();
                }
            }
        });
        ui.end_row();
        ui.label(tr("Options"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut x.header, tr("Header row"));
            ui.checkbox(&mut x.one_row_per_label, tr("One row per label"));
            ui.checkbox(&mut x.bom, tr("Byte order mark"));
        });
        ui.end_row();
        ui.label(tr("Markers"));
        ui.horizontal(|ui| {
            for c in Category::ALL {
                let mut on = x.categories.is_empty() || x.categories.contains(&c);
                if ui.checkbox(&mut on, c.title()).changed() {
                    if x.categories.is_empty() {
                        x.categories = Category::ALL.to_vec();
                    }
                    x.categories.retain(|k| *k != c);
                    if on {
                        x.categories.push(c);
                        x.categories.sort();
                    }
                    if x.categories.len() == Category::ALL.len() {
                        x.categories.clear();
                    }
                }
            }
        });
        ui.end_row();
        ui.label(tr("Columns"));
        ui.horizontal_wrapped(|ui| {
            let mut remove = None;
            let mut left = None;
            for (i, c) in x.columns.iter().enumerate() {
                ui.group(|ui| {
                    ui.label(c.title());
                    if i > 0 && ui.small_button("◀").on_hover_text(tr("Move left")).clicked() {
                        left = Some(i);
                    }
                    if x.columns.len() > 1 && icons::button(ui, Icon::Close, tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = left {
                x.columns.swap(i - 1, i);
            }
            if let Some(i) = remove {
                x.columns.remove(i);
            }
            ui.menu_button("+", |ui| {
                let missing: Vec<Column> = Column::ALL.into_iter().filter(|c| !x.columns.contains(c)).collect();
                for c in missing {
                    if ui.button(c.title()).clicked() {
                        x.columns.push(c);
                    }
                }
            });
        });
        ui.end_row();
    });
    let new_name = x.name.trim().to_string();
    if new_name != old_name {
        if new_name.is_empty() || all.iter().any(|y| y.name == new_name && y.name != old_name && !marking::is_builtin(&y.name)) {
            x.name = old_name;
        } else if s.scheme == old_name || s.scheme.is_empty() {
            s.scheme = new_name;
        }
    }
}

