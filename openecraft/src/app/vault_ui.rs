//! Autodesk Vault window: signing in, browsing and searching the folders of the vault,
//! importing one of its files (project, parts, 2D / 3D model of a part, drawing,
//! document, file of another system) and putting the project, exports and connector
//! exports into the vault. The client is `crate::vault`.

use openecraft_ui::icons::{self, Icon};
use super::App;
use crate::io::{Loaded, Purpose, PROJECT_EXT};
use crate::vault::{Origin, Reply, Use};
use egui::RichText;
use openecraft_api::params::text;
use openecraft_client::vault::{item_records, Entry, FileVersion};
use openecraft_core::integration::{self, CONNECTORS};
use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::part_editor::{self as pe, ModelKind};
use serde_json::Map;

/// Example address of a Vault server (not translated).
const URL_HINT: &str = "https://vault.example.com";

/// What is put into the vault.
#[derive(Clone, Debug, Default, PartialEq)]
enum What {
    #[default]
    Project,
    /// An export of the registry, by name.
    Export(String),
    /// A connector of the integration layer, by index.
    Connector(usize),
}

#[derive(Default)]
pub struct VaultUi {
    /// Use of the selected file; None: by its name.
    using: Option<Use>,
    /// Project part that gets a downloaded model.
    model_part: String,
    /// Connector reading a file of another system.
    connector: usize,
    what: What,
    name: String,
    comment: String,
    /// History revision of the project when it was checked in.
    saving_rev: u64,
}

enum Action {
    SignIn,
    SignOut,
    Refresh,
    Up(usize),
    Enter(String, String),
    Search,
    Import(FileVersion, Use),
    Upload,
    CheckIn,
    SyncItems,
}

impl App {
    /// Handles the answers of the vault.
    pub(super) fn poll_vault(&mut self, ctx: &egui::Context) {
        for reply in self.vault.poll(ctx) {
            match reply {
                Reply::Downloaded(using, file, Ok(bytes)) => self.use_vault_file(ctx, using, &file, bytes),
                Reply::Uploaded(project, Ok(up)) => {
                    if project {
                        let folder = if up.version.parent_folder_id.is_empty() { self.vault.folder() } else { up.version.parent_folder_id.clone() };
                        self.vault.origin = Some(Origin { folder, name: up.version.name.clone() });
                        if self.session.history.rev == self.vault_ui.saving_rev {
                            self.session.mark_saved();
                        }
                    }
                    self.notify(ctx, self.vault.status.clone());
                }
                Reply::Items(Ok(found)) => {
                    if found.is_empty() {
                        self.notify(ctx, tr("No Vault item matches a part of the project"));
                    } else {
                        self.integration_review("plm", "Autodesk Vault", item_records(&found));
                        self.notify(ctx, tr_fmt!("{} Vault items found: review the changes in the integration window", found.len()));
                    }
                }
                Reply::SignedIn(Err(_)) | Reply::Downloaded(_, _, Err(_)) | Reply::Uploaded(_, Err(_)) | Reply::Items(Err(_)) => self.notify_error(ctx, self.vault.status.clone()),
                Reply::SignedIn(Ok(_)) | Reply::Contents(..) | Reply::SignedOut => {}
            }
        }
    }

    /// A downloaded file of the vault is used as chosen.
    fn use_vault_file(&mut self, ctx: &egui::Context, using: Use, file: &FileVersion, bytes: Vec<u8>) {
        let loaded = |purpose| Loaded { purpose, name: file.name.clone(), bytes: bytes.clone() };
        match using {
            Use::Project => match openecraft_core::archive::read_project(&bytes) {
                Ok((prj, conv)) => {
                    // Saving goes back to the vault (Check in project), not to a local file.
                    self.replace_project(prj, None);
                    let folder = if file.parent_folder_id.is_empty() { self.vault.folder() } else { file.parent_folder_id.clone() };
                    self.vault.origin = Some(Origin { folder, name: file.name.clone() });
                    let note = conv.message().unwrap_or_else(|| tr_fmt!("Opened {} from Vault (version {})", file.name, file.version_number()));
                    self.notify(ctx, note);
                }
                Err(e) => self.notify_error(ctx, e),
            },
            Use::Parts => self.import_parts(ctx, "", &bytes),
            Use::Model => {
                let number = self.vault_ui.model_part.clone();
                let kind = if file.name.to_lowercase().ends_with(".svg") { ModelKind::Front2d } else { ModelKind::Mesh3d };
                let part = self.session.prj.parts.iter_mut().chain(self.parts_db.parts.iter_mut()).find(|p| p.number == number);
                let msg = match part {
                    Some(part) => pe::apply_model(part, kind, &file.name, &bytes).unwrap_or_else(|e| tr_fmt!("Model import failed: {}", e)),
                    None => tr_fmt!("Model import failed: {}", tr_fmt!("part {} not found", number)),
                };
                self.notify(ctx, msg);
            }
            Use::Drawing => self.import_drawing(ctx, &loaded(Purpose::Drawing)),
            Use::Document => self.load_document(ctx, &loaded(Purpose::Document)),
            Use::Integration => {
                let id = CONNECTORS[self.vault_ui.connector.min(CONNECTORS.len() - 1)].id;
                self.integration_review(id, &file.name, String::from_utf8_lossy(&bytes).into_owned());
            }
            Use::Save => {
                let ext = file.name.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_else(|| "bin".into());
                self.export_bytes(ctx, file.name.clone(), (tr("File"), &ext), &bytes);
            }
        }
    }

    pub(super) fn vault_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut actions = Vec::new();
        ui.scope(|ui| {
            self.vault_login(ui, &mut actions);
            if self.vault.signed_in {
                ui.separator();
                self.vault_browser(ui, &mut actions);
                ui.separator();
                self.vault_import(ui, &mut actions);
                ui.separator();
                self.vault_export(ui, &mut actions);
                ui.separator();
                if ui
                    .button(tr("Update part links from Vault items"))
                    .on_hover_text(tr("Finds the Vault items of the project parts by item number or Part Number property and shows the changes of item ID, revision and lifecycle state for review"))
                    .clicked()
                {
                    actions.push(Action::SyncItems);
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if self.vault.busy() {
                    ui.spinner();
                }
                ui.label(RichText::new(&self.vault.status).weak());
            });
        });
        for a in actions {
            self.vault_action(ctx, a);
        }
    }

    fn vault_login(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let v = &mut self.vault;
        if v.signed_in {
            ui.horizontal(|ui| {
                ui.label(tr_fmt!("Signed in to {} as {}", v.vault_name, v.user));
                ui.weak(v.url.trim());
                if ui.button(tr("Sign out")).clicked() {
                    actions.push(Action::SignOut);
                }
            });
            return;
        }
        egui::Grid::new("vault_login").num_columns(2).show(ui, |ui| {
            ui.label(tr("Server"));
            ui.add(egui::TextEdit::singleline(&mut v.url).hint_text(URL_HINT).desired_width(320.0));
            ui.end_row();
            ui.label(tr("Vault"));
            ui.add(egui::TextEdit::singleline(&mut v.vault_name).desired_width(320.0));
            ui.end_row();
            ui.label(tr("User name"));
            ui.add(egui::TextEdit::singleline(&mut v.user).desired_width(320.0));
            ui.end_row();
            ui.label(tr("Password"));
            let r = ui.add(egui::TextEdit::singleline(&mut v.password).password(true).desired_width(320.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                actions.push(Action::SignIn);
            }
            ui.end_row();
            ui.label(tr("Access token"));
            ui.add(egui::TextEdit::singleline(&mut v.token).password(true).hint_text(tr("Instead of user name and password")).desired_width(320.0));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(!v.url.trim().is_empty() && !v.busy(), egui::Button::new(tr("Sign in"))).clicked() {
                actions.push(Action::SignIn);
            }
            ui.weak(tr("Vault Professional 2025.2 or newer (Vault Data API)"));
        });
    }

    fn vault_browser(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let v = &mut self.vault;
        ui.horizontal_wrapped(|ui| {
            if icons::text_button(ui, Icon::Rotate, tr("Refresh")).clicked() {
                actions.push(Action::Refresh);
            }
            if ui.link("$").clicked() {
                actions.push(Action::Up(0));
            }
            for (i, (_, name)) in v.path.iter().enumerate() {
                ui.label("/");
                if ui.link(name).clicked() {
                    actions.push(Action::Up(i + 1));
                }
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut v.search).hint_text(tr("Search below this folder")).desired_width(320.0));
            if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || icons::text_button(ui, Icon::Find, tr("Search")).clicked() {
                actions.push(Action::Search);
            }
        });
        egui::ScrollArea::vertical().id_salt("vault_entries").max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
            egui::Grid::new("vault_grid").num_columns(4).striped(true).show(ui, |ui| {
                ui.strong(tr("Name"));
                ui.strong(tr("Version"));
                ui.strong(tr("Revision"));
                ui.strong(tr("State"));
                ui.end_row();
                for (i, e) in v.entries.iter().enumerate() {
                    match e {
                        Entry::Folder(f) => {
                            if ui.selectable_label(false, format!("🗀 {}", f.name)).clicked() {
                                actions.push(Action::Enter(f.id.clone(), f.name.clone()));
                            }
                            ui.label("");
                            ui.label("");
                            ui.label("");
                        }
                        Entry::File(f) => {
                            let name = if f.is_checked_out { format!("{} 🔒", f.name) } else { f.name.clone() };
                            let r = ui.selectable_label(v.selected == Some(i), name);
                            if r.clicked() {
                                v.selected = Some(i);
                                self.vault_ui.using = None;
                            }
                            if f.is_checked_out {
                                r.on_hover_text(tr("Checked out"));
                            }
                            ui.label(f.version_number().to_string());
                            ui.label(&f.revision);
                            ui.label(&f.state);
                        }
                        Entry::Item(it) => {
                            ui.label(format!("{} {}", it.number, it.title));
                            ui.label(it.version.round().to_string());
                            ui.label(&it.revision);
                            ui.label(&it.state);
                        }
                    }
                    ui.end_row();
                }
            });
        });
        if v.total > v.entries.len() {
            ui.weak(tr_fmt!("{} of {} entries shown: search to narrow the list", v.entries.len(), v.total));
        }
    }

    fn vault_import(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let file = match self.vault.selected.and_then(|i| self.vault.entries.get(i)) {
            Some(Entry::File(f)) => f.clone(),
            _ => {
                ui.weak(tr("Select a file to import it"));
                return;
            }
        };
        let st = &mut self.vault_ui;
        let using = st.using.unwrap_or_else(|| Use::guess(&file.name));
        ui.horizontal_wrapped(|ui| {
            ui.strong(&file.name);
            egui::ComboBox::from_id_salt("vault_use").width(240.0).selected_text(using.title()).show_ui(ui, |ui| {
                for u in Use::ALL {
                    if ui.selectable_label(u == using, u.title()).clicked() {
                        st.using = Some(u);
                    }
                }
            });
            match using {
                Use::Model => {
                    ui.label(tr("Part"));
                    let shown = if st.model_part.is_empty() { tr("(choose a part)") } else { st.model_part.as_str() };
                    egui::ComboBox::from_id_salt("vault_model_part").width(200.0).selected_text(shown).show_ui(ui, |ui| {
                        for p in self.session.prj.parts.iter().chain(self.parts_db.parts.iter()) {
                            ui.selectable_value(&mut st.model_part, p.number.clone(), &p.number);
                        }
                    });
                }
                Use::Integration => {
                    let c = &CONNECTORS[st.connector.min(CONNECTORS.len() - 1)];
                    egui::ComboBox::from_id_salt("vault_connector").width(260.0).selected_text(format!("{} – {}", c.system.title(), tr(c.title))).show_ui(ui, |ui| {
                        for (i, c) in CONNECTORS.iter().enumerate() {
                            ui.selectable_value(&mut st.connector, i, format!("{} – {}", c.system.title(), tr(c.title)));
                        }
                    });
                }
                _ => {}
            }
            let ready = using != Use::Model || !st.model_part.is_empty();
            if ui.add_enabled_ui(ready, |ui| icons::text_button(ui, Icon::Download, tr("Import"))).inner.clicked() {
                actions.push(Action::Import(file.clone(), using));
            }
        });
    }

    fn what_title(&self, what: &What) -> String {
        match what {
            What::Project => tr("Project file").to_string(),
            What::Export(name) => self.plugins.exporter(name).map(|e| text(&e.title).to_string()).unwrap_or_else(|| name.clone()),
            What::Connector(i) => CONNECTORS.get(*i).map(|c| format!("{} – {}", c.system.title(), tr(c.title))).unwrap_or_default(),
        }
    }

    /// File name in the vault for `what`.
    fn vault_name_for(&self, what: &What) -> String {
        match what {
            What::Project => self.vault.origin.as_ref().map(|o| o.name.clone()).unwrap_or_else(|| self.suggested_name(PROJECT_EXT)),
            What::Export(name) => self.plugins.exporter(name).map(|e| self.suggested_name(&e.extension)).unwrap_or_default(),
            What::Connector(i) => CONNECTORS.get(*i).map(|c| self.suggested_name(c.extension).replacen('.', &format!("_{}.", c.id), 1)).unwrap_or_default(),
        }
    }

    fn vault_export(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if let Some(o) = &self.vault.origin {
            if icons::text_button(ui, Icon::Upload, &tr_fmt!("Check in project as new version of {}", o.name)).on_hover_text(tr("Puts the project into the folder it was opened from")).clicked() {
                actions.push(Action::CheckIn);
            }
        }
        if self.vault_ui.name.is_empty() {
            self.vault_ui.name = self.vault_name_for(&self.vault_ui.what);
        }
        let mut what = self.vault_ui.what.clone();
        let selected = self.what_title(&what);
        let options: Vec<(What, String)> = std::iter::once(What::Project)
            .chain(self.plugins.exporters().map(|e| What::Export(e.name.clone())))
            .chain((0..CONNECTORS.len()).map(What::Connector))
            .map(|o| {
                let title = self.what_title(&o);
                (o, title)
            })
            .collect();
        let st = &mut self.vault_ui;
        egui::Grid::new("vault_export").num_columns(2).show(ui, |ui| {
            ui.label(tr("Put into the vault"));
            egui::ComboBox::from_id_salt("vault_what").width(360.0).selected_text(selected).show_ui(ui, |ui| {
                for (o, title) in &options {
                    if ui.selectable_label(*o == what, title).clicked() {
                        what = o.clone();
                    }
                }
            });
            ui.end_row();
            ui.label(tr("File name"));
            ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(360.0));
            ui.end_row();
            ui.label(tr("Comment"));
            ui.add(egui::TextEdit::singleline(&mut st.comment).desired_width(360.0));
            ui.end_row();
        });
        if what != self.vault_ui.what {
            self.vault_ui.name = self.vault_name_for(&what);
            self.vault_ui.what = what;
        }
        let target = self.vault.folder_path();
        if ui.add_enabled_ui(!self.vault.busy(), |ui| icons::text_button(ui, Icon::Upload, &tr_fmt!("Upload to {}", target))).inner
            .on_hover_text(tr("A file of the same name in the folder gets a new version (checked out and in), otherwise the file is added"))
            .clicked()
        {
            actions.push(Action::Upload);
        }
    }

    /// Content of what is put into the vault.
    fn vault_content(&mut self, what: &What) -> Result<Vec<u8>, String> {
        match what {
            What::Project => Ok(self.session.prj.to_json().into_bytes()),
            What::Export(name) => {
                let e = self.plugins.exporter(name).cloned().ok_or_else(|| tr_fmt!("Unknown format: {}", name))?;
                let mut extra = Map::new();
                if let Some(h) = self.harness_ui.sel.clone().or_else(|| self.session.prj.harnesses.first().map(|h| h.tag.clone())).filter(|_| e.params.iter().any(|p| p.name == "harness")) {
                    extra.insert("harness".into(), h.into());
                }
                self.exporter_output(&e, None, extra).map(|(_, bytes)| bytes)
            }
            What::Connector(i) => {
                let c = CONNECTORS.get(*i).ok_or_else(|| tr_fmt!("Unknown connector: {}", i))?;
                let today = self.today();
                Ok(integration::export(&mut self.session.prj, &self.session.analysis, c, &today).into_bytes())
            }
        }
    }

    fn vault_action(&mut self, ctx: &egui::Context, a: Action) {
        match a {
            Action::SignIn => self.vault.sign_in(),
            Action::SignOut => self.vault.sign_out(),
            Action::Refresh => self.vault.refresh(),
            Action::Up(depth) => self.vault.up_to(depth),
            Action::Enter(id, name) => self.vault.enter(&id, &name),
            Action::Search => self.vault.find(),
            Action::Import(file, Use::Project) => self.request_replace(ctx, super::Replace::Vault(Box::new(file))),
            Action::Import(file, using) => self.vault.download(file, using),
            Action::Upload => {
                let what = self.vault_ui.what.clone();
                let name = self.vault_ui.name.trim().to_string();
                if name.is_empty() {
                    return self.notify(ctx, tr_fmt!("Missing parameter: {}", tr("File name")));
                }
                match self.vault_content(&what) {
                    Ok(data) => {
                        self.vault_ui.saving_rev = self.session.history.rev;
                        let folder = self.vault.folder();
                        let comment = std::mem::take(&mut self.vault_ui.comment);
                        self.vault.upload(&folder, &name, data, &comment, what == What::Project);
                    }
                    Err(e) => self.notify_error(ctx, tr_fmt!("Export failed: {}", e)),
                }
            }
            Action::CheckIn => self.check_in_project(),
            Action::SyncItems => {
                let numbers = self.session.prj.parts.iter().map(|p| p.number.clone()).collect();
                self.vault.lookup_items(numbers);
            }
        }
    }

    /// Checks the project in as a new version of the file it was opened from.
    pub(super) fn check_in_project(&mut self) {
        let Some(o) = self.vault.origin.clone() else { return };
        self.vault_ui.saving_rev = self.session.history.rev;
        let comment = std::mem::take(&mut self.vault_ui.comment);
        self.vault.upload(&o.folder, &o.name, self.session.prj.to_json().into_bytes(), &comment, true);
    }
}
