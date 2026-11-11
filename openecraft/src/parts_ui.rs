//! Parts management window and part selection dialog. The part widgets are shared
//! with the parts manager (`openecraft_ui::part_editor`).

use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::icons::{self, Icon};
use openecraft_core::model::Project;
use openecraft_core::parts::{self, Part, PartFilter, PartsDb};
use egui::{vec2, RichText};
use openecraft_ui::client::{FileUse, Reply, ServerClient};
use openecraft_ui::part_editor::{self as pe, EditorState, ModelKind, PartTab};
use openecraft_ui::portal::{self, Browser, BrowserEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Database,
    Project,
}

/// Requests the application has to carry out (file dialogs, messages).
pub enum PartsAction {
    Import,
    /// Pick a 2D / 3D model file for the selected part.
    ImportModel(ModelKind),
    Export { name: String, filter: (&'static str, &'static str), content: String },
    /// A file downloaded from the data portal.
    SaveFile { name: String, data: Vec<u8> },
    /// A macro library (JSON) downloaded from the data portal.
    ImportMacros(String),
    Message(String),
}

/// Where the part dropped onto a page comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum DropSource {
    /// In the project or the parts database: placed at once.
    Local,
    /// Only in the example catalog: copied into the parts database first.
    Catalog,
    /// Neither: downloaded from the parts server into the parts database first.
    Server,
}

/// Where a dropped part comes from (project parts, parts database, example catalog,
/// else the parts server).
pub fn drop_source(prj: &Project, db: &PartsDb, number: &str) -> DropSource {
    if parts::lookup(prj, Some(db), number).is_some() {
        DropSource::Local
    } else if parts::catalog().iter().any(|p| p.number == number) {
        DropSource::Catalog
    } else {
        DropSource::Server
    }
}

/// A part dropped onto a page that is downloaded from the parts server first: the page
/// (its id) and the place, both kept until the part has arrived.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingDrop {
    pub number: String,
    pub page: u64,
    pub at: openecraft_core::geom::P,
}

/// Id of the part selection window.
const SELECTOR: &str = "part_selection";

struct SelectRequest {
    tag: String,
}

pub struct PartsUi {
    pub open: bool,
    source: Source,
    filter: PartFilter,
    selected: Option<String>,
    editor: EditorState,
    select: Option<SelectRequest>,
    sel_filter: PartFilter,
    sel_matching: bool,
    sel_part: Option<String>,
    /// Connection to the global parts server.
    pub server: ServerClient,
    show_server: bool,
    /// Data portal window (search on the parts server).
    pub show_portal: bool,
    portal: Browser,
    /// Part (source, number) that receives the next imported model.
    model_target: Option<(Source, String)>,
}

impl Default for PartsUi {
    fn default() -> Self {
        PartsUi {
            open: false,
            source: Source::Database,
            filter: PartFilter::default(),
            selected: None,
            editor: EditorState::default(),
            select: None,
            sel_filter: PartFilter::default(),
            sel_matching: true,
            sel_part: None,
            server: ServerClient::default(),
            show_server: false,
            show_portal: false,
            portal: Browser::new(),
            model_target: None,
        }
    }
}

impl PartsUi {
    /// Puts a model read from the file `name` (see [`pe::read_model`]) into the part it
    /// was requested for.
    pub fn assign_model(&mut self, name: &str, model: pe::Model, db: &mut PartsDb, prj: &mut Project) -> Result<String, String> {
        let (source, number) = self.model_target.take().ok_or("no part selected for the model")?;
        let list = match source {
            Source::Database => &mut db.parts,
            Source::Project => &mut prj.parts,
        };
        let part = list.iter_mut().find(|p| p.number == number).ok_or(tr_fmt!("part {} not found", number))?;
        Ok(pe::assign_model(part, name, model))
    }

    /// Applies finished parts server requests (call every frame). Downloaded parts
    /// are merged into the database; downloaded files are handed to the application.
    pub fn poll_server(&mut self, ctx: &egui::Context, db: &mut PartsDb) -> Vec<PartsAction> {
        let mut actions = Vec::new();
        for reply in self.server.poll() {
            if self.server.read_only() && matches!(reply, Reply::Versions(_)) {
                self.portal.apply(&reply, &self.server.kind.update_candidates(&db.parts));
            } else {
                self.portal.apply(&reply, &db.parts);
            }
            match reply {
                Reply::Parts(Ok(list)) => {
                    let n = list.parts.len();
                    let (added, updated) = db.merge(list.parts);
                    self.server.status = tr_fmt!("Downloaded {} parts (revision {}): {} new, {} updated", n, list.revision, added, updated);
                }
                Reply::File(name, FileUse::Save, Ok(data)) => actions.push(PartsAction::SaveFile { name, data }),
                Reply::File(name, FileUse::Macros, Ok(data)) => match String::from_utf8(data) {
                    Ok(text) => actions.push(PartsAction::ImportMacros(text)),
                    Err(_) => self.server.status = tr_fmt!("Not a macro file: {}", name),
                },
                Reply::SignedIn(Ok(_)) => self.server.favorites(ctx),
                _ => {}
            }
            // Searches and other background requests of the browser are not reported.
            if !self.show_portal || self.server.failed() {
                actions.push(PartsAction::Message(self.server.status.clone()));
            }
        }
        actions
    }

    /// The data portal window: faceted search on the parts server, adding parts to
    /// the database and the update check of the database.
    pub fn portal(&mut self, ui: &mut egui::Ui, db: &mut PartsDb) -> Vec<PartsAction> {
        let ctx = &ui.ctx().clone();
        let mut actions = Vec::new();
        ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                if portal::kind_choice(ui, &mut self.server) {
                    self.portal = Browser::new();
                }
                ui.label(tr("Server URL"));
                ui.add(egui::TextEdit::singleline(&mut self.server.url).desired_width(280.0));
                if self.server.read_only() {
                    let (label, hint) = portal::token_texts(&self.server);
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(&mut self.server.token).password(true).desired_width(160.0).hint_text(hint));
                }
                if ui.add_enabled(!self.server.busy(), egui::Button::new(tr("Test connection"))).clicked() {
                    self.server.check(ctx);
                    self.portal.refresh();
                }
            });
            ui.separator();
            // Results are dragged from here onto a schematic page (imported on the drop).
            self.portal.drag_parts = true;
            if let Some(BrowserEvent::ApplyUpdates) = portal::browser(ui, &mut self.portal, &mut self.server, &db.parts) {
                if let Some(updates) = &self.portal.updates {
                    let n = parts::portal::apply_updates(&mut db.parts, &self.portal.remote, updates);
                    actions.push(PartsAction::Message(tr_fmt!("{} parts of the database updated – synchronize the project parts in parts management", n)));
                }
                self.portal.updates_applied();
            }
        });
        actions
    }

    /// Server URL, token and transfer buttons.
    fn server_panel(&mut self, ui: &mut egui::Ui, db: &PartsDb, selected: Option<Part>) {
        let ctx = ui.ctx().clone();
        egui::Grid::new("server").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(tr("Server type"));
            if portal::kind_choice(ui, &mut self.server) {
                self.portal = Browser::new();
            }
            ui.end_row();
            ui.label(tr("Server URL"));
            ui.add(egui::TextEdit::singleline(&mut self.server.url).desired_width(320.0));
            ui.end_row();
            let (label, hint) = portal::token_texts(&self.server);
            ui.label(label);
            ui.add(egui::TextEdit::singleline(&mut self.server.token).password(true).desired_width(320.0).hint_text(hint));
            ui.end_row();
        });
        let busy = self.server.busy();
        // The EPLAN Data Portal is read in the data portal window.
        let full = !self.server.read_only();
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!busy, egui::Button::new(tr("Test connection"))).clicked() {
                self.server.check(&ctx);
            }
            if ui.add_enabled_ui(!busy && full, |ui| icons::text_button(ui, Icon::Download, tr("Download into database"))).inner
                .on_hover_text(tr("Adds new parts and updates existing ones in the local parts database"))
                .clicked()
            {
                self.server.download(&ctx);
            }
            let upload_one = ui.add_enabled_ui(!busy && full && selected.is_some(), |ui| icons::text_button(ui, Icon::Upload, tr("Upload selected part"))).inner;
            if upload_one.clicked() {
                if let Some(p) = &selected {
                    self.server.upload(&ctx, std::slice::from_ref(p));
                }
            }
            if ui.add_enabled_ui(!busy && full && !db.parts.is_empty(), |ui| icons::text_button(ui, Icon::Upload, &tr_fmt!("Upload database ({})", db.parts.len()))).inner
                .on_hover_text(tr("Adds / replaces all parts of the local database on the server"))
                .clicked()
            {
                self.server.upload(&ctx, &db.parts);
            }
            if busy {
                ui.spinner();
            }
        });
        if !full {
            ui.weak(tr("The EPLAN Data Portal only offers searching and reading parts: open the data portal window to add its parts."));
        }
        if !self.server.status.is_empty() {
            let ok = !self.server.failed();
            let color = if ok { ui.visuals().text_color() } else { crate::style::error() };
            ui.label(RichText::new(&self.server.status).color(color));
        }
    }

    /// Opens the management window with a part selected.
    pub fn show_part(&mut self, number: &str, in_project: bool) {
        self.open = true;
        self.source = if in_project { Source::Project } else { Source::Database };
        self.selected = Some(number.to_string());
    }

    /// Opens the part selection dialog for a device.
    pub fn select_for(&mut self, tag: &str) {
        self.select = Some(SelectRequest { tag: tag.to_string() });
        self.sel_part = None;
    }

    pub fn manager(&mut self, ui: &mut egui::Ui, db: &mut PartsDb, prj: &mut Project) -> Vec<PartsAction> {
        let mut actions = Vec::new();
        ui.scope(|ui| {
            // ---------------------------------------------------------- toolbar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.source, Source::Database, tr("🗄 Parts database"));
                ui.selectable_value(&mut self.source, Source::Project, tr_fmt!("📁 Project parts ({})", prj.parts.len()));
                ui.separator();
                let list: &mut Vec<Part> = match self.source {
                    Source::Database => &mut db.parts,
                    Source::Project => &mut prj.parts,
                };
                if icons::text_button(ui, Icon::Plus, tr("New")).clicked() {
                    let number = parts::unique_number(list, "NEW.PART");
                    list.push(Part { number: number.clone(), manufacturer: "NEW".into(), ..Default::default() });
                    self.selected = Some(number);
                    self.editor.tab = PartTab::General;
                }
                let sel_idx = self.selected.as_ref().and_then(|n| list.iter().position(|p| &p.number == n));
                if ui.add_enabled_ui(sel_idx.is_some(), |ui| icons::text_button(ui, Icon::Copy, tr("Copy"))).inner.clicked() {
                    let mut p = list[sel_idx.unwrap()].clone();
                    p.number = parts::unique_number(list, &format!("{}_COPY", p.number));
                    self.selected = Some(p.number.clone());
                    list.push(p);
                }
                if ui.add_enabled_ui(sel_idx.is_some(), |ui| icons::text_button(ui, Icon::Delete, tr("Delete"))).inner.clicked() {
                    list.remove(sel_idx.unwrap());
                    self.selected = None;
                }
                ui.separator();
                ui.menu_button(tr("Extras"), |ui| {
                    if ui.button(tr("Import parts (CSV / JSON)…")).clicked() {
                        actions.push(PartsAction::Import);
                    }
                    let src = match self.source {
                        Source::Database => db.parts.clone(),
                        Source::Project => prj.parts.clone(),
                    };
                    let base = if self.source == Source::Database { "parts_database" } else { "project_parts" };
                    if ui.button(tr("Export as CSV…")).clicked() {
                        actions.push(PartsAction::Export { name: format!("{base}.csv"), filter: ("CSV", "csv"), content: parts::to_csv(&src) });
                    }
                    if ui.button(tr("Export as EPLAN parts XML…")).on_hover_text(tr("For the import into EPLAN parts management (P_ARTICLE_* attributes, texts in en_US)")).clicked() {
                        actions.push(PartsAction::Export { name: format!("{base}.xml"), filter: ("XML", "xml"), content: openecraft_parts::to_eplan_xml(&src) });
                    }
                    if ui.button(tr("Export as JSON…")).clicked() {
                        let json = serde_json::to_string_pretty(&PartsDb { parts: src }).unwrap_or_default();
                        actions.push(PartsAction::Export { name: format!("{base}.json"), filter: ("JSON", "json"), content: json });
                    }
                    ui.separator();
                    if ui.button(tr("Synchronize project parts from database")).on_hover_text(tr("Updates the project parts with the current database data")).clicked() {
                        let n = parts::sync_project_from_db(prj, db);
                        actions.push(PartsAction::Message(tr_fmt!("{} project parts updated", n)));
                    }
                    if ui.button(tr("Transfer project parts to database")).on_hover_text(tr("Adds parts that only exist in the project to the database")).clicked() {
                        let n = parts::transfer_project_to_db(prj, db);
                        actions.push(PartsAction::Message(tr_fmt!("{} parts added to the database", n)));
                    }
                    if ui.button(tr("Remove unused project parts")).clicked() {
                        let n = parts::purge_unused(prj);
                        actions.push(PartsAction::Message(tr_fmt!("{} unused project parts removed", n)));
                    }
                    ui.separator();
                    if ui
                        .button(tr("Refresh example catalog parts (GEN.*)"))
                        .on_hover_text(tr("Replaces the GEN.* parts in the database with the current example catalog (including models)"))
                        .clicked()
                    {
                        let catalog = parts::default_catalog();
                        let n = catalog.len();
                        for p in catalog {
                            db.upsert(p);
                        }
                        db.sort();
                        actions.push(PartsAction::Message(tr_fmt!("{} example catalog parts refreshed", n)));
                    }
                    if ui.button(tr("Add missing parts of the example catalog")).clicked() {
                        let mut n = 0;
                        for p in parts::default_catalog() {
                            if db.get(&p.number).is_none() {
                                db.parts.push(p);
                                n += 1;
                            }
                        }
                        actions.push(PartsAction::Message(tr_fmt!("{} catalog parts added", n)));
                    }
                });
                ui.separator();
                ui.toggle_value(&mut self.show_server, tr("🌐 Parts server"))
                    .on_hover_text(tr("Exchange parts with a global parts database (openecraft-parts-server) or read the EPLAN Data Portal"));
                ui.toggle_value(&mut self.show_portal, tr("🔎 Data portal"))
                    .on_hover_text(tr("Search the parts server by catalog, product group and technical data, add parts and check the database for updates"));
            });
            ui.separator();
            if self.show_server {
                let selected = self.selected.as_ref().and_then(|n| match self.source {
                    Source::Database => db.get(n).cloned(),
                    Source::Project => prj.parts.iter().find(|p| &p.number == n).cloned(),
                });
                self.server_panel(ui, db, selected);
                ui.separator();
            }

            // ---------------------------------------------------------- body
            let numbers: Vec<String> = db.parts.iter().chain(prj.parts.iter()).map(|p| p.number.clone()).collect();
            let list: &mut Vec<Part> = match self.source {
                Source::Database => &mut db.parts,
                Source::Project => &mut prj.parts,
            };
            let h = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(300.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                    let refs: Vec<&Part> = list.iter().collect();
                    pe::part_filter(ui, "mgr", &refs, &mut self.filter);
                    egui::ScrollArea::vertical().id_salt("ptree").auto_shrink([false, false]).show(ui, |ui| {
                        // Parts are dragged from here onto a schematic page.
                        pe::part_tree_with(ui, "mgr", &refs, &self.filter, &mut self.selected, true);
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    let idx = self.selected.as_ref().and_then(|n| list.iter().position(|p| &p.number == n));
                    match idx {
                        Some(i) => {
                            let duplicate = list.iter().filter(|p| p.number == list[i].number).count() > 1;
                            let before = list[i].number.clone();
                            if let Some(kind) = pe::part_editor(ui, &mut list[i], &mut self.editor, &numbers, duplicate) {
                                self.model_target = Some((self.source, list[i].number.clone()));
                                actions.push(PartsAction::ImportModel(kind));
                            }
                            if list[i].number != before {
                                self.selected = Some(list[i].number.clone());
                            }
                        }
                        None => {
                            ui.weak(tr("Select a part in the tree or create a new one."));
                            ui.add_space(8.0);
                            match self.source {
                                Source::Database => ui.label(tr("The parts database is shared by all projects. Parts are copied into a project when they are assigned to a device.")),
                                Source::Project => ui.label(tr("Project parts are the copies stored in this project; they are used for the bill of materials and the project check.")),
                            };
                        }
                    }
                });
            });
        });
        actions
    }

    /// Part selection dialog for a device. Adds the chosen part to the device.
    pub fn selector(&mut self, ctx: &egui::Context, db: &PartsDb, prj: &mut Project) {
        let Some(req) = &self.select else { return };
        let tag = req.tag.clone();
        let mut open = true;
        let mut chosen: Option<String> = None;
        // Database parts plus parts that exist only in the project.
        let mut all: Vec<Part> = db.parts.clone();
        for p in &prj.parts {
            if db.get(&p.number).is_none() {
                all.push(p.clone());
            }
        }
        let placed = parts::placed_functions(prj, &tag);
        let main = parts::main_kind(prj, &tag);
        // The list takes the room left between the rows above and below it: sized from the
        // available height, the window grew by those rows every frame up to the screen.
        let window = egui::Window::new(tr_fmt!("Part selection – {}", tag)).id(egui::Id::unique(SELECTOR));
        window.open(&mut open).default_size([760.0, 480.0]).collapsible(false).show(ctx, |ui| {
            egui::Panel::top("sel_options").show(ui, |ui| {
                ui.checkbox(&mut self.sel_matching, tr("Device selection"))
                    .on_hover_text(tr("Only parts whose function template provides the device's main function"));
            });
            egui::Panel::bottom("sel_buttons").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(self.sel_part.is_some(), |ui| icons::text_button(ui, Icon::Check, tr("Add to device"))).inner.clicked() {
                        chosen = self.sel_part.clone();
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.select = None;
                    }
                });
            });
            egui::CentralPanel::default().show(ui, |ui| {
                let shown: Vec<&Part> = all
                    .iter()
                    .filter(|p| !self.sel_matching || placed.is_empty() || parts::fits_device(p, &placed, main))
                    .collect();
                let h = ui.available_height();
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(300.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                        pe::part_filter(ui, "sel", &shown, &mut self.sel_filter);
                        egui::ScrollArea::vertical().id_salt("seltree").auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(n) = pe::part_tree(ui, "sel", &shown, &self.sel_filter, &mut self.sel_part) {
                                chosen = Some(n);
                            }
                        });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt("selpart").show(ui, |ui| {
                        match self.sel_part.as_ref().and_then(|n| all.iter().find(|p| &p.number == n)) {
                            Some(p) => pe::part_summary(ui, p),
                            None => {
                                ui.weak(tr("Select a part. Double click adds it to the device."));
                            }
                        }
                    });
                });
            });
        });
        if let Some(number) = chosen {
            parts::add_to_device(prj, db, &tag, &number, 1);
            self.select = None;
        }
        if !open {
            self.select = None;
        }
    }
}

#[cfg(test)]
mod tests;
