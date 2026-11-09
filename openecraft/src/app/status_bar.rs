//! The status bar: position of the cursor (and typed coordinates), grid, layer, zoom,
//! the hint of the tool, the work area, the language, the lock of the page, the state
//! of the servers and the messages of the session. Segments open what they show.

use super::*;
use crate::commands::Cmd;

impl App {
    #[allow(clippy::too_many_lines)]
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let on_page = self.active_doc == Pane::Page;
        let in_layout = self.active_doc == Pane::LayoutView;
        // The hint fills the space between the left and the right segments; a tool
        // other than Select keeps its hint before what is under the pointer.
        let hint = if in_layout {
            // The panel layout: what the tool expects, then the position or the last message.
            let tool = LAYOUT_TOOLS.iter().find(|c| self.cmd_checked(**c) == Some(true)).map(|c| tr(crate::commands::def(*c).tip)).unwrap_or("");
            match (tool.is_empty(), self.panel_ui.status.is_empty()) {
                (true, _) => self.panel_ui.status.clone(),
                (false, true) => tool.to_string(),
                (false, false) => format!("{}   |   {}", tool, self.panel_ui.status),
            }
        } else if !on_page {
            self.panel_ui.status.clone()
        } else if self.editor.status.is_empty() {
            self.editor.hint()
        } else if self.editor.tool == Tool::Select {
            self.editor.status.clone()
        } else {
            format!("{}   |   {}", self.editor.hint(), self.editor.status)
        };
        // Narrow windows leave out the less needed segments instead of drawing them
        // over each other (the typed coordinates, the layer, the grid, the file name).
        let width = ui.available_width();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if on_page {
                let pos = match self.editor.hover {
                    Some(p) if width > 900.0 => format!("x {:>7.1}  y {:>7.1} mm", p.x, p.y),
                    Some(p) => format!("{:.0} / {:.0}", p.x, p.y),
                    None => "x –  y –".to_string(),
                };
                ui.monospace(pos).on_hover_text(tr("Position of the cursor"));
                if width > 1100.0 {
                    self.coordinate_field(ui);
                } else {
                    // Narrow windows: the field opens from a button.
                    let r = icons::button(ui, Icon::Keyboard, tr("Type a point"));
                    egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| self.coordinate_field(ui));
                }
                ui.separator();
                if width > 800.0 {
                    let grid = if self.editor.show_grid { tr_fmt!("Grid {} mm", self.editor.grid) } else { tr("Grid off").to_string() };
                    let r = ui.add(egui::Button::new(grid).frame(false)).on_hover_text(tr("Grid size"));
                    egui::Popup::menu(&r).show(|ui| {
                        self.cmd_button(ui, Cmd::Grid);
                        self.cmd_menu(ui, Cmd::GridSize);
                    });
                }
                if width > 1000.0 {
                    let layer = if self.editor.active_layer.is_empty() { tr("(default)").to_string() } else { self.editor.active_layer.clone() };
                    let r = ui.add(egui::Button::new(tr_fmt!("Layer: {}", layer)).frame(false)).on_hover_text(tr("Layer"));
                    egui::Popup::menu(&r).show(|ui| self.cmd_menu(ui, Cmd::ActiveLayer));
                }
                let r = ui.add(egui::Button::new(format!("{:.0} %", self.editor.zoom / crate::editor::ZOOM_ACTUAL * 100.0)).frame(false)).on_hover_text(tr("Zoom: 100% ≈ real size on a 96 dpi screen. Click fits the page."));
                if r.clicked() {
                    self.editor.fit_pending = true;
                }
                ui.separator();
            }
            if in_layout {
                if let Some(z) = self.panel_ui.zoom_percent() {
                    if ui.add(egui::Button::new(format!("{z:.0} %")).frame(false)).on_hover_text(tr("Zoom: click fits the view.")).clicked() {
                        self.panel_ui.fit();
                    }
                    ui.separator();
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Messages of the session: the newest for a few seconds, all on click.
                let r = icons::button(ui, Icon::List, tr("Messages of this session"));
                egui::Popup::menu(&r).show(|ui| {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        let normal = ui.visuals().text_color();
                        for m in self.messages.iter().rev() {
                            ui.horizontal(|ui| {
                                if m.level == crate::style::Level::Error {
                                    icons::show(ui, Icon::Error, 14.0);
                                }
                                let r = ui.label(RichText::new(&m.text).color(m.level.color(self.dark, normal)));
                                if !m.detail.is_empty() {
                                    r.on_hover_text(&m.detail);
                                }
                            });
                            if !m.detail.is_empty() {
                                ui.weak(&m.detail);
                            }
                        }
                        if self.messages.is_empty() {
                            ui.weak(tr("(no entries)"));
                        }
                    });
                });
                if let Some((msg, t)) = &self.message {
                    // Errors stay longer than the other messages.
                    let shown = if msg.level == crate::style::Level::Error { 12.0 } else { 6.0 };
                    if ctx.input(|i| i.time) - t < shown {
                        let r = ui.label(RichText::new(&msg.text).color(msg.level.color(self.dark, ui.visuals().text_color())));
                        if !msg.detail.is_empty() {
                            r.on_hover_text(&msg.detail);
                        }
                    }
                }
                ui.separator();
                // Servers: project server, parts server.
                if let Some(sh) = &self.remote.shared {
                    let r = ui.label(RichText::new(tr_fmt!("{} (project server, revision {})", sh.name, sh.revision)).small());
                    if r.on_hover_text(tr("Project server")).clicked() {
                        self.show_pane(Pane::Tool(ToolPane::ProjectServer));
                    }
                }
                // A shared project is saved on the project server (shown before), not in a file.
                let file = match self.session.file_name.as_deref() {
                    Some(f) => io::short_name(f).to_string(),
                    None if self.remote.shared.is_some() => String::new(),
                    // A project without a file and without changes (the demo, a new
                    // one) has nothing to save yet.
                    None if !self.session.dirty() => String::new(),
                    None => tr("unsaved").to_string(),
                };
                if width > 700.0 && !(file.is_empty() && !self.session.dirty()) {
                    ui.weak(format!("{}{}", file, if self.session.dirty() { " *" } else { "" }));
                    ui.separator();
                }
                let lang = openecraft_i18n::lang();
                let r = ui.add(egui::Button::new(lang.native_name()).frame(false)).on_hover_text(tr("Language"));
                egui::Popup::menu(&r).show(|ui| self.cmd_menu(ui, Cmd::Language));
                if on_page {
                    let pg = &self.session.prj.pages[self.session.page];
                    if pg.locked {
                        icons::show(ui, Icon::Lock, 14.0).on_hover_text(tr("Released page – read only"));
                    } else if let Some(user) = self.page_locked_by() {
                        icons::show(ui, Icon::Lock, 14.0).on_hover_text(tr_fmt!("Locked by {} – read only", user));
                    }
                }
                self.update_status(ui);
                for label in self.jobs_running() {
                    ui.label(RichText::new(label).small());
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.separator();
                }
                if self.session.routing() {
                    ui.label(RichText::new(tr("Updating the 3D routing")).small()).on_hover_text(tr("Cables and pipes are routed again in the background; until then the 3D view shows their earlier routes."));
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.separator();
                }
                // The project check applies in every workspace.
                let n = self.session.analysis.issues.iter().filter(|i| i.severity == Severity::Error && !i.accepted).count();
                let w = self.session.analysis.issues.iter().filter(|i| i.severity == Severity::Warning && !i.accepted).count();
                let c = crate::style::colors(self.dark);
                let r = ui.add(egui::Button::new(RichText::new(super::errors_warnings(n, w)).color(if n > 0 { c.error } else if w > 0 { c.warning } else { ui.visuals().weak_text_color() })).frame(false));
                if r.on_hover_text(tr("Results of the project check")).clicked() {
                    self.show_tab(LeftTab::Check);
                }
                ui.separator();
                if on_page {
                    // Work area: the navigators show its pages and devices.
                    let areas: Vec<String> = self.session.prj.structures.work_areas.iter().map(|w| w.name.clone()).collect();
                    if (!areas.is_empty() || !self.work_area.is_empty()) && width > 900.0 {
                        let shown = if self.work_area.is_empty() { tr("(whole project)").to_string() } else { self.work_area.clone() };
                        egui::ComboBox::from_id_salt("status_work_area").selected_text(shown).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.work_area, String::new(), tr("(whole project)"));
                            for a in areas {
                                ui.selectable_value(&mut self.work_area, a.clone(), a);
                            }
                        });
                        ui.label(tr("Work area"));
                    }
                }
                // Long hints are cut off (the whole text shows on hover).
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(hint).small()).truncate());
                });
            });
        });
    }
}

/// The tools of the panel layout whose tooltip says what they expect.
const LAYOUT_TOOLS: [Cmd; 17] = [
    Cmd::LayoutPlotBoundary,
    Cmd::LayoutReferencePoint,
    Cmd::LayoutMeasure,
    Cmd::LayoutSelect,
    Cmd::LayoutRail,
    Cmd::LayoutDuct,
    Cmd::LayoutBusbar,
    Cmd::LayoutCutout,
    Cmd::LayoutRoutingPath,
    Cmd::LayoutReservation,
    Cmd::LayoutDrawWalls,
    Cmd::LayoutInnerWalls,
    Cmd::LayoutRoomDoor,
    Cmd::LayoutRoomWindow,
    Cmd::LayoutRoomPenetration,
    Cmd::LayoutRoomEntry,
    Cmd::LayoutPipeRun,
];
