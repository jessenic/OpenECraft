//! Plot frames and page templates (tabs of the forms window), the "save page as
//! template" dialog and the files of document pages.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use openecraft_core::document;
use openecraft_core::frame::{self, Labels, Paper, PlotFrame};
use crate::io::{Loaded, Purpose};
use openecraft_core::model::PageKind;
use openecraft_core::page_templates::{self, Library};
use egui::{pos2, vec2, Color32, RichText, Sense, Stroke};

#[derive(Default)]
pub struct PagesUi {
    /// Name of the frame shown in the plot frame tab.
    frame: String,
    /// New name of the edited frame.
    rename: String,
    /// Name entered in the "save page as template" dialog (open while set).
    template_name: Option<String>,
    /// Document page that gets the next loaded file; `None`: a new page.
    pub replace_document: Option<usize>,
}

pub(super) enum TemplateAction {
    Import,
    Export,
}

/// Sketch of a frame: paper, border, strips and title block.
fn frame_preview(ui: &mut egui::Ui, f: &PlotFrame) {
    let (w, h) = f.size();
    let k = (220.0 / w).min(160.0 / h);
    let (resp, painter) = ui.allocate_painter(vec2(w * k + 4.0, h * k + 4.0), Sense::hover());
    let o = resp.rect.min + vec2(2.0, 2.0);
    let r = |a: openecraft_core::geom::P, b: openecraft_core::geom::P| egui::Rect::from_min_max(o + vec2(a.x, a.y) * k, o + vec2(b.x, b.y) * k);
    let line = Stroke::new(1.0, ui.visuals().text_color());
    let thin = Stroke::new(0.5, ui.visuals().weak_text_color());
    painter.rect_filled(r(openecraft_core::geom::p(0.0, 0.0), openecraft_core::geom::p(w, h)), 0.0, Color32::WHITE);
    let (b, a) = (f.border(), f.area());
    painter.rect_stroke(r(b.min, b.max), 0.0, line, egui::StrokeKind::Inside);
    if f.strips {
        for c in 1..f.columns.max(1) {
            let x = o.x + (a.min.x + c as f32 * f.column_width()) * k;
            painter.line_segment([pos2(x, o.y + b.min.y * k), pos2(x, o.y + a.min.y * k)], thin);
        }
        for i in 1..f.rows.max(1) {
            let y = o.y + (a.min.y + i as f32 * f.row_height()) * k;
            painter.line_segment([pos2(o.x + b.min.x * k, y), pos2(o.x + a.min.x * k, y)], thin);
        }
        painter.rect_stroke(r(a.min, a.max), 0.0, thin, egui::StrokeKind::Inside);
    }
    let t = f.title_origin();
    painter.rect_filled(r(t, b.max), 0.0, Color32::from_gray(210));
}

impl App {
    pub(super) fn frame_tab(&mut self, ui: &mut egui::Ui) {
        let names = frame::names(&self.session.prj);
        let pu = &mut self.pages_ui;
        let prj = &mut self.session.prj;
        if !names.contains(&pu.frame) {
            pu.frame = frame::page_frame_name(prj, self.session.page).to_string();
            pu.rename = pu.frame.clone();
        }
        let default = if prj.forms.default_frame.is_empty() { frame::DEFAULT.to_string() } else { prj.forms.default_frame.clone() };
        let custom = prj.forms.frames.iter().position(|f| f.name == pu.frame);
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Plot frame"));
            egui::ComboBox::from_id_salt("frame_sel").selected_text(frame::display_name(&pu.frame)).width(200.0).show_ui(ui, |ui| {
                for n in &names {
                    let tag = if frame::is_builtin(n) { tr("  (built-in)") } else { "" };
                    if ui.selectable_label(&pu.frame == n, format!("{}{tag}", frame::display_name(n))).clicked() {
                        pu.frame = n.clone();
                        pu.rename = n.clone();
                    }
                }
            });
            if ui.button(tr("Copy as new")).on_hover_text(tr("Editable copy of this plot frame")).clicked() {
                let mut f = frame::find(prj, &pu.frame);
                let mut name = tr_fmt!("{} (copy)", frame::display_name(&f.name));
                while names.contains(&name) {
                    name.push('+');
                }
                f.name = name.clone();
                prj.forms.frames.push(f);
                pu.frame = name.clone();
                pu.rename = name;
            }
            let is_default = pu.frame == default;
            if ui.add_enabled(!is_default, egui::Button::new(tr("Use as project default"))).on_hover_text(tr("Frame of all pages that do not choose one")).clicked() {
                prj.forms.default_frame = pu.frame.clone();
            }
            if let Some(i) = custom {
                let used = prj.pages.iter().filter(|p| p.frame == pu.frame).count();
                if ui.add_enabled(used == 0 && !is_default, egui::Button::new(tr("Delete"))).on_hover_text(tr("Only frames no page uses can be deleted")).clicked() {
                    prj.forms.frames.remove(i);
                    pu.frame.clear();
                }
            }
        });
        if pu.frame.is_empty() {
            return;
        }
        ui.weak(tr_fmt!("Project default: {}", frame::display_name(&default)));
        ui.separator();
        let mut f = frame::find(prj, &pu.frame);
        let used = prj.pages.iter().enumerate().filter(|(i, _)| frame::page_frame_name(prj, *i) == pu.frame).count();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.add_enabled_ui(custom.is_some(), |ui| {
                    egui::Grid::new("frame_props").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label(tr("Name"));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut pu.rename).desired_width(160.0));
                            let new = pu.rename.trim().to_string();
                            let ok = !new.is_empty() && new != f.name && !names.contains(&new);
                            if ui.add_enabled(ok, egui::Button::new(tr("Rename"))).on_hover_text(tr("Pages and templates using the frame follow")).clicked() {
                                for pg in prj.pages.iter_mut().filter(|p| p.frame == f.name) {
                                    pg.frame = new.clone();
                                }
                                for t in prj.page_templates.iter_mut().filter(|t| t.frame == f.name) {
                                    t.frame = new.clone();
                                }
                                if prj.forms.default_frame == f.name {
                                    prj.forms.default_frame = new.clone();
                                }
                                f.name = new.clone();
                                pu.frame = new;
                            }
                        });
                        ui.end_row();
                        ui.label(tr("Paper"));
                        egui::ComboBox::from_id_salt("frame_paper").selected_text(f.paper.name()).show_ui(ui, |ui| {
                            for paper in Paper::ALL {
                                ui.selectable_value(&mut f.paper, paper, paper.name());
                            }
                        });
                        ui.end_row();
                        if f.paper == Paper::Custom {
                            ui.label(tr("Width / height (mm)"));
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut f.custom[0]).range(50.0..=5000.0));
                                ui.add(egui::DragValue::new(&mut f.custom[1]).range(50.0..=5000.0));
                            });
                        } else {
                            ui.label(tr("Orientation"));
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut f.landscape, true, tr("Landscape"));
                                ui.radio_value(&mut f.landscape, false, tr("Portrait"));
                            });
                        }
                        ui.end_row();
                        ui.label(tr("Border distance (mm)"));
                        ui.add(egui::DragValue::new(&mut f.margin).range(0.0..=50.0).speed(0.5));
                        ui.end_row();
                        ui.label(tr("Show generated frame"));
                        ui.checkbox(&mut f.visible, "");
                        ui.end_row();
                        ui.label(tr("Column / row strips"));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut f.strips, "");
                            ui.add_enabled(f.strips, egui::DragValue::new(&mut f.strip).range(2.0..=20.0).speed(0.5).suffix(" mm"));
                        });
                        ui.end_row();
                        ui.label(tr("Columns / rows"));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut f.columns).range(1..=60));
                            ui.add(egui::DragValue::new(&mut f.rows).range(1..=40));
                        });
                        ui.end_row();
                        for (label, value, salt) in [(tr("Column labels"), &mut f.column_labels, "col_labels"), (tr("Row labels"), &mut f.row_labels, "row_labels")] {
                            ui.label(label);
                            egui::ComboBox::from_id_salt(salt).selected_text(value.title()).show_ui(ui, |ui| {
                                for l in [Labels::Numbers, Labels::ZeroBasedNumbers, Labels::Letters] {
                                    ui.selectable_value(value, l, l.title());
                                }
                            });
                            ui.end_row();
                        }
                    });
                });
                if custom.is_none() {
                    ui.weak(tr("Built-in frames cannot be changed; copy one to adapt it."));
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                frame_preview(ui, &f);
                ui.label(RichText::new(f.summary()).strong());
                let a = f.area();
                ui.label(tr_fmt!("Drawing area {} x {} mm, column width {} mm", format!("{:.0}", a.width()), format!("{:.0}", a.height()), format!("{:.1}", f.column_width())));
                ui.label(tr_fmt!("Used by {} pages", used));
            });
        });
        if let Some(i) = custom {
            if prj.forms.frames[i] != f {
                prj.forms.frames[i] = f;
            }
        }
    }

    pub(super) fn template_tab(&mut self, ui: &mut egui::Ui) -> Option<TemplateAction> {
        let mut action = None;
        let mut insert = None;
        let mut remove = None;
        ui.horizontal(|ui| {
            if ui.button(tr("Save current page as template…")).clicked() {
                self.open_save_template();
            }
            if ui.button(tr("Import…")).on_hover_text(tr("Page templates (JSON) from another project")).clicked() {
                action = Some(TemplateAction::Import);
            }
            if ui.add_enabled(!self.session.prj.page_templates.is_empty(), egui::Button::new(tr("Export…"))).on_hover_text(tr("The project's own page templates as a file")).clicked() {
                action = Some(TemplateAction::Export);
            }
        });
        ui.weak(tr("A template sets the page type, plot frame, title block, structure and description of a new page and places its content."));
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            egui::Grid::new("page_templates").num_columns(6).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.strong(tr("Name"));
                ui.strong(tr("Page type"));
                ui.strong(tr("Plot frame"));
                ui.strong(tr("Description"));
                ui.strong(tr("Content"));
                ui.label("");
                ui.end_row();
                for t in page_templates::all(&self.session.prj) {
                    let own = self.session.prj.page_templates.iter().any(|x| x.name == t.name);
                    let name = page_templates::display_name(&t.name);
                    ui.label(if own { RichText::new(&name).strong() } else { RichText::new(tr_fmt!("{} (built-in)", name)) });
                    ui.label(t.kind.label());
                    ui.label(if t.frame.is_empty() { tr("(project default)").to_string() } else { frame::display_name(&t.frame) });
                    ui.label(&t.description);
                    let n = t.content.symbols.len() + t.content.wires.len() + t.content.texts.len() + t.content.shapes.len();
                    ui.label(n.to_string());
                    ui.horizontal(|ui| {
                        if ui.button(tr("Insert page")).on_hover_text(tr("New page after the current one")).clicked() {
                            insert = Some(t.clone());
                        }
                        if own && icons::button(ui, Icon::Delete, tr("Delete")).clicked() {
                            remove = Some(t.name.clone());
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if let Some(t) = insert {
            self.insert_template(&t);
        }
        if let Some(name) = remove {
            page_templates::remove(&mut self.session.prj, &name);
        }
        action
    }

    pub(super) fn insert_template(&mut self, t: &page_templates::PageTemplate) {
        let at = (self.session.page + 1).min(self.session.prj.pages.len());
        let i = page_templates::insert(&mut self.session.prj, t, at);
        self.goto(i, 0);
    }

    pub(super) fn template_action(&mut self, ctx: &egui::Context, action: TemplateAction) {
        match action {
            TemplateAction::Import => self.io.open(ctx, Purpose::PageTemplates),
            TemplateAction::Export => {
                let lib = Library { page_templates: self.session.prj.page_templates.clone() };
                self.export(ctx, "page_templates.json".into(), (tr("OpenECraft page templates"), "json"), lib.to_json());
            }
        }
    }

    pub(super) fn open_save_template(&mut self) {
        let pg = &self.session.prj.pages[self.session.page];
        let name = if pg.description.is_empty() { tr("My page").to_string() } else { pg.description.clone() };
        self.pages_ui.template_name = Some(name);
    }

    pub(super) fn save_template_window(&mut self, ctx: &egui::Context) {
        let Some(mut name) = self.pages_ui.template_name.take() else { return };
        let (window, esc) = self.dialogs.window(ctx, tr("Save page as template"), "save_page_template", None);
        let mut open = !esc;
        let mut save = false;
        let mut cancel = false;
        let exists = self.session.prj.page_templates.iter().any(|t| t.name == name.trim());
        window.open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(tr("The page type, plot frame, title block, structure, description and the whole content are saved."));
            ui.horizontal(|ui| {
                ui.label(tr("Name"));
                ui.text_edit_singleline(&mut name);
            });
            if exists {
                ui.colored_label(crate::style::lock(), tr("A template of this name is replaced."));
            }
            ui.horizontal(|ui| {
                save = ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("Save"))).clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if save {
            let t = page_templates::from_page(&self.session.prj, self.session.page, &name);
            page_templates::save(&mut self.session.prj, t);
            self.notify(ctx, tr_fmt!("Page template {} saved", name.trim()));
        } else if open && !cancel {
            self.pages_ui.template_name = Some(name);
        }
    }

    /// A file chosen for a document page: replaces the file of the page waiting for
    /// one, otherwise becomes a new page after the current one.
    pub(super) fn load_document(&mut self, ctx: &egui::Context, loaded: &Loaded) {
        let name = crate::io::short_name(&loaded.name).to_string();
        match self.pages_ui.replace_document.take() {
            Some(i) if i < self.session.prj.pages.len() && self.session.prj.pages[i].kind == PageKind::Document => {
                self.session.prj.pages[i].document = Some(document::Document::new(&name, &loaded.bytes));
            }
            _ => {
                let i = document::add_page(&mut self.session.prj, self.session.page, &name, &loaded.bytes);
                self.goto(i, 0);
            }
        }
        self.notify(ctx, tr_fmt!("Document {} embedded ({})", name, document::size_text(loaded.bytes.len())));
    }

    pub(super) fn save_document(&mut self, ctx: &egui::Context, page: usize) {
        let Some(doc) = self.session.prj.pages[page].document.clone() else { return };
        match doc.bytes() {
            Ok(bytes) => {
                let ext = doc.extension();
                match self.io.save_bytes_as(&doc.name, (doc.kind().title(), &ext), &bytes) {
                    Ok(Some(path)) => self.notify_ok(ctx, tr_fmt!("Exported {}", crate::io::short_name(&path))),
                    Ok(None) => {}
                    Err(e) => self.notify_error(ctx, tr_fmt!("Export failed: {}", e)),
                }
            }
            Err(e) => self.notify_error(ctx, e),
        }
    }
}
