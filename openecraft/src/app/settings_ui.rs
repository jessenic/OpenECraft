//! The settings dialog: one window with a tree of the project settings (wire numbering,
//! cross references, wire crossings, checks, marking, settings scheme, and the tools
//! that keep further project settings) and the user settings (appearance, keys and
//! gestures, ribbon, servers).

use super::*;
use crate::commands;
use crate::gestures::{self, Binding, Clash, MouseAction, Role, View};
use openecraft_ui::nav::Mod;
use crate::ribbon::{self, CustomGroup, TABS};
use crate::shortcuts;

/// UI scales offered.
pub(super) const SCALES: [f32; 7] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 2.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Node {
    WireNumbering,
    CrossReferences,
    Wires,
    Checks,
    Marking,
    Scheme,
    ProjectTools,
    Appearance,
    Startup,
    Shortcuts,
    Ribbon,
    Servers,
    Updates,
}

impl Node {
    const PROJECT: [Node; 7] = [Node::WireNumbering, Node::CrossReferences, Node::Wires, Node::Checks, Node::Marking, Node::Scheme, Node::ProjectTools];
    const USER: [Node; 6] = [Node::Appearance, Node::Startup, Node::Shortcuts, Node::Ribbon, Node::Servers, Node::Updates];

    fn title(self) -> &'static str {
        match self {
            Node::WireNumbering => tr("Wire numbering"),
            Node::CrossReferences => tr("Cross references"),
            Node::Wires => tr("Wire crossings"),
            Node::Checks => tr("Check settings"),
            Node::Marking => tr("Marking"),
            Node::Scheme => tr("Settings scheme"),
            Node::ProjectTools => tr("Further project settings"),
            Node::Appearance => tr("Appearance"),
            Node::Startup => tr("On startup"),
            Node::Shortcuts => tr("Keys and gestures"),
            Node::Ribbon => tr("Ribbon"),
            Node::Servers => tr("Servers"),
            Node::Updates => tr("Updates"),
        }
    }
}

pub(super) struct SettingsUi {
    open: bool,
    node: Node,
    filter: String,
    /// Title of a new own group and its tab.
    new_group: String,
    /// Command search of the own groups.
    add_filter: String,
    ribbon_tab: usize,
    /// Own group (index) whose deletion waits for the confirmation.
    confirm_group: Option<usize>,
    /// Part of the keys and gestures shown.
    section: Section,
    /// Mouse action whose gesture is recorded, and whether it is added to the others.
    recording: Option<(MouseAction, bool)>,
    /// Why the gesture recorded last was not taken.
    refused: bool,
}

/// The parts of the keys and gestures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Section {
    #[default]
    Keys,
    Modifiers,
    Mouse,
    WheelTouch,
}

impl Section {
    const ALL: [Section; 4] = [Section::Keys, Section::Modifiers, Section::Mouse, Section::WheelTouch];

    fn title(self) -> &'static str {
        match self {
            Section::Keys => tr("Keyboard shortcuts"),
            Section::Modifiers => tr("Modifier keys"),
            Section::Mouse => tr("Mouse"),
            Section::WheelTouch => tr("Wheel and touch"),
        }
    }
}

impl Default for SettingsUi {
    fn default() -> Self {
        SettingsUi {
            open: false,
            node: Node::Appearance,
            filter: String::new(),
            new_group: String::new(),
            add_filter: String::new(),
            ribbon_tab: 0,
            confirm_group: None,
            section: Section::Keys,
            recording: None,
            refused: false,
        }
    }
}

impl SettingsUi {
    pub fn open_at(&mut self, node: Node) {
        self.open = true;
        self.node = node;
        if node == Node::Shortcuts {
            self.section = Section::Keys;
        }
    }

    /// A mouse gesture is being recorded: Esc cancels it instead of closing the dialog.
    pub fn recording(&self) -> bool {
        self.open && self.recording.is_some()
    }

    /// Opens the dialog at the node shown last.
    pub fn open(&mut self) {
        self.open = true;
    }
}

impl App {
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings.open {
            self.recording = None;
            self.settings.recording = None;
            return;
        }
        // One size for every node: the close button stays where it is.
        let screen = ctx.content_rect();
        let size = egui::vec2(860.0f32.min(screen.width() - 40.0), 560.0f32.min(screen.height() - 120.0));
        let (window, esc) = self.dialogs.window(ctx, tr("Settings"), "settings", None);
        let mut open = !esc;
        window.open(&mut open).fixed_size(size).show(ctx, |ui| {
            egui::Panel::left("settings_tree").resizable(true).default_size(200.0).show(ui, |ui| {
                ui.label(RichText::new(tr("Project")).strong());
                for n in Node::PROJECT {
                    if ui.selectable_label(self.settings.node == n, n.title()).clicked() {
                        self.settings.node = n;
                    }
                }
                ui.add_space(8.0);
                ui.label(RichText::new(tr("User")).strong());
                for n in Node::USER {
                    if ui.selectable_label(self.settings.node == n, n.title()).clicked() {
                        self.settings.node = n;
                    }
                }
            });
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading(self.settings.node.title());
                ui.separator();
                egui::ScrollArea::vertical().id_salt("settings_body").auto_shrink([false, false]).show(ui, |ui| match self.settings.node {
                    Node::WireNumbering => self.wire_numbering_body(ui),
                    Node::CrossReferences => self.xref_body(ui),
                    Node::Wires => self.wires_body(ui),
                    Node::Checks => self.check_settings_body(ui),
                    Node::Marking => self.marking_body(ui),
                    Node::Scheme => self.scheme_settings(ui),
                    Node::ProjectTools => self.project_tool_links(ui),
                    Node::Appearance => self.appearance_settings(ui),
                    Node::Startup => self.startup_settings(ui),
                    Node::Shortcuts => self.input_settings(ui),
                    Node::Ribbon => self.ribbon_settings(ui),
                    Node::Servers => self.server_links(ui),
                    Node::Updates => self.update_settings(ui),
                });
            });
        });
        self.settings.open = open;
    }

    fn scheme_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(tr(commands::def(Cmd::ExportSettings).tip));
        ui.horizontal(|ui| {
            for c in [Cmd::ExportSettings, Cmd::ImportSettings] {
                let d = commands::def(c);
                if icons::text_button(ui, d.icon, tr(d.title)).clicked() {
                    self.run(&ui.ctx().clone(), c);
                }
            }
        });
    }

    /// The tools that hold further project settings, as links.
    fn project_tool_links(&mut self, ui: &mut egui::Ui) {
        ui.weak(tr("These settings are edited in their panes:"));
        for c in [Cmd::Potentials, Cmd::Structure, Cmd::PropertyDatabase, Cmd::ProjectLanguages, Cmd::Forms, Cmd::Process, Cmd::Configurator, Cmd::ProjectProperties] {
            let d = commands::def(c);
            ui.horizontal(|ui| {
                // The pane opens beside the dialog, which stays open.
                if icons::text_button(ui, d.icon, tr(d.title)).clicked() {
                    self.run(&ui.ctx().clone(), c);
                }
                if !d.tip.is_empty() {
                    ui.weak(tr(d.tip));
                }
            });
        }
    }

    fn server_links(&mut self, ui: &mut egui::Ui) {
        ui.weak(tr("Addresses and sign-in of the servers are set in their panes:"));
        for c in [Cmd::DataPortal, Cmd::ProjectServer, Cmd::Vault, Cmd::ManagePlugins] {
            let d = commands::def(c);
            ui.horizontal(|ui| {
                // The pane opens beside the dialog, which stays open.
                if icons::text_button(ui, d.icon, tr(d.title)).clicked() {
                    self.run(&ui.ctx().clone(), c);
                }
                if !d.tip.is_empty() {
                    ui.weak(tr(d.tip));
                }
            });
        }
    }

    /// What opens when the editor starts: the start page, the last project, the project
    /// server or a chosen local file or server project.
    fn startup_settings(&mut self, ui: &mut egui::Ui) {
        use crate::startup::{ServerProject, Startup, Target, MODES};
        let local = FileIo::can_save_in_place();
        ui.weak(tr("A project file opened from the file manager opens instead. Unsaved changes of the last session are never dropped without asking."));
        ui.add_space(4.0);
        let mode = self.shell.startup.mode();
        for (m, title, tip) in MODES {
            if ui.radio(mode == m, tr(title)).clicked() {
                // A specific project starts as the open one.
                let target = match (self.remote.project(), &self.session.file_name) {
                    (Some(p), _) => Target::Server(p),
                    (None, Some(f)) if local => Target::File(f.clone()),
                    _ => Target::Server(ServerProject { url: self.remote.url.clone(), name: String::new() }),
                };
                self.shell.startup.set_mode(m, target);
            }
            ui.indent(("startup_tip", title), |ui| ui.weak(tr(tip)));
        }
        let Startup::Project(target) = &mut self.shell.startup else { return };
        ui.separator();
        ui.horizontal(|ui| {
            let file = matches!(target, Target::File(_));
            if local && ui.radio(file, tr("Local file")).clicked() && !file {
                *target = Target::File(self.session.file_name.clone().unwrap_or_default());
            }
            if ui.radio(!file, tr("Project on the project server")).clicked() && file {
                *target = Target::Server(ServerProject { url: self.remote.url.clone(), name: String::new() });
            }
        });
        let mut list = false;
        egui::Grid::new("startup_target").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| match target {
            Target::File(path) => {
                ui.label(tr("File"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
                    let r = ui.add_enabled(!self.recent.is_empty(), egui::Button::new(tr("Recent projects")));
                    egui::Popup::menu(&r).show(|ui| {
                        for p in &self.recent {
                            if ui.button(p.as_str()).clicked() {
                                *path = p.clone();
                            }
                        }
                    });
                });
                ui.end_row();
            }
            Target::Server(p) => {
                ui.label(tr("Server"));
                ui.add(egui::TextEdit::singleline(&mut p.url).desired_width(360.0));
                ui.end_row();
                ui.label(tr("Project"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(200.0).hint_text(tr("Name on the server")));
                    let r = ui.add_enabled(!self.remote.projects.is_empty(), egui::Button::new(tr("Projects on the server")));
                    egui::Popup::menu(&r).show(|ui| {
                        for s in &self.remote.projects {
                            if ui.button(s.name.as_str()).clicked() {
                                p.name = s.name.clone();
                            }
                        }
                    });
                    list = icons::button(ui, Icon::Rotate, tr("List projects")).clicked();
                });
                ui.end_row();
            }
        });
        if list {
            if let Startup::Project(Target::Server(p)) = &self.shell.startup {
                if self.remote.shared.is_none() {
                    self.remote.url = p.url.clone();
                }
            }
            self.remote.list();
        }
    }

    fn appearance_settings(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Grid::new("appearance").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label(tr("Theme"));
            ui.horizontal(|ui| self.cmd_menu(ui, Cmd::Theme));
            ui.end_row();
            ui.label(tr("UI scale"));
            ui.horizontal_wrapped(|ui| {
                for s in SCALES {
                    ui.selectable_value(&mut self.shell.ui_scale, s, format!("{:.0} %", s * 100.0));
                }
            });
            ui.end_row();
            ui.label(tr("Tablet mode"));
            let mut tablet = self.shell.tablet();
            if ui.checkbox(&mut tablet, tr(commands::def(Cmd::TabletMode).tip)).changed() {
                self.shell.set_tablet(tablet);
            }
            ui.end_row();
            ui.label(tr("Ribbon"));
            ui.checkbox(&mut self.shell.ribbon_minimized, tr("Minimize the ribbon"));
            ui.end_row();
            ui.label(tr("Title bar"));
            ui.checkbox(&mut self.shell.frame_stats, tr("Show the frame time and the frames per second"));
            ui.end_row();
            ui.label(tr("Schematic crosshair"));
            ui.checkbox(&mut self.shell.full_crosshair, tr("Extend guides to canvas edges"));
            ui.end_row();
            ui.label(tr("3D view"));
            ui.horizontal(|ui| {
                let r = ui.checkbox(&mut self.shell.gpu_3d, tr("Draw with the graphics card"));
                r.on_hover_text(tr("Draws the 3D view of the panel layout with OpenGL 3 / WebGL 2, much faster for large rooms; off: drawn by the processor"));
                if self.shell.gpu_3d && self.panel_ui.gpu_failed() {
                    ui.colored_label(crate::style::warning(), tr("not available, drawn by the processor"));
                }
            });
            ui.end_row();
            ui.label(tr("Data portal"));
            ui.checkbox(&mut self.shell.catalog_logos, tr("Show the logos of the catalogs"))
                .on_hover_text(tr("The logos of the manufacturers' catalogs on the parts server, beside the details of a part"));
            ui.end_row();
            ui.label(tr("Wall opacity"));
            let mut percent = self.shell.wall_opacity * 100.0;
            let r = ui.add(egui::Slider::new(&mut percent, 0.0..=100.0).step_by(5.0).suffix(" %"));
            if r.on_hover_text(tr("Walls, ceilings and doors seen from the outside in the 3D view of the panel layout")).changed() {
                self.shell.wall_opacity = percent / 100.0;
            }
            ui.end_row();
            ui.label(tr("Language"));
            // All languages at once: a list that scrolls hid most of them.
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(420.0);
                for l in openecraft_i18n::Lang::ALL {
                    if ui.selectable_label(openecraft_i18n::lang() == l, l.native_name()).clicked() {
                        openecraft_i18n::set_lang(l);
                        openecraft_ui::fonts::follow_language(&ctx);
                    }
                }
            });
            ui.end_row();
        });
    }

    /// Keyboard shortcuts, modifier keys, mouse gestures, wheel and touch.
    fn input_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for s in Section::ALL {
                if ui.selectable_value(&mut self.settings.section, s, s.title()).changed() {
                    self.recording = None;
                    self.settings.recording = None;
                }
            }
        });
        ui.separator();
        match self.settings.section {
            Section::Keys => self.shortcut_settings(ui),
            Section::Modifiers => self.modifier_settings(ui),
            Section::Mouse => self.mouse_settings(ui),
            Section::WheelTouch => self.navigation_settings(ui),
        }
    }

    /// The modifier of every function done with one held: a choice of Shift, Ctrl, Alt
    /// or off, and the default.
    fn modifier_settings(&mut self, ui: &mut egui::Ui) {
        ui.weak(tr("Held while clicking, moving or scrolling. Ctrl is Cmd on macOS."));
        let conflicts = self.editor.gestures.conflicts();
        let error = crate::style::colors(self.dark).error;
        let name = |m: Option<Mod>| m.map_or_else(|| tr("Off").to_string(), |m| m.name().to_string());
        let g = &mut self.editor.gestures;
        egui::Grid::new("modifier_grid").num_columns(3).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for r in Role::ALL {
                ui.label(tr(r.title()));
                let now = g.modifier(r);
                let mut chosen = now;
                let clash = conflicts.iter().any(|(_, ids)| ids.contains(&r.id()));
                let shown = RichText::new(name(now)).monospace();
                egui::ComboBox::from_id_salt(("modifier", r.id())).selected_text(if clash { shown.color(error) } else { shown }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, None, tr("Off"));
                    for m in Mod::ALL {
                        ui.selectable_value(&mut chosen, Some(m), m.name());
                    }
                });
                if chosen != now {
                    g.set_modifier(r, chosen);
                }
                if ui.add_enabled(!g.modifier_is_default(r), egui::Button::new(tr("Default")).small()).clicked() {
                    g.set_modifier(r, r.default());
                }
                ui.end_row();
            }
        });
        self.gesture_conflicts(ui, &conflicts, |c| matches!(c, Clash::Modifier(_)));
        ui.separator();
        if ui.button(tr("Reset all to the defaults")).clicked() {
            self.editor.gestures.reset_modifiers();
        }
    }

    /// The buttons of every mouse action, grouped by the views: record, add, clear,
    /// default.
    fn mouse_settings(&mut self, ui: &mut egui::Ui) {
        ui.weak(tr("Record: do the gesture in the field that appears, holding a modifier if wanted (Esc cancels). Add keeps the gestures there are. Right click in the select tool opens the context menu."));
        let conflicts = self.editor.gestures.conflicts();
        for (title, view) in [(tr("Pages, panel layout and plans"), View::Flat), (tr("3D view"), View::Spatial)] {
            ui.add_space(6.0);
            ui.label(RichText::new(title).strong());
            for a in MouseAction::ALL.into_iter().filter(|a| a.view() == view) {
                ui.separator();
                self.mouse_row(ui, a, &conflicts);
            }
        }
        if self.settings.refused {
            ui.colored_label(crate::style::warning(), tr("Not taken: the left button alone selects, draws and moves; hold a modifier or use another button."));
        }
        self.gesture_conflicts(ui, &conflicts, |c| matches!(c, Clash::Mouse(..)));
        ui.separator();
        if ui.button(tr("Reset all to the defaults")).clicked() {
            self.editor.gestures.reset_mouse();
            self.settings.recording = None;
        }
    }

    /// A mouse action: its title, then how it is done, its gestures and the buttons,
    /// wrapping in narrow dialogs and long languages; the recording field below.
    fn mouse_row(&mut self, ui: &mut egui::Ui, a: MouseAction, conflicts: &[(Clash, Vec<&'static str>)]) {
        let recording = self.settings.recording.filter(|(r, _)| *r == a);
        let g = &mut self.editor.gestures;
        let bindings = g.mouse(a);
        ui.label(tr(a.title()));
        let text = bindings.iter().map(Binding::text).collect::<Vec<_>>().join(" / ");
        let shown = if recording.is_some() {
            RichText::new(tr("Do the gesture…")).italics()
        } else if text.is_empty() {
            RichText::new("–").weak()
        } else {
            RichText::new(text).monospace()
        };
        let clash = conflicts.iter().any(|(_, ids)| ids.contains(&a.id()));
        let shown = if clash { shown.color(crate::style::colors(self.dark).error) } else { shown };
        ui.horizontal_wrapped(|ui| {
            ui.weak(tr(a.kind().title()));
            ui.label(shown);
            ui.add_space(8.0);
            if ui.small_button(tr("Record")).clicked() {
                self.settings.recording = Some((a, false));
                self.settings.refused = false;
            }
            if ui.small_button(tr("Add")).on_hover_text(tr("Another gesture for the same function")).clicked() {
                self.settings.recording = Some((a, true));
                self.settings.refused = false;
            }
            if ui.add_enabled(!bindings.is_empty(), egui::Button::new(tr("Clear")).small()).on_hover_text(tr("No gesture")).clicked() {
                g.set_mouse(a, Vec::new());
            }
            if ui.add_enabled(!g.mouse_is_default(a), egui::Button::new(tr("Default")).small()).clicked() {
                g.set_mouse(a, a.defaults());
            }
        });
        if let Some((a, add)) = recording {
            self.gesture_pad(ui, a, add);
        }
    }

    /// The field a mouse gesture is recorded on.
    fn gesture_pad(&mut self, ui: &mut egui::Ui, a: MouseAction, add: bool) {
        let size = egui::vec2(ui.available_width().min(560.0), 80.0);
        let (resp, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let v = ui.visuals();
        painter.rect(resp.rect, 6.0, v.extreme_bg_color, v.selection.stroke, egui::StrokeKind::Inside);
        let text = tr_fmt!("{} – {} here (Esc cancels)", tr(a.title()), tr(a.kind().title()));
        painter.text(resp.rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), v.text_color());
        let (events, mods, esc) = ui.input(|i| (i.events.clone(), i.modifiers, i.key_pressed(Key::Escape)));
        if esc {
            self.settings.recording = None;
            return;
        }
        let Some(b) = gestures::recorded(a.kind(), &resp, &events, mods) else { return };
        self.settings.refused = !a.accepts(&b);
        if self.settings.refused {
            return;
        }
        if add {
            self.editor.gestures.add_mouse(a, b);
        } else {
            self.editor.gestures.set_mouse(a, vec![b]);
        }
        self.settings.recording = None;
    }

    /// The conflicts of the gestures `of` picks, as on the keyboard page.
    fn gesture_conflicts(&self, ui: &mut egui::Ui, conflicts: &[(Clash, Vec<&'static str>)], of: impl Fn(&Clash) -> bool) {
        for (clash, ids) in conflicts.iter().filter(|(c, _)| of(c)) {
            let names: Vec<&str> = ids.iter().map(|id| tr(gestures::title(id))).collect();
            ui.colored_label(crate::style::colors(self.dark).error, tr_fmt!("{} is used by: {}", clash.text(), names.join(", ")));
        }
    }

    /// What the wheel, a trackpad and a touch screen do in the 2D and 3D views.
    fn navigation_settings(&mut self, ui: &mut egui::Ui) {
        use openecraft_ui::nav::{Fingers, Scroll, WheelSourcePolicy};
        let nav = &mut self.shell.nav;
        // One block per setting: its name, then its options indented, a line between.
        let block = |ui: &mut egui::Ui, title: &str, first: bool, body: &mut dyn FnMut(&mut egui::Ui)| {
            if !first {
                ui.add_space(4.0);
                ui.separator();
            }
            ui.label(RichText::new(title).strong());
            ui.indent(title, |ui| body(ui));
        };
        block(ui, tr("Scrolling"), true, &mut |ui| {
            for s in Scroll::ALL {
                ui.radio_value(&mut nav.scroll, s, tr(s.title()));
            }
            ui.weak(match nav.scroll {
                Scroll::Auto => tr("Automatic mode pans with identified trackpad input, zooms with mouse-wheel input, and uses the wheel-source policy below for unknown sources."),
                Scroll::Zoom => tr("Scrolling up and down zooms at the pointer, sideways (horizontal wheel, Shift + wheel) pans."),
                Scroll::Pan => tr("Scrolling pans in every direction; pinch or Ctrl + scroll to zoom."),
            });
        });
        block(ui, tr("Wheel source"), false, &mut |ui| {
            for policy in WheelSourcePolicy::ALL {
                ui.radio_value(
                    &mut nav.wheel_source_policy,
                    policy,
                    tr(policy.title()),
                );
            }
            ui.weak(tr("Assume Windows trackpad (default) pans unknown point-based or fractional-line scrolling; whole-line and page scrolling zoom. Strict treats every unknown source as a mouse wheel."));
        });
        block(ui, tr("Direction"), false, &mut |ui| {
            ui.checkbox(&mut nav.invert, tr("Invert the direction of panning by scrolling"));
        });
        block(ui, tr("Two fingers"), false, &mut |ui| {
            for f in Fingers::ALL {
                ui.radio_value(&mut nav.two_fingers, f, tr(f.title()));
            }
            ui.weak(tr("On a touch screen and a trackpad. Pan only: pinching does not zoom; zoom only: scrolling with two fingers on a trackpad zooms too, where the system tells trackpads apart. One finger works like the left mouse button."));
        });
        block(ui, tr("Three fingers"), false, &mut |ui| {
            ui.radio_value(&mut nav.three_fingers, None, tr("Like two fingers"));
            for f in Fingers::ALL {
                ui.radio_value(&mut nav.three_fingers, Some(f), tr(f.title()));
            }
            ui.weak(tr("On a touch screen; trackpads leave three-finger gestures to the system."));
        });
        ui.add_space(4.0);
        ui.separator();
        ui.weak(tr("The modifiers that turn the wheel sideways or make it zoom are set under Modifier keys."));
        ui.separator();
        if ui.button(tr("Reset all to the defaults")).clicked() {
            self.shell.nav = Default::default();
        }
    }

    /// Every command and key of the page editor with its keys: record, clear, default.
    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        ui.weak(tr("Record: press the new key with its modifiers (Esc cancels). Esc and copy / cut / paste stay fixed."));
        ui.add(egui::TextEdit::singleline(&mut self.settings.filter).hint_text(tr("Filter…")));
        let conflicts = self.editor.shortcuts.conflicts();
        let f = self.settings.filter.to_lowercase();
        // Grouped like the ribbon; a filter opens every group with a match.
        for (gi, (group, ids)) in shortcuts::groups().into_iter().enumerate() {
            let ids: Vec<&str> = ids
                .into_iter()
                .filter(|id| f.is_empty() || tr(shortcuts::title(id)).to_lowercase().contains(&f) || self.editor.shortcuts.text(id).to_lowercase().contains(&f))
                .collect();
            if ids.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(RichText::new(tr(group)).strong()).id_salt(("shortcut_group", gi)).default_open(gi == 0).open((!f.is_empty()).then_some(true)).show(ui, |ui| {
                egui::Grid::new(("shortcut_grid", gi)).num_columns(3).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                    for id in ids {
                        self.shortcut_row(ui, id, &conflicts);
                    }
                });
            });
        }
        for (key, ids) in &conflicts {
            let names: Vec<&str> = ids.iter().map(|id| tr(shortcuts::title(id))).collect();
            ui.colored_label(crate::style::colors(self.dark).error, tr_fmt!("{} is used by: {}", key.text(), names.join(", ")));
        }
        ui.separator();
        if ui.button(tr("Reset all to the defaults")).clicked() {
            self.editor.shortcuts.reset_all();
        }
    }

    /// A binding of the shortcut editor: title, keys, Record / Clear / Default.
    fn shortcut_row(&mut self, ui: &mut egui::Ui, id: &'static str, conflicts: &[(shortcuts::Shortcut, Vec<&'static str>)]) {
        let text = self.editor.shortcuts.text(id);
        ui.label(tr(shortcuts::title(id)));
        let clash = conflicts.iter().any(|(_, v)| v.contains(&id));
        let shown = if self.recording == Some(id) {
            RichText::new(tr("Press a key…")).italics()
        } else if text.is_empty() {
            RichText::new("–").weak()
        } else {
            RichText::new(text.clone()).monospace()
        };
        ui.label(if clash { shown.color(crate::style::colors(self.dark).error) } else { shown });
        if shortcuts::fixed(id).is_some() {
            ui.weak(tr("(fixed)"));
            ui.end_row();
            return;
        }
        ui.horizontal(|ui| {
            if ui.small_button(tr("Record")).clicked() {
                self.recording = Some(id);
            }
            if ui.add_enabled(!text.is_empty(), egui::Button::new(tr("Clear")).small()).on_hover_text(tr("No shortcut")).clicked() {
                self.editor.shortcuts.set(id, Vec::new());
            }
            if ui.add_enabled(!self.editor.shortcuts.is_default(id), egui::Button::new(tr("Default")).small()).clicked() {
                self.editor.shortcuts.reset(id);
            }
        });
        ui.end_row();
    }

    /// Hide groups, own groups of commands, export and import of the customisation.
    #[allow(clippy::too_many_lines)]
    fn ribbon_settings(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.weak(tr("Hide the groups you do not need and put the commands you use most into own groups."));
        ui.horizontal(|ui| {
            if icons::text_button(ui, Icon::Upload, tr("Export…")).clicked() {
                let json = self.ribbon.custom.to_json();
                self.export(&ctx, "ribbon.json".into(), (tr("Ribbon (JSON)"), "json"), json);
            }
            if icons::text_button(ui, Icon::Download, tr("Import…")).clicked() {
                self.io.open(&ctx, Purpose::Ribbon);
            }
            if ui.button(tr("Reset the ribbon")).clicked() {
                self.ribbon.custom = Default::default();
            }
        });
        ui.separator();
        let tabs: Vec<&ribbon::Tab> = TABS.iter().filter(|t| !t.groups.is_empty()).collect();
        ui.horizontal_wrapped(|ui| {
            for (i, t) in tabs.iter().enumerate() {
                ui.selectable_value(&mut self.settings.ribbon_tab, i, tr(t.title));
            }
        });
        let tab = tabs[self.settings.ribbon_tab.min(tabs.len() - 1)];
        ui.label(RichText::new(tr("Groups shown")).strong());
        for g in tab.groups {
            let mut shown = !self.ribbon.custom.is_hidden(tab, g);
            if ui.checkbox(&mut shown, tr(g.title)).changed() {
                self.ribbon.custom.set_hidden(tab, g, !shown);
            }
        }
        ui.separator();
        ui.label(RichText::new(tr("Own groups")).strong());
        let mut remove = None;
        let mut add: Option<(usize, String)> = None;
        let mut drop: Option<(usize, usize)> = None;
        for (gi, g, cmds) in self.ribbon.custom.groups_of(tab) {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&g.title).strong());
                    if icons::button(ui, Icon::Delete, tr("Delete the group")).clicked() {
                        self.settings.confirm_group = Some(gi);
                    }
                });
                if self.settings.confirm_group == Some(gi) {
                    ui.horizontal(|ui| {
                        ui.colored_label(crate::style::warning(), tr_fmt!("Delete the group {}?", g.title));
                        if ui.button(tr("Delete")).clicked() {
                            remove = Some(gi);
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            self.settings.confirm_group = None;
                        }
                    });
                }
                for (ci, c) in cmds.iter().enumerate() {
                    ui.horizontal(|ui| {
                        icons::show(ui, commands::def(*c).icon, 16.0);
                        ui.label(tr(commands::def(*c).title));
                        if icons::button(ui, Icon::Close, tr("Remove from the group")).clicked() {
                            drop = Some((gi, ci));
                        }
                    });
                }
                let r = icons::text_button(ui, Icon::Plus, tr("Add a command"));
                egui::Popup::menu(&r).show(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.settings.add_filter).hint_text(tr("Filter…")));
                    let f = self.settings.add_filter.to_lowercase();
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for d in commands::ALL.iter().filter(|d| f.is_empty() || tr(d.title).to_lowercase().contains(&f)) {
                            if ui.button(tr(d.title)).clicked() {
                                add = Some((gi, d.id.to_string()));
                                ui.close();
                            }
                        }
                    });
                });
            });
        }
        if let Some(gi) = remove {
            self.ribbon.custom.groups.remove(gi);
            self.settings.confirm_group = None;
        }
        if let Some((gi, id)) = add {
            if !self.ribbon.custom.groups[gi].commands.contains(&id) {
                self.ribbon.custom.groups[gi].commands.push(id);
            }
        }
        if let Some((gi, ci)) = drop {
            // The visible commands skip unknown ids; remove by the shown index.
            let known: Vec<usize> = self.ribbon.custom.groups[gi].commands.iter().enumerate().filter(|(_, id)| commands::by_id(id).is_some()).map(|(i, _)| i).collect();
            if let Some(&i) = known.get(ci) {
                self.ribbon.custom.groups[gi].commands.remove(i);
            }
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.settings.new_group).hint_text(tr("name")).desired_width(160.0));
            if ui.add_enabled(!self.settings.new_group.trim().is_empty(), egui::Button::new(tr("New group"))).clicked() {
                self.ribbon.custom.groups.push(CustomGroup { tab: tab.key.to_string(), title: self.settings.new_group.trim().to_string(), commands: Vec::new() });
                self.settings.new_group.clear();
            }
        });
    }

    /// A ribbon customisation file was opened.
    pub(super) fn import_ribbon(&mut self, ctx: &egui::Context, text: &str) {
        match ribbon::Custom::from_json(text) {
            Ok(c) => {
                self.ribbon.custom = c;
                self.notify(ctx, tr("Ribbon imported"));
            }
            Err(e) => self.notify_error(ctx, tr_fmt!("Import failed: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests;
