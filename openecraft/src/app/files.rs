//! Files of the editor: saving, exports through the registry, imports of the files the
//! user opens (projects, parts, libraries, drawings, tables, …).

use super::*;
use crate::jobs::Job;
use openecraft_ui::part_editor as pe;

/// What an export or import on another thread brings back.
pub(super) enum Done {
    /// The file name, the file type (title and extension) and the content.
    Export { name: String, title: String, extension: String, out: Result<Vec<u8>, String> },
    /// A model file read for the part it was chosen for.
    Model { name: String, model: Result<pe::Model, String> },
    Parts(Result<Vec<parts::Part>, String>),
}

impl App {
    pub(super) fn suggested_name(&self, ext: &str) -> String {
        let base: String = self
            .session.prj
            .info
            .name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        format!("{}.{ext}", if base.is_empty() { "project".into() } else { base })
    }

    pub(super) fn save(&mut self, ctx: &egui::Context, save_as: bool) {
        if self.remote.shared.is_some() && !save_as {
            self.save_remote(Vec::new());
            return;
        }
        // A project opened from the vault (and not saved locally since) goes back to it.
        if self.vault.origin.is_some() && self.vault.signed_in && self.session.file_name.is_none() && !save_as {
            self.check_in_project();
            return;
        }
        let json = self.session.prj.to_json();
        let result = match (&self.session.file_name, save_as || !FileIo::can_save_in_place()) {
            (Some(path), false) => self.io.save_to(path, &json).map(|_| Some(path.clone())),
            _ => {
                let name = self.session.file_name.as_deref().map(io::short_name).map(String::from);
                let name = name.unwrap_or_else(|| self.suggested_name(PROJECT_EXT));
                self.io.save_as(&name, (tr("OpenECraft project"), PROJECT_EXT), &json)
            }
        };
        match result {
            Ok(Some(path)) => {
                self.notify_ok(ctx, tr_fmt!("Saved {}", io::short_name(&path)));
                self.remember_recent(&path);
                self.session.file_name = Some(path);
                self.session.mark_saved();
                // Saved locally: no longer a copy of the server project.
                self.recovery = None;
            }
            Ok(None) => {}
            Err(e) => self.notify_error(ctx, tr_fmt!("Saving failed: {}", e)),
        }
    }

    pub(super) fn export(&mut self, ctx: &egui::Context, name: String, filter: (&str, &str), content: String) {
        self.export_bytes(ctx, name, filter, content.as_bytes());
    }

    /// Adds title blocks, report schemes and plot frames of a form library (file or
    /// the company library of the parts server).
    fn import_forms(&mut self, ctx: &egui::Context, text: &str) {
        match openecraft_core::forms::FormLibrary::from_json(text) {
            Ok(lib) => {
                let (blocks, schemes) = openecraft_core::forms::merge(&mut self.session.prj, lib);
                self.notify(ctx, tr_fmt!("Imported {} title blocks and {} report schemes", blocks, schemes));
            }
            Err(e) => self.notify_error(ctx, tr_fmt!("Not a title block file: {}", e)),
        }
    }

    pub(super) fn parts_actions(&mut self, ctx: &egui::Context, actions: Vec<PartsAction>) {
        for action in actions {
            match action {
                PartsAction::Import => self.io.open(ctx, Purpose::Parts),
                PartsAction::ImportModel(kind) => self.io.open(ctx, Purpose::Model(kind)),
                PartsAction::Export { name, filter, content } => self.export(ctx, name, filter, content),
                PartsAction::SaveFile { name, data } => {
                    let ext = name.rsplit('.').next().filter(|e| *e != name).unwrap_or("bin").to_string();
                    self.export_bytes(ctx, name.clone(), (tr("File"), &ext), &data);
                }
                PartsAction::ImportMacros(text) => self.import_macros(ctx, &text),
                PartsAction::ImportForms(text) => self.import_forms(ctx, &text),
                PartsAction::Message(m) => self.notify(ctx, m),
            }
        }
    }

    /// Runs an exporter on the project (with its texts in `lang` if given) and saves
    /// the file. Exporters that take a page get the current one.
    /// Runs an export; exports with parameters beyond the page and the file name ask
    /// for them first.
    pub(super) fn run_exporter(&mut self, ctx: &egui::Context, e: &openecraft_api::Exporter, lang: Option<&str>) {
        if e.params.iter().any(|p| p.name != "page" && p.name != "file") {
            let page = self.session.prj.pages[self.session.page].name.clone();
            let harness = self.harness_ui.sel.clone().or_else(|| self.session.prj.harnesses.first().map(|h| h.tag.clone())).unwrap_or_default();
            let texts = e
                .params
                .iter()
                .map(|p| match p.name.as_ref() {
                    "page" => page.clone(),
                    "harness" => harness.clone(),
                    _ => String::new(),
                })
                .collect();
            self.export_form = Some((e.clone(), lang.map(String::from), texts));
            return;
        }
        self.run_exporter_with(ctx, e, lang, Map::new());
    }

    /// The window with the parameters of an export.
    pub(super) fn export_form_window(&mut self, ctx: &egui::Context) {
        let Some((e, lang, mut texts)) = self.export_form.take() else { return };
        let (window, esc) = self.dialogs.window(ctx, text(&e.title).to_string(), "export_parameters", None);
        let mut open = !esc;
        let mut run = false;
        let mut cancel = false;
        window.open(&mut open).resizable(false).show(ctx, |ui| {
            egui::Grid::new("export_params").num_columns(2).show(ui, |ui| {
                for (p, t) in e.params.iter().zip(texts.iter_mut()).filter(|(p, _)| p.name != "file") {
                    let name = super::ribbon_ui::humanize(&p.name);
                    let name = if p.required { format!("{name} *") } else { name };
                    ui.label(name).on_hover_text(text(&p.help));
                    if p.kind == openecraft_api::params::Kind::Bool {
                        let mut on = t.trim() == "true";
                        if ui.checkbox(&mut on, "").changed() {
                            *t = on.to_string();
                        }
                    } else {
                        ui.add(egui::TextEdit::singleline(t).hint_text(text(&p.help)).desired_width(320.0));
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                run = icons::text_button(ui, Icon::Download, tr("Export…")).clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if run {
            let params: Vec<openecraft_api::params::Param> = e.params.iter().filter(|p| p.name != "file").cloned().collect();
            let shown: Vec<String> = e.params.iter().zip(&texts).filter(|(p, _)| p.name != "file").map(|(_, t)| t.clone()).collect();
            match super::plugins_ui::form_args(&params, &shown) {
                Ok(args) => {
                    self.run_exporter_with(ctx, &e, lang.as_deref(), args);
                    return;
                }
                Err(err) => self.notify_error(ctx, err),
            }
        }
        if open && !cancel {
            self.export_form = Some((e, lang, texts));
        }
    }

    /// Runs an export on another thread (with an analysis of its own, so that the
    /// editor goes on); the file is saved once it is ready.
    pub(super) fn run_exporter_with(&mut self, ctx: &egui::Context, e: &openecraft_api::Exporter, lang: Option<&str>, extra: Map<String, serde_json::Value>) {
        let (title, extension) = (text(&e.title).to_string(), e.extension.clone());
        let (name, args) = self.export_args(e, lang, extra);
        let mut snapshot = self.session.snapshot(&self.plugins);
        let (e, lang) = (e.clone(), lang.map(String::from));
        let label = tr_fmt!("Exporting {}", name);
        self.jobs.push(Job::start(label, ctx, move || {
            let out = match lang.as_deref() {
                Some(l) => export_in(&snapshot.prj, snapshot.reports.clone(), &e, l, &args),
                None => {
                    let an = snapshot.analyse();
                    (e.run)(&snapshot.prj, &an, &Args(&args))
                }
            };
            Done::Export { name, title, extension, out: out.map(|o| o.into_bytes()) }
        }));
    }

    /// File name and content of an export, made at once; exporters that take a page get
    /// the current one.
    pub(super) fn exporter_output(&self, e: &openecraft_api::Exporter, lang: Option<&str>, extra: Map<String, serde_json::Value>) -> Result<(String, Vec<u8>), String> {
        let (name, args) = self.export_args(e, lang, extra);
        let result = match lang {
            Some(l) => export_in(&self.session.prj, self.plugins.reports().cloned().collect(), e, l, &args),
            None => (e.run)(&self.session.prj, &self.session.analysis, &Args(&args)),
        };
        result.map(|out| (name, out.into_bytes()))
    }

    /// The finished exports and imports.
    pub(super) fn poll_jobs(&mut self, ctx: &egui::Context) {
        let mut i = 0;
        while i < self.jobs.len() {
            let Some(done) = self.jobs[i].take() else {
                i += 1;
                continue;
            };
            let job = self.jobs.remove(i);
            match done {
                Ok(Done::Export { name, title, extension, out: Ok(out) }) => self.export_bytes(ctx, name, (&title, &extension), &out),
                Ok(Done::Export { out: Err(err), .. }) => self.notify_error(ctx, tr_fmt!("Export failed: {}", err)),
                Ok(Done::Model { name, model }) => {
                    let msg = match model.and_then(|m| self.parts_ui.assign_model(&name, m, &mut self.parts_db, &mut self.session.prj)) {
                        Ok(m) => m,
                        Err(e) => tr_fmt!("Model import failed: {}", e),
                    };
                    self.notify(ctx, msg);
                }
                Ok(Done::Parts(list)) => self.add_parts(ctx, list),
                Err(err) => self.notify_error(ctx, tr_fmt!("{} stopped with an error: {}", job.label, err)),
            }
        }
    }

    /// What runs on another thread, for the status bar.
    pub(super) fn jobs_running(&self) -> Vec<&str> {
        self.jobs.iter().map(|j| j.label.as_str()).collect()
    }

    /// File name and arguments of an export; exporters that take a page get the current
    /// one.
    fn export_args(&self, e: &openecraft_api::Exporter, lang: Option<&str>, extra: Map<String, serde_json::Value>) -> (String, Map<String, serde_json::Value>) {
        let takes = |n: &str| e.params.iter().any(|p| p.name == n);
        let page = self.session.prj.pages[self.session.page].name.clone();
        let base = self.suggested_name("x");
        let base = base.trim_end_matches(".x");
        let mut stem = if takes("page") {
            format!("{base}_page_{page}")
        } else if e.extension == "csv" {
            format!("{base}_{}", e.name)
        } else {
            base.to_string()
        };
        if let Some(l) = lang.filter(|l| *l != self.session.prj.languages.source) {
            stem = format!("{stem}_{l}");
        }
        let name = format!("{stem}.{}", e.extension);
        let mut args = extra;
        if takes("page") && !args.contains_key("page") {
            args.insert("page".into(), page.into());
        }
        if takes("file") {
            args.insert("file".into(), name.clone().into());
        }
        (name, args)
    }

    pub(super) fn export_bytes(&mut self, ctx: &egui::Context, name: String, filter: (&str, &str), content: &[u8]) {
        match self.io.save_bytes_as(&name, filter, content) {
            Ok(Some(path)) => self.notify_ok(ctx, tr_fmt!("Exported {}", io::short_name(&path))),
            Ok(None) => {}
            Err(e) => self.notify_error(ctx, tr_fmt!("Export failed: {}", e)),
        }
    }

    pub(super) fn export_dwg(&mut self, ctx: &egui::Context, name: String, content: Result<Vec<u8>, String>) {
        match content {
            Ok(dwg) => self.export_bytes(ctx, name, (tr("DWG drawing"), "dwg"), &dwg),
            Err(e) => self.notify_error(ctx, e),
        }
    }

    pub(super) fn poll_loaded(&mut self, ctx: &egui::Context) {
        let Some(loaded) = self.io.poll() else { return };
        match loaded.purpose {
            Purpose::Project => self.open_loaded_project(ctx, &loaded),
            Purpose::ProjectTemplate => match openecraft_core::archive::read_project(&loaded.bytes) {
                Ok((t, _)) => {
                    let prj = openecraft_core::project_templates::from_template(&t, tr("New project"), &openecraft_ui::io::today());
                    self.replace_project(prj, None);
                }
                Err(e) => self.notify_failure(ctx, tr_fmt!("{} is not an OpenECraft project template.", io::short_name(&loaded.name)), e),
            },
            Purpose::Parts => self.import_parts(ctx, &loaded.name, &loaded.bytes),
            Purpose::Model(kind) => {
                // STEP, IGES and Parasolid files are tessellated on another thread.
                let (name, bytes) = (loaded.name.clone(), loaded.bytes.clone());
                let label = tr_fmt!("Reading {}", io::short_name(&name));
                self.jobs.push(Job::start(label, ctx, move || {
                    let model = pe::read_model(kind, &name, &bytes);
                    Done::Model { name, model }
                }));
            }
            Purpose::PlcTable => match openecraft_core::plc::import_symbol_table(&mut self.session.prj, &loaded.text()) {
                Ok(n) => self.notify(ctx, tr_fmt!("{} PLC function texts updated", n)),
                Err(e) => self.notify_error(ctx, tr_fmt!("Symbol table import failed: {}", e)),
            },
            Purpose::Symbols | Purpose::Macros if openecraft_edz::is_edz(&loaded.bytes) => {
                self.import_parts(ctx, &loaded.name, &loaded.bytes);
            }
            Purpose::Symbols | Purpose::Macros if loaded.name.to_ascii_lowercase().ends_with(".ema") => {
                match openecraft_core::symlib::ema::decode_with_library(&openecraft_edz::decode_text(&loaded.bytes), &self.session.prj.symbols) {
                    Ok(variants) => {
                        let n = openecraft_core::symlib::ema::import_project(&mut self.session.prj, &variants);
                        for m in variants.iter().filter_map(openecraft_core::macros::eplan_window) {
                            match self.macro_lib.macros.iter_mut().find(|old| old.name == m.name) {
                                Some(old) if old.clip.symbol_defs.is_empty() => old.clip.symbol_defs = m.clip.symbol_defs,
                                None => self.macro_lib.upsert(m),
                                _ => {}
                            }
                        }
                        let issues = variants.iter().filter(|v| !v.warnings.is_empty()).count();
                        self.symed.open = true;
                        self.notify(ctx, tr_fmt!("Added {} symbols. {} variants have unresolved graphics or functions.", n, issues));
                    }
                    Err(e) => self.notify_error(ctx, tr_fmt!("Not a symbol library: {}", e)),
                }
            }
            Purpose::Macros => self.import_macros(ctx, &loaded.text()),
            Purpose::Symbols => match openecraft_core::symlib::SymbolLibrary::from_json(&loaded.text()) {
                Ok(lib) => {
                    let (added, replaced) = openecraft_core::symlib::merge(&mut self.session.prj.symbols, lib);
                    self.symed.open = true;
                    self.notify(ctx, tr_fmt!("{} symbols added, {} replaced", added, replaced));
                }
                Err(e) => self.notify_error(ctx, tr_fmt!("Not a symbol library: {}", e)),
            },
            Purpose::Forms => self.import_forms(ctx, &loaded.text()),
            Purpose::Drawing => self.import_drawing(ctx, &loaded),
            Purpose::Table(kind) => {
                let text = if openecraft_core::archive::is_archive(&loaded.bytes) { openecraft_core::xlsx::first_sheet_csv(&loaded.bytes) } else { Ok(loaded.text()) };
                match text {
                    Ok(text) => self.import_table(ctx, kind, &text),
                    Err(e) => self.notify_error(ctx, tr_fmt!("Table import failed: {}", e)),
                }
            }
            Purpose::Dictionary => match translation::from_csv(&mut self.session.prj, &loaded.text()) {
                Ok(n) => self.notify(ctx, tr_fmt!("{} translations imported", n)),
                Err(e) => self.notify_error(ctx, tr_fmt!("Import failed: {}", e)),
            },
            Purpose::OutputTarget => self.output_reports(ctx, &loaded),
            Purpose::Aml | Purpose::DeviceDescription => self.plc_file_loaded(ctx, loaded.purpose, &loaded.name, &loaded.text()),
            Purpose::StructureTemplate => match structure::Structures::from_json(&loaded.text()) {
                Ok(file) => {
                    let n = self.session.prj.structures.merge(&file);
                    self.notify(ctx, tr_fmt!("{} structure identifiers added", n));
                }
                Err(e) => self.notify_error(ctx, e),
            },
            Purpose::Document => self.load_document(ctx, &loaded),
            Purpose::Image => self.load_image(ctx, &loaded),
            Purpose::FormImage | Purpose::Logo => {
                let name = crate::io::short_name(&loaded.name).to_string();
                match openecraft_core::forms::picture(&name, &loaded.bytes) {
                    Some(pic) if loaded.purpose == Purpose::Logo => self.session.prj.info.logo = Some(pic),
                    Some(pic) => self.forms_ui.picture_loaded(pic),
                    None => self.notify(ctx, tr_fmt!("{} is not a PNG or JPEG image", name)),
                }
            }
            Purpose::PageTemplates => match openecraft_core::page_templates::Library::from_json(&loaded.text()) {
                Ok(lib) => {
                    let n = openecraft_core::page_templates::merge(&mut self.session.prj, lib);
                    self.notify(ctx, tr_fmt!("Imported {} page templates", n));
                }
                Err(e) => self.notify_error(ctx, tr_fmt!("Not a page template file: {}", e)),
            },
            Purpose::Compare => match Project::from_json(&loaded.text()) {
                Ok(prj) => self.compare_ui.set_file(io::short_name(&loaded.name), prj),
                Err(e) => self.notify_error(ctx, e),
            },
            Purpose::ChangeRequests => match openecraft_core::review::import_requests(&mut self.session.prj, &loaded.text()) {
                Ok((added, updated)) => self.notify(ctx, tr_fmt!("{} change requests added, {} updated", added, updated)),
                Err(e) => self.notify_error(ctx, e),
            },
            Purpose::PdfComments => {
                let (user, date) = (self.user_name(), self.today());
                match openecraft_core::pdf_comments::import(&mut self.session.prj, &loaded.bytes, &user, &date) {
                    Ok(r) => {
                        self.editor.review_layer = true;
                        let mut msg = tr_fmt!("{} comments, {} replies and {} markups imported", r.comments, r.replies, r.markups);
                        if r.skipped > 0 {
                            msg = format!("{msg}\n{}", tr_fmt!("{} comments were imported before", r.skipped));
                        }
                        if r.outside > 0 {
                            msg = format!("{msg}\n{}", tr_fmt!("{} comments are on pages the project does not have", r.outside));
                        }
                        self.notify(ctx, msg);
                    }
                    Err(e) => self.notify_error(ctx, e),
                }
            }
            Purpose::Preplanning => self.import_preplanning(ctx, &loaded.bytes),
            Purpose::SettingsScheme => match openecraft_core::settings_scheme::apply(&mut self.session.prj, &loaded.text()) {
                Ok(parts) => {
                    let names: Vec<&str> = parts.iter().map(|p| tr(p)).collect();
                    self.notify(ctx, tr_fmt!("Settings taken over: {}", names.join(", ")));
                }
                Err(e) => self.notify_error(ctx, e),
            },
            Purpose::CheckScheme => match openecraft_core::checks::apply_scheme(&mut self.session.prj, &loaded.text()) {
                Ok(()) => self.notify(ctx, tr("Check scheme taken over")),
                Err(e) => self.notify_error(ctx, e),
            },
            Purpose::RuleSets => self.import_rule_sets(ctx, &loaded.text()),
            Purpose::PropertyLibrary => self.import_property_library(ctx, &loaded.text()),
            Purpose::Subproject => self.insert_subproject(ctx, &loaded.bytes),
            Purpose::Integration => self.integration_file_loaded(&loaded.name, loaded.text()),
            Purpose::Ribbon => self.import_ribbon(ctx, &loaded.text()),
            Purpose::Favorites => match self.insert_center.import_favorites(&loaded.text()) {
                Ok(n) => self.notify(ctx, tr_fmt!("{} favorites added", n)),
                Err(e) => self.notify_error(ctx, tr_fmt!("Import failed: {}", e)),
            },
        }
    }

    /// A project file or archive replaces the open project.
    pub(super) fn open_loaded_project(&mut self, ctx: &egui::Context, loaded: &Loaded) {
        match openecraft_core::archive::read_project(&loaded.bytes) {
            Ok((prj, conv)) => {
                // An archive is opened like a new project: saving writes a project file.
                let file = (!openecraft_core::archive::is_archive(&loaded.bytes)).then(|| loaded.name.clone());
                self.replace_project(prj, file);
                let note = conv.message().unwrap_or_else(|| tr_fmt!("Opened {}", io::short_name(&loaded.name)));
                self.notify_ok(ctx, note);
            }
            Err(e) => self.notify_failure(ctx, tr_fmt!("{} could not be opened as an OpenECraft project.", io::short_name(&loaded.name)), e),
        }
    }

    /// Imports a parts file (CSV, JSON, BMEcat, EPLAN XML or EDZ) into the database; it
    /// is read on another thread.
    pub(super) fn import_parts(&mut self, ctx: &egui::Context, name: &str, data: &[u8]) {
        let bytes = data.to_vec();
        let path = name.to_owned();
        let mappings: Vec<_> = self.session.prj.symbols.iter().filter(|s| s.key.starts_with("user:eplan-lib:")).cloned().collect();
        self.jobs.push(Job::start(tr("Reading the parts file").to_string(), ctx, move || {
            Done::Parts(openecraft_edz::folder::read_with_library(std::path::Path::new(&path), &bytes, &mappings).map(|package| package.parts))
        }));
    }

    /// Adds the parts read from a parts file to the database.
    fn add_parts(&mut self, ctx: &egui::Context, list: Result<Vec<parts::Part>, String>) {
        match list {
            Ok(list) => {
                let n = list.len();
                for p in list {
                    if !p.eplan_variants.is_empty() {
                        if let Some(old) = self.session.prj.parts.iter_mut().find(|old| old.number == p.number) {
                            old.eplan_variants = p.eplan_variants.clone();
                            old.mounting_data.rail = p.mounting_data.rail;
                        }
                    }
                    if !p.picture.is_empty() {
                        if let Some(old) = self.session.prj.parts.iter_mut().find(|old| old.number == p.number) {
                            old.picture.clone_from(&p.picture);
                        }
                    }
                    self.parts_db.upsert(p);
                }
                self.parts_db.sort();
                parts::link_unassigned_plc_parts(&mut self.session.prj, &self.parts_db, None);
                self.session.prj.refresh_rail_mounts();
                self.notify(ctx, tr_fmt!("Imported {} parts into the database", n));
            }
            Err(e) => self.notify_error(ctx, tr_fmt!("Parts import failed: {}", e)),
        }
    }

    /// Merges a macro library file into the macro library.
    pub(super) fn import_macros(&mut self, ctx: &egui::Context, text: &str) {
        match openecraft_core::macros::MacroLibrary::from_json(text) {
            Ok(lib) => {
                let n = self.macro_lib.merge(lib);
                self.notify(ctx, tr_fmt!("Imported {} macros", n));
            }
            Err(e) => self.notify_error(ctx, tr_fmt!("Not a macro file: {}", e)),
        }
    }

    /// A DXF / DWG drawing becomes a new page after the current one.
    pub(super) fn import_drawing(&mut self, ctx: &egui::Context, loaded: &Loaded) {
        let name = io::short_name(&loaded.name);
        let description = name.rsplit_once('.').map(|x| x.0).unwrap_or(name).to_string();
        let is_dwg = openecraft_core::dwg::is_dwg(&loaded.bytes);
        match openecraft_core::dwg::import_drawing(&mut self.session.prj, &loaded.bytes, &description, self.session.page, Default::default()) {
            Ok(r) => {
                self.goto(r.page, 0);
                let page = self.session.prj.pages[r.page].name.clone();
                let msg = match (is_dwg, r.skipped.is_empty()) {
                    (false, true) => tr_fmt!("DXF imported as page {}: {} lines, {} texts", page, r.lines, r.texts),
                    (false, false) => tr_fmt!("DXF imported as page {}: {} lines, {} texts (not imported: {})", page, r.lines, r.texts, r.skipped.join(", ")),
                    (true, true) => tr_fmt!("DWG imported as page {}: {} lines, {} texts", page, r.lines, r.texts),
                    (true, false) => tr_fmt!("DWG imported as page {}: {} lines, {} texts (not imported: {})", page, r.lines, r.texts, r.skipped.join(", ")),
                };
                self.notify(ctx, msg);
            }
            Err(e) if is_dwg => self.notify_error(ctx, tr_fmt!("DWG import failed: {}", e)),
            Err(e) => self.notify_error(ctx, tr_fmt!("DXF import failed: {}", e)),
        }
    }
}

/// Runs the exporter `e` on the project with its texts in `lang` (and an analysis in
/// that language).
fn export_in(prj: &Project, reports: Vec<openecraft_api::Report>, e: &openecraft_api::Exporter, lang: &str, args: &Map<String, serde_json::Value>) -> Result<openecraft_api::Export, String> {
    translation::with_language(prj, lang, |prj| {
        let mut an = Analysis::build(prj);
        an.set_reports(reports);
        (e.run)(prj, &an, &Args(args))
    })
}
