//! Runs the commands of `crate::commands` for the ribbon, the backstage, the quick
//! access row, the context menus, the command search and the shortcuts: whether a
//! command can run now, whether a toggle is on, the entries of the menus that change
//! with the project, and the global shortcuts.

use super::*;
use crate::commands::{self, Kind};
use openecraft_core::arrange::{self, Align};
use openecraft_core::review::{MarkupKind, Pen};
use openecraft_core::{conndef, graphics, routing};

impl App {
    /// Whether a command can run now.
    pub(super) fn cmd_enabled(&self, cmd: Cmd) -> bool {
        let has_sel = !self.editor.selection.is_empty();
        let pages = self.session.prj.pages.len();
        let page = self.session.page;
        if commands::changes_page(cmd) && !self.page_editable() {
            return false;
        }
        match cmd {
            Cmd::Undo => self.session.history.can_undo(),
            Cmd::Redo => self.session.history.can_redo(),
            Cmd::Cut | Cmd::Copy | Cmd::Delete | Cmd::Rotate | Cmd::Mirror | Cmd::MoveBy | Cmd::MultipleCopy | Cmd::SaveSelectionAsMacro | Cmd::AutoConnectSelection => has_sel,
            Cmd::Paste => self.editor.clip.is_some(),
            Cmd::Align => self.editor.selection.len() >= 2,
            Cmd::Distribute => self.editor.selection.len() >= 3,
            Cmd::PreviousPage => page > 0,
            Cmd::NextPage => page + 1 < pages,
            Cmd::MovePageUp => page > 0 && openecraft_core::sections::of_page(&self.session.prj, page - 1) == openecraft_core::sections::of_page(&self.session.prj, page),
            Cmd::MovePageDown => page + 1 < pages && openecraft_core::sections::of_page(&self.session.prj, page + 1) == openecraft_core::sections::of_page(&self.session.prj, page),
            Cmd::DeletePage => self.session.may_delete_page(page, self.page_held_by_other(page)),
            Cmd::CheckInVault => self.vault.origin.is_some() && self.vault.signed_in && !self.vault.busy(),
            Cmd::PdfInLanguage => !self.session.prj.languages.targets.is_empty(),
            Cmd::UpdateReportBlocks => !self.session.prj.report_blocks.is_empty(),
            Cmd::RenameDevice | Cmd::PartSelection => self.selected_tag().is_some(),
            Cmd::ShowInLayout | Cmd::ShowIn3d => self.selected_tag().is_some_and(|t| openecraft_core::panel::locate(&self.session.prj, &t).is_some()),
            Cmd::Quit => FileIo::has_own_window(),
            Cmd::SaveAs => FileIo::can_save_in_place(),
            Cmd::NextPageTab | Cmd::PreviousPageTab | Cmd::ClosePageTab => self.page_tabs.len() > 1,
            Cmd::LayoutDoorInside | Cmd::LayoutDoor | Cmd::LayoutFront => self.panel_ui.view == crate::panel_ui::View::Front && !self.panel_ui.in_room,
            Cmd::LayoutWalls => self.panel_ui.view != crate::panel_ui::View::ThreeD,
            Cmd::LayoutBusbar | Cmd::LayoutCutout | Cmd::LayoutRoutingPath | Cmd::LayoutReservation | Cmd::LayoutHeat | Cmd::LayoutNcData => !self.panel_ui.in_room,
            Cmd::LayoutWallPart | Cmd::LayoutFloorPart | Cmd::LayoutPipeRun | Cmd::LayoutFloorPlan | Cmd::LayoutDrawWalls | Cmd::LayoutInnerWalls | Cmd::LayoutRoomDoor | Cmd::LayoutRoomWindow | Cmd::LayoutRoomPenetration | Cmd::LayoutRoomEntry => self.panel_ui.in_room,
            Cmd::LayoutRackPart => self.session.prj.layout.enclosures.get(self.panel_ui.enclosure).is_some_and(openecraft_core::panel::is_rack),
            Cmd::LayoutLineup | Cmd::LayoutSite | Cmd::LayoutHideFrontWalls | Cmd::LayoutHideCeiling | Cmd::LayoutOpenDucts => self.panel_ui.view == crate::panel_ui::View::ThreeD,
            _ => true,
        }
    }

    /// Whether the current page can be edited by this user.
    pub(super) fn page_editable(&self) -> bool {
        let page = self.session.page;
        self.session.may_edit_page(page, self.page_held_by_other(page) || self.remote.viewer())
    }

    /// Whether a toggle is on; `None` for commands that are no toggles.
    pub(super) fn cmd_checked(&self, cmd: Cmd) -> Option<bool> {
        use crate::panel_ui::View;
        let tool = &self.editor.tool;
        let on = match cmd {
            Cmd::AutoConnect => self.editor.autoconnect,
            Cmd::Route => self.editor.autoroute,
            Cmd::Grid => self.editor.show_grid,
            Cmd::ConnectionPoints => self.editor.show_pins,
            Cmd::RevisionMarks => self.editor.show_revision_marks,
            Cmd::MacroBoxes => self.editor.show_macro_boxes,
            Cmd::WireColours => self.session.prj.wire_colors,
            Cmd::WorkspaceSchematic => self.workspace == Workspace::Schematic,
            Cmd::WorkspacePanel => self.workspace == Workspace::Panel,
            Cmd::WorkspaceWiring => self.workspace == Workspace::Wiring,
            Cmd::WorkspaceMounting => self.workspace == Workspace::Mounting,
            Cmd::WorkspaceReview => self.workspace == Workspace::Review,
            Cmd::WorkspacePreplanning => self.workspace == Workspace::Preplanning,
            Cmd::SelectTool => *tool == Tool::Select,
            Cmd::WireTool => *tool == Tool::Wire,
            Cmd::TextTool => *tool == Tool::Text && self.editor.text_proto.is_none(),
            Cmd::ConnDefTool => *tool == Tool::ConnDef,
            Cmd::CableDefTool => *tool == Tool::CableDef,
            Cmd::JunctionTool => *tool == Tool::Junction(JunctionKind::Tee),
            Cmd::CornerTool => *tool == Tool::Junction(JunctionKind::Corner),
            Cmd::CrossingTool => *tool == Tool::Junction(JunctionKind::Crossing),
            Cmd::BridgeTool => *tool == Tool::Junction(JunctionKind::Bridge),
            Cmd::LineTool => *tool == Tool::Line,
            Cmd::RectTool => *tool == Tool::Rect,
            Cmd::CircleTool => *tool == Tool::Circle,
            Cmd::EllipseTool => *tool == Tool::Ellipse,
            Cmd::ArcTool => *tool == Tool::Arc,
            Cmd::PolylineTool => *tool == Tool::Polyline,
            Cmd::PolygonTool => *tool == Tool::Polygon,
            Cmd::CommentTool => *tool == Tool::Comment,
            Cmd::DimAligned => *tool == Tool::Dimension(DimKind::Aligned),
            Cmd::DimHorizontal => *tool == Tool::Dimension(DimKind::Horizontal),
            Cmd::DimVertical => *tool == Tool::Dimension(DimKind::Vertical),
            Cmd::LayoutSelect => self.panel_ui.tool == crate::panel_ui::Tool::Select,
            Cmd::LayoutRail => self.panel_ui.tool == crate::panel_ui::Tool::Rail,
            Cmd::LayoutDuct => self.panel_ui.tool == crate::panel_ui::Tool::Duct,
            Cmd::LayoutBusbar => self.panel_ui.tool == crate::panel_ui::Tool::Busbar,
            Cmd::LayoutCutout => self.panel_ui.tool == crate::panel_ui::Tool::Cutout,
            Cmd::LayoutRoutingPath => self.panel_ui.tool == crate::panel_ui::Tool::RoutingPath,
            Cmd::LayoutReservation => self.panel_ui.tool == crate::panel_ui::Tool::Reservation,
            Cmd::LayoutPipeRun => self.panel_ui.tool == crate::panel_ui::Tool::PipeRun,
            Cmd::LayoutFront => self.panel_ui.view == View::Front && self.panel_ui.face == openecraft_core::panel::Face::Plate,
            Cmd::LayoutDoor => self.panel_ui.view == View::Front && self.panel_ui.face == openecraft_core::panel::Face::Door && !self.panel_ui.door_inside,
            Cmd::LayoutDoorInside => self.panel_ui.view == View::Front && self.panel_ui.face == openecraft_core::panel::Face::Door && self.panel_ui.door_inside,
            Cmd::Layout3d => self.panel_ui.view == View::ThreeD,
            Cmd::LayoutFloorPlan => self.panel_ui.view == View::Plan,
            Cmd::LayoutDrawWalls => self.panel_ui.tool == crate::panel_ui::Tool::Walls,
            Cmd::LayoutInnerWalls => self.panel_ui.tool == crate::panel_ui::Tool::InnerWalls,
            Cmd::LayoutRoomDoor => self.panel_ui.tool == crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Door),
            Cmd::LayoutRoomWindow => self.panel_ui.tool == crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Window),
            Cmd::LayoutRoomPenetration => self.panel_ui.tool == crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Penetration),
            Cmd::LayoutRoomEntry => self.panel_ui.tool == crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Entry),
            Cmd::LayoutWires => self.panel_ui.show_wires,
            Cmd::LayoutDuctFill => self.panel_ui.show_fill,
            Cmd::LayoutHeat => self.panel_ui.show_heat,
            Cmd::LayoutLineup => self.panel_ui.show_lineup,
            Cmd::LayoutSite => self.panel_ui.show_site,
            Cmd::LayoutHideFrontWalls => self.panel_ui.hide_front_walls,
            Cmd::LayoutHideCeiling => self.panel_ui.hide_ceiling,
            Cmd::LayoutOpenDucts => self.panel_ui.open_ducts,
            Cmd::WiringMode => !self.mounting_ui.active,
            Cmd::MountingMode => self.mounting_ui.active,
            Cmd::TabletMode => self.shell.tablet(),
            _ => return None,
        };
        Some(on)
    }

    /// The device tag of the selected symbol.
    pub(super) fn selected_tag(&self) -> Option<String> {
        let page = self.session.prj.pages.get(self.session.page)?;
        page.symbols.iter().find(|s| self.editor.selection.contains(&s.id) && !s.tag.is_empty()).map(|s| s.tag.clone())
    }

    /// Runs a command.
    #[allow(clippy::too_many_lines)]
    pub(super) fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        if !self.cmd_enabled(cmd) {
            return;
        }
        // From a docked pane (a context menu): once the panes are back in the dock.
        if self.in_dock {
            self.deferred.push(super::Deferred::Cmd(cmd));
            return;
        }
        let def = commands::def(cmd);
        if let Some(pane) = def.pane {
            self.backstage = None;
            if matches!(pane, Pane::LayoutSpaces | Pane::LayoutView | Pane::LayoutProperties) && self.workspace.is_workshop() {
                self.set_workspace(Workspace::Panel);
            }
            self.show_pane(pane);
            return;
        }
        if def.kind == Kind::Menu {
            // Menus open in the ribbon; from the command search, a key or a key tip
            // their entries open in a small window.
            self.search.menu = Some(cmd);
            return;
        }
        let replaces = matches!(cmd, Cmd::NewProject | Cmd::NewFromTemplateFile | Cmd::OpenDemo | Cmd::Open)
            || (cmd == Cmd::OpenExample && self.pending_example.is_some());
        if replaces {
            self.request_replace(ctx, Replace::Cmd(cmd));
        } else {
            self.exec(ctx, cmd);
        }
    }

    /// Runs a command without asking (`run` asked before one that replaces the project).
    #[allow(clippy::too_many_lines)]
    pub(super) fn exec(&mut self, ctx: &egui::Context, cmd: Cmd) {
        let page = self.session.page;
        match cmd {
            // ------------------------------------------------------------ file
            Cmd::Backstage => self.backstage = if self.backstage.is_some() { None } else { Some(crate::ribbon::Page::Home) },
            Cmd::StartPage => {}
            Cmd::NewProject => self.replace_project(Project::new_default(), None),
            Cmd::NewFromTemplateFile => self.io.open(ctx, Purpose::ProjectTemplate),
            Cmd::OpenDemo => self.replace_project(sample::demo_project(), None),
            Cmd::OpenExample => match self.pending_example.take().and_then(sample::example) {
                Some(prj) => self.replace_project(prj, None),
                // Without a chosen example: the start page lists them.
                None => {
                    self.backstage = None;
                    self.show_pane(Pane::Tool(ToolPane::Start));
                }
            },
            Cmd::Open => self.io.open(ctx, Purpose::Project),
            Cmd::Save => self.save(ctx, false),
            Cmd::SaveAs => self.save(ctx, true),
            Cmd::SaveAsProjectTemplate => {
                let t = openecraft_core::project_templates::as_template(&self.session.prj);
                let name = format!("{}_template.oec", self.session.prj.info.name.replace(' ', "_"));
                self.export(ctx, name, (tr("OpenECraft project template"), "oec"), t.to_json());
            }
            Cmd::CheckInVault => self.check_in_project(),
            Cmd::ImportDrawing => self.io.open(ctx, Purpose::Drawing),
            Cmd::ImportSettings => self.io.open(ctx, Purpose::SettingsScheme),
            Cmd::ExportSettings => {
                let json = openecraft_core::settings_scheme::export(&self.session.prj);
                self.export(ctx, "settings_scheme.json".into(), (tr("Settings scheme (JSON)"), "json"), json);
            }
            Cmd::ImportParts => self.io.open(ctx, Purpose::Parts),
            Cmd::ImportMacros => self.io.open(ctx, Purpose::Macros),
            Cmd::ImportSymbols => self.io.open(ctx, Purpose::Symbols),
            Cmd::ImportPageTemplates => self.io.open(ctx, Purpose::PageTemplates),
            Cmd::PrintAll | Cmd::PrintPage => {
                let sel: Vec<usize> = if cmd == Cmd::PrintAll { (0..self.session.prj.pages.len()).collect() } else { vec![page] };
                let html = openecraft_core::svg::print_document_pages(&self.session.prj, &self.session.analysis, &sel);
                if let Err(e) = openecraft_ui::io::print_html(&html) {
                    self.notify_error(ctx, tr_fmt!("Printing failed: {}", e));
                }
            }
            Cmd::ProjectProperties => self.show_project = true,
            Cmd::Settings => self.settings.open(),
            Cmd::Tutorial => self.help_chapter(1),
            Cmd::About => self.help_about(),
            Cmd::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            // ------------------------------------------------------------ home
            Cmd::Cut | Cmd::Copy => {
                let mut clip = ops::copy(&self.session.prj.pages[page], &self.editor.selection);
                clip.include_project_data(&self.session.prj);
                ctx.copy_text(clip.to_text());
                self.editor.clip = Some(clip);
                if cmd == Cmd::Cut {
                    ops::delete(&mut self.session.prj.pages[page], &self.editor.selection);
                    self.editor.selection.clear();
                }
            }
            Cmd::Paste => {
                if let Some(clip) = self.editor.clip.clone() {
                    let at = self.editor.paste_point();
                    self.editor.selection = ops::paste(&mut self.session.prj, page, &clip, at);
                    self.editor.set_tool(Tool::Select);
                    let n = self.editor.selection.len();
                    self.notify(ctx, super::counted(n, tr("1 object pasted"), tr_fmt!("{} objects pasted", n)));
                }
            }
            Cmd::MultipleCopy => self.arrange_ui.show_copy = true,
            Cmd::MoveBy => self.arrange_ui.show_move = true,
            Cmd::NewSchematicPage => self.add_page(PageKind::Schematic),
            Cmd::NewSubPage => {
                let at = ops::add_subpage(&mut self.session.prj, page);
                self.goto(at, 0);
            }
            Cmd::NewDocumentPage => {
                self.pages_ui.replace_document = None;
                self.io.open(ctx, Purpose::Document);
            }
            Cmd::NewSection => self.new_section(),
            Cmd::PageProperties => self.page_props = Some(page),
            Cmd::RenumberPages => self.numbering_ui.show_pages = true,
            Cmd::MovePageUp => self.session.page = ops::move_page(&mut self.session.prj, page, page - 1),
            Cmd::MovePageDown => self.session.page = ops::move_page(&mut self.session.prj, page, page + 1),
            // A page with contents is deleted after the confirmation (`delete_page_window`).
            Cmd::DeletePage if self.session.page_items(page) > 0 => self.confirm_delete_page = Some(self.session.prj.pages[page].id),
            Cmd::DeletePage => self.delete_page(ctx, page),
            Cmd::SavePageAsTemplate => self.open_save_template(),
            Cmd::Undo => {
                self.editor.cancel();
                self.session.undo();
            }
            Cmd::Redo => {
                self.editor.cancel();
                self.session.redo();
            }
            Cmd::Delete => {
                ops::delete(&mut self.session.prj.pages[page], &self.editor.selection);
                self.editor.selection.clear();
            }
            Cmd::Rotate => {
                if matches!(self.editor.tool, Tool::Place(_)) {
                    self.editor.place_rot = (self.editor.place_rot + 1) % 4;
                } else {
                    routing::rotate(&mut self.session.prj, page, &self.editor.selection);
                }
            }
            Cmd::Mirror => {
                if matches!(self.editor.tool, Tool::Place(_)) {
                    self.editor.place_mirror = !self.editor.place_mirror;
                } else {
                    ops::mirror(&mut self.session.prj.pages[page], &self.editor.selection);
                }
            }
            Cmd::Find => self.find_ui.open = true,
            Cmd::CommandSearch => self.search.open(),
            Cmd::SelectAll => {
                let pg = &self.session.prj.pages[page];
                self.editor.selection = ops::all_ids(pg).into_iter().filter(|id| graphics::item_layer(pg, *id).is_none_or(|l| !graphics::locked(&self.session.prj, l))).collect();
            }
            Cmd::SelectTool => self.set_tool(Tool::Select),
            // ------------------------------------------------------------ insert
            Cmd::PathFunctionText => {
                self.set_tool(Tool::Text);
                self.editor.text_proto = Some(TextItem { text: "Function text".into(), path_function: true, ..Default::default() });
            }
            Cmd::WireTool => self.set_tool(Tool::Wire),
            Cmd::TextTool => self.set_tool(Tool::Text),
            Cmd::ConnDefTool => self.set_tool(Tool::ConnDef),
            Cmd::CableDefTool => self.set_tool(Tool::CableDef),
            Cmd::JunctionTool => self.set_tool(Tool::Junction(JunctionKind::Tee)),
            Cmd::CornerTool => self.set_tool(Tool::Junction(JunctionKind::Corner)),
            Cmd::CrossingTool => self.set_tool(Tool::Junction(JunctionKind::Crossing)),
            Cmd::BridgeTool => self.set_tool(Tool::Junction(JunctionKind::Bridge)),
            Cmd::InterruptionPoint => self.set_tool(Tool::Place("intr")),
            Cmd::PotentialPoint => self.set_tool(Tool::Place("pot")),
            Cmd::InsertMacro => self.show_pane(Pane::Tool(ToolPane::Macros)),
            Cmd::SaveSelectionAsMacro => self.save_as_macro(false),
            Cmd::SavePageAsMacro => self.save_as_macro(true),
            Cmd::LineTool => self.set_tool(Tool::Line),
            Cmd::RectTool => self.set_tool(Tool::Rect),
            Cmd::CircleTool => self.set_tool(Tool::Circle),
            Cmd::EllipseTool => self.set_tool(Tool::Ellipse),
            Cmd::ArcTool => self.set_tool(Tool::Arc),
            Cmd::PolylineTool => self.set_tool(Tool::Polyline),
            Cmd::PolygonTool => self.set_tool(Tool::Polygon),
            Cmd::InsertImage => self.io.open(ctx, Purpose::Image),
            Cmd::DimAligned => self.set_tool(Tool::Dimension(DimKind::Aligned)),
            Cmd::DimHorizontal => self.set_tool(Tool::Dimension(DimKind::Horizontal)),
            Cmd::DimVertical => self.set_tool(Tool::Dimension(DimKind::Vertical)),
            Cmd::CommentTool => self.set_tool(Tool::Comment),
            // ------------------------------------------------------------ edit
            Cmd::AutoConnect => self.editor.autoconnect = !self.editor.autoconnect,
            Cmd::Route => {
                self.editor.autoroute = !self.editor.autoroute;
                self.editor.cancel();
            }
            Cmd::AutoConnectSelection => {
                let n = routing::apply_autoconnect(&mut self.session.prj, page, &self.editor.selection);
                self.notify(ctx, super::counted(n, tr("1 connection drawn automatically"), tr_fmt!("{} connections drawn automatically", n)));
            }
            Cmd::JoinCollinear => {
                let symbols = self.session.prj.symbol_table();
                let n = routing::merge_collinear(&symbols, &mut self.session.prj.pages[page]);
                self.editor.selection.clear();
                self.notify(ctx, tr_fmt!("{} wire pieces joined", n));
            }
            Cmd::ApplyConnDefs => {
                let n = conndef::apply_all(&mut self.session.prj);
                self.notify(ctx, tr_fmt!("{} wires changed", n));
            }
            Cmd::Properties => self.open_properties(ctx),
            Cmd::RenameDevice => {
                if let Some(tag) = self.selected_tag() {
                    self.rename_device = Some((tag.clone(), tag));
                }
            }
            Cmd::PartSelection => {
                if let Some(tag) = self.selected_tag() {
                    self.parts_ui.select_for(&tag);
                }
            }
            Cmd::ShowInLayout | Cmd::ShowIn3d => {
                if let Some(tag) = self.selected_tag() {
                    self.focus_device(&tag, cmd == Cmd::ShowIn3d);
                }
            }
            // ------------------------------------------------------------ view
            Cmd::FitPage => self.editor.fit_pending = true,
            Cmd::ZoomActual => self.editor.zoom_center(crate::editor::ZOOM_ACTUAL / self.editor.zoom),
            Cmd::ZoomIn => self.editor.zoom_center(1.25),
            Cmd::ZoomOut => self.editor.zoom_center(0.8),
            Cmd::PreviousPage => self.goto(page - 1, 0),
            Cmd::NextPage => self.goto(page + 1, 0),
            Cmd::NextPane | Cmd::PreviousPane => self.cycle_pane(cmd == Cmd::NextPane),
            Cmd::NextPageTab | Cmd::PreviousPageTab => self.cycle_page_tab(cmd == Cmd::NextPageTab),
            Cmd::ClosePageTab => {
                if let Some(id) = self.session.prj.pages.get(page).map(|p| p.id) {
                    self.close_page_tab(id);
                }
            }
            Cmd::Grid => self.editor.show_grid = !self.editor.show_grid,
            Cmd::ConnectionPoints => self.editor.show_pins = !self.editor.show_pins,
            Cmd::RevisionMarks => self.editor.show_revision_marks = !self.editor.show_revision_marks,
            Cmd::MacroBoxes => self.editor.show_macro_boxes = !self.editor.show_macro_boxes,
            Cmd::WireColours => self.session.prj.wire_colors = !self.session.prj.wire_colors,
            Cmd::WorkspaceSchematic => self.set_workspace(Workspace::Schematic),
            Cmd::WorkspacePanel => self.set_workspace(Workspace::Panel),
            Cmd::WorkspaceWiring => self.set_workspace(Workspace::Wiring),
            Cmd::WorkspaceMounting => self.set_workspace(Workspace::Mounting),
            Cmd::WorkspaceReview => self.set_workspace(Workspace::Review),
            Cmd::WorkspacePreplanning => self.set_workspace(Workspace::Preplanning),
            Cmd::SaveWorkspace => self.layout_name = Some(String::new()),
            Cmd::ResetLayout => self.reset_layout(),
            Cmd::MinimizeRibbon => self.shell.ribbon_minimized = !self.shell.ribbon_minimized,
            // ------------------------------------------------------------ engineering
            Cmd::DeviceNumbering => self.numbering_ui.show_devices = true,
            Cmd::WireNumbering => self.settings.open_at(settings_ui::Node::WireNumbering),
            Cmd::UpdateReportBlocks => self.generate_report_blocks(ctx),
            Cmd::CheckSettings => self.settings.open_at(settings_ui::Node::Checks),
            Cmd::CrossReferences => self.settings.open_at(settings_ui::Node::CrossReferences),
            Cmd::Marking => self.settings.open_at(settings_ui::Node::Marking),
            Cmd::Shortcuts => self.settings.open_at(settings_ui::Node::Shortcuts),
            Cmd::CustomizeRibbon => self.settings.open_at(settings_ui::Node::Ribbon),
            // ------------------------------------------------------------ layout space
            Cmd::NewEnclosure => {
                self.set_workspace(Workspace::Panel);
                self.panel_ui.new_enclosure_dialog(&self.session.prj, &self.parts_db);
            }
            Cmd::LayoutFloorPlan => self.panel_ui.floor_plan(),
            Cmd::LayoutDrawWalls => self.panel_ui.choose_tool(crate::panel_ui::Tool::Walls),
            Cmd::LayoutInnerWalls => self.panel_ui.choose_tool(crate::panel_ui::Tool::InnerWalls),
            Cmd::LayoutRoomDoor => self.panel_ui.choose_tool(crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Door)),
            Cmd::LayoutRoomWindow => self.panel_ui.choose_tool(crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Window)),
            Cmd::LayoutRoomPenetration => self.panel_ui.choose_tool(crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Penetration)),
            Cmd::LayoutRoomEntry => self.panel_ui.choose_tool(crate::panel_ui::Tool::Opening(openecraft_core::panel::Opening::Entry)),
            Cmd::NewRoom => {
                self.set_workspace(Workspace::Panel);
                self.panel_ui.new_room(&mut self.session.prj);
            }
            Cmd::LayoutSelect => self.panel_ui.choose_tool(crate::panel_ui::Tool::Select),
            Cmd::LayoutRail => self.panel_ui.choose_tool(crate::panel_ui::Tool::Rail),
            Cmd::LayoutDuct => self.panel_ui.choose_tool(crate::panel_ui::Tool::Duct),
            Cmd::LayoutBusbar => self.panel_ui.choose_tool(crate::panel_ui::Tool::Busbar),
            Cmd::LayoutCutout => self.panel_ui.choose_tool(crate::panel_ui::Tool::Cutout),
            Cmd::LayoutRoutingPath => self.panel_ui.choose_tool(crate::panel_ui::Tool::RoutingPath),
            Cmd::LayoutReservation => self.panel_ui.choose_tool(crate::panel_ui::Tool::Reservation),
            Cmd::LayoutPipeRun => self.panel_ui.choose_tool(crate::panel_ui::Tool::PipeRun),
            Cmd::LayoutFront => self.panel_ui.front_face(openecraft_core::panel::Face::Plate, false),
            Cmd::LayoutDoor => self.panel_ui.front_face(openecraft_core::panel::Face::Door, false),
            Cmd::LayoutDoorInside => self.panel_ui.front_face(openecraft_core::panel::Face::Door, true),
            Cmd::Layout3d => {
                use crate::panel_ui::View;
                self.panel_ui.view = if self.panel_ui.view == View::ThreeD { View::Front } else { View::ThreeD };
            }
            Cmd::LayoutWires => self.panel_ui.show_wires = !self.panel_ui.show_wires,
            Cmd::LayoutDuctFill => self.panel_ui.show_fill = !self.panel_ui.show_fill,
            Cmd::LayoutHeat => self.panel_ui.show_heat = !self.panel_ui.show_heat,
            Cmd::LayoutLineup => self.panel_ui.toggle_lineup(),
            Cmd::LayoutSite => self.panel_ui.toggle_site(),
            Cmd::LayoutHideFrontWalls => self.panel_ui.hide_front_walls = !self.panel_ui.hide_front_walls,
            Cmd::LayoutHideCeiling => self.panel_ui.hide_ceiling = !self.panel_ui.hide_ceiling,
            Cmd::LayoutOpenDucts => self.panel_ui.open_ducts = !self.panel_ui.open_ducts,
            Cmd::LayoutFit => self.panel_ui.fit(),
            Cmd::LayoutReports => {
                self.report_kind = ReportKind::PanelWiring.into();
                self.show_pane(Pane::Tool(ToolPane::Reports));
            }
            Cmd::LayoutMountingPage => {
                if let Some(e) = self.session.prj.layout.enclosures.get(self.panel_ui.enclosure) {
                    let id = e.id;
                    self.add_page(PageKind::Mounting(id));
                    self.set_workspace(Workspace::Schematic);
                }
            }
            // ------------------------------------------------------------ wiring
            Cmd::WiringMode => {
                self.mounting_ui.active = false;
                if self.workspace == Workspace::Mounting {
                    self.set_workspace(Workspace::Wiring);
                }
            }
            Cmd::MountingMode => {
                self.mounting_ui.active = true;
                if self.workspace == Workspace::Wiring {
                    self.set_workspace(Workspace::Mounting);
                }
            }
            Cmd::WiringExport => {
                if self.mounting_ui.active {
                    self.mounting_ui.export = true;
                } else {
                    self.wiring_ui.export = true;
                }
            }
            Cmd::TabletMode => {
                let on = !self.shell.tablet();
                self.shell.set_tablet(on);
            }
            _ => {}
        }
    }

    /// Deletes page `page` (undo brings it back).
    pub(super) fn delete_page(&mut self, ctx: &egui::Context, page: usize) {
        match ops::delete_page(&mut self.session.prj, page) {
            Ok(_) => {
                self.session.page = page.min(self.session.prj.pages.len() - 1);
                self.editor.selection.clear();
            }
            Err(e) => self.notify_error(ctx, tr(e)),
        }
    }

    /// Sets a tool of the page editor and brings the page to the front.
    pub(super) fn set_tool(&mut self, tool: Tool) {
        self.editor.set_tool(tool);
        if crate::dock::find(&self.dock, Pane::Page).is_some() || self.workspace.is_workshop() {
            self.show_pane(Pane::Page);
        } else {
            self.set_workspace(Workspace::Schematic);
        }
        self.backstage = None;
    }

    /// The menu of a menu command (ribbon, context menus).
    #[allow(clippy::too_many_lines)]
    pub(super) fn cmd_menu(&mut self, ui: &mut egui::Ui, cmd: Cmd) {
        let ctx = ui.ctx().clone();
        let page = self.session.page;
        if commands::changes_page(cmd) && !self.page_editable() {
            ui.weak(tr("The page is read only"));
            return;
        }
        match cmd {
            Cmd::NewPage => {
                for (label, kind) in [
                    (tr("New schematic page"), PageKind::Schematic),
                    (tr("New cover sheet"), PageKind::Cover),
                    (tr("New overview page"), PageKind::Overview),
                    (tr("New graphics page"), PageKind::Graphic),
                ] {
                    if ui.button(label).clicked() {
                        self.add_page(kind);
                        ui.close();
                    }
                }
                for c in [Cmd::NewSubPage, Cmd::NewDocumentPage] {
                    self.cmd_button(ui, c);
                }
                ui.menu_button(tr("New page from template"), |ui| {
                    for t in openecraft_core::page_templates::all(&self.session.prj) {
                        if ui.button(openecraft_core::page_templates::display_name(&t.name)).on_hover_text(t.kind.label()).clicked() {
                            self.insert_template(&t);
                            ui.close();
                        }
                    }
                });
                ui.menu_button(tr("New report page"), |ui| {
                    for (k, title) in self.report_list() {
                        if ui.button(title).clicked() {
                            self.add_page(PageKind::Report(k));
                            ui.close();
                        }
                    }
                });
                ui.menu_button(tr("New mounting layout page"), |ui| {
                    let encs: Vec<(u64, String)> = self.session.prj.layout.enclosures.iter().map(|e| (e.id, e.name.clone())).collect();
                    for (id, name) in encs {
                        if ui.button(name).clicked() {
                            self.add_page(PageKind::Mounting(id));
                            ui.close();
                        }
                    }
                    if self.session.prj.layout.enclosures.is_empty() {
                        ui.weak(tr("Create an enclosure in the panel layout first."));
                    }
                });
            }
            Cmd::Align => {
                for a in Align::ALL {
                    if ui.button(super::arrange_ui::align_title(a)).clicked() {
                        arrange::align(&mut self.session.prj, page, &self.editor.selection, a);
                        ui.close();
                    }
                }
            }
            Cmd::Distribute => {
                for (label, horizontal) in [(tr("Horizontally"), true), (tr("Vertically"), false)] {
                    if ui.button(label).clicked() {
                        arrange::distribute(&mut self.session.prj, page, &self.editor.selection, horizontal);
                        ui.close();
                    }
                }
            }
            Cmd::SpecialText => {
                ui.label(RichText::new(tr("Special text: project and page properties such as {page} or {prop:Name} are filled in when the page is drawn")).weak());
                for (ph, meaning) in openecraft_core::forms::PLACEHOLDERS.iter().filter(|(ph, _)| *ph != "{{ }}") {
                    if ui.button(format!("{}  {}", tr(meaning), ph)).clicked() {
                        self.set_tool(Tool::Text);
                        self.editor.text_proto = Some(TextItem { text: (*ph).to_string(), ..Default::default() });
                        ui.close();
                    }
                }
            }
            Cmd::Markup => {
                let tool = self.editor.tool.clone();
                for pen in [Pen::Red, Pen::Green] {
                    ui.label(RichText::new(pen.title()).color(super::review_ui::pen_color(pen)).strong());
                    for kind in MarkupKind::ALL {
                        let t = Tool::Markup(kind, pen);
                        if ui.selectable_label(tool == t, RichText::new(kind.title()).color(super::review_ui::pen_color(pen))).clicked() {
                            self.review_ui.pen = pen;
                            self.set_tool(t);
                            ui.close();
                        }
                    }
                }
            }
            Cmd::ActiveLayer => {
                ui.selectable_value(&mut self.editor.active_layer, String::new(), tr("(default)"));
                for n in graphics::layer_names(&self.session.prj).into_iter().filter(|n| !n.is_empty()) {
                    let usable = crate::editor::layer_usable(&self.session.prj, &n);
                    ui.add_enabled_ui(usable, |ui| ui.selectable_value(&mut self.editor.active_layer, n.clone(), n))
                        .inner
                        .on_disabled_hover_text(tr("A hidden or locked layer cannot take new items"));
                }
                ui.separator();
                self.cmd_button(ui, Cmd::LayersPane);
            }
            Cmd::GridSize => {
                for g in super::arrange_ui::GRIDS {
                    ui.selectable_value(&mut self.editor.grid, g, format!("{g:.0} mm"));
                }
            }
            Cmd::Panes => {
                // One submenu per area keeps the menu short.
                for (title, panes) in crate::dock::menu_groups() {
                    ui.menu_button(tr(title), |ui| {
                        for pane in panes {
                            let mut shown = self.pane_shown(pane);
                            let title = self.pane_title(pane).text().to_string();
                            // The page is never hidden: its entry only brings it back.
                            let can_hide = pane != Pane::Page;
                            let tip = commands::for_pane(pane).map(|c| tr(commands::def(c).tip)).unwrap_or("");
                            let r = ui.add_enabled(can_hide || !shown, egui::Checkbox::new(&mut shown, title));
                            let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
                            if r.changed() {
                                if shown {
                                    self.show_pane(pane);
                                } else {
                                    self.floating.retain(|p| *p != pane);
                                    crate::dock::close(&mut self.dock, pane);
                                }
                            }
                        }
                    });
                }
                ui.separator();
                self.cmd_button(ui, Cmd::ResetLayout);
            }
            Cmd::SavedWorkspaces => self.workspaces_menu(ui),
            Cmd::Theme => {
                let mut pref = ctx.options(|o| o.theme_preference);
                for (p, label) in [(egui::ThemePreference::System, tr("System")), (egui::ThemePreference::Light, tr("Light")), (egui::ThemePreference::Dark, tr("Dark"))] {
                    ui.selectable_value(&mut pref, p, label);
                }
                if pref != ctx.options(|o| o.theme_preference) {
                    ctx.set_theme(pref);
                }
            }
            Cmd::UiScale => {
                for s in settings_ui::SCALES {
                    ui.selectable_value(&mut self.shell.ui_scale, s, format!("{:.0} %", s * 100.0));
                }
            }
            Cmd::Language => {
                for l in openecraft_i18n::Lang::ALL {
                    if ui.selectable_label(openecraft_i18n::lang() == l, l.native_name()).clicked() {
                        openecraft_i18n::set_lang(l);
                        openecraft_ui::fonts::follow_language(&ctx);
                    }
                }
            }
            Cmd::PdfInLanguage => {
                for l in self.session.prj.languages.all() {
                    if ui.button(translation::language_name(&l)).clicked() {
                        self.export_pdf_in(&ctx, &l);
                        ui.close();
                    }
                }
            }
            Cmd::LayoutAccessory => self.panel_ui.accessory_menu(ui, &self.session.prj, &self.parts_db),
            Cmd::Layout3dMacro => self.panel_ui.macro_menu(ui, &mut self.session.prj),
            Cmd::LayoutWallPart => self.panel_ui.wall_part_menu(ui, &self.session.prj, &self.parts_db),
            Cmd::LayoutRackPart => self.panel_ui.rack_part_menu(ui, &mut self.session.prj, &self.parts_db),
            Cmd::LayoutFloorPart => self.panel_ui.floor_part_menu(ui, &self.session.prj, &self.parts_db),
            Cmd::LayoutWalls => self.panel_ui.walls_menu(ui, &self.session.prj),
            Cmd::LayoutNcData => {
                for (label, format) in [(tr("DXF drawing"), NcFormat::Dxf), ("DWG", NcFormat::Dwg), (tr("Drilling list (CSV)"), NcFormat::Csv)] {
                    if ui.button(label).clicked() {
                        self.panel_ui.export = Some((self.panel_ui.enclosure, self.panel_ui.face, format));
                        ui.close();
                    }
                }
            }
            _ => {}
        }
    }

    /// A menu entry that runs a command (with its shortcut and tooltip).
    pub(super) fn cmd_button(&mut self, ui: &mut egui::Ui, cmd: Cmd) {
        let def = commands::def(cmd);
        let keys = self.editor.shortcuts.cmd_text(cmd);
        let enabled = self.cmd_enabled(cmd);
        let title = tr(def.title);
        let r = match self.cmd_checked(cmd) {
            Some(on) => ui.add_enabled(enabled, egui::Button::selectable(on, title).shortcut_text(keys)),
            None => ui.add_enabled(enabled, egui::Button::new(title).shortcut_text(keys)),
        };
        let r = if def.tip.is_empty() { r } else { r.on_hover_text(tr(def.tip)) };
        if r.clicked() {
            let ctx = ui.ctx().clone();
            self.run(&ctx, cmd);
            ui.close();
        }
    }

    /// The shortcuts of the commands (outside text fields); keys of the page editor go
    /// to it while it is the active document.
    pub(super) fn command_keys(&mut self, ctx: &egui::Context) {
        // Esc closes the dialog on top first (not while a key is recorded, the key tips
        // are shown or a modal question waits).
        let esc = self.recording.is_none()
            && self.ribbon.tips.is_none()
            && !self.search.is_open()
            && ctx.memory(|m| m.top_modal_layer().is_none())
            && ctx.input(|i| i.key_pressed(Key::Escape) && i.modifiers.is_none());
        let order: Vec<egui::Id> = ctx.memory(|m| m.layer_ids().filter(|l| l.order == egui::Order::Middle).map(|l| l.id).collect());
        if self.dialogs.begin(esc, &order) {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        }
        if self.recording.is_some() {
            self.record_shortcut(ctx);
            return;
        }
        if self.ribbon.tips.is_some() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let page_active = self.active_doc == Pane::Page && self.backstage.is_none();
        self.editor.keys_active = page_active && !self.search.is_open();
        self.panel_ui.keys_active = self.active_doc == Pane::LayoutView;
        let sc = self.editor.shortcuts.clone();
        let mut hit = Vec::new();
        ctx.input_mut(|i| {
            for d in commands::ALL.iter().filter(|d| !d.canvas_keys) {
                let keys = sc.keys(d.id);
                if keys.is_empty() {
                    continue;
                }
                // Plain keys only act on the page (and never while typing).
                let plain = keys.iter().all(|k| !k.command && !k.alt && !matches!(k.key, Key::F1 | Key::F2 | Key::F4 | Key::F6 | Key::Insert | Key::PageUp | Key::PageDown));
                if (typing && !keys.iter().any(|k| k.command || k.alt)) || (plain && !page_active) {
                    continue;
                }
                if sc.consume(i, d.id) {
                    hit.push(d.cmd);
                }
            }
        });
        for cmd in hit {
            // F1 opens the help of the pane in use; the Help button the general help.
            if cmd == Cmd::Help {
                match super::help_pane::key_help(self.active_pane, self.active_doc) {
                    Some(chapter) => self.help_chapter(chapter),
                    None => self.show_pane(Pane::Tool(ToolPane::Help)),
                }
            } else {
                self.run(ctx, cmd);
            }
        }
        if self.backstage.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.backstage = None;
        }
    }

    /// F6: the next pane of the workspace becomes the active one.
    fn cycle_pane(&mut self, forward: bool) {
        let panes = crate::dock::visible(&self.dock);
        if panes.is_empty() {
            return;
        }
        let at = self.active_pane.and_then(|p| panes.iter().position(|q| *q == p)).unwrap_or(0);
        let next = if forward { (at + 1) % panes.len() } else { (at + panes.len() - 1) % panes.len() };
        let pane = panes[next];
        self.show_pane(pane);
        self.focus_pane = Some(pane);
    }

    /// Ctrl+Tab: the next page tab.
    fn cycle_page_tab(&mut self, forward: bool) {
        let Some(cur) = self.session.prj.pages.get(self.session.page).map(|p| p.id) else { return };
        let Some(at) = self.page_tabs.iter().position(|id| *id == cur) else { return };
        let n = self.page_tabs.len();
        let next = self.page_tabs[if forward { (at + 1) % n } else { (at + n - 1) % n }];
        if let Some(i) = self.session.prj.pages.iter().position(|p| p.id == next) {
            self.goto(i, 0);
        }
    }

    pub(super) fn close_page_tab(&mut self, id: u64) {
        if self.page_tabs.len() < 2 {
            return;
        }
        let at = self.page_tabs.iter().position(|t| *t == id).unwrap_or(0);
        self.page_tabs.retain(|t| *t != id);
        if self.session.prj.pages.get(self.session.page).map(|p| p.id) == Some(id) {
            let next = self.page_tabs[at.min(self.page_tabs.len() - 1)];
            if let Some(i) = self.session.prj.pages.iter().position(|p| p.id == next) {
                self.goto(i, 0);
            }
        }
    }
}
