//! Project management window: project database (folders, search, batch changes of
//! project properties, verification of many projects), subprojects, options that
//! show or hide model content, archives and the verification of the open project.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use openecraft_core::archive::{self, ArchiveOptions};
use crate::io::{self, Purpose};
use openecraft_core::model::*;
use openecraft_core::projects::{self, Batch, Database, Entry};
use openecraft_core::issue::Severity;
use openecraft_core::{options, subproject, verify};
use egui::{Color32, RichText};
use openecraft_ui::io as fs;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Database,
    Subprojects,
    Options,
    Archive,
    Verification,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum BatchOp {
    #[default]
    Set,
    Remove,
    Replace,
    Rename,
}

impl BatchOp {
    fn title(self) -> &'static str {
        match self {
            BatchOp::Set => tr("Set value"),
            BatchOp::Remove => tr("Remove property"),
            BatchOp::Replace => tr("Replace text"),
            BatchOp::Rename => tr("Rename property"),
        }
    }
}

#[derive(Default)]
pub struct ProjectsUi {
    pub open: bool,
    tab: Tab,
    /// Stored by the app.
    pub db: Database,
    query: String,
    /// Files of the selected entries.
    selected: BTreeSet<String>,
    verify_scan: bool,
    op: BatchOp,
    key: String,
    /// Value to set, text to find or new property name.
    value: String,
    with: String,
    log: Vec<String>,
    sub_name: String,
    sub_description: String,
    sub_structure: String,
    sub_pages: BTreeSet<u64>,
    new_option: String,
    comment: String,
    pdf: bool,
    compact: bool,
    result: Option<verify::Verification>,
    repaired: Vec<String>,
}

enum Action {
    Open(String),
    Goto(usize, u64),
    Notify(String),
    Extract,
    Insert,
    OpenArchive,
    Archive,
    Batch(Vec<String>),
}

fn scan(folder: &str, verify: bool) -> Vec<Entry> {
    fs::files_in(folder, projects::is_project_file).into_iter().map(|p| Entry::load(&p, verify)).collect()
}

fn severity_icon(ui: &egui::Ui, s: Severity) -> RichText {
    match s {
        Severity::Error => RichText::new("⛔").color(crate::style::error()),
        Severity::Warning => RichText::new("⚠").color(Color32::from_rgb(220, 160, 0)),
        Severity::Info => RichText::new("ℹ").color(ui.visuals().weak_text_color()),
    }
}

impl App {
    pub(super) fn projects_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut action = None;
        ui.scope(|ui| {
            ui.horizontal(|ui| {
                let st = &mut self.projects_ui;
                ui.selectable_value(&mut st.tab, Tab::Database, tr("Project database"));
                ui.selectable_value(&mut st.tab, Tab::Subprojects, tr("Subprojects"));
                ui.selectable_value(&mut st.tab, Tab::Options, tr("Options"));
                ui.selectable_value(&mut st.tab, Tab::Archive, tr("Archive and version"));
                ui.selectable_value(&mut st.tab, Tab::Verification, tr("Verification"));
            });
            ui.separator();
            action = match self.projects_ui.tab {
                Tab::Database => self.database_tab(ui),
                Tab::Subprojects => self.subprojects_tab(ui),
                Tab::Options => self.options_tab(ui),
                Tab::Archive => self.archive_tab(ui),
                Tab::Verification => self.verification_tab(ui),
            };
        });
        match action {
            Some(Action::Open(path)) => match fs::read_file(&path).and_then(|b| io::read_project(&path, &b)) {
                Ok((prj, conv, is_archive)) => {
                    let r = super::Replace::Project(Box::new(prj), (!is_archive).then(|| path.clone()));
                    if let Some(r) = self.discard.request(self.session.dirty(), r) {
                        self.perform(ctx, r);
                        self.notify_ok(ctx, conv.message().unwrap_or_else(|| tr_fmt!("Opened {}", io::short_name(&path))));
                    }
                }
                Err(e) => self.notify_failure(ctx, tr("The file could not be opened as an OpenECraft project."), e),
            },
            Some(Action::Goto(page, id)) => self.goto(page, id),
            Some(Action::Notify(msg)) => self.notify(ctx, msg),
            Some(Action::Extract) => self.extract_subproject(ctx),
            Some(Action::Insert) => self.io.open(ctx, Purpose::Subproject),
            Some(Action::OpenArchive) => self.io.open(ctx, Purpose::Project),
            Some(Action::Archive) => {
                let st = &self.projects_ui;
                let opts = ArchiveOptions { comment: st.comment.clone(), date: fs::today(), pdf: st.pdf, compact: st.compact };
                let bytes = archive::archive(&self.session.prj, &opts);
                self.export_bytes(ctx, self.suggested_name(archive::ARCHIVE_EXT), (tr("OpenECraft project archive"), archive::ARCHIVE_EXT), &bytes);
            }
            Some(Action::Batch(paths)) => self.run_batch(ctx, paths),
            None => {}
        }
    }

    // --------------------------------------------------------------- database

    #[allow(clippy::too_many_lines)]
    fn database_tab(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        if !fs::has_file_system() {
            ui.label(tr("The project database needs the desktop version: the browser cannot search folders."));
            return None;
        }
        let mut action = None;
        let st = &mut self.projects_ui;
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::FolderOpen, tr("Add folder…")).on_hover_text(tr("Adds the project files and archives of a folder and its subfolders")).clicked() {
                if let Some(folder) = fs::pick_folder() {
                    let found = scan(&folder, st.verify_scan);
                    st.db.update_folder(&folder, found);
                }
            }
            if icons::text_button(ui, Icon::Rotate, tr("Update all")).clicked() {
                for folder in st.db.folders.clone() {
                    let found = scan(&folder, st.verify_scan);
                    st.db.update_folder(&folder, found);
                }
            }
            ui.checkbox(&mut st.verify_scan, tr("Verify while reading"));
        });
        let mut remove = None;
        for f in &st.db.folders {
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::Delete, tr("Remove the folder from the database")).clicked() {
                    remove = Some(f.clone());
                }
                ui.label(f);
            });
        }
        if let Some(f) = remove {
            st.db.remove_folder(&f);
        }
        ui.horizontal(|ui| {
            ui.label(tr("Search"));
            ui.add(egui::TextEdit::singleline(&mut st.query).desired_width(320.0))
                .on_hover_text(tr("Terms separated by ';': key=value (equal), key~text (contains) or text found anywhere"));
            if ui.button(tr("Select all")).clicked() {
                st.selected = st.db.search(&st.query).iter().map(|e| e.path.clone()).collect();
            }
            if ui.button(tr("Select none")).clicked() {
                st.selected.clear();
            }
        });
        let found: Vec<Entry> = st.db.search(&st.query).into_iter().cloned().collect();
        let open_file = self.session.file_name.clone();
        egui::ScrollArea::vertical().id_salt("project_db").max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
            egui::Grid::new("project_db_grid").striped(true).num_columns(9).show(ui, |ui| {
                ui.label("");
                for h in [tr("Name"), tr("Description"), tr("Company"), tr("Date"), tr("Revision"), tr("Pages"), tr("Check"), tr("File")] {
                    ui.strong(h);
                }
                ui.end_row();
                for e in &found {
                    let mut on = st.selected.contains(&e.path);
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            st.selected.insert(e.path.clone());
                        } else {
                            st.selected.remove(&e.path);
                        }
                    }
                    if !e.error.is_empty() {
                        ui.colored_label(crate::style::error(), &e.error);
                        for _ in 0..6 {
                            ui.label("");
                        }
                    } else {
                        let name = if open_file.as_deref() == Some(e.path.as_str()) { RichText::new(&e.name).strong() } else { RichText::new(&e.name) };
                        if ui.link(name).on_hover_text(tr("Open the project")).clicked() {
                            action = Some(Action::Open(e.path.clone()));
                        }
                        ui.label(&e.description);
                        ui.label(&e.company);
                        ui.label(&e.date);
                        ui.label(&e.revision);
                        ui.label(e.pages.to_string());
                        match e.verified {
                            Some((0, 0)) => ui.label("✔"),
                            Some((err, warn)) => ui.label(super::errors_warnings(err, warn)),
                            None => ui.label("–"),
                        };
                    }
                    let mut file = io::short_name(&e.path).to_string();
                    if e.archive {
                        file = tr_fmt!("{} (archive)", file);
                    }
                    if !e.parent.is_empty() {
                        file = tr_fmt!("{} (subproject of {})", file, e.parent);
                    }
                    ui.label(file).on_hover_text(&e.path);
                    ui.end_row();
                }
            });
            if found.is_empty() {
                ui.label(tr("No projects. Add a folder with project files."));
            }
        });
        let selected: Vec<String> = st.selected.iter().filter(|p| st.db.entries.iter().any(|e| &e.path == *p && e.error.is_empty())).cloned().collect();
        ui.horizontal(|ui| {
            ui.label(tr_fmt!("{} projects selected", selected.len()));
            if ui.add_enabled(!selected.is_empty(), egui::Button::new(tr("Verify selected"))).clicked() {
                for p in &selected {
                    if let Ok(bytes) = fs::read_file(p) {
                        st.db.upsert(Entry::read(p, &bytes, true));
                    }
                }
            }
        });
        ui.separator();
        ui.strong(tr("Change project properties of the selected projects"));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("batch_op").selected_text(st.op.title()).show_ui(ui, |ui| {
                for op in [BatchOp::Set, BatchOp::Remove, BatchOp::Replace, BatchOp::Rename] {
                    ui.selectable_value(&mut st.op, op, op.title());
                }
            });
            ui.label(tr("Property"));
            let keys: Vec<String> = projects::FIELDS.iter().map(|f| f.to_string()).chain(st.db.property_keys()).collect();
            egui::ComboBox::from_id_salt("batch_key").selected_text(st.key.clone()).show_ui(ui, |ui| {
                if st.op == BatchOp::Replace {
                    ui.selectable_value(&mut st.key, "*".into(), tr("* (all)"));
                }
                for k in keys {
                    ui.selectable_value(&mut st.key, k.clone(), k);
                }
            });
            ui.add(egui::TextEdit::singleline(&mut st.key).desired_width(140.0)).on_hover_text(tr("Field (name, description, company, creator, date, revision) or project property"));
            match st.op {
                BatchOp::Set => {
                    ui.label(tr("Value"));
                    ui.add(egui::TextEdit::singleline(&mut st.value).desired_width(160.0));
                }
                BatchOp::Remove => {}
                BatchOp::Replace => {
                    ui.label(tr("Find"));
                    ui.add(egui::TextEdit::singleline(&mut st.value).desired_width(110.0));
                    ui.label(tr("Replace with"));
                    ui.add(egui::TextEdit::singleline(&mut st.with).desired_width(110.0));
                }
                BatchOp::Rename => {
                    ui.label(tr("New name"));
                    ui.add(egui::TextEdit::singleline(&mut st.value).desired_width(140.0));
                }
            }
            if ui.add_enabled(!selected.is_empty() && !st.key.trim().is_empty(), egui::Button::new(tr_fmt!("Apply to {} projects", selected.len()))).clicked() {
                action = Some(Action::Batch(selected.clone()));
            }
        });
        if !st.log.is_empty() {
            egui::ScrollArea::vertical().id_salt("batch_log").max_height(100.0).show(ui, |ui| {
                for l in &st.log {
                    ui.label(l);
                }
            });
        }
        action
    }

    fn run_batch(&mut self, ctx: &egui::Context, paths: Vec<String>) {
        let st = &mut self.projects_ui;
        let key = st.key.trim().to_string();
        let mut b = Batch::default();
        match st.op {
            BatchOp::Set => {
                b.set.insert(key, st.value.clone());
            }
            BatchOp::Remove => b.remove.push(key),
            BatchOp::Replace => b.replace.push((key, st.value.clone(), st.with.clone())),
            BatchOp::Rename => b.rename.push((key, st.value.trim().to_string())),
        }
        let mut log = Vec::new();
        let date = fs::today();
        for path in paths {
            let name = io::short_name(&path).to_string();
            // The open project is changed in the editor (one undo step), not in its file.
            if self.session.file_name.as_deref() == Some(path.as_str()) {
                match b.apply(&mut self.session.prj) {
                    Ok(c) => log.push(tr_fmt!("{}: {} changes in the open project (not saved yet)", name, c.len())),
                    Err(e) => log.push(format!("{name}: {e}")),
                }
                continue;
            }
            match b.apply_on_disk(&path, &date, true) {
                Ok((c, Some(entry))) => {
                    log.push(tr_fmt!("{}: {} changes saved", name, c.len()));
                    self.projects_ui.db.upsert(entry);
                }
                Ok((_, None)) => log.push(tr_fmt!("{}: nothing to change", name)),
                Err(e) => log.push(format!("{name}: {e}")),
            }
        }
        let n = log.len();
        self.projects_ui.log = log;
        self.notify(ctx, tr_fmt!("Project properties changed in {} projects", n));
    }

    // ------------------------------------------------------------- subprojects

    fn subprojects_tab(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let st = &mut self.projects_ui;
        if let Some(p) = &self.session.prj.subprojects.parent {
            ui.label(tr_fmt!("This project is subproject {} of {} (taken out {}). Save it and insert it into the main project when the work is done.", p.name, p.project, p.exported));
            ui.separator();
        }
        let page_name = |id: &u64| self.session.prj.pages.iter().find(|p| p.id == *id).map(|p| p.name.clone()).unwrap_or_else(|| "?".into());
        let mut discard = None;
        if self.session.prj.subprojects.children.is_empty() {
            ui.label(tr("No subprojects were taken out of this project."));
        } else {
            egui::Grid::new("subprojects_grid").striped(true).num_columns(4).show(ui, |ui| {
                for h in [tr("Name"), tr("Pages"), tr("State"), ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for c in &self.session.prj.subprojects.children {
                    ui.label(&c.name).on_hover_text(&c.description);
                    ui.label(c.pages.iter().map(page_name).collect::<Vec<_>>().join(", "));
                    if c.out {
                        ui.label(tr_fmt!("Out since {} (pages locked)", c.exported));
                        if ui.button(tr("Discard")).on_hover_text(tr("Gives up a subproject: its pages are unlocked again")).clicked() {
                            discard = Some(c.name.clone());
                        }
                    } else {
                        ui.label(tr_fmt!("Inserted {}", c.inserted));
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        }
        if let Some(name) = discard {
            if let Err(e) = subproject::discard(&mut self.session.prj, &name) {
                action = Some(Action::Notify(e));
            }
        }
        if ui.button(tr("Insert edited subproject…")).on_hover_text(tr("Its pages replace the pages taken out")).clicked() {
            action = Some(Action::Insert);
        }
        ui.separator();
        ui.strong(tr("Take out a subproject"));
        egui::Grid::new("new_subproject").num_columns(2).show(ui, |ui| {
            ui.label(tr("Name"));
            ui.text_edit_singleline(&mut st.sub_name);
            ui.end_row();
            ui.label(tr("Description"));
            ui.text_edit_singleline(&mut st.sub_description);
            ui.end_row();
            ui.label(tr("Structure"));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.sub_structure).hint_text(tr("=A1+CAB1")).desired_width(140.0));
                if ui.button(tr("Choose pages")).on_hover_text(tr("Chooses the pages whose structure starts with this")).clicked() {
                    if let Ok(ids) = subproject::select_pages(&self.session.prj, &[], &st.sub_structure) {
                        st.sub_pages = ids.into_iter().collect();
                    }
                }
            });
            ui.end_row();
        });
        egui::ScrollArea::vertical().id_salt("sub_pages").max_height(180.0).show(ui, |ui| {
            for pg in &self.session.prj.pages {
                if matches!(pg.kind, PageKind::Mounting(_)) {
                    continue;
                }
                let mut on = st.sub_pages.contains(&pg.id);
                let label = format!("{} {}", openecraft_core::structure::page_full_name(pg), pg.description);
                if ui.add_enabled(!pg.locked || on, egui::Checkbox::new(&mut on, label)).changed() {
                    if on {
                        st.sub_pages.insert(pg.id);
                    } else {
                        st.sub_pages.remove(&pg.id);
                    }
                }
            }
        });
        let n = st.sub_pages.len();
        if ui.add_enabled(n > 0 && !st.sub_name.trim().is_empty(), egui::Button::new(tr_fmt!("Take out {} pages…", n))).clicked() {
            action = Some(Action::Extract);
        }
        action
    }

    fn extract_subproject(&mut self, ctx: &egui::Context) {
        let st = &self.projects_ui;
        let pages: Vec<u64> = self.session.prj.pages.iter().filter(|p| st.sub_pages.contains(&p.id)).map(|p| p.id).collect();
        let mut edited = self.session.prj.clone();
        let sub = match subproject::extract(&mut edited, &st.sub_name, &st.sub_description, &pages, &fs::today()) {
            Ok(s) => s,
            Err(e) => return self.notify_error(ctx, e),
        };
        let name: String = st.sub_name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        match self.io.save_as(&format!("{name}.{}", crate::io::PROJECT_EXT), (tr("OpenECraft project"), crate::io::PROJECT_EXT), &sub.to_json()) {
            Ok(Some(path)) => {
                self.session.prj = edited;
                self.projects_ui.sub_pages.clear();
                self.notify(ctx, tr_fmt!("Subproject saved as {}: its pages are locked until it is inserted", io::short_name(&path)));
            }
            Ok(None) => {}
            Err(e) => self.notify_error(ctx, tr_fmt!("Saving failed: {}", e)),
        }
    }

    pub(super) fn insert_subproject(&mut self, ctx: &egui::Context, loaded: &io::Loaded) {
        let result = io::read_project(&loaded.name, &loaded.bytes).and_then(|(sub, _, _)| subproject::insert(&mut self.session.prj, &sub, &fs::today()));
        match result {
            Ok(r) => {
                self.editor.selection.clear();
                let msg = tr_fmt!("Subproject {} inserted: {} pages replaced by {}", r.name, r.pages_removed, r.pages_inserted);
                self.notify(ctx, msg);
            }
            Err(e) => self.notify_error(ctx, e),
        }
    }

    // ----------------------------------------------------------------- options

    fn options_tab(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.label(tr("Content of an inactive option is hidden: it is not drawn, evaluated, reported or exported."));
        let page_id = self.session.prj.pages.get(self.session.page).map(|p| p.id).unwrap_or(0);
        let selection: Vec<u64> = self.editor.selection.iter().copied().collect();
        if let Some(o) = self.session.prj.model_options.option_of(page_id) {
            ui.label(tr_fmt!("The current page belongs to option {}.", o.name));
        }
        let mut set_active = None;
        let mut assign = None;
        let mut remove = None;
        let config = self.session.prj.configuration.clone();
        let names: Vec<String> = self.session.prj.model_options.options.iter().map(|o| o.name.clone()).collect();
        egui::Grid::new("model_options").striped(true).num_columns(6).show(ui, |ui| {
            for h in [tr("Active"), tr("Name"), tr("Description"), tr("Condition"), tr("Content"), ""] {
                ui.strong(h);
            }
            ui.end_row();
            for (i, name) in names.iter().enumerate() {
                let pages = options::page_count(&self.session.prj, &self.session.prj.model_options.options[i]);
                let o = &mut self.session.prj.model_options.options[i];
                let mut active = o.active;
                let by_hand = o.condition.trim().is_empty();
                if ui.add_enabled(by_hand, egui::Checkbox::new(&mut active, "")).on_disabled_hover_text(tr("Follows its condition: use Apply conditions")).changed() {
                    set_active = Some((name.clone(), active));
                }
                ui.label(name);
                ui.add(egui::TextEdit::singleline(&mut o.description).desired_width(180.0));
                let r = ui.add(egui::TextEdit::singleline(&mut o.condition).hint_text(tr("HEATER=yes")).desired_width(140.0));
                if !o.condition.trim().is_empty() {
                    if let Err(e) = config.holds(&o.condition) {
                        r.on_hover_text(e);
                        ui.colored_label(crate::style::error(), "⚠");
                    }
                }
                ui.label(tr_fmt!("{} objects, {} pages", o.members.len(), pages));
                ui.horizontal(|ui| {
                    if ui.add_enabled(!selection.is_empty(), egui::Button::new(tr("Assign selection"))).clicked() {
                        assign = Some((name.clone(), selection.clone()));
                    }
                    if ui.button(tr("Assign page")).on_hover_text(tr("Makes the current page content of the option")).clicked() {
                        assign = Some((name.clone(), vec![page_id]));
                    }
                    if icons::button(ui, Icon::Delete, tr("Removes the option; its content stays in the project")).clicked() {
                        remove = Some(name.clone());
                    }
                });
                ui.end_row();
            }
        });
        let st = &mut self.projects_ui;
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.new_option).hint_text(tr("New option")).desired_width(160.0));
            if ui.add_enabled_ui(!st.new_option.trim().is_empty(), |ui| icons::text_button(ui, Icon::Plus, tr("Add"))).inner.clicked() {
                match options::define(&mut self.session.prj, &st.new_option, "", "") {
                    Ok(()) => st.new_option.clear(),
                    Err(e) => action = Some(Action::Notify(e)),
                }
            }
            if ui.add_enabled(!selection.is_empty(), egui::Button::new(tr("Take selection out of its option"))).clicked() {
                options::unassign(&mut self.session.prj, &selection);
            }
            if ui.button(tr("Take page out of its option")).clicked() {
                options::unassign(&mut self.session.prj, &[page_id]);
            }
            if ui.button(tr("Apply conditions")).on_hover_text(tr("Switches the options with a condition by the configurator options")).clicked() {
                match options::apply_conditions(&mut self.session.prj) {
                    Ok(s) => action = Some(Action::Notify(tr_fmt!("{} options switched", s.len()))),
                    Err(e) => action = Some(Action::Notify(e)),
                }
            }
        });
        let result = if let Some((name, active)) = set_active {
            options::set_active(&mut self.session.prj, &name, active).map(|_| ())
        } else if let Some((name, ids)) = assign {
            options::assign(&mut self.session.prj, &name, &ids).map(|_| ())
        } else if let Some(name) = remove {
            options::remove(&mut self.session.prj, &name)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            action = Some(Action::Notify(e));
        }
        // Hidden objects can no longer be selected.
        let prj = &self.session.prj;
        self.editor.selection.retain(|id| options::in_pages(prj, *id));
        action
    }

    // ----------------------------------------------------------------- archive

    fn archive_tab(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let st = &mut self.projects_ui;
        ui.label(tr_fmt!("File format version {}. Files of older versions are converted when they are opened.", FORMAT));
        ui.separator();
        ui.label(tr("An archive (.oecz) is a ZIP file with the project, a manifest and the embedded documents and images as files of their own."));
        ui.label(tr("A split project (.oecp) is a folder with one small file per page, device, part and embedded file, for version control such as git: File › Save as split project."));
        egui::Grid::new("archive_opts").num_columns(2).show(ui, |ui| {
            ui.label(tr("Comment"));
            ui.text_edit_singleline(&mut st.comment);
            ui.end_row();
        });
        ui.checkbox(&mut st.pdf, tr("Add a PDF of all pages"));
        ui.checkbox(&mut st.compact, tr("Leave out stored baselines and wiring marks"));
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::Download, tr("Archive project…")).clicked() {
                action = Some(Action::Archive);
            }
            if icons::text_button(ui, Icon::FolderOpen, tr("Open archive…")).clicked() {
                action = Some(Action::OpenArchive);
            }
        });
        action
    }

    // ------------------------------------------------------------ verification

    fn verification_tab(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.label(tr("Checks the project data (ids, references, device data, options, subprojects) together with the engineering checks."));
        ui.horizontal(|ui| {
            if ui.button(tr("Verify project")).clicked() {
                self.projects_ui.result = Some(verify::verify(&self.session.prj));
                self.projects_ui.repaired.clear();
            }
            let repairable = self.projects_ui.result.as_ref().map_or(0, |v| v.repairable());
            if ui.add_enabled(repairable > 0, egui::Button::new(tr_fmt!("Repair {} findings", repairable))).clicked() {
                self.projects_ui.repaired = verify::repair(&mut self.session.prj);
                self.projects_ui.result = Some(verify::verify(&self.session.prj));
            }
        });
        let st = &self.projects_ui;
        for r in &st.repaired {
            ui.label(format!("✔ {r}"));
        }
        let Some(v) = &st.result else { return None };
        ui.label(tr_fmt!(
            "{} errors, {} warnings, {} notes in the project data; engineering check: {} errors, {} warnings",
            v.count(Severity::Error),
            v.count(Severity::Warning),
            v.count(Severity::Info),
            v.check_errors,
            v.check_warnings
        ));
        egui::ScrollArea::vertical().id_salt("verify_findings").auto_shrink([false, true]).show(ui, |ui| {
            for f in &v.findings {
                ui.horizontal_wrapped(|ui| {
                    ui.label(severity_icon(ui, f.severity));
                    let r = ui.add(egui::Label::new(&f.message).sense(egui::Sense::click()));
                    if f.repairable {
                        ui.weak(tr("(repairable)"));
                    }
                    if let Some((page, id)) = f.target {
                        if r.on_hover_text(tr("Click to show")).clicked() {
                            action = Some(Action::Goto(page, id));
                        }
                    }
                });
            }
            if v.findings.is_empty() {
                ui.label(tr("✔ The project data is consistent."));
            }
        });
        action
    }
}
