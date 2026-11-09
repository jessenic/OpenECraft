//! Plugins menu and windows: commands of the plugins with a form for their parameters,
//! plugin reports and exports, and the plugin manager (enable / disable, load errors).

use super::App;
use egui::RichText;
use openecraft_api::params::{text, Kind, Param};
use openecraft_api::{Command, Origin, Table, PLUGIN_PATH_VAR};
use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::icons::{self, Icon};
use serde_json::{Map, Value};

/// Storage key of the disabled plugin ids.
pub const STORAGE_KEY: &str = "disabled_plugins";
pub const FOLDERS_KEY: &str = "plugin_folders";

#[derive(Default)]
pub struct PluginsUi {
    /// Plugin manager window.
    pub open: bool,
    folder: String,
    folders: Vec<String>,
    /// Command whose parameters are entered, with the text typed per parameter.
    command: Option<(Command, Vec<String>)>,
    /// Answer of the last command run.
    result: Option<Result<String, String>>,
    /// Report shown: its title and table.
    report: Option<(String, String, Result<Table, String>)>,
}

impl PluginsUi {
    pub(super) fn with_folders(folders: Vec<String>) -> Self {
        Self { folders, ..Default::default() }
    }

    pub(super) fn folders(&self) -> &[String] {
        &self.folders
    }

    fn load_folder(&mut self, plugins: &mut openecraft_api::Plugins) -> bool {
        let folder = self.folder.trim();
        if folder.is_empty() || self.folders.iter().any(|p| p == folder) {
            return false;
        }
        let errors = plugins.errors.len();
        let loaded = plugins.list().len();
        plugins.load_dir(std::path::Path::new(folder));
        if plugins.errors.len() != errors && plugins.list().len() == loaded {
            return false;
        }
        self.folders.push(folder.to_string());
        true
    }
}

/// Arguments from the texts of the form; empty fields are left out.
pub(super) fn form_args(params: &[Param], texts: &[String]) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (p, t) in params.iter().zip(texts) {
        if t.trim().is_empty() {
            continue;
        }
        let v = p.kind.parse(t).ok_or_else(|| tr_fmt!("Parameter {} has the wrong type", p.name))?;
        out.insert(p.name.to_string(), v);
    }
    Ok(out)
}

impl App {
    /// Opens the parameter form of a command (runs it at once when it has none).
    pub(super) fn start_plugin_command(&mut self, c: Command) {
        self.plugin_cmds.used(&c.name);
        let texts = vec![String::new(); c.params.len()];
        self.plugins_ui.result = None;
        if c.params.is_empty() {
            self.run_plugin_command(&c, &[]);
        }
        self.plugins_ui.command = Some((c, texts));
    }

    fn run_plugin_command(&mut self, c: &Command, texts: &[String]) {
        let result = form_args(&c.params, texts).and_then(|args| self.plugins.run(c, &mut self.session.prj, &args));
        if result.is_ok() {
            self.find_ui.clear_results();
        }
        self.plugins_ui.result = Some(result.map(|v| match v {
            Value::String(s) => s,
            Value::Null => tr("Done.").to_string(),
            v => serde_json::to_string_pretty(&v).unwrap_or_default(),
        }));
    }

    fn show_plugin_report(&mut self, name: String, title: String) {
        let table = match self.plugins.report(&name) {
            Some(r) => (r.run)(&self.session.prj, &self.session.analysis),
            None => return,
        };
        self.plugins_ui.report = Some((name, title, table));
    }

    /// Plugins whose checks, reports or menu entries changed: the analysis is rebuilt.
    fn plugins_changed(&mut self) {
        self.session.invalidate();
    }

    pub(super) fn plugins_windows(&mut self, ctx: &egui::Context) {
        self.plugin_command_window(ctx);
        self.plugin_report_window(ctx);
    }

    /// Manage plugins: the plugins with what they add, switched on and off.
    pub(super) fn plugins_pane(&mut self, ui: &mut egui::Ui) {
        let mut toggled = Vec::new();
        let mut loaded = false;
        ui.scope(|ui| {
            ui.weak(tr_fmt!("Plugins add commands, checks, reports and exports. External plugins are loaded from the folders in the environment variable {} (one subfolder with a plugin.json per plugin).", PLUGIN_PATH_VAR));
            ui.label(tr("Plugin folder"));
            ui.add(egui::TextEdit::singleline(&mut self.plugins_ui.folder).desired_width(f32::INFINITY))
                .on_hover_text(tr("Folder containing plugin subfolders with plugin.json files"));
            if ui.add_enabled(!self.plugins_ui.folder.trim().is_empty(), egui::Button::new(tr("Load plugins"))).clicked() {
                loaded = self.plugins_ui.load_folder(&mut self.plugins);
            }
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for l in self.plugins.list() {
                    ui.horizontal(|ui| {
                        let mut on = l.enabled;
                        if ui.checkbox(&mut on, RichText::new(text(&l.info.name)).strong()).changed() {
                            toggled.push((l.info.id.clone(), on));
                        }
                        ui.weak(&l.info.version);
                        match &l.origin {
                            Origin::BuiltIn => ui.weak(tr("built-in")),
                            Origin::External(dir) => ui.weak(dir.display().to_string()),
                        };
                    });
                    ui.indent(&l.info.id, |ui| {
                        if !l.info.description.is_empty() {
                            ui.label(text(&l.info.description));
                        }
                        let r = &l.registry;
                        ui.weak(tr_fmt!("{} commands, {} checks, {} reports, {} exports", r.commands.len(), r.checks.len(), r.reports.len(), r.exporters.len()));
                    });
                }
                for e in &self.plugins.errors {
                    ui.label(RichText::new(e).color(crate::style::error()));
                }
            });
        });
        if loaded || !toggled.is_empty() {
            for (id, on) in toggled {
                self.plugins.set_enabled(&id, on);
            }
            self.plugins_changed();
        }
    }

    /// Plugin commands: every command of the plugins by plugin, with a search; the star
    /// pins a command to the Plugins tab.
    pub(super) fn plugin_commands_pane(&mut self, ui: &mut egui::Ui) {
        let mut start = None;
        let mut star = None;
        ui.horizontal(|ui| {
            icons::show(ui, Icon::Find, 16.0);
            ui.add(egui::TextEdit::singleline(&mut self.plugin_cmds.query).hint_text(tr("Search (* and ? as placeholders)…")).desired_width(f32::INFINITY));
        });
        ui.separator();
        let searching = !self.plugin_cmds.query.trim().is_empty();
        let mut found = 0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (plugin, commands) in self.plugin_groups() {
                let hits: Vec<&Command> = commands
                    .iter()
                    .filter(|c| self.plugin_cmds.matches(&[&super::ribbon_ui::humanize(&c.name), text(&c.help), &plugin, &c.name]))
                    .collect();
                if hits.is_empty() {
                    continue;
                }
                found += hits.len();
                let header = format!("{plugin} ({})", hits.len());
                egui::CollapsingHeader::new(header).id_salt(("plugin_commands", &plugin)).open(searching.then_some(true)).show(ui, |ui| {
                    for c in hits {
                        ui.horizontal(|ui| {
                            let fav = self.plugin_cmds.is_favorite(&c.name);
                            let tip = if fav { tr("Remove from favorites") } else { tr("Add to favorites") };
                            if icons::toggle_button(ui, if fav { Icon::StarFilled } else { Icon::Star }, tip, fav).clicked() {
                                star = Some(c.name.clone());
                            }
                            if icons::text_button(ui, Icon::Plugin, &super::ribbon_ui::humanize(&c.name)).on_hover_text(text(&c.help)).clicked() {
                                start = Some(c.clone());
                            }
                            ui.add(egui::Label::new(RichText::new(text(&c.help)).weak()).truncate());
                        });
                    }
                });
            }
            if found == 0 {
                ui.weak(tr("(no entries)"));
            }
        });
        if let Some(name) = star {
            self.plugin_cmds.toggle_favorite(&name);
        }
        if let Some(c) = start {
            self.start_plugin_command(c);
        }
    }

    fn plugin_command_window(&mut self, ctx: &egui::Context) {
        let Some((cmd, mut texts)) = self.plugins_ui.command.take() else { return };
        let (window, esc) = self.dialogs.window(ctx, super::ribbon_ui::humanize(&cmd.name), "plugin_command", None);
        let mut open = !esc;
        let mut run = false;
        let mut cancel = false;
        window.open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(text(&cmd.help));
            if !cmd.params.is_empty() {
                egui::Grid::new("plugin_params").num_columns(2).show(ui, |ui| {
                    for (p, t) in cmd.params.iter().zip(texts.iter_mut()) {
                        let name = super::ribbon_ui::humanize(&p.name);
                        let name = if p.required { format!("{name} *") } else { name };
                        ui.label(name).on_hover_text(text(&p.help));
                        if p.kind == Kind::Bool && !p.required {
                            let label=match Kind::Bool.parse(t) {Some(Value::Bool(true))=>tr("Enabled"),Some(Value::Bool(false))=>tr("Disabled"),_=>tr("Use plugin default")};
                            egui::ComboBox::from_id_salt((&cmd.name,&p.name)).selected_text(label).show_ui(ui,|ui| {
                                ui.selectable_value(t,String::new(),tr("Use plugin default"));
                                ui.selectable_value(t,"true".into(),tr("Enabled"));
                                ui.selectable_value(t,"false".into(),tr("Disabled"));
                            });
                        } else if p.kind == Kind::Bool {
                            let mut on = Kind::Bool.parse(t) == Some(Value::Bool(true));
                            if ui.checkbox(&mut on, "").changed() {
                                *t = on.to_string();
                            }
                        } else {
                            ui.add(egui::TextEdit::singleline(t).hint_text(p.kind.hint()).desired_width(260.0));
                        }
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    run = icons::text_button(ui, Icon::Next, tr("Run")).clicked();
                    cancel = ui.button(tr("Cancel")).clicked();
                });
            }
            match &self.plugins_ui.result {
                Some(Ok(out)) => {
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| ui.monospace(out));
                }
                Some(Err(e)) => {
                    ui.separator();
                    ui.label(RichText::new(e).color(crate::style::error()));
                }
                None => {}
            }
        });
        if run {
            self.run_plugin_command(&cmd, &texts);
        }
        if open && !cancel {
            self.plugins_ui.command = Some((cmd, texts));
        }
    }

    fn plugin_report_window(&mut self, ctx: &egui::Context) {
        let Some((name, title, table)) = self.plugins_ui.report.take() else { return };
        let mut open = true;
        let (mut jump, mut export, mut refresh) = (None, false, false);
        egui::Window::new(title.as_str()).open(&mut open).default_size([700.0, 420.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                export = ui.add_enabled_ui(table.is_ok(), |ui| icons::text_button(ui, Icon::Download, tr("Export CSV"))).inner.clicked();
                refresh = icons::text_button(ui, Icon::Rotate, tr("Update")).clicked();
            });
            ui.separator();
            match &table {
                Ok(t) => {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        egui::Grid::new("plugin_report_grid").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                            for h in t.header_texts() {
                                ui.label(RichText::new(h).strong());
                            }
                            ui.end_row();
                            for (row, target) in t.rows.iter().zip(t.targets.iter().chain(std::iter::repeat(&None))) {
                                for (i, cell) in row.iter().enumerate() {
                                    if i == 0 && target.is_some() {
                                        if ui.link(cell).clicked() {
                                            jump = *target;
                                        }
                                    } else {
                                        ui.label(cell);
                                    }
                                }
                                ui.end_row();
                            }
                        });
                        if t.rows.is_empty() {
                            ui.weak(tr("(no entries)"));
                        }
                    });
                }
                Err(e) => {
                    ui.label(RichText::new(e).color(crate::style::error()));
                }
            }
        });
        if let (true, Ok(t)) = (export, &table) {
            self.export(ctx, format!("{name}.csv"), ("CSV", "csv"), t.to_csv_bom());
        }
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
        if refresh {
            self.show_plugin_report(name, title);
        } else if open {
            self.plugins_ui.report = Some((name, title, table));
        }
    }
}

#[cfg(test)]
mod tests;
