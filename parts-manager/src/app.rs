//! Main window of the parts manager: connection, part tree, part editor.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::session::{FollowUp, Session};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText};
use openecraft_parts::{date_from_unix, to_csv, PartFilter};
use openecraft_parts::portal::{AttachmentKind, Role};
use openecraft_ui::client::{Reply, ServerClient};
use openecraft_ui::io::{short_name, FileIo, FileKind, Loaded};
use openecraft_ui::logos::Logos;
use openecraft_ui::part_editor::{self as pe, EditorState, ModelKind};

/// What a file is opened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// CSV / JSON parts file, uploaded to the server.
    Parts,
    /// 2D / 3D model of the part in the editor.
    Model(ModelKind),
    /// File stored with the part on the server (datasheet, image, macro …).
    Attachment,
    /// Logo of a catalog.
    Logo,
}

impl FileKind for Purpose {
    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Purpose::Parts => (tr("Parts (CSV, JSON, BMEcat, EPLAN XML, EDZ)"), &["csv", "json", "xml", "txt", "edz"]),
            Purpose::Model(kind) => kind.filter(),
            Purpose::Logo => (tr("Images (PNG, JPEG, GIF, WebP, SVG)"), &["png", "jpg", "jpeg", "gif", "webp", "svg"]),
            Purpose::Attachment => (
                tr("Part files (documents, images, models, macros)"),
                &["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "dxf", "dwg", "step", "stp", "igs", "iges", "x_t", "x_b", "stl", "obj", "jt", "ifc", "json", "xml", "txt", "csv", "zip"],
            ),
        }
    }
}

/// What the main area shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Parts,
    Catalogs,
    Users,
}

/// Action waiting for confirmation.
enum Confirm {
    /// Discard unsaved changes, then open this part (`None`: start a new part).
    Discard(Option<String>),
    /// Delete this part on the server.
    Delete(String),
    DeleteCatalog(String),
    DeleteUser(String),
}

pub struct ManagerApp {
    client: ServerClient,
    session: Session,
    editor: EditorState,
    io: FileIo<Purpose>,
    filter: PartFilter,
    confirm: Option<Confirm>,
    /// Part number a model import was requested for.
    model_target: Option<String>,
    /// Catalog a logo was chosen for.
    logo_target: Option<String>,
    logos: Logos,
    /// Show the logos of the catalogs in the list (stored).
    show_logos: bool,
    view: View,
    /// Kind of the next uploaded file (`None`: by file name).
    attach_kind: Option<AttachmentKind>,
    user_name: String,
    password: String,
    started: bool,
    title: String,
    /// Language the status text was made in.
    shown_lang: openecraft_i18n::Lang,
}

const URL_KEY: &str = "server_url";
const LOGOS_KEY: &str = "show_logos";
/// Largest logo the server takes.
const MAX_LOGO: usize = 512 * 1024;

impl ManagerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> ManagerApp {
        let mut client = ServerClient::default();
        openecraft_i18n::init(cc.storage.and_then(|s| eframe::get_value::<String>(s, openecraft_ui::language::STORAGE_KEY)).as_deref());
        openecraft_ui::fonts::install_system_fallbacks(&cc.egui_ctx);
        if let Some(url) = cc.storage.and_then(|s| eframe::get_value::<String>(s, URL_KEY)) {
            client.url = url;
        }
        ManagerApp {
            client,
            session: Session::default(),
            editor: EditorState::default(),
            io: FileIo::default(),
            filter: PartFilter::default(),
            confirm: None,
            model_target: None,
            logo_target: None,
            logos: Logos::default(),
            show_logos: cc.storage.and_then(|s| eframe::get_value::<bool>(s, LOGOS_KEY)).unwrap_or(true),
            view: View::Parts,
            attach_kind: None,
            user_name: String::new(),
            password: String::new(),
            started: false,
            title: String::new(),
            shown_lang: openecraft_i18n::lang(),
        }
    }

    /// True if the signed in user (or the administrator token) may change parts.
    fn writable(&self) -> bool {
        self.client.info.as_ref().is_some_and(|i| i.can_edit()) && !self.client.busy()
    }

    /// Manages users: by role or by a grant of the server's policy.
    fn is_admin(&self) -> bool {
        self.client.info.as_ref().is_some_and(|i| i.is_admin()) || self.client.user().is_some_and(|u| u.role == Role::Admin)
    }

    fn poll(&mut self, ctx: &egui::Context) {
        for reply in self.client.poll() {
            self.logos.receive(&reply);
            match &reply {
                Reply::Info(Ok(_)) => {
                    // Another server, or the same again: its logos may differ.
                    self.logos.clear();
                    self.client.catalogs(ctx);
                }
                Reply::SignedIn(Ok(_)) | Reply::SignedOut => {
                    self.password.clear();
                    self.client.check(ctx);
                }
                _ => {}
            }
            match self.session.apply(&reply) {
                Some(FollowUp::Reload) => self.client.download(ctx),
                Some(FollowUp::Delete(number)) => self.client.delete(ctx, &number),
                Some(FollowUp::Attach(files)) => {
                    self.client.download(ctx);
                    for f in files {
                        self.client.attach(ctx, &f.part, &f.name, Some(f.kind), f.data);
                    }
                }
                None => {}
            }
        }
        if let Some(loaded) = self.io.poll() {
            self.loaded(ctx, loaded);
        }
    }

    fn loaded(&mut self, ctx: &egui::Context, loaded: Loaded<Purpose>) {
        match loaded.purpose {
            Purpose::Parts if openecraft_edz::is_edz(&loaded.bytes) || (loaded.name.to_ascii_lowercase().ends_with(".xml") && openecraft_parts::is_eplan_xml(&loaded.text())) => match self.session.import_package(&loaded.name,&loaded.bytes) {
                Ok(parts) => self.client.upload(ctx, &parts),
                Err(e) => self.client.status = tr_fmt!("Parts import failed: {}", e),
            },
            Purpose::Parts => self.client.upload_file(ctx, loaded.bytes),
            Purpose::Attachment => match self.model_target.take() {
                Some(number) => self.client.attach(ctx, &number, short_name(&loaded.name), self.attach_kind, loaded.bytes),
                None => self.client.status = tr_fmt!("{}: the part is no longer open", short_name(&loaded.name)),
            },
            Purpose::Logo => match self.logo_target.take() {
                Some(_) if loaded.bytes.len() > MAX_LOGO => self.client.status = tr_fmt!("{}: a logo has at most 512 kB", short_name(&loaded.name)),
                Some(id) => self.client.set_catalog_logo(ctx, &id, short_name(&loaded.name), loaded.bytes),
                None => self.client.status = tr_fmt!("{}: the catalog is no longer open", short_name(&loaded.name)),
            },
            Purpose::Model(kind) => {
                let target = self.model_target.take();
                let draft = self.session.draft.as_mut().filter(|d| Some(&d.part.number) == target.as_ref());
                self.client.status = match draft {
                    Some(d) => pe::apply_model(&mut d.part, kind, &loaded.name, &loaded.bytes)
                        .map(|m| tr_fmt!("{} – save to upload it", m))
                        .unwrap_or_else(|e| tr_fmt!("Model import failed: {}", e)),
                    None => tr_fmt!("{}: the part is no longer open", short_name(&loaded.name)),
                };
            }
        }
    }

    /// Opens a part or starts a new one, asking first if changes would be lost.
    fn open(&mut self, number: Option<String>) {
        if self.session.dirty() {
            self.confirm = Some(Confirm::Discard(number));
            return;
        }
        match number {
            Some(n) => self.session.select(&n),
            None => self.session.new_part(),
        }
    }

    fn save(&mut self, ctx: &egui::Context) {
        if let (Some(d), None) = (&self.session.draft, self.session.save_problem()) {
            let part = d.part.clone();
            self.client.save(ctx, &part);
        }
    }

    fn export(&mut self, csv: bool) {
        let (name, filter, content) = if csv {
            ("parts.csv", ("CSV", "csv"), to_csv(&self.session.parts))
        } else {
            ("parts.json", ("JSON", "json"), serde_json::to_string_pretty(&self.session.database()).unwrap_or_default())
        };
        self.client.status = match self.io.save_as(name, filter, &content) {
            Ok(Some(path)) => tr_fmt!("Exported {} parts to {}", self.session.parts.len(), short_name(&path)),
            Ok(None) => return,
            Err(e) => tr_fmt!("Export failed: {}", e),
        };
    }

    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("OpenECraft parts manager")).strong());
            ui.separator();
            ui.label(tr("Server"));
            let url = ui.add(egui::TextEdit::singleline(&mut self.client.url).desired_width(220.0));
            ui.label(tr("Access token"));
            let token = ui.add(
                egui::TextEdit::singleline(&mut self.client.token).password(true).desired_width(210.0).hint_text(tr("needed for changes")),
            );
            let enter = (url.lost_focus() || token.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.add_enabled(!self.client.busy(), egui::Button::new(tr("Connect"))).clicked() || enter {
                self.client.check(&ctx);
            }
            ui.separator();
            match self.client.info.clone() {
                Some(info) => {
                    ui.label(format!("{} {}", info.name, info.version));
                    match &info.user {
                        Some(u) => {
                            let text = if u.role == Role::Manufacturer {
                                tr_fmt!("{} ({}, {})", u.name, tr(u.role.title()), u.manufacturer)
                            } else {
                                tr_fmt!("{} ({})", u.name, tr(u.role.title()))
                            };
                            ui.label(RichText::new(text).color(Color32::from_rgb(60, 160, 90)));
                            if ui.add_enabled(!self.client.busy(), egui::Button::new(tr("Sign out"))).on_hover_text(tr("Ends the session and forgets the token")).clicked() {
                                self.client.sign_out(&ctx);
                            }
                        }
                        None if info.writable => {
                            ui.add(egui::TextEdit::singleline(&mut self.user_name).desired_width(120.0).hint_text(tr("User name")));
                            let pw = ui.add(egui::TextEdit::singleline(&mut self.password).password(true).desired_width(120.0).hint_text(tr("Password")));
                            let enter = pw.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if ui.add_enabled(!self.client.busy() && !self.user_name.trim().is_empty(), egui::Button::new(tr("Sign in"))).clicked() || enter {
                                self.client.sign_in(&ctx, &self.user_name, &self.password);
                            }
                        }
                        None => {
                            ui.label(RichText::new(tr("read-only server")).color(Color32::from_rgb(220, 140, 0)))
                                .on_hover_text(tr("Start openecraft-parts-server with --token to allow changes"));
                        }
                    }
                    ui.weak(tr_fmt!("{} parts, {} catalogs, {}", info.parts, info.catalogs, info.storage));
                }
                None => {
                    ui.weak(tr("not connected"));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut pref = ui.ctx().options(|o| o.theme_preference);
                egui::ComboBox::from_id_salt("theme").selected_text(format!("{pref:?}")).width(70.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut pref, egui::ThemePreference::System, tr("System"));
                    ui.selectable_value(&mut pref, egui::ThemePreference::Light, tr("Light"));
                    ui.selectable_value(&mut pref, egui::ThemePreference::Dark, tr("Dark"));
                });
                if pref != ui.ctx().options(|o| o.theme_preference) {
                    ui.ctx().set_theme(pref);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let connected = self.session.loaded;
        let writable = self.writable();
        let has_draft = self.session.draft.is_some();
        let dirty = self.session.dirty();
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.view, View::Parts, tr("Parts"));
            if ui.selectable_value(&mut self.view, View::Catalogs, tr("Catalogs")).clicked() {
                self.client.catalogs(&ctx);
            }
            if self.is_admin() && ui.selectable_value(&mut self.view, View::Users, tr("Users")).clicked() {
                self.client.users(&ctx);
            }
            ui.separator();
            if self.view != View::Parts {
                openecraft_ui::language::menu(ui);
                ui.checkbox(&mut self.show_logos, tr("Logos")).on_hover_text(tr("Show the logos of the catalogs"));
                if self.client.busy() {
                    ui.spinner();
                }
                return;
            }
            // Without the right to change parts there is nothing a new part could become.
            let no_right = tr("Sign in or connect with the access token to change parts");
            if ui.add_enabled(connected && writable, egui::Button::new(tr("➕ New"))).on_disabled_hover_text(no_right).clicked() {
                self.open(None);
            }
            if ui
                .add_enabled(has_draft && writable, egui::Button::new(tr("⎘ Copy")))
                .on_hover_text(tr("New part based on the open one"))
                .on_disabled_hover_text(no_right)
                .clicked()
            {
                self.session.copy_part();
            }
            let save = ui
                .add_enabled(writable && dirty && self.session.save_problem().is_none(), egui::Button::new(tr("✔ Save")))
                .on_hover_text(tr("Create / update the part on the server (Ctrl+S)"));
            if save.clicked() {
                self.save(&ctx);
            }
            if ui.add_enabled(dirty, egui::Button::new(tr("⟲ Revert"))).on_hover_text(tr("Discard the changes")).clicked() {
                self.session.revert();
            }
            let deletable = self.session.draft.as_ref().and_then(|d| d.original.clone());
            if ui.add_enabled(writable && deletable.is_some(), egui::Button::new(tr("🗑 Delete"))).clicked() {
                self.confirm = deletable.map(Confirm::Delete);
            }
            ui.separator();
            if ui.add_enabled(!self.client.busy() && connected, egui::Button::new(tr("⟳ Reload"))).on_hover_text(tr("Load all parts from the server again")).clicked() {
                self.client.download(&ctx);
            }
            ui.menu_button(tr("Import / export"), |ui| {
                if ui
                    .add_enabled(writable, egui::Button::new(tr("⬆ Upload parts file (CSV / JSON)…")))
                    .on_hover_text(tr("Adds new parts and replaces existing ones on the server"))
                    .clicked()
                {
                    self.io.open(&ctx, Purpose::Parts);
                }
                ui.separator();
                if ui.add_enabled(connected, egui::Button::new(tr("Export as CSV…"))).clicked() {
                    self.export(true);
                }
                if ui.add_enabled(connected, egui::Button::new(tr("Export as JSON…"))).clicked() {
                    self.export(false);
                }
            });
            openecraft_ui::language::menu(ui);
            ui.checkbox(&mut self.show_logos, tr("Logos")).on_hover_text(tr("Show the logos of the catalogs"));
            if self.client.busy() {
                ui.spinner();
            }
        });
    }

    fn part_list(&mut self, ui: &mut egui::Ui) {
        let refs: Vec<_> = self.session.parts.iter().collect();
        pe::part_filter(ui, "parts", &refs, &mut self.filter);
        if let Some(d) = self.session.draft.as_ref().filter(|d| d.original.is_none()) {
            ui.label(RichText::new(tr_fmt!("➕ {} (not saved)", d.part.number)).italics());
        }
        egui::ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]).show(ui, |ui| {
            let refs: Vec<_> = self.session.parts.iter().collect();
            let before = self.session.selected().map(String::from);
            let mut selected = before.clone();
            pe::part_tree(ui, "parts", &refs, &self.filter, &mut selected);
            if selected != before {
                self.open(selected);
            }
        });
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        if !self.session.loaded {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.heading(tr("Connect to a parts server"));
                ui.label(tr("Enter the address of an openecraft-parts-server and click Connect."));
                ui.label(tr("The access token is only needed for changes; it is not stored."));
                ui.add_space(8.0);
                ui.weak(tr("Start a server with:  openecraft-parts-server --token <secret>"));
            });
            return;
        }
        let numbers: Vec<String> = self.session.parts.iter().map(|p| p.number.clone()).collect();
        let problem = self.session.save_problem();
        let dirty = self.session.dirty();
        let may_change = self.session.may_change(self.client.info.as_ref());
        let Some(d) = self.session.draft.as_mut() else {
            ui.weak(tr("Select a part in the tree or create a new one."));
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(&d.part.number);
            match (&d.original, dirty) {
                (None, _) => ui.label(RichText::new(tr("new part, not saved")).color(Color32::from_rgb(220, 140, 0))),
                (Some(_), true) => ui.label(RichText::new(tr("changed, not saved")).color(Color32::from_rgb(220, 140, 0))),
                (Some(_), false) => ui.weak(tr("saved")),
            };
            if let Some(original) = d.original.as_ref().filter(|o| **o != d.part.number) {
                ui.weak(tr_fmt!("(renamed from {})", original));
            }
        });
        // Files and history of saved parts
        if let Some(original) = d.original.clone() {
            let ctx = ui.ctx().clone();
            let can_change = self.client.info.as_ref().is_some_and(|i| i.may_edit(&d.part.manufacturer)) && !self.client.busy() && !dirty;
            egui::CollapsingHeader::new(tr_fmt!("Files and history ({} files)", d.part.attachments.len())).id_salt("files").show(ui, |ui| {
                let mut remove = None;
                for a in &d.part.attachments {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(a.kind.title())).weak());
                        ui.hyperlink_to(&a.name, self.client.endpoint().file_url(&a.id));
                        ui.label(RichText::new(pe::size_text(a.size)).weak());
                        if ui.add_enabled(can_change, egui::Button::new("🗑").small()).on_hover_text(tr("Remove the file from the part")).clicked() {
                            remove = Some(a.id.clone());
                        }
                    });
                }
                if let Some(id) = remove {
                    self.client.detach(&ctx, &original, &id);
                }
                ui.horizontal(|ui| {
                    let kind_text = self.attach_kind.map_or(tr("by file name"), |k| tr(k.title()));
                    egui::ComboBox::from_id_salt("attach_kind").selected_text(kind_text).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.attach_kind, None, tr("by file name"));
                        for k in AttachmentKind::ALL {
                            ui.selectable_value(&mut self.attach_kind, Some(k), tr(k.title()));
                        }
                    });
                    let upload = ui.add_enabled(can_change, egui::Button::new(tr("⬆ Upload file…")));
                    let upload = if dirty { upload.on_disabled_hover_text(tr("Save the part first")) } else { upload };
                    if upload.clicked() {
                        self.model_target = Some(original.clone());
                        self.io.open(&ctx, Purpose::Attachment);
                    }
                });
                match &self.session.history {
                    Some((n, h)) if *n == original => {
                        for e in h {
                            ui.label(format!("{} · {} · {} · {}", tr_fmt!("Revision {}", e.revision), date_from_unix(e.time), tr(e.change.title()), e.user));
                        }
                    }
                    _ => {
                        if ui.add_enabled(!self.client.busy(), egui::Button::new(tr("Show history"))).clicked() {
                            self.client.history(&ctx, &original);
                        }
                    }
                }
            });
        }
        ui.separator();
        let duplicate = problem == Some("Part number already exists");
        // Parts the caller may not change are shown, not edited.
        let editable = may_change && !self.client.busy();
        if !may_change {
            ui.colored_label(Color32::from_rgb(220, 140, 0), tr("Read only: sign in with a user who may change parts of this manufacturer"));
        }
        let picked = ui.add_enabled_ui(editable, |ui| pe::part_editor(ui, &mut d.part, &mut self.editor, &numbers, duplicate)).inner;
        if let Some(kind) = picked {
            self.model_target = Some(d.part.number.clone());
            let ctx = ui.ctx().clone();
            self.io.open(&ctx, Purpose::Model(kind));
        }
    }

    fn catalog_list(&mut self, ui: &mut egui::Ui) {
        let manufacturer = self.client.user().map(|u| u.manufacturer.clone()).unwrap_or_default();
        if ui.add_enabled(self.writable(), egui::Button::new(tr("➕ New catalog"))).clicked() {
            self.session.select_catalog(None, &manufacturer);
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("catalogs").auto_shrink([false, false]).show(ui, |ui| {
            let selected = self.session.catalog.as_ref().and_then(|d| d.original.clone());
            let mut open = None;
            for c in &self.session.catalogs {
                let text = format!("{} – {} ({})", c.manufacturer, c.name, c.parts);
                ui.horizontal(|ui| {
                    if self.show_logos {
                        self.logos.show_in_column(ui, &mut self.client, c, 18.0);
                    }
                    if ui.selectable_label(selected.as_deref() == Some(c.id.as_str()), text).clicked() {
                        open = Some(c.id.clone());
                    }
                });
            }
            if let Some(id) = open {
                self.session.select_catalog(Some(&id), "");
            }
        });
    }

    fn catalog_editor(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let to_save = self.session.catalog_to_save();
        let may_edit = |m: &str| self.client.info.as_ref().is_some_and(|i| i.may_edit(m));
        let busy = self.client.busy();
        let logo_target = self.session.logo_target().map(String::from);
        let Some(d) = self.session.catalog.as_mut() else {
            ui.weak(tr("Select a catalog or create a new one. Parts belong to a catalog through their catalog id (part editor, Data portal tab)."));
            return;
        };
        let editable = may_edit(&d.catalog.manufacturer) && !busy;
        let logo_target = logo_target.filter(|_| editable);
        egui::Grid::new("catalog").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(tr("Catalog id"));
            if d.original.is_some() {
                ui.monospace(&d.catalog.id);
            } else {
                ui.add(egui::TextEdit::singleline(&mut d.catalog.id).desired_width(240.0).hint_text(tr("from the name")).font(egui::TextStyle::Monospace));
            }
            ui.end_row();
            for (label, value) in [(tr("Name"), &mut d.catalog.name), (tr("Manufacturer"), &mut d.catalog.manufacturer), (tr("Version"), &mut d.catalog.version)] {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(value).desired_width(320.0));
                ui.end_row();
            }
            ui.label(tr("Description"));
            ui.add(egui::TextEdit::multiline(&mut d.catalog.description).desired_rows(3).desired_width(320.0));
            ui.end_row();
            ui.label(tr("Logo"));
            ui.horizontal(|ui| {
                // The logo of the stored catalog, whatever the list shows.
                if self.logos.show(ui, &mut self.client, &d.catalog, 40.0).is_none() && d.catalog.logo.is_none() {
                    ui.weak(tr("none"));
                }
                let r = ui.add_enabled(logo_target.is_some(), egui::Button::new(tr("Choose logo…")));
                let r = if d.original.is_some() && editable && logo_target.is_none() { r.on_disabled_hover_text(tr("Save the catalog first")) } else { r };
                if r.clicked() {
                    self.logo_target = logo_target.clone();
                    self.io.open(&ctx, Purpose::Logo);
                }
                if d.catalog.logo.is_some() && ui.add_enabled(logo_target.is_some(), egui::Button::new(tr("Remove logo"))).clicked() {
                    if let Some(id) = &logo_target {
                        self.client.delete_catalog_logo(&ctx, id);
                    }
                }
            });
            ui.end_row();
            if d.original.is_some() {
                ui.label(tr("Parts"));
                ui.label(d.catalog.parts.to_string());
                ui.end_row();
                ui.label(tr("Last change"));
                ui.label(date_from_unix(d.catalog.modified));
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            match &to_save {
                Ok(c) => {
                    if ui.add_enabled(editable, egui::Button::new(tr("✔ Save"))).clicked() {
                        self.client.save_catalog(&ctx, c);
                    }
                }
                Err(problem) => {
                    ui.add_enabled(false, egui::Button::new(tr("✔ Save")));
                    ui.colored_label(Color32::from_rgb(220, 140, 0), tr(problem));
                }
            }
            if let Some(id) = d.original.clone() {
                let deletable = editable && d.catalog.parts == 0;
                let button = ui.add_enabled(deletable, egui::Button::new(tr("🗑 Delete")));
                if d.catalog.parts > 0 {
                    button.on_disabled_hover_text(tr("Only catalogs without parts can be deleted"));
                } else if button.clicked() {
                    self.confirm = Some(Confirm::DeleteCatalog(id));
                }
                if ui.button(tr("Show parts")).clicked() {
                    self.filter.text = d.catalog.id.clone();
                    self.view = View::Parts;
                }
            }
        });
    }

    fn user_list(&mut self, ui: &mut egui::Ui) {
        if ui.add_enabled(!self.client.busy(), egui::Button::new(tr("➕ New user"))).clicked() {
            self.session.select_user(None);
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("users").auto_shrink([false, false]).show(ui, |ui| {
            let selected = self.session.user.as_ref().filter(|d| !d.new).map(|d| d.name.clone());
            let mut open = None;
            for u in &self.session.users {
                let text = if u.provider.is_empty() { format!("{} – {}", u.name, tr(u.role.title())) } else { format!("{} – {} ({})", u.name, tr(u.role.title()), u.provider) };
                if ui.selectable_label(selected.as_deref() == Some(u.name.as_str()), text).clicked() {
                    open = Some(u.name.clone());
                }
            }
            if let Some(n) = open {
                self.session.select_user(Some(&n));
            }
        });
    }

    fn user_editor(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let problem = self.session.user_problem();
        let busy = self.client.busy();
        let Some(d) = self.session.user.as_mut() else {
            ui.weak(tr("Select a user or create a new one."));
            ui.add_space(6.0);
            ui.label(tr("Readers keep favorites, manufacturers maintain the parts and catalogs of their manufacturer, editors all parts, administrators also the users."));
            return;
        };
        egui::Grid::new("user").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(tr("User name"));
            if d.new {
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(200.0));
            } else {
                ui.strong(&d.name);
            }
            ui.end_row();
            ui.label(tr("Role"));
            egui::ComboBox::from_id_salt("role").selected_text(tr(d.update.role.title())).show_ui(ui, |ui| {
                for r in Role::ALL {
                    ui.selectable_value(&mut d.update.role, r, tr(r.title()));
                }
            });
            ui.end_row();
            if d.update.role == Role::Manufacturer {
                ui.label(tr("Manufacturer"));
                ui.add(egui::TextEdit::singleline(&mut d.update.manufacturer).desired_width(200.0));
                ui.end_row();
            }
            if d.provider.is_empty() {
                ui.label(if d.new { tr("Password") } else { tr("New password") });
                ui.add(egui::TextEdit::singleline(&mut d.update.password).password(true).desired_width(200.0).hint_text(if d.new { "" } else { tr("unchanged if empty") }));
                ui.end_row();
                ui.label(tr("Groups"));
                ui.add(egui::TextEdit::singleline(&mut d.groups).desired_width(200.0).hint_text(tr("separated by commas")))
                    .on_hover_text(tr("Grants of the server's access settings can refer to them"));
                ui.end_row();
            } else {
                ui.label(tr("Directory"));
                ui.label(&d.provider).on_hover_text(tr("The directory checks the password and sets the groups at every sign-in"));
                ui.end_row();
                ui.label(tr("Groups"));
                ui.add(egui::Label::new(&d.groups).wrap());
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!busy && problem.is_none(), egui::Button::new(tr("✔ Save"))).clicked() {
                let update = d.request();
                self.client.save_user(&ctx, d.name.trim(), &update);
            }
            if !d.new && ui.add_enabled(!busy, egui::Button::new(tr("🗑 Delete"))).clicked() {
                self.confirm = Some(Confirm::DeleteUser(d.name.clone()));
            }
            if let Some(p) = problem {
                ui.colored_label(Color32::from_rgb(220, 140, 0), tr(p));
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else { return };
        let (text, button) = match confirm {
            Confirm::Discard(_) => {
                let n = self.session.draft.as_ref().map(|d| d.part.number.clone()).unwrap_or_default();
                (tr_fmt!("Discard the unsaved changes of {}?", n), tr_const!("Discard"))
            }
            Confirm::Delete(n) => (tr_fmt!("Delete part {} from the server? This cannot be undone.", n), tr_const!("🗑 Delete")),
            Confirm::DeleteCatalog(id) => (tr_fmt!("Delete catalog {} from the server?", id), tr_const!("🗑 Delete")),
            Confirm::DeleteUser(n) => (tr_fmt!("Delete user {}? Their sessions and favorites are removed.", n), tr_const!("🗑 Delete")),
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("confirm")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(text);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr(button)).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr("Cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => match self.confirm.take() {
                Some(Confirm::Discard(number)) => {
                    self.session.revert();
                    self.open(number);
                }
                Some(Confirm::Delete(number)) => self.client.delete(ctx, &number),
                Some(Confirm::DeleteCatalog(id)) => self.client.delete_catalog(ctx, &id),
                Some(Confirm::DeleteUser(name)) => self.client.delete_user(ctx, &name),
                None => {}
            },
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

impl eframe::App for ManagerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            self.client.check(&ctx);
        }
        self.poll(&ctx);
        if openecraft_i18n::lang() != self.shown_lang {
            // The status was made in the language before.
            self.shown_lang = openecraft_i18n::lang();
            self.client.status.clear();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))) && self.writable() {
            self.save(&ctx);
        }

        egui::Panel::top("connection").show(ui, |ui| self.connection_bar(ui));
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            let color = if self.client.failed() { Color32::from_rgb(220, 60, 60) } else { ui.visuals().text_color() };
            ui.label(RichText::new(&self.client.status).color(color));
        });
        if self.view == View::Users && !self.is_admin() {
            self.view = View::Parts;
        }
        match self.view {
            View::Parts => {
                egui::Panel::left("parts").resizable(true).default_size(300.0).show(ui, |ui| self.part_list(ui));
                egui::CentralPanel::default().show(ui, |ui| self.editor(ui));
            }
            View::Catalogs => {
                egui::Panel::left("catalogs").resizable(true).default_size(300.0).show(ui, |ui| self.catalog_list(ui));
                egui::CentralPanel::default().show(ui, |ui| self.catalog_editor(ui));
            }
            View::Users => {
                egui::Panel::left("users").resizable(true).default_size(300.0).show(ui, |ui| self.user_list(ui));
                egui::CentralPanel::default().show(ui, |ui| self.user_editor(ui));
            }
        }
        self.dialogs(&ctx);

        let title = format!("{} – {}{}", tr("OpenECraft parts manager"), self.client.url, if self.session.dirty() { " *" } else { "" });
        if title != self.title {
            openecraft_ui::io::set_title(&ctx, &title);
            self.title = title;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, URL_KEY, &self.client.url);
        eframe::set_value(storage, LOGOS_KEY, &self.show_logos);
        eframe::set_value(storage, openecraft_ui::language::STORAGE_KEY, &openecraft_i18n::lang().code());
        eframe::set_value(storage, openecraft_ui::language::STORAGE_KEY, &openecraft_i18n::lang().code());
    }
}
