//! The backstage view behind the File tab: a left column of pages and commands (home,
//! new, open, save, import, export, print, info, settings, help, quit) and the page
//! chosen on the right. Esc or the arrow at the top goes back to the project.

use super::*;
use crate::commands;
use crate::ribbon::{Entry, Page, BACKSTAGE};

impl App {
    pub(super) fn backstage_ui(&mut self, ui: &mut egui::Ui, page: Page) {
        let ctx = ui.ctx().clone();
        egui::Panel::left("backstage_nav").exact_size(250.0).show(ui, |ui| {
            ui.add_space(6.0);
            if icons::text_button(ui, Icon::ChevronLeft, tr("Back")).on_hover_text(tr("Back to the project (Esc)")).clicked() {
                self.backstage = None;
            }
            ui.add_space(8.0);
            for e in BACKSTAGE {
                match *e {
                    Entry::Page(p) => {
                        let r = backstage_entry(ui, p.icon(), tr(p.title()), p == page);
                        if r.clicked() {
                            self.backstage = Some(p);
                        }
                    }
                    Entry::Command(c) => {
                        let d = commands::def(c);
                        let enabled = self.cmd_enabled(c);
                        let title = self.save_title(c);
                        let r = ui.add_enabled_ui(enabled, |ui| backstage_entry(ui, d.icon, &title, false)).inner;
                        let tip = self.cmd_tip(c);
                        let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
                        if r.clicked() {
                            self.run(&ctx, c);
                        }
                    }
                    Entry::Separator => {
                        ui.separator();
                    }
                }
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("backstage_page").auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(8.0);
                ui.heading(tr(page.title()));
                ui.add_space(8.0);
                match page {
                    Page::Home => self.backstage_home(ui),
                    Page::New => self.backstage_new(ui),
                    Page::Open => self.backstage_open(ui),
                    Page::Export => self.backstage_export(ui),
                    Page::Info => self.backstage_info(ui),
                    _ => {}
                }
                if !matches!(page, Page::Home) {
                    ui.add_space(12.0);
                    for &c in page.commands() {
                        self.backstage_command(ui, c);
                    }
                }
            });
        });
    }

    /// Tooltip of a command; the one of Save says where the project goes.
    pub(super) fn cmd_tip(&self, c: Cmd) -> String {
        if c != Cmd::Save {
            return tr(commands::def(c).tip).to_string();
        }
        let check_in = self.vault.origin.is_some() && self.vault.signed_in && self.remote.shared.is_none() && self.session.file_name.is_none();
        if check_in {
            self.save_title(c)
        } else {
            self.origin().save_tip()
        }
    }

    /// Title of the save command: where the project goes (file, project server, Vault, download).
    fn save_title(&self, c: Cmd) -> String {
        if c != Cmd::Save {
            return tr(commands::def(c).title).to_string();
        }
        let check_in = self.vault.origin.as_ref().filter(|_| self.vault.signed_in && self.remote.shared.is_none() && self.session.file_name.is_none());
        if self.remote.shared.is_some() {
            tr("Save to project server").to_string()
        } else if let Some(o) = check_in {
            tr_fmt!("Check in {} to Vault", o.name)
        } else if FileIo::can_save_in_place() {
            tr("Save").to_string()
        } else {
            tr("Download project").to_string()
        }
    }

    /// A command as a wide row with its tooltip below the title.
    fn backstage_command(&mut self, ui: &mut egui::Ui, c: Cmd) {
        let d = commands::def(c);
        let enabled = self.cmd_enabled(c);
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                let r = icons::text_button(ui, d.icon, &self.save_title(c));
                let tip = self.cmd_tip(c);
                if !tip.is_empty() {
                    ui.weak(tip);
                }
                if r.clicked() {
                    self.run(&ui.ctx().clone(), c);
                }
            });
        });
    }

    fn backstage_home(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr_fmt!("Project: {}", self.session.prj.info.name)).strong());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for c in [Cmd::NewProject, Cmd::Open, Cmd::OpenDemo, Cmd::StartPage, Cmd::Help] {
                let d = commands::def(c);
                if tile(ui, d.icon, tr(d.title)).clicked() {
                    self.run(&ui.ctx().clone(), c);
                    self.backstage = None;
                }
            }
        });
        ui.add_space(12.0);
        self.recent_list(ui);
    }

    /// Projects opened or saved lately (desktop: open by click).
    pub(super) fn recent_list(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr("Recent projects")).strong());
        if self.recent.is_empty() {
            ui.weak(tr("Projects you open or save appear here."));
            return;
        }
        let mut open = None;
        let mut forget = None;
        for path in &self.recent {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::File, 16.0);
                let r = ui.link(io::short_name(path)).on_hover_text(path);
                if r.clicked() {
                    open = Some(path.clone());
                }
                ui.weak(path);
                if icons::button(ui, Icon::Close, tr("Remove from the list")).clicked() {
                    forget = Some(path.clone());
                }
            });
        }
        if let Some(p) = open {
            self.request_replace(&ui.ctx().clone(), Replace::Path(p));
        }
        if let Some(p) = forget {
            self.recent.retain(|x| *x != p);
        }
    }

    fn backstage_new(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr("New project from template")).strong());
        ui.horizontal_wrapped(|ui| {
            for (name, desc) in openecraft_core::project_templates::BUILTIN {
                if tile(ui, Icon::Template, tr(name)).on_hover_text(tr(desc)).clicked() {
                    if let Some(t) = openecraft_core::project_templates::builtin(name) {
                        let prj = openecraft_core::project_templates::from_template(&t, tr("New project"), &openecraft_ui::io::today());
                        self.request_replace(&ui.ctx().clone(), Replace::Project(Box::new(prj), None));
                    }
                }
            }
        });
        self.backstage_examples(ui);
    }

    fn backstage_examples(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        self.example_list(ui);
    }

    fn backstage_open(&mut self, ui: &mut egui::Ui) {
        self.recent_list(ui);
        ui.add_space(8.0);
        if let Some(sh) = &self.remote.shared {
            ui.label(tr_fmt!("{} (project server, revision {})", sh.name, sh.revision));
        }
    }

    fn backstage_export(&mut self, ui: &mut egui::Ui) {
        let exporters: Vec<openecraft_api::Exporter> = self.plugins.exporters().cloned().collect();
        ui.horizontal_wrapped(|ui| {
            for e in exporters {
                if export_entry(ui, export_icon(&e), &format!("{}…", text(&e.title)), &e.extension).clicked() {
                    let ctx = ui.ctx().clone();
                    self.run_exporter(&ctx, &e, None);
                }
            }
        });
    }

    fn backstage_info(&mut self, ui: &mut egui::Ui) {
        let prj = &self.session.prj;
        let errors = self.session.analysis.issues.iter().filter(|i| i.severity == Severity::Error && !i.accepted).count();
        let warnings = self.session.analysis.issues.iter().filter(|i| i.severity == Severity::Warning && !i.accepted).count();
        let source = self.origin().label();
        egui::Grid::new("backstage_info").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
            for (k, v) in [
                (tr("Project name"), prj.info.name.clone()),
                (tr("Description"), prj.info.description.clone()),
                (tr("Company"), prj.info.company.clone()),
                (tr("Created by"), prj.info.creator.clone()),
                (tr("Revision"), prj.info.revision.clone()),
                (tr("Pages"), prj.pages.len().to_string()),
                (tr("Devices"), self.session.index.devices.len().to_string()),
                (tr("Messages"), super::errors_warnings(errors, warnings)),
                (tr("File"), self.session.file_name.clone().unwrap_or_else(|| tr("unsaved").to_string())),
                (tr("Saved to"), source.clone()),
            ] {
                ui.label(RichText::new(k).strong());
                ui.label(v);
                ui.end_row();
            }
        });
    }
}

fn backstage_entry(ui: &mut egui::Ui, icon: Icon, title: &str, selected: bool) -> egui::Response {
    let h = 30.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, title));
    let v = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 3.0, v.selection.bg_fill);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, v.widgets.hovered.weak_bg_fill);
    }
    let color = if enabled { v.text_color() } else { v.weak_text_color() };
    icons::paint(ui.painter(), egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 6.0), egui::Vec2::splat(18.0)), icon, color);
    ui.painter().text(egui::pos2(rect.min.x + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, title, egui::TextStyle::Button.resolve(ui.style()), color);
    resp
}

/// A large square tile with an icon and a title (backstage, start page).
/// The icon of an export: what it puts out.
fn export_icon(e: &openecraft_api::Exporter) -> Icon {
    let name: &str = e.name.as_ref();
    match e.extension.to_ascii_lowercase().as_str() {
        _ if name.starts_with("plc") || name == "aml" => Icon::Plc,
        _ if name.starts_with("harness") || name.starts_with("kbl") || name == "nailboard" => Icon::Harness,
        _ if name.starts_with("marking") => Icon::Marking,
        _ if name.starts_with("preplanning") => Icon::Preplanning,
        _ if name == "dictionary" => Icon::Languages,
        _ if name == "device_tags" => Icon::Device,
        _ if name == "manufacturing_package" || name == "copper_bar_program" => Icon::Drill,
        "pdf" | "html" => Icon::Print,
        "svg" => Icon::Image,
        "dxf" | "dwg" => Icon::Line,
        "gltf" | "stp" | "igs" | "ifc" | "jt" | "x_t" => Icon::Cube,
        "csv" | "xlsx" => Icon::Table,
        _ => Icon::Upload,
    }
}

/// An entry of the export page: icon, title and the file type.
fn export_entry(ui: &mut egui::Ui, icon: Icon, title: &str, extension: &str) -> egui::Response {
    let size = egui::vec2(330.0, 44.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let v = ui.visuals();
    let stroke = if resp.hovered() { v.widgets.hovered.bg_stroke } else { v.widgets.noninteractive.bg_stroke };
    ui.painter().rect(rect, 6.0, v.faint_bg_color, stroke, egui::StrokeKind::Inside);
    icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.min.x + 22.0, rect.center().y), egui::Vec2::splat(24.0)), icon, v.text_color());
    let ext = ui.painter().layout_no_wrap(format!(".{extension}"), egui::TextStyle::Small.resolve(ui.style()), v.weak_text_color());
    let galley = ui.painter().layout(title.to_string(), egui::TextStyle::Small.resolve(ui.style()), v.text_color(), size.x - 60.0 - ext.size().x);
    ui.painter().galley(egui::pos2(rect.min.x + 42.0, rect.center().y - galley.size().y / 2.0), galley, v.text_color());
    ui.painter().galley(egui::pos2(rect.max.x - 8.0 - ext.size().x, rect.center().y - ext.size().y / 2.0), ext, v.weak_text_color());
    resp
}

pub(super) fn tile(ui: &mut egui::Ui, icon: Icon, title: &str) -> egui::Response {
    let size = egui::vec2(120.0, 92.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let v = ui.visuals();
    let stroke = if resp.hovered() { v.widgets.hovered.bg_stroke } else { v.widgets.noninteractive.bg_stroke };
    ui.painter().rect(rect, 6.0, v.faint_bg_color, stroke, egui::StrokeKind::Inside);
    icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 32.0), egui::Vec2::splat(32.0)), icon, v.text_color());
    let galley = ui.painter().layout(title.to_string(), egui::TextStyle::Small.resolve(ui.style()), v.text_color(), size.x - 10.0);
    ui.painter().galley(egui::pos2(rect.center().x - galley.size().x / 2.0, rect.min.y + 56.0), galley, v.text_color());
    resp
}
