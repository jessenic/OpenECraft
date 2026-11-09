//! The docked panes (see `crate::dock`): every pane drawn by the app, its tab title,
//! the pane menu (float, close, help), the page tabs above the page and the panes
//! floating as windows.

use super::*;
use crate::dock::{self, Pane};

/// What the pane menu of a tab bar asked for.
enum PaneRequest {
    Show(Pane),
    Float(Pane),
    Close(Pane),
    Help(Pane),
}

/// Draws the panes of the tile tree with the app.
struct Docked<'a> {
    app: &'a mut App,
    idx: &'a Index,
    request: Option<PaneRequest>,
    documents: usize,
    /// Tabs of bars too narrow for their titles: icons with tooltips instead.
    compact: Vec<egui_tiles::TileId>,
}

/// Draws a tab the way egui_tiles does (the title, the close button, dragging) with
/// the given margin beside the title.
struct PlainTab(egui::WidgetText, f32);

impl egui_tiles::Behavior<Pane> for PlainTab {
    fn pane_ui(&mut self, _ui: &mut egui::Ui, _tile: egui_tiles::TileId, _pane: &mut Pane) -> egui_tiles::UiResponse {
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, _pane: &Pane) -> egui::WidgetText {
        self.0.clone()
    }

    fn tab_title_for_tile(&mut self, _tiles: &egui_tiles::Tiles<Pane>, _tile: egui_tiles::TileId) -> egui::WidgetText {
        self.0.clone()
    }

    fn tab_title_spacing(&self, _visuals: &egui::Visuals) -> f32 {
        self.1
    }
}

/// The icon of a pane (of the command that shows it).
fn pane_icon(pane: Pane) -> Icon {
    crate::commands::for_pane(pane).map(|c| crate::commands::def(c).icon).unwrap_or(Icon::File)
}

impl egui_tiles::Behavior<Pane> for Docked<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile: egui_tiles::TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        self.app.pane_ui(ui, *pane, self.idx);
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        self.app.pane_title(*pane)
    }

    /// A tab holding several panes (split inside a tab) is named after them.
    fn tab_title_for_tile(&mut self, tiles: &egui_tiles::Tiles<Pane>, tile: egui_tiles::TileId) -> egui::WidgetText {
        let mut panes = Vec::new();
        let mut open = vec![tile];
        while let Some(id) = open.pop() {
            match tiles.get(id) {
                Some(egui_tiles::Tile::Pane(p)) => panes.push(*p),
                Some(egui_tiles::Tile::Container(c)) => open.extend(c.children_vec().into_iter().rev()),
                None => {}
            }
        }
        match panes.as_slice() {
            [pane] => self.app.pane_title(*pane),
            _ => panes.iter().map(|p| self.app.pane_title(*p).text().to_string()).collect::<Vec<_>>().join(" + ").into(),
        }
    }

    fn is_tab_closable(&self, tiles: &egui_tiles::Tiles<Pane>, tile: egui_tiles::TileId) -> bool {
        tiles.get_pane(&tile).is_some_and(|p| p.has_close_button() && (!p.is_document() || self.documents > 1))
    }

    /// Tabs of a bar too narrow for their titles show their icon (the title on hover).
    fn tab_ui(&mut self, tiles: &mut egui_tiles::Tiles<Pane>, ui: &mut egui::Ui, id: egui::Id, tile: egui_tiles::TileId, state: &egui_tiles::TabState) -> egui::Response {
        let title = self.tab_title_for_tile(tiles, tile);
        let compact = self.compact.contains(&tile);
        let pane = tiles.get_pane(&tile).copied();
        match pane {
            Some(pane) if compact => {
                // Blanks as wide as the icon painted over them.
                let mut plain = PlainTab("    ".into(), 2.0);
                let compact_state = egui_tiles::TabState { closable: false, ..state.clone() };
                let r = plain.tab_ui(tiles, ui, id, tile, &compact_state);
                let color = if state.active { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                icons::paint(ui.painter(), egui::Rect::from_center_size(r.rect.center(), egui::Vec2::splat(13.0)), pane_icon(pane), color);
                r.on_hover_text(title.text())
            }
            _ => PlainTab(title, 8.0).tab_ui(tiles, ui, id, tile, state),
        }
    }

    fn top_bar_right_ui(&mut self, tiles: &egui_tiles::Tiles<Pane>, ui: &mut egui::Ui, _tile: egui_tiles::TileId, tabs: &egui_tiles::Tabs, _scroll: &mut f32) {
        let Some(pane) = tabs.active.and_then(|id| tiles.get_pane(&id)).copied() else { return };
        // Titles that do not fit: icons, and a list of all tabs of the bar.
        let font = egui::TextStyle::Button.resolve(ui.style());
        let needed: f32 = tabs
            .children
            .iter()
            .map(|c| {
                let title = self.tab_title_for_tile(tiles, *c).text().to_string();
                let close = if self.is_tab_closable(tiles, *c) { 16.0 } else { 0.0 };
                ui.painter().layout_no_wrap(title, font.clone(), egui::Color32::WHITE).size().x + 16.0 + close
            })
            .sum();
        let compact = tabs.children.len() > 1 && needed > ui.available_width() - 34.0;
        ui.add_space(4.0);
        let r = icons::button(ui, Icon::Dots, tr("Pane menu"));
        egui::Popup::menu(&r).show(|ui| {
            if icons::text_button(ui, Icon::Float, tr("Float")).on_hover_text(tr("Show the pane as a window")).clicked() {
                self.request = Some(PaneRequest::Float(pane));
                ui.close();
            }
            if pane != Pane::Page && (!pane.is_document() || self.documents > 1) && icons::text_button(ui, Icon::Close, tr("Close")).clicked() {
                self.request = Some(PaneRequest::Close(pane));
                ui.close();
            }
            if icons::text_button(ui, Icon::Help, tr("Help")).clicked() {
                self.request = Some(PaneRequest::Help(pane));
                ui.close();
            }
        });
        if compact {
            self.compact.extend(tabs.children.iter().copied());
            let r = icons::button(ui, Icon::ChevronDown, tr("All tabs"));
            egui::Popup::menu(&r).show(|ui| {
                for c in &tabs.children {
                    let Some(p) = tiles.get_pane(c).copied() else { continue };
                    let title = self.tab_title_for_tile(tiles, *c);
                    let picked = ui
                        .horizontal(|ui| {
                            icons::show(ui, pane_icon(p), 14.0);
                            ui.selectable_label(Some(*c) == tabs.active, title).clicked()
                        })
                        .inner;
                    if picked {
                        self.request = Some(PaneRequest::Show(p));
                        ui.close();
                    }
                }
            });
        }
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions { all_panes_must_have_tabs: true, ..Default::default() }
    }
}

impl App {
    /// The panes of the workspace as the user arranged them.
    pub(super) fn docked(&mut self, ui: &mut egui::Ui, idx: &Index) {
        let mut tree = std::mem::replace(&mut self.dock, egui_tiles::Tree::empty(egui::Id::unique("dock_swap")));
        let documents = dock::panes(&tree).iter().filter(|p| p.is_document()).count();
        self.in_dock = true;
        let mut behavior = Docked { app: self, idx, request: None, documents, compact: Vec::new() };
        tree.ui(&mut behavior, ui);
        let request = behavior.request.take();
        self.in_dock = false;
        // egui_tiles shows the grab hand over tabs, which Windows draws as the move cross.
        ui.ctx().output_mut(|o| {
            if o.cursor_icon == egui::CursorIcon::Grab {
                o.cursor_icon = egui::CursorIcon::Default;
            }
        });
        if tree.root().is_none() || dock::panes(&tree).is_empty() {
            tree = dock::default_tree_for(self.workspace);
        }
        self.dock = tree;
        // A click on a tab shows another document: keys and the status bar follow it.
        if !self.floating.contains(&self.active_doc) {
            self.active_doc = dock::shown_document(&self.dock, self.active_doc);
        }
        match request {
            Some(PaneRequest::Show(p)) => self.show_pane(p),
            Some(PaneRequest::Float(p)) => {
                dock::close(&mut self.dock, p);
                if dock::find(&self.dock, p).is_none() {
                    self.shell.placement.set(p, true);
                    self.show_pane(p);
                }
            }
            Some(PaneRequest::Close(p)) => dock::close(&mut self.dock, p),
            Some(PaneRequest::Help(p)) => self.help_for(p),
            None => {}
        }
    }

    /// Panes shown as windows, opening at the right of the editor; Dock puts them into
    /// the workspace (and they open docked from then on).
    pub(super) fn floating_panes(&mut self, ctx: &egui::Context) {
        // A pane docked in the workspace switched to is shown there only.
        self.floating.retain(|p| dock::find(&self.dock, *p).is_none());
        let idx = std::mem::take(&mut self.session.index);
        let screen = ctx.content_rect();
        for (i, pane) in self.floating.clone().into_iter().enumerate() {
            let mut open = true;
            let mut dock_back = false;
            let title = self.pane_title(pane).text().to_string();
            let id = egui::Id::unique(("floating", pane));
            let [w, h] = match pane {
                Pane::Tool(t) => t.window_size(),
                _ => [640.0, 440.0],
            };
            let size = egui::vec2(w.min(screen.width() * 0.9), h.min(screen.height() * 0.8));
            // Over the right side of the editor (the Insert center and the properties),
            // so that the page stays visible; egui keeps where and how large the user
            // left the window.
            let at = egui::pos2(screen.right() - 16.0, screen.top() + 140.0) + egui::vec2(-24.0, 24.0) * (i % 8) as f32;
            if self.raise == Some(pane) {
                self.raise = None;
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            }
            let max = egui::vec2(screen.width() * 0.95, screen.height() * 0.9);
            egui::Window::new(title).id(id).open(&mut open).default_size(size).max_size(max).constrain(true).default_pos(at).pivot(egui::Align2::RIGHT_TOP).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if icons::text_button(ui, Icon::Panes, tr("Dock")).on_hover_text(tr("Put the pane back into the workspace")).clicked() {
                        dock_back = true;
                    }
                });
                ui.separator();
                self.pane_ui(ui, pane, &idx);
            });
            if !open || dock_back {
                self.floating.retain(|p| *p != pane);
                if !open {
                    // The flag that asked for the pane would open it again next frame.
                    self.forget_request(pane);
                }
                if dock_back {
                    self.shell.placement.set(pane, false);
                    self.show_pane(pane);
                }
            }
        }
        self.session.index = idx;
    }

    /// The content of a pane.
    pub(super) fn pane_ui(&mut self, ui: &mut egui::Ui, pane: Pane, idx: &Index) {
        // Clicks make a pane the active one: keys go to the document worked in.
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            self.active_pane = Some(pane);
            self.focus_pane = None;
            if pane.is_document() {
                self.active_doc = pane;
            }
        }
        if self.focus_pane == Some(pane) {
            ui.painter().rect_stroke(ui.max_rect().shrink(1.0), 2.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
        }
        match pane {
            Pane::Page => self.page_pane(ui, idx),
            Pane::LayoutView => self.layout_view_pane(ui),
            Pane::WiringWorkbench => self.wiring_workbench_pane(ui),
            pane => {
                egui::Frame::new().inner_margin(4).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    match pane {
                        Pane::Navigator(tab) => self.navigator(ui, idx, tab),
                        Pane::Library => self.insert_center_pane(ui),
                        Pane::Properties => {
                            egui::ScrollArea::vertical().id_salt("props").auto_shrink([false, false]).show(ui, |ui| self.properties(ui, idx));
                        }
                        Pane::LayoutSpaces => self.layout_spaces_pane(ui),
                        Pane::LayoutProperties => self.layout_properties_pane(ui),
                        Pane::WiringChecklist => self.wiring_checklist_pane(ui),
                        Pane::Tool(t) => self.tool_pane(ui, t),
                        Pane::Page | Pane::LayoutView | Pane::WiringWorkbench => {}
                    }
                });
            }
        }
    }

    fn tool_pane(&mut self, ui: &mut egui::Ui, t: ToolPane) {
        use ToolPane as T;
        match t {
            T::Start => self.start_page(ui),
            T::Help => self.help_pane(ui),
            T::Reports => self.reports_pane(ui),
            T::Tables => self.tables_pane(ui),
            T::Find => self.find_pane(ui),
            T::Cables => self.cables_pane(ui),
            T::Potentials => self.potentials_pane(ui),
            T::TerminalStrips => self.strips_pane(ui),
            T::Plugs => self.plugs_pane(ui),
            T::PlcOverview => self.plc_overview_pane(ui),
            T::PlcHardware => self.plc_hw_pane(ui),
            T::Macros => self.macros_pane(ui),
            T::Harnesses => self.harness_pane(ui),
            T::FieldWiring => self.field_pane(ui),
            T::Buildings => self.buildings_pane(ui),
            T::Revisions => self.revisions_pane(ui),
            T::Compare => self.compare_pane(ui),
            T::ChangeRequests => self.requests_pane(ui),
            T::Preplanning => self.preplanning_pane(ui),
            T::SingleLine => self.singleline_pane(ui),
            T::Process => self.process_pane(ui),
            T::Electrical => self.electrical_pane(ui),
            T::Integration => self.integration_pane(ui),
            T::Languages => self.languages_pane(ui),
            T::Configurator => self.config_pane(ui),
            T::Projects => self.projects_pane(ui),
            T::Structure => self.structure_pane(ui),
            T::PropertyDatabase => self.property_db_pane(ui),
            T::SymbolEditor => self.symbol_editor_pane(ui),
            T::Forms => self.forms_pane(ui),
            T::ReportBlocks => self.report_blocks_pane(ui),
            T::Parts => self.parts_pane(ui),
            T::DataPortal => self.portal_pane(ui),
            T::ProjectServer => self.remote_pane(ui),
            T::Vault => self.vault_pane(ui),
            T::Plugins => self.plugins_pane(ui),
            T::PluginCommands => self.plugin_commands_pane(ui),
        }
    }

    /// Shows a navigator tab (opens it again if it was closed).
    pub(super) fn show_tab(&mut self, tab: LeftTab) {
        self.show_pane(Pane::Navigator(tab));
    }

    /// Title of a pane's tab; the messages and review tabs count what is open.
    pub(super) fn pane_title(&self, pane: Pane) -> egui::WidgetText {
        match pane {
            // The page tabs below name the page.
            Pane::Page => tr("Page").into(),
            Pane::Properties => tr("Properties").into(),
            Pane::Navigator(LeftTab::Check) => {
                let n = self.session.analysis.issues.iter().filter(|i| i.severity == Severity::Error && !i.accepted).count();
                if n > 0 {
                    RichText::new(tr_fmt!("Messages ({})", n)).color(crate::style::colors(self.dark).error).into()
                } else {
                    tr("Messages").into()
                }
            }
            Pane::Navigator(LeftTab::Review) => {
                let n = self.session.prj.review.live_comments().filter(|c| !c.status.is_done()).count();
                if n > 0 { tr_fmt!("Review ({})", n).into() } else { tr("Review").into() }
            }
            Pane::Tool(ToolPane::Reports) => tr("Reports").into(),
            Pane::Tool(ToolPane::Find) => tr("Find").into(),
            Pane::Tool(ToolPane::Tables) => tr("Edit in table").into(),
            pane => match crate::commands::for_pane(pane) {
                Some(c) => tr(crate::commands::def(c).title).trim_end_matches('…').to_string().into(),
                None => String::new().into(),
            },
        }
    }

    /// The page being edited, with the page tabs above it.
    fn page_pane(&mut self, ui: &mut egui::Ui, idx: &Index) {
        let ctx = ui.ctx().clone();
        self.page_tabs_bar(ui);
        let page = self.session.page;
        let locked_by = self.page_locked_by();
        self.editor.read_only = locked_by.is_some() || self.remote.viewer();
        self.editor.read_only_reason = match locked_by {
            Some(user) => tr_fmt!("Locked by {} on the project server – read only", user),
            None if self.remote.viewer() => tr("You may only view this project.").to_string(),
            None => String::new(),
        };
        self.editor.review_selected = self.review_ui.selected;
        let out = self.editor.ui(ui, &mut self.session.prj, page, idx, &self.session.analysis);
        if out.open_properties {
            self.open_properties(&ctx);
        }
        if let Some(url) = out.open_url {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        if let Some(msg) = out.message {
            self.notify(&ctx, msg);
        }
        if let (Some(id), Some(part)) = (out.placed, out.placed_part) {
            self.placed(page, id, &part);
            if matches!(self.editor.tool, Tool::Place(_)) {
                self.arm_part(&part,None);
            }
        } else if let Some(id) = out.placed {
            if let Some(s) = self.session.prj.pages[page].symbols.iter().find(|s| s.id == id && s.sym.starts_with("user:ema:")) {
                let tag = s.tag.clone();
                parts::link_unassigned_plc_parts(&mut self.session.prj, &self.parts_db, Some(&tag));
            }
        }
        if let Some((number, at)) = out.dropped_part {
            self.drop_part(&ctx, page, number, at);
        }
        if let Some(resp) = &out.response {
            // A right click while the menu is open closes it; the next frame opens it
            // again at the new spot.
            let menu = egui::Popup::default_response_id(resp);
            let reopen = menu.with("reopen");
            let mut popup = egui::Popup::context_menu(resp);
            if ctx.data_mut(|d| d.remove_temp::<bool>(reopen)).is_some() {
                popup = popup.open_memory(egui::SetOpenCommand::Bool(true));
            } else if resp.secondary_clicked() && egui::Popup::is_id_open(&ctx, menu) {
                ctx.data_mut(|d| d.insert_temp(reopen, true));
                ctx.request_repaint();
            }
            popup.show(|ui| self.page_context_menu(ui));
        }
        if let Some(input) = out.review {
            self.review_input(input);
        } else if let Some(at) = out.clicked {
            self.review_pick(at);
        }
    }

    /// A symbol was placed with a part chosen in the Insert center: the part goes to
    /// its device.
    pub(super) fn placed(&mut self, page_i: usize, id: u64, number: &str) {
        parts::assign_placed_function_part(&mut self.session.prj, &self.parts_db, page_i, id, number);
    }

    /// Tabs of the pages opened lately; a click shows the page, the cross closes the tab.
    fn page_tabs_bar(&mut self, ui: &mut egui::Ui) {
        let current = self.session.prj.pages.get(self.session.page).map(|p| p.id);
        let mut goto = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &id in &self.page_tabs {
                let Some((i, pg)) = self.session.prj.pages.iter().enumerate().find(|(_, p)| p.id == id) else { continue };
                let name = structure::page_full_name(pg);
                let full = if pg.description.is_empty() { name } else { format!("{name}  {}", pg.description) };
                let label = dock::truncate_label(&full, 28);
                let mut r = ui.selectable_label(Some(id) == current, RichText::new(&label).small());
                if label != full {
                    r = r.on_hover_text(&full);
                }
                if r.clicked() {
                    goto = Some(i);
                }
                if r.middle_clicked() {
                    close = Some(id);
                }
                r.context_menu(|ui| {
                    if ui.button(tr("Page properties…")).clicked() {
                        self.page_props = Some(i);
                        ui.close();
                    }
                    if self.page_tabs.len() > 1 && ui.button(tr("Close page tab")).clicked() {
                        close = Some(id);
                        ui.close();
                    }
                });
                if self.page_tabs.len() > 1 && Some(id) == current && icons::button(ui, Icon::Close, tr("Close page tab")).clicked() {
                    close = Some(id);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Plus, tr("New schematic page")).clicked() {
                    self.add_page(PageKind::Schematic);
                }
            });
        });
        if let Some(i) = goto {
            self.goto(i, 0);
        }
        if let Some(id) = close {
            self.close_page_tab(id);
        }
    }

    /// Right click on the page: the commands for the selection or the page.
    /// On a page that cannot be edited only the entries that change nothing are shown.
    fn page_context_menu(&mut self, ui: &mut egui::Ui) {
        let has_sel = !self.editor.selection.is_empty();
        let editable = self.page_editable();
        let shown = |c: Cmd| editable || !crate::commands::changes_page(c);
        if has_sel {
            if self.selected_tag().is_some() {
                for c in [Cmd::Properties, Cmd::RenameDevice, Cmd::PartSelection].into_iter().filter(|c| shown(*c)) {
                    self.cmd_button(ui, c);
                }
                // A device placed in a cabinet or room: there in the layout or in 3D.
                let placed: Vec<Cmd> = [Cmd::ShowInLayout, Cmd::ShowIn3d].into_iter().filter(|c| self.cmd_enabled(*c)).collect();
                for c in placed {
                    self.cmd_button(ui, c);
                }
                ui.separator();
            }
            for c in [Cmd::Cut, Cmd::Copy, Cmd::Delete, Cmd::Rotate, Cmd::Mirror].into_iter().filter(|c| shown(*c)) {
                self.cmd_button(ui, c);
            }
            if editable {
                ui.menu_button(tr("Align"), |ui| self.cmd_menu(ui, Cmd::Align));
                ui.menu_button(tr("Distribute"), |ui| self.cmd_menu(ui, Cmd::Distribute));
            }
            for c in [Cmd::MoveBy, Cmd::MultipleCopy, Cmd::AutoConnectSelection, Cmd::SaveSelectionAsMacro].into_iter().filter(|c| shown(*c)) {
                self.cmd_button(ui, c);
            }
        } else {
            for c in [Cmd::Paste, Cmd::SelectAll, Cmd::InsertCenter, Cmd::WireTool, Cmd::TextTool].into_iter().filter(|c| shown(*c)) {
                self.cmd_button(ui, c);
            }
            ui.separator();
            for c in [Cmd::PageProperties, Cmd::NewSchematicPage, Cmd::FitPage, Cmd::JoinCollinear].into_iter().filter(|c| shown(*c)) {
                self.cmd_button(ui, c);
            }
        }
    }
}
