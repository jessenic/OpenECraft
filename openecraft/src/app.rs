//! Main application window: the ribbon with the quick access row and the backstage,
//! the docked panes (page, navigators, Insert center, properties, panel layout, wiring
//! assistant, tools), the status bar and the dialogs.

use openecraft_i18n::{tr, tr_fmt};
use crate::commands::Cmd;
use crate::editor::{symbol_icon, Editor, Tool};
use crate::session::Session;
use crate::workspaces::Workspace;
use openecraft_core::index::Index;
use crate::io::{self, FileIo, Loaded, Purpose, PROJECT_EXT};
use openecraft_core::parts::{self, PartsDb};
use crate::panel_ui::{NcFormat, PanelUi};
use crate::mounting_ui::MountingUi;
use crate::wiring_ui::WiringUi;
use crate::parts_ui::{PartsAction, PartsUi};
use openecraft_core::model::*;
use openecraft_core::netlist::{pin_designation, PinRef};
use openecraft_core::ops;
use openecraft_core::analysis::Analysis;
use openecraft_core::issue::Severity;
use openecraft_core::structure::{self, Aspect, Dt};
use openecraft_core::symbols::{self, Category, Func};
use openecraft_core::{potentials, properties, sample, translation, wirenum};
use openecraft_ui::icons::{self, Icon};
use egui::{Key, RichText};
use openecraft_api::params::{text, Args};
use openecraft_client::PortalKind;
use serde_json::Map;
use std::collections::HashMap;

mod arrange_ui;
mod backstage;
mod cables_ui;
mod check_ui;
mod command_search;
mod commands_ui;
mod dialog;
mod dock_ui;
mod files;
mod find_ui;
mod help_pane;
mod insert_center_ui;
mod layout_panes;
mod navigator;
mod numbering_ui;
mod properties_panel;
mod ribbon_ui;
mod settings_ui;
mod start_page;
mod status_bar;
mod windows;
mod xref_ui;
mod compare_ui;
mod engineering;
mod field_ui;
mod form_layout_ui;
mod forms_ui;
mod buildings_ui;
mod harness_ui;
mod integration_ui;
mod languages_ui;
mod config_ui;
mod drawing_ui;
mod electrical_ui;
mod plc_ui;
mod plugins_ui;
mod plugs_ui;
mod tables_ui;
mod pages_ui;
mod page_dnd;
mod part_drop;
mod sections_ui;
mod potentials_ui;
mod projects_ui;
mod properties_ui;
mod structure_ui;
mod preplanning_ui;
mod workspaces_ui;
mod process_ui;
mod singleline_ui;
mod remote_ui;
mod requests_ui;
mod review_ui;
mod revisions_ui;
mod report_blocks_ui;
mod symbol_editor;
mod vault_ui;

use crate::dock::{LeftTab, Pane, ToolPane};

/// Storage keys of the shell.
const RECENT_KEY: &str = "recent_projects";
const UI_KEY: &str = "shell";

/// What the shell remembers besides the panes: ribbon state, scale, first start.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct ShellState {
    ribbon_minimized: bool,
    ui_scale: f32,
    tablet: bool,
    /// Tablet mode for this session only (`--wiring`, `#wiring`), not stored.
    #[serde(skip)]
    tablet_once: bool,
    /// The project had unsaved changes when the app was closed.
    dirty: bool,
    /// The start page was shown once.
    started: bool,
    workspace: String,
    /// Arrangements of the workspaces not shown (key → dock JSON).
    docks: Vec<(String, String)>,
    /// Panes the user floated or docked against their default (current workspace).
    placement: crate::dock::Placement,
    /// Placement and floating panes of the other workspaces.
    sides: crate::dock::Sides,
    /// Opacity of the walls in front in the 3D view of the panel layout, 0 … 1.
    wall_opacity: f32,
    /// The title bar shows the frame time and the frames per second.
    frame_stats: bool,
    /// The 3D view of the panel layout is drawn by the graphics card.
    gpu_3d: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        ShellState {
            ribbon_minimized: false,
            ui_scale: 1.0,
            tablet: false,
            tablet_once: false,
            dirty: false,
            started: false,
            workspace: "schematic".into(),
            docks: Vec::new(),
            placement: crate::dock::Placement::default(),
            sides: crate::dock::Sides::default(),
            wall_opacity: crate::panel_ui::WALL_OPACITY,
            frame_stats: false,
            gpu_3d: true,
        }
    }
}

impl ShellState {
    /// The touch sizes are shown: chosen in the settings or for this session.
    fn tablet(&self) -> bool {
        self.tablet || self.tablet_once
    }

    /// Switching tablet mode off also ends the one of this session.
    fn set_tablet(&mut self, on: bool) {
        self.tablet = on;
        if !on {
            self.tablet_once = false;
        }
    }
}

/// What a docked pane asked for while the panes were drawn (`App::in_dock`).
#[derive(Clone, Debug, PartialEq)]
enum Deferred {
    Cmd(Cmd),
    /// A device shown in the panel layout (`true`: in the 3D view).
    Focus(String, bool),
}

/// An action that replaces the project or ends the app: with unsaved changes it waits
/// for the answer of the discard dialog (`App::request_replace`).
#[derive(Clone, Debug)]
enum Replace {
    /// A command that replaces the project (New, Open, demo, example, template file).
    Cmd(Cmd),
    /// A project at hand (built-in template, project database).
    Project(Box<Project>, Option<String>),
    /// A project file to load (recent list, file given at the start).
    Path(String),
    /// A project of the project server.
    Shared(String),
    /// A project file of the vault.
    Vault(Box<openecraft_client::vault::FileVersion>),
    /// The window closes.
    Close,
}

/// A message of the status bar and of the message history.
#[derive(Clone, Debug, Default)]
struct Message {
    text: String,
    /// Technical detail (shown on hover).
    detail: String,
    level: crate::style::Level,
}

pub struct App {
    /// The open project with its history, analysis and saved state.
    session: Session,
    editor: Editor,
    io: FileIo,
    /// Arrangement of the panes of the current workspace.
    dock: egui_tiles::Tree<Pane>,
    /// Arrangements of the other workspaces.
    docks: HashMap<Workspace, egui_tiles::Tree<Pane>>,
    /// Panes shown as windows (tools, pane menu › Float).
    floating: Vec<Pane>,
    /// The floating pane to bring to the front.
    raise: Option<Pane>,
    /// The document pane the user worked in last (keys go to it).
    active_doc: Pane,
    /// The pane last clicked (F6 cycles from it).
    active_pane: Option<Pane>,
    /// Pages shown as tabs above the page (ids).
    page_tabs: Vec<u64>,
    lib_filter: String,
    device_filter: String,
    connection_filter: String,
    report_filter: String,
    /// Rename device dialog: old tag and the tag entered.
    rename_device: Option<(String, String)>,
    /// Active work area (name); empty = the whole project.
    work_area: String,
    structure_ui: structure_ui::StructureUi,
    properties_ui: properties_ui::PropertiesUi,
    projects_ui: projects_ui::ProjectsUi,
    pages_ui: pages_ui::PagesUi,
    show_reports: bool,
    report_kind: ReportId,
    show_project: bool,
    parts_db: PartsDb,
    parts_ui: PartsUi,
    workspace: Workspace,
    panel_ui: PanelUi,
    wiring_ui: WiringUi,
    mounting_ui: MountingUi,
    show_help: bool,
    show_shortcuts: bool,
    /// Saved workspaces (View › Workspaces) and the name typed for a new one.
    layouts: Vec<crate::workspaces::Layout>,
    /// Name of the workspace being saved (the window is open while it is set).
    layout_name: Option<String>,
    /// What waits for the user to keep or drop the unsaved changes.
    discard: crate::session::Guard<Replace>,
    /// The user agreed to close: the next close request is not cancelled.
    closing: bool,
    /// Page (id) whose deletion waits for the confirmation.
    confirm_delete_page: Option<u64>,
    /// Saved workspace whose deletion waits for the confirmation.
    confirm_remove_layout: Option<String>,
    /// Example project chosen in a list, opened by `Cmd::OpenExample`.
    pending_example: Option<&'static str>,
    /// The docked panes are being drawn (their tree is taken out of `dock`): commands
    /// and device jumps wait in `deferred` until it is back.
    in_dock: bool,
    deferred: Vec<Deferred>,
    /// Exports and imports running on another thread.
    jobs: Vec<crate::jobs::Job<files::Done>>,
    /// Frame time and frames per second for the title bar.
    frame_stats: crate::frame_stats::FrameStats,
    /// Export whose parameters are being entered: exporter, language, text per parameter.
    export_form: Option<(openecraft_api::Exporter, Option<String>, Vec<String>)>,
    /// Binding whose new shortcut is being recorded.
    recording: Option<&'static str>,
    show_cables: bool,
    show_numbering: bool,
    numbering_ui: numbering_ui::NumberingUi,
    check_ui: check_ui::CheckUi,
    arrange_ui: arrange_ui::ArrangeUi,
    show_xref: bool,
    find_ui: find_ui::FindUi,
    /// Pages tab grouped by structure.
    page_tree: bool,
    /// A page or section dragged in the Pages tab.
    page_drag: page_dnd::DragState,
    /// Macros, terminal strips, PLC overview, revisions.
    eng: engineering::EngineeringUi,
    macro_lib: openecraft_core::macros::MacroLibrary,
    symed: symbol_editor::SymbolEditor,
    forms_ui: forms_ui::FormsUi,
    report_blocks_ui: report_blocks_ui::ReportBlocksUi,
    harness_ui: harness_ui::HarnessUi,
    buildings_ui: buildings_ui::BuildingsUi,
    field_ui: field_ui::FieldUi,
    plugs_ui: plugs_ui::PlugsUi,
    plc_ui: plc_ui::PlcUi,
    integration_ui: integration_ui::IntegrationUi,
    process_ui: process_ui::ProcessUi,
    preplanning_ui: preplanning_ui::PreplanningUi,
    electrical_ui: electrical_ui::ElectricalUi,
    singleline_ui: singleline_ui::SingleLineUi,
    languages_ui: languages_ui::LanguagesUi,
    tables_ui: tables_ui::TablesUi,
    config_ui: config_ui::ConfigUi,
    /// Wire numbering: also renumber / remove manually set numbers.
    numbering_overwrite: bool,
    page_props: Option<usize>,
    /// Section whose properties are shown.
    section_props: Option<u64>,
    message: Option<(Message, f64)>,
    /// The messages of this session (status bar › messages).
    messages: Vec<Message>,
    title: String,
    /// Language the texts kept between frames were made in.
    shown_lang: openecraft_i18n::Lang,
    /// Project server: shared projects and locks.
    remote: crate::remote::Remote,
    /// Autodesk Vault: browsing, imports and check-ins.
    vault: crate::vault::Vault,
    vault_ui: vault_ui::VaultUi,
    /// Comments and markup.
    review_ui: review_ui::ReviewUi,
    revisions_ui: revisions_ui::RevisionsUi,
    compare_ui: compare_ui::CompareUi,
    requests_ui: requests_ui::RequestsUi,
    /// Built-in and external plugins (`openecraft_api`).
    plugins: openecraft_api::Plugins,
    plugins_ui: plugins_ui::PluginsUi,
    /// The autosaved project and parts database (IndexedDB in the browser).
    autosave: crate::autosave::Autosave,
    /// Ribbon: active tab, customisation, key tips.
    ribbon: ribbon_ui::RibbonUi,
    /// Open page of the backstage (File tab).
    backstage: Option<crate::ribbon::Page>,
    /// Project files opened or saved lately (desktop).
    recent: Vec<String>,
    settings: settings_ui::SettingsUi,
    search: command_search::SearchUi,
    insert_center: crate::insert_center::InsertCenter,
    /// Favorite and recently used plugin commands.
    plugin_cmds: crate::plugin_commands::PluginCommands,
    help: help_pane::HelpUi,
    /// The dialogs shown (Esc closes the one on top).
    dialogs: dialog::Dialogs,
    shell: ShellState,
    /// The properties of the selection in a window (F2, double click).
    props_window: bool,
    /// Where the properties window opens (top left corner).
    props_at: egui::Pos2,
    /// Tab of the symbol properties.
    props_tab: properties_panel::PropTab,
    /// Part to add to the device placed next (Insert center › Devices).
    /// The pane F6 moved to (drawn with a frame until the next click).
    focus_pane: Option<Pane>,
    /// The dark theme is shown.
    dark: bool,
    /// The selection changed: navigators scroll to it (this frame).
    scroll_nav: bool,
    /// Selection and page of the last frame (to see a change).
    last_selection: (usize, Vec<u64>),
    /// The touch sizes of the tablet mode are applied (or not).
    tablet_applied: Option<bool>,
    /// A part dropped onto a page that is being downloaded from the parts server.
    pending_drop: Option<crate::parts_ui::PendingDrop>,
}

impl App {
    #[allow(clippy::too_many_lines)]
    pub fn new(cc: &eframe::CreationContext<'_>) -> App {
        // System fonts for scripts the bundled fonts lack, on screen and in PDF exports.
        let system_fonts = openecraft_ui::fonts::system_fallbacks();
        openecraft_ui::fonts::install(&cc.egui_ctx, &system_fonts);
        for (_, bytes) in system_fonts {
            if openecraft_core::pdf::set_fallback_font(bytes) {
                break;
            }
        }
        let mut prj: Option<Project> = None;
        let mut file_name = None;
        let mut parts_db: Option<PartsDb> = None;
        let mut server_url: Option<String> = None;
        let mut eplan_url: Option<String> = None;
        let mut server_kind: Option<PortalKind> = None;
        let mut remote = crate::remote::Remote::default();
        let mut vault = crate::vault::Vault::default();
        let mut macro_lib = None;
        let mut disabled_plugins: Vec<String> = Vec::new();
        let mut language: Option<String> = None;
        let mut shortcuts = crate::shortcuts::Shortcuts::default();
        let mut layouts: Vec<crate::workspaces::Layout> = Vec::new();
        let mut projects_ui = projects_ui::ProjectsUi::default();
        let mut dock = None;
        let mut recent: Vec<String> = Vec::new();
        let mut shell = ShellState::default();
        let mut ribbon = ribbon_ui::RibbonUi::default();
        let mut insert_center = crate::insert_center::InsertCenter::default();
        let mut plugin_cmds = crate::plugin_commands::PluginCommands::default();
        let mut stored = false;
        if let Some(storage) = cc.storage {
            stored = storage.get_string("file_name").is_some();
            language = eframe::get_value(storage, openecraft_ui::language::STORAGE_KEY);
            // Packed; stores written before have the plain project.
            prj = storage.get_string("project_packed").and_then(|s| openecraft_core::archive::unpack(&s).ok()).or_else(|| eframe::get_value(storage, "project"));
            file_name = eframe::get_value(storage, "file_name").flatten();
            parts_db = eframe::get_value(storage, "parts_db");
            server_url = eframe::get_value(storage, "parts_server_url");
            eplan_url = eframe::get_value(storage, "eplan_portal_url");
            server_kind = eframe::get_value(storage, "parts_server_kind");
            macro_lib = eframe::get_value(storage, "macros");
            if let Some(db) = eframe::get_value(storage, "project_database") {
                projects_ui.db = db;
            }
            disabled_plugins = eframe::get_value(storage, plugins_ui::STORAGE_KEY).unwrap_or_default();
            if let Some(stored) = eframe::get_value(storage, crate::shortcuts::STORAGE_KEY) {
                shortcuts = crate::shortcuts::Shortcuts::from_storage(&stored);
            }
            layouts = eframe::get_value(storage, crate::workspaces::STORAGE_KEY).unwrap_or_default();
            dock = storage.get_string(crate::dock::STORAGE_KEY).and_then(|s| crate::dock::from_json(&s));
            recent = eframe::get_value(storage, RECENT_KEY).unwrap_or_default();
            shell = eframe::get_value(storage, UI_KEY).unwrap_or_default();
            if let Some(c) = storage.get_string(crate::ribbon::STORAGE_KEY).and_then(|s| crate::ribbon::Custom::from_json(&s).ok()) {
                ribbon.custom = c;
            }
            insert_center = eframe::get_value(storage, crate::insert_center::STORAGE_KEY).unwrap_or_default();
            plugin_cmds = eframe::get_value(storage, crate::plugin_commands::STORAGE_KEY).unwrap_or_default();
            if let Some(url) = eframe::get_value(storage, "project_server_url") {
                remote.url = url;
            }
            if let Some(user) = eframe::get_value::<String>(storage, "project_server_user").filter(|u| !u.is_empty()) {
                remote.user = user;
            }
            if let Some(url) = eframe::get_value(storage, "vault_url") {
                vault.url = url;
            }
            if let Some(name) = eframe::get_value::<String>(storage, "vault_name").filter(|n| !n.is_empty()) {
                vault.vault_name = name;
            }
            if let Some(user) = eframe::get_value(storage, "vault_user") {
                vault.user = user;
            }
        }
        openecraft_i18n::init(language.as_deref());
        openecraft_ui::fonts::follow_language(&cc.egui_ctx);
        let parts_db = parts_db.unwrap_or_default();
        let autosave = crate::autosave::Autosave::start(openecraft_ui::io::Store::new("OpenECraft"), &cc.egui_ctx, &parts_db);
        let mut parts_ui = PartsUi::default();
        if let Some(url) = server_url {
            parts_ui.server.set_url_of(PortalKind::OpenECraft, url);
        }
        if let Some(url) = eplan_url {
            parts_ui.server.set_url_of(PortalKind::Eplan, url);
        }
        parts_ui.server.set_kind(server_kind.unwrap_or_default());
        let prj = prj.unwrap_or_else(sample::demo_project);
        let unsaved = shell.dirty;
        let workspace = Workspace::from_key(&shell.workspace).unwrap_or(Workspace::Schematic);
        let dock = dock.unwrap_or_else(|| crate::dock::default_tree_for(workspace));
        let docks: HashMap<Workspace, egui_tiles::Tree<Pane>> =
            shell.docks.iter().filter_map(|(k, json)| Some((Workspace::from_key(k)?, crate::dock::from_json(json)?))).filter(|(w, _)| *w != workspace).collect();
        let mut plugins = openecraft_api::Plugins::new(Vec::<String>::new());
        plugins.load_env();
        for id in &disabled_plugins {
            plugins.set_enabled(id, false);
        }
        let first_start = !shell.started && !stored;
        let mut app = App {
            session: Session::new(prj, file_name),
            editor: {
                let mut editor = Editor::default();
                editor.shortcuts = shortcuts;
                editor
            },
            io: FileIo::default(),
            dock,
            docks,
            floating: Vec::new(),
            raise: None,
            active_doc: Pane::Page,
            active_pane: None,
            page_tabs: Vec::new(),
            lib_filter: String::new(),
            device_filter: String::new(),
            connection_filter: String::new(),
            report_filter: String::new(),
            rename_device: None,
            work_area: String::new(),
            structure_ui: Default::default(),
            properties_ui: Default::default(),
            projects_ui,
            pages_ui: Default::default(),
            show_reports: false,
            report_kind: ReportKind::DeviceList.into(),
            show_project: false,
            parts_db,
            parts_ui,
            workspace,
            panel_ui: PanelUi::default(),
            wiring_ui: WiringUi::default(),
            mounting_ui: MountingUi::default(),
            show_help: false,
            show_shortcuts: false,
            layouts,
            layout_name: None,
            discard: Default::default(),
            closing: false,
            confirm_delete_page: None,
            confirm_remove_layout: None,
            pending_example: None,
            in_dock: false,
            deferred: Vec::new(),
            jobs: Vec::new(),
            frame_stats: crate::frame_stats::FrameStats::default(),
            export_form: None,
            recording: None,
            show_cables: false,
            show_numbering: false,
            numbering_ui: Default::default(),
            check_ui: Default::default(),
            arrange_ui: Default::default(),
            show_xref: false,
            find_ui: Default::default(),
            page_tree: false,
            page_drag: Default::default(),
            eng: Default::default(),
            macro_lib: macro_lib.unwrap_or_default(),
            symed: Default::default(),
            forms_ui: Default::default(),
            report_blocks_ui: Default::default(),
            harness_ui: Default::default(),
            buildings_ui: Default::default(),
            field_ui: Default::default(),
            plugs_ui: Default::default(),
            plc_ui: Default::default(),
            integration_ui: Default::default(),
            process_ui: Default::default(),
            preplanning_ui: Default::default(),
            electrical_ui: Default::default(),
            singleline_ui: Default::default(),
            languages_ui: Default::default(),
            tables_ui: Default::default(),
            config_ui: Default::default(),
            numbering_overwrite: false,
            page_props: None,
            section_props: None,
            message: None,
            messages: Vec::new(),
            title: String::new(),
            shown_lang: openecraft_i18n::lang(),
            remote,
            vault,
            vault_ui: Default::default(),
            review_ui: Default::default(),
            revisions_ui: Default::default(),
            compare_ui: Default::default(),
            requests_ui: Default::default(),
            plugins,
            plugins_ui: Default::default(),
            autosave,
            ribbon,
            backstage: None,
            recent,
            settings: Default::default(),
            search: Default::default(),
            insert_center,
            plugin_cmds,
            help: Default::default(),
            dialogs: Default::default(),
            shell,
            props_window: false,
            props_at: egui::Pos2::ZERO,
            props_tab: Default::default(),
            focus_pane: None,
            scroll_nav: false,
            last_selection: (0, Vec::new()),
            tablet_applied: None,
            pending_drop: None,
            dark: cc.egui_ctx.theme() == egui::Theme::Dark,
        };
        crate::style::apply(&cc.egui_ctx);
        if unsaved {
            app.session.mark_unsaved();
        }
        app.mounting_ui.active = workspace == Workspace::Mounting;
        app.ribbon.follow_workspace(workspace);
        app.active_doc = crate::dock::panes(&app.dock).into_iter().find(|p| p.is_document()).unwrap_or(Pane::Page);
        if first_start {
            // The start page comes first; the demo project waits behind it.
            app.show_pane(Pane::Tool(ToolPane::Start));
        }
        app.shell.started = true;
        // On the desktop the stored values are there at once.
        app.autosave.poll(&mut app.session, &mut app.parts_db);
        app
    }

    /// Opens the wiring assistant instead of the schematic and a project file, e.g. one
    /// double-clicked in the file manager (desktop).
    pub fn start(mut self, wiring: bool, file: Option<String>) -> Self {
        if wiring {
            self.set_workspace(Workspace::Wiring);
            self.shell.tablet_once = true;
        }
        if let Some(path) = file {
            // The project restored from the last session may have unsaved changes.
            if let Some(Replace::Path(path)) = self.discard.request(self.session.dirty(), Replace::Path(path)) {
                self.io.open_path(&path, Purpose::Project);
            }
        }
        self
    }

    /// The reports of the registry (built-in and of plugins): id and title.
    fn report_list(&self) -> Vec<(ReportId, String)> {
        self.plugins.reports().map(|r| (ReportId::new(&r.name), text(&r.title).to_string())).collect()
    }

    /// Title of a report: from the registry, otherwise of the built-in kind or the name.
    pub(super) fn report_title(&self, id: ReportId) -> String {
        self.plugins.report(id.as_str()).map(|r| text(&r.title).to_string()).unwrap_or_else(|| id.title())
    }

    fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.post(ctx, Message { text: msg.into(), ..Default::default() });
    }

    /// A message of something done (saved, opened, exported).
    fn notify_ok(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.post(ctx, Message { text: msg.into(), detail: String::new(), level: crate::style::Level::Ok });
    }

    /// A message of something done only in part.
    fn notify_warning(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.post(ctx, Message { text: msg.into(), detail: String::new(), level: crate::style::Level::Warning });
    }

    /// A message of something that failed.
    fn notify_error(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.post(ctx, Message { text: msg.into(), detail: String::new(), level: crate::style::Level::Error });
    }

    /// A failure told in a plain sentence, the technical `detail` on hover.
    fn notify_failure(&mut self, ctx: &egui::Context, msg: impl Into<String>, detail: impl Into<String>) {
        self.post(ctx, Message { text: msg.into(), detail: detail.into(), level: crate::style::Level::Error });
    }

    fn post(&mut self, ctx: &egui::Context, msg: Message) {
        self.messages.push(msg.clone());
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
        self.message = Some((msg, ctx.input(|i| i.time)));
    }

    /// Every action that replaces the project (or closes the window) comes here: with
    /// unsaved changes the discard dialog asks first.
    fn request_replace(&mut self, ctx: &egui::Context, r: Replace) {
        if let Some(r) = self.discard.request(self.session.dirty(), r) {
            self.perform(ctx, r);
        }
    }

    /// Runs an action the user agreed to (or one that drops no changes).
    fn perform(&mut self, ctx: &egui::Context, r: Replace) {
        match r {
            Replace::Cmd(cmd) => self.exec(ctx, cmd),
            Replace::Project(prj, file) => self.replace_project(*prj, file),
            Replace::Path(path) => {
                self.io.open_path(&path, Purpose::Project);
                self.backstage = None;
            }
            Replace::Shared(name) => self.remote.open(&name),
            Replace::Vault(file) => self.vault.download(*file, crate::vault::Use::Project),
            Replace::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn replace_project(&mut self, prj: Project, file_name: Option<String>) {
        self.remote.close();
        // Another project must not be checked in as a version of the vault's file.
        self.vault.origin = None;
        if let Some(path) = &file_name {
            self.remember_recent(path);
        }
        self.session.replace(prj, file_name);
        self.find_ui.clear_results();
        parts::link_unassigned_plc_parts(&mut self.session.prj, &self.parts_db, None);
        self.editor.selection.clear();
        self.editor.cancel();
        self.editor.fit_pending = true;
        self.page_tabs.clear();
        self.backstage = None;
    }

    /// Puts a project file at the top of the recent files.
    fn remember_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(12);
    }

    fn goto(&mut self, page: usize, id: u64) {
        if page >= self.session.prj.pages.len() {
            return;
        }
        self.session.page = page;
        self.editor.cancel();
        self.editor.selection.clear();
        if self.workspace.is_workshop() || crate::dock::find(&self.dock, Pane::Page).is_none() {
            self.set_workspace(Workspace::Schematic);
        }
        self.show_pane(Pane::Page);
        if id != 0 {
            self.editor.selection.insert(id);
            if let Some(b) = ops::item_bounds(&self.session.prj.symbol_table(), &self.session.prj.pages[page], id) {
                if self.editor.zoom < 3.0 {
                    self.editor.zoom = 4.0;
                }
                self.editor.center_on = Some(b.center());
            }
        }
    }

    // ------------------------------------------------------------------ pages

    fn add_page(&mut self, kind: PageKind) {
        let at = (self.session.page + 1).min(self.session.prj.pages.len());
        self.session.page = ops::add_page(&mut self.session.prj, kind, at);
        self.editor.selection.clear();
        self.show_pane(Pane::Page);
    }

    // ------------------------------------------------------------------ panes and workspaces

    /// Shows a pane: the active tab where it is docked, otherwise a window (tools) or a
    /// tab of the area it belongs to. A floating pane comes to the front.
    pub(super) fn show_pane(&mut self, pane: Pane) {
        let docked = crate::dock::find(&self.dock, pane).is_some();
        if self.floating.contains(&pane) || (!docked && self.shell.placement.floats(pane)) {
            if !self.floating.contains(&pane) {
                self.floating.push(pane);
            }
            self.raise = Some(pane);
            self.active_pane = Some(pane);
            return;
        }
        crate::dock::show(&mut self.dock, pane);
        if pane.is_document() {
            self.active_doc = pane;
        }
        self.active_pane = Some(pane);
    }

    pub(super) fn pane_shown(&self, pane: Pane) -> bool {
        crate::dock::find(&self.dock, pane).is_some() || self.floating.contains(&pane)
    }

    /// Shows a placed device in the panel layout (`three_d`: in the 3D view).
    pub(super) fn focus_device(&mut self, tag: &str, three_d: bool) {
        if self.in_dock {
            self.deferred.push(Deferred::Focus(tag.to_string(), three_d));
        } else if self.panel_ui.focus_device(&self.session.prj, tag, three_d) {
            self.set_workspace(Workspace::Panel);
            self.show_pane(Pane::LayoutView);
        }
    }

    /// Switches the workspace: its own arrangement of the panes comes back.
    pub(super) fn set_workspace(&mut self, w: Workspace) {
        match w {
            Workspace::Mounting => self.mounting_ui.active = true,
            Workspace::Wiring => self.mounting_ui.active = false,
            _ => {}
        }
        if w == self.workspace {
            return;
        }
        let tree = self.docks.remove(&w).unwrap_or_else(|| crate::dock::default_tree_for(w));
        let old = std::mem::replace(&mut self.dock, tree);
        self.docks.insert(self.workspace, old);
        // Floating panes and where panes open belong to the workspace.
        let mut side = crate::dock::Side { placement: std::mem::take(&mut self.shell.placement), floating: std::mem::take(&mut self.floating) };
        self.shell.sides.switch(self.workspace, w, &mut side);
        self.shell.placement = side.placement;
        self.floating = side.floating;
        self.workspace = w;
        self.ribbon.follow_workspace(w);
        self.active_doc = crate::dock::panes(&self.dock).into_iter().find(|p| p.is_document()).unwrap_or(Pane::Page);
    }

    /// Puts the panes of the current workspace back at their default places and closes
    /// its floating panes.
    pub(super) fn reset_layout(&mut self) {
        let mut side = crate::dock::Side { placement: std::mem::take(&mut self.shell.placement), floating: std::mem::take(&mut self.floating) };
        crate::dock::reset(self.workspace, &mut self.dock, &mut side);
        self.shell.placement = side.placement;
        self.floating = side.floating;
        self.active_doc = crate::dock::panes(&self.dock).into_iter().find(|p| p.is_document()).unwrap_or(Pane::Page);
    }

    /// Tool windows asked for by a flag of the tools (`open = true`) become panes; a
    /// flag follows whether its pane is shown.
    fn open_requests(&mut self, before: bool) {
        let shown: Vec<Pane> = crate::dock::panes(&self.dock).into_iter().chain(self.floating.iter().copied()).collect();
        let mut show = Vec::new();
        for (flag, pane) in self.request_flags() {
            let is = shown.contains(&pane);
            if before && *flag && !is {
                show.push(pane);
            } else {
                *flag = is;
            }
        }
        for p in show {
            self.show_pane(p);
        }
        // Settings shown as a node of the settings dialog.
        for (flag, node) in [
            (&mut self.show_numbering, settings_ui::Node::WireNumbering),
            (&mut self.show_xref, settings_ui::Node::CrossReferences),
            (&mut self.check_ui.show_settings, settings_ui::Node::Checks),
            (&mut self.eng.show_marking, settings_ui::Node::Marking),
            (&mut self.show_shortcuts, settings_ui::Node::Shortcuts),
        ] {
            if std::mem::take(flag) {
                self.settings.open_at(node);
            }
        }
    }

    /// The flags of the tools with the pane each opens.
    /// Status texts were made in the language before: they would stay in it.
    fn language_changed(&mut self) {
        self.shown_lang = openecraft_i18n::lang();
        self.remote.status.clear();
        self.parts_ui.server.status.clear();
        self.panel_ui.status.clear();
        self.vault.status.clear();
        self.message = None;
    }

    /// Clears the flag that asks for `pane` (the pane was closed).
    pub(super) fn forget_request(&mut self, pane: Pane) {
        for (flag, p) in self.request_flags() {
            if p == pane {
                *flag = false;
            }
        }
    }

    fn request_flags(&mut self) -> Vec<(&mut bool, Pane)> {
        use ToolPane as T;
        vec![
            (&mut self.show_reports, Pane::Tool(T::Reports)),
            (&mut self.show_cables, Pane::Tool(T::Cables)),
            (&mut self.show_help, Pane::Tool(T::Help)),
            (&mut self.eng.show_macros, Pane::Tool(T::Macros)),
            (&mut self.eng.show_strips, Pane::Tool(T::TerminalStrips)),
            (&mut self.eng.show_potentials, Pane::Tool(T::Potentials)),
            (&mut self.eng.show_plc, Pane::Tool(T::PlcOverview)),
            (&mut self.eng.show_revisions, Pane::Tool(T::Revisions)),
            (&mut self.find_ui.open, Pane::Tool(T::Find)),
            (&mut self.forms_ui.open, Pane::Tool(T::Forms)),
            (&mut self.report_blocks_ui.open, Pane::Tool(T::ReportBlocks)),
            (&mut self.harness_ui.open, Pane::Tool(T::Harnesses)),
            (&mut self.field_ui.open, Pane::Tool(T::FieldWiring)),
            (&mut self.buildings_ui.open, Pane::Tool(T::Buildings)),
            (&mut self.plugs_ui.open, Pane::Tool(T::Plugs)),
            (&mut self.plc_ui.open, Pane::Tool(T::PlcHardware)),
            (&mut self.integration_ui.open, Pane::Tool(T::Integration)),
            (&mut self.process_ui.open, Pane::Tool(T::Process)),
            (&mut self.preplanning_ui.open, Pane::Tool(T::Preplanning)),
            (&mut self.electrical_ui.open, Pane::Tool(T::Electrical)),
            (&mut self.singleline_ui.open, Pane::Tool(T::SingleLine)),
            (&mut self.languages_ui.open, Pane::Tool(T::Languages)),
            (&mut self.tables_ui.open, Pane::Tool(T::Tables)),
            (&mut self.config_ui.open, Pane::Tool(T::Configurator)),
            (&mut self.compare_ui.open, Pane::Tool(T::Compare)),
            (&mut self.requests_ui.open, Pane::Tool(T::ChangeRequests)),
            (&mut self.structure_ui.open, Pane::Tool(T::Structure)),
            (&mut self.properties_ui.open, Pane::Tool(T::PropertyDatabase)),
            (&mut self.projects_ui.open, Pane::Tool(T::Projects)),
            (&mut self.parts_ui.open, Pane::Tool(T::Parts)),
            (&mut self.parts_ui.show_portal, Pane::Tool(T::DataPortal)),
            (&mut self.symed.open, Pane::Tool(T::SymbolEditor)),
            (&mut self.remote.window, Pane::Tool(T::ProjectServer)),
            (&mut self.vault.window, Pane::Tool(T::Vault)),
            (&mut self.plugins_ui.open, Pane::Tool(T::Plugins)),
        ]
    }

    /// Keeps the current page among the page tabs; drops tabs of deleted pages.
    fn sync_page_tabs(&mut self) {
        let ids: Vec<u64> = self.session.prj.pages.iter().map(|p| p.id).collect();
        self.page_tabs.retain(|id| ids.contains(id));
        if let Some(cur) = self.session.prj.pages.get(self.session.page).map(|p| p.id) {
            if !self.page_tabs.contains(&cur) {
                self.page_tabs.push(cur);
                while self.page_tabs.len() > 10 {
                    let drop = self.page_tabs.iter().position(|id| *id != cur).unwrap_or(0);
                    self.page_tabs.remove(drop);
                }
            }
        }
    }

    /// UI scale and theme of the shell (tablet mode enlarges everything).
    fn apply_scale(&mut self, ctx: &egui::Context) {
        let tablet = self.shell.tablet();
        let scale = if tablet { self.shell.ui_scale.max(1.4) } else { self.shell.ui_scale }.clamp(0.75, 2.0);
        if (ctx.zoom_factor() - scale).abs() > 0.001 {
            ctx.set_zoom_factor(scale);
        }
        if self.tablet_applied != Some(tablet) {
            crate::style::touch(ctx, tablet);
            self.tablet_applied = Some(tablet);
        }
    }
}

/// A count as a whole sentence of the catalog: `one` for 1, `many` (with the number)
/// otherwise.
pub(super) fn counted(n: usize, one: &str, many: String) -> String {
    if n == 1 { one.to_string() } else { many }
}

/// "2 errors, 1 warning": the catalog has the sentence for one and several of each.
pub(super) fn errors_warnings(errors: usize, warnings: usize) -> String {
    match (errors == 1, warnings == 1) {
        (true, true) => tr("1 error, 1 warning").to_string(),
        (true, false) => tr_fmt!("1 error, {} warnings", warnings),
        (false, true) => tr_fmt!("{} errors, 1 warning", errors),
        (false, false) => tr_fmt!("{} errors, {} warnings", errors, warnings),
    }
}

/// An entry dragged into the page or clicked: the drag source lies over the entry and
/// takes its clicks, so it senses them as well.
fn drag_source<P: std::any::Any + Send + Sync>(ui: &mut egui::Ui, id: egui::Id, payload: P, add: impl FnOnce(&mut egui::Ui) -> egui::Response) -> egui::Response {
    let r = ui.dnd_drag_source(id, payload, add);
    r.inner.union(r.response.interact(egui::Sense::click()))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_stats.record(ctx.input(|i| i.time), frame.info().cpu_usage);
        self.dark = ui.visuals().dark_mode;
        crate::style::set_dark(self.dark);
        self.apply_scale(&ctx);
        self.panel_ui.wall_opacity = self.shell.wall_opacity;
        self.panel_ui.gpu_3d = self.shell.gpu_3d && frame.gl().is_some();
        // Closing the window with unsaved changes asks first (the second close, after
        // the answer, goes through).
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing && self.discard.request(self.session.dirty(), Replace::Close).is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        self.poll_loaded(&ctx);
        self.poll_jobs(&ctx);
        self.poll_remote(&ctx);
        self.poll_vault(&ctx);
        self.autosave.poll(&mut self.session, &mut self.parts_db);
        let actions = self.parts_ui.poll_server(&ctx, &mut self.parts_db);
        self.parts_actions(&ctx, actions);

        // The device index follows every committed change; while an edit may be in
        // progress (mouse down, text field focused, drag) it is rebuilt every frame.
        let editing = ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused().is_some()) || self.editor.busy() || self.panel_ui.busy() || self.symed.busy() || self.forms_ui.busy();
        self.session.refresh(&self.plugins, editing);
        // The routes searched in the background show as soon as they are ready.
        if self.session.routing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // A project just opened shows once it is analysed; until then only what is
        // being done.
        if let Some(progress) = self.session.opening() {
            self.opening_window(ui, progress);
            return;
        }
        self.finish_part_drop(&ctx);
        self.key_tips(&ctx);
        self.command_keys(&ctx);
        self.session.clamp_page();
        self.sync_page_tabs();
        let now = (self.session.page, self.editor.selection.iter().copied().collect::<Vec<u64>>());
        self.scroll_nav = now != self.last_selection;
        self.last_selection = now;
        let idx = std::mem::take(&mut self.session.index);

        if openecraft_i18n::lang() != self.shown_lang {
            self.language_changed();
        }
        self.open_requests(true);
        self.layout_frame();
        egui::Panel::top("ribbon").show(ui, |ui| self.ribbon_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        match self.backstage {
            Some(page) => {
                egui::CentralPanel::default().show(ui, |ui| self.backstage_ui(ui, page));
            }
            None => {
                let fill = ui.visuals().panel_fill;
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(fill)).show(ui, |ui| self.docked(ui, &idx));
            }
        }
        for d in std::mem::take(&mut self.deferred) {
            match d {
                Deferred::Cmd(cmd) => self.run(&ctx, cmd),
                Deferred::Focus(tag, three_d) => self.focus_device(&tag, three_d),
            }
        }
        self.open_requests(false);
        self.layout_requests(&ctx, &idx);
        self.session.index = idx;
        self.windows(&ctx);

        // Commit finished edits as one undo step.
        let can_commit = !ctx.input(|i| i.pointer.any_down()) && ctx.memory(|m| m.focused().is_none()) && !self.editor.busy() && !self.panel_ui.busy() && !self.symed.busy() && !self.forms_ui.busy();
        if self.session.commit(can_commit) {
            // Undo, the title and the status bar show the change at once.
            ctx.request_repaint();
        }
        if let Some(r) = self.discard.after_save(self.session.dirty(), self.remote.busy() || self.vault.busy()) {
            self.perform(&ctx, r);
        }
        self.check_locks(&ctx);

        let mut title = format!(
            "OpenECraft – {}{}",
            self.session.prj.info.name,
            if self.session.dirty() { " *" } else { "" }
        );
        if let Some(stats) = self.frame_stats.text().filter(|_| self.shell.frame_stats) {
            title = format!("{title}   ({stats})");
        }
        if title != self.title {
            openecraft_ui::io::set_title(&ctx, &title);
            self.title = title;
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The autosave of the last `save` is written before the app ends.
        self.autosave.finish();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // The project and the parts database go to the app's store (IndexedDB in the
        // browser); until it works they are also kept here, the project packed.
        if self.autosave.save(&self.session, &self.parts_db) {
            storage.set_string("project_packed", String::new());
            storage.set_string("parts_db", String::new());
        } else {
            storage.set_string("project_packed", openecraft_core::archive::pack(&self.session.prj));
            eframe::set_value(storage, "parts_db", &self.parts_db);
        }
        storage.set_string("project", String::new());
        eframe::set_value(storage, "file_name", &self.session.file_name);
        eframe::set_value(storage, "parts_server_url", &self.parts_ui.server.url_of(PortalKind::OpenECraft));
        eframe::set_value(storage, "eplan_portal_url", &self.parts_ui.server.url_of(PortalKind::Eplan));
        eframe::set_value(storage, "parts_server_kind", &self.parts_ui.server.kind);
        eframe::set_value(storage, "macros", &self.macro_lib);
        eframe::set_value(storage, "project_database", &self.projects_ui.db);
        eframe::set_value(storage, "project_server_url", &self.remote.url);
        eframe::set_value(storage, "project_server_user", &self.remote.user);
        eframe::set_value(storage, "vault_url", &self.vault.url);
        eframe::set_value(storage, "vault_name", &self.vault.vault_name);
        eframe::set_value(storage, "vault_user", &self.vault.user);
        eframe::set_value(storage, plugins_ui::STORAGE_KEY, &self.plugins.disabled());
        eframe::set_value(storage, crate::shortcuts::STORAGE_KEY, &self.editor.shortcuts.to_storage());
        eframe::set_value(storage, crate::workspaces::STORAGE_KEY, &self.layouts);
        storage.set_string(crate::dock::STORAGE_KEY, crate::dock::to_json(&self.dock));
        storage.set_string(crate::ribbon::STORAGE_KEY, self.ribbon.custom.to_json());
        eframe::set_value(storage, crate::insert_center::STORAGE_KEY, &self.insert_center);
        eframe::set_value(storage, crate::plugin_commands::STORAGE_KEY, &self.plugin_cmds);
        eframe::set_value(storage, RECENT_KEY, &self.recent);
        let mut shell = self.shell.clone();
        shell.dirty = self.session.dirty();
        shell.workspace = self.workspace.key().to_string();
        shell.docks = self.docks.iter().map(|(w, t)| (w.key().to_string(), crate::dock::to_json(t))).collect();
        eframe::set_value(storage, UI_KEY, &shell);
        eframe::set_value(storage, openecraft_ui::language::STORAGE_KEY, &openecraft_i18n::lang().code());
    }
}

#[cfg(test)]
mod tests;
