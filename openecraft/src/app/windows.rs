//! The dialogs that stay windows (project and page properties, part selection, export
//! parameters, workspaces), the progress of opening a project, the Reports pane and the
//! keyboard shortcuts.

use super::*;
use crate::shortcuts::Shortcut;
use openecraft_core::analysis::{Progress, Step};

/// Least width of the name and value fields of a project property.
const PROPERTY_NAME_WIDTH: f32 = 120.0;
const PROPERTY_VALUE_WIDTH: f32 = 160.0;
/// Least height of the list of project properties before it scrolls.
const PROPERTY_LIST_HEIGHT: f32 = 200.0;

/// Widths of the name and value fields of a project property in a row `width` wide that
/// also holds a button `button` wide, `gap` between them: the name gets two fifths, the
/// value the rest, neither less than its least width. Set explicitly because egui
/// keeps a grid column as narrow as it was in the first frame.
pub(super) fn property_widths(width: f32, button: f32, gap: f32) -> (f32, f32) {
    let rest = width - button - 2.0 * gap;
    let name = (rest * 0.4).max(PROPERTY_NAME_WIDTH);
    (name, (rest - name).max(PROPERTY_VALUE_WIDTH))
}

/// Renames the project property `old` to `new` unless `new` is empty or taken; true
/// when renamed.
pub(super) fn rename_property(props: &mut std::collections::BTreeMap<String, String>, old: &str, new: String) -> bool {
    if new.is_empty() || props.contains_key(&new) {
        return false;
    }
    match props.remove(old) {
        Some(v) => {
            props.insert(new, v);
            true
        }
        None => false,
    }
}

impl App {
    pub(super) fn closing_window(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let shutdown = self.shutdown.as_mut().expect("closing job");
        let complete = shutdown.poll();
        let step = shutdown.step();
        let mut cancel = false;
        egui::CentralPanel::default().show(ui, |_| {});
        egui::Modal::new(egui::Id::unique("closing_app")).show(&ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(tr("Closing OpenECraft"));
            ui.label(RichText::new(&self.session.prj.info.name).strong());
            if let Some(error) = &shutdown.error {
                ui.colored_label(crate::style::error(), tr("Could not prepare the recovery data."));
                ui.label(error);
                cancel = ui.button(tr("Cancel")).clicked();
            } else {
                ui.label(tr_fmt!("Step {} of {}: {}", step + 1, crate::shutdown::STEPS.len(), tr(crate::shutdown::STEPS[step])));
                ui.add(egui::ProgressBar::new(step as f32 / crate::shutdown::STEPS.len() as f32).animate(true));
                ui.spinner();
            }
        });
        if cancel {
            self.shutdown = None;
            self.updater.restart = false;
        } else if complete {
            self.closing = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    pub(super) fn windows(&mut self, ctx: &egui::Context) {
        self.discard_window(ctx);
        self.reconnect_window(ctx);
        self.delete_page_window(ctx);
        self.remove_layout_window(ctx);
        self.export_form_window(ctx);
        self.save_layout_window(ctx);
        self.project_properties_window(ctx);
        self.parts_management_and_part_selection_window(ctx);
        self.page_properties_window(ctx);
        self.section_properties_window(ctx);
    }

    /// What is done while a project opens (its analysis is built in the background):
    /// the project, the step and how far it is, instead of the panes.
    pub(super) fn opening_window(&mut self, ui: &mut egui::Ui, progress: Progress) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |_| {});
        let steps = Step::ALL;
        let i = steps.iter().position(|s| *s == progress.step).unwrap_or(0);
        let within = if progress.total > 0 { progress.done as f32 / progress.total as f32 } else { 0.0 };
        let step = if progress.total > 0 { tr_fmt!("{} ({} of {})", progress.step.title(), progress.done + 1, progress.total) } else { progress.step.title().to_string() };
        egui::Modal::new(egui::Id::unique("opening_project")).show(&ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(tr("Opening the project"));
            ui.label(RichText::new(&self.session.prj.info.name).strong());
            if let Some(file) = &self.session.file_name {
                ui.weak(openecraft_ui::io::short_name(file));
            }
            ui.add_space(8.0);
            ui.label(tr_fmt!("Step {} of {}: {}", i + 1, steps.len(), step));
            ui.add(egui::ProgressBar::new((i as f32 + within) / steps.len() as f32).animate(true));
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    /// Asks before a new or opened project replaces one with unsaved changes or the
    /// window closes. After Save the action runs once the project is saved.
    fn discard_window(&mut self, ctx: &egui::Context) {
        if self.discard.pending().is_none() {
            return;
        }
        let saving = self.discard.saving();
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("confirm_discard")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(tr_fmt!("{} has unsaved changes. Discard them?", self.session.prj.info.name));
            if saving {
                ui.weak(tr("Saving…"));
            } else if self.discard.not_saved {
                ui.colored_label(crate::style::error(), tr("The project was not saved."));
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(!saving, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr("Save")).clicked() {
                        answer = Some(Some(true));
                    }
                    if ui.button(tr("Discard changes")).clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button(tr("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        answer = Some(None);
                    }
                });
            });
        });
        match answer {
            Some(Some(true)) => {
                self.discard.save();
                self.run(ctx, Cmd::Save);
            }
            Some(Some(false)) => {
                if let Some(r) = self.discard.discard() {
                    self.perform(ctx, r);
                }
            }
            Some(None) => self.discard.cancel(),
            None => {}
        }
    }

    /// Asks before a page with contents is deleted.
    fn delete_page_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.confirm_delete_page else { return };
        let Some(page) = self.session.prj.pages.iter().position(|p| p.id == id) else {
            self.confirm_delete_page = None;
            return;
        };
        let name = self.session.prj.pages[page].name.clone();
        let items = self.session.page_items(page);
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("confirm_delete_page")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(if items == 1 { tr_fmt!("Delete page {}? It holds 1 item.", name) } else { tr_fmt!("Delete page {}? It holds {} items.", name, items) });
            ui.weak(tr("Undo brings the page back."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Delete page")).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(false);
                }
            });
        });
        if let Some(delete) = answer {
            self.confirm_delete_page = None;
            if delete && self.session.may_delete_page(page, self.page_held_by_other(page)) {
                self.delete_page(ctx, page);
            }
        }
    }

    /// The Reports pane: the reports of the registry on the left, the table on the right.
    #[allow(clippy::too_many_lines)]
    pub(super) fn reports_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut jump = None;
        let mut export = None;
        let mut export_xlsx = None;
        let mut add_page = None;
        let reports = self.report_list();
        egui::Panel::left("report_list").resizable(true).default_size(220.0).show(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.report_filter).hint_text(tr("Filter…")).desired_width(f32::INFINITY));
            let f = self.report_filter.to_lowercase();
            egui::ScrollArea::vertical().id_salt("report_list").auto_shrink([false, false]).show(ui, |ui| {
                for (k, title) in reports.iter().filter(|(_, t)| f.is_empty() || t.to_lowercase().contains(&f)) {
                    ui.selectable_value(&mut self.report_kind, *k, title);
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let kind = self.report_kind;
            ui.horizontal_wrapped(|ui| {
                ui.heading(self.report_title(kind));
                ui.separator();
                if icons::text_button(ui, Icon::Download, tr("Export CSV")).clicked() {
                    export = Some(kind);
                }
                if icons::text_button(ui, Icon::Download, tr("Export Excel")).clicked() {
                    export_xlsx = Some(kind);
                }
                if icons::text_button(ui, Icon::FileNew, tr("Insert as report page")).clicked() {
                    add_page = Some(kind);
                }
                if icons::text_button(ui, Icon::Forms, tr("Form…")).on_hover_text(tr("Columns, filter, sort order and rows of the report pages")).clicked() {
                    self.forms_ui.open_report(kind);
                }
                if icons::text_button(ui, Icon::Report, tr("Report blocks…")).on_hover_text(tr("Generate report pages per strip, plug, cable or device")).clicked() {
                    self.report_blocks_ui.open = true;
                }
            });
            ui.separator();
            let Some(table) = self.session.analysis.report(&self.session.prj, kind) else { return };
            egui::ScrollArea::both().id_salt("report_table").auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("report").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                    for h in table.header_texts() {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (row, target) in table.rows.iter().zip(&table.targets) {
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
                if table.rows.is_empty() {
                    ui.weak(tr("(no entries)"));
                }
            });
        });
        if let Some((pg, id)) = jump {
            self.goto(pg, id);
        }
        if let Some(kind) = export_xlsx {
            if let Some(t) = self.session.analysis.report(&self.session.prj, kind) {
                let title = self.report_title(kind);
                let bytes = openecraft_core::xlsx::write(&[(title.clone(), t.sheet_rows())]);
                let name = format!("{}.xlsx", title.replace(' ', "_").to_lowercase());
                self.export_bytes(ctx, name, ("Excel", "xlsx"), &bytes);
            }
        }
        if let Some(kind) = export {
            if let Some(t) = self.session.analysis.report(&self.session.prj, kind) {
                let csv = t.to_csv_bom();
                let name = format!("{}.csv", self.report_title(kind).replace(' ', "_").to_lowercase());
                self.export(ctx, name, ("CSV", "csv"), csv);
            }
        }
        if let Some(kind) = add_page {
            self.add_page(PageKind::Report(kind));
        }
    }

    /// Project properties.
    pub(super) fn project_properties_window(&mut self, ctx: &egui::Context) {
    if self.show_project {
        let (window, esc) = self.dialogs.window(ctx, tr("Project properties"), "project_properties", None);
        let mut open = !esc;
        let prj_tb_names = openecraft_core::forms::names(&self.session.prj);
        window.open(&mut open).default_width(520.0).min_width(360.0).show(ctx, |ui| {
            let info = &mut self.session.prj.info;
            let line = |ui: &mut egui::Ui, text: &mut String| ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
            egui::Grid::new("prj").num_columns(2).show(ui, |ui| {
                ui.label(tr("Project name"));
                line(ui, &mut info.name);
                ui.end_row();
                ui.label(tr("Description"));
                ui.add(egui::TextEdit::multiline(&mut info.description).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label(tr("Company"));
                line(ui, &mut info.company);
                ui.end_row();
                ui.label(tr("Created by"));
                line(ui, &mut info.creator);
                ui.end_row();
                ui.label(tr("Date"));
                line(ui, &mut info.date);
                ui.end_row();
                ui.label(tr("Revision"));
                line(ui, &mut info.revision);
                ui.end_row();
                ui.label(tr("Title block"));
                let forms = &mut self.session.prj.forms;
                let current = if forms.default_title_block.is_empty() { openecraft_core::forms::STANDARD.to_string() } else { forms.default_title_block.clone() };
                egui::ComboBox::from_id_salt("prjtb").selected_text(&current).show_ui(ui, |ui| {
                    for n in prj_tb_names.iter().cloned() {
                        if ui.selectable_label(n == current, &n).clicked() {
                            forms.default_title_block = if n == openecraft_core::forms::STANDARD { String::new() } else { n };
                        }
                    }
                });
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new(tr("Project properties")).strong()).on_hover_text(tr("Shown in title blocks with {prop:Name}"));
            let props = &mut self.session.prj.info.properties;
            let mut remove = None;
            let mut rename = None;
            // The list fills the height the user gives the dialog (at least a few rows)
            // and scrolls; the slack below keeps the window from growing with it.
            let height = (ui.available_height() - 2.0 * ui.spacing().interact_size.y).max(PROPERTY_LIST_HEIGHT);
            egui::ScrollArea::vertical().id_salt("prjprops").max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                let gap = ui.spacing().item_spacing.x;
                // A floating scroll bar is drawn over the rows: keep the remove buttons clear of it.
                let scroll = &ui.spacing().scroll;
                let bar = if scroll.floating { scroll.bar_width + scroll.bar_inner_margin } else { 0.0 };
                let (name_w, value_w) = property_widths(ui.available_width() - bar, ui.spacing().interact_size.y, gap);
                for (k, v) in props.iter_mut() {
                    ui.horizontal(|ui| {
                        // The name is applied when the field loses focus: renaming
                        // re-sorts the rows, so a rename per key stroke would move the
                        // text cursor to another property.
                        let id = ui.make_persistent_id(("prjprop", k.as_str()));
                        let mut name = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| k.clone());
                        let r = ui.add(egui::TextEdit::singleline(&mut name).id(id.with("edit")).desired_width(name_w));
                        if r.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            rename = Some((k.clone(), name.clone()));
                        }
                        if r.has_focus() {
                            ui.data_mut(|d| d.insert_temp(id, name));
                        } else {
                            ui.data_mut(|d| d.remove::<String>(id));
                        }
                        ui.add(egui::TextEdit::singleline(v).desired_width(value_w));
                        if icons::button(ui, Icon::Delete, tr("Remove")).clicked() {
                            remove = Some(k.clone());
                        }
                    });
                }
            });
            if let Some((old, new)) = rename {
                rename_property(props, &old, new);
            }
            if let Some(k) = remove {
                props.remove(&k);
            }
            if icons::text_button(ui, Icon::Plus, tr("Property")).clicked() {
                let mut n = props.len() + 1;
                while props.contains_key(&format!("Property{n}")) {
                    n += 1;
                }
                props.insert(format!("Property{n}"), String::new());
            }
        });
        self.show_project = open;
    }

    }

    /// The part selection and the small dialogs of the tools (they stay windows).
    pub(super) fn parts_management_and_part_selection_window(&mut self, ctx: &egui::Context) {
        self.parts_ui.selector(ctx, &self.parts_db, &mut self.session.prj);
        self.rename_device_window(ctx);
        self.numbering_windows(ctx);
        self.arrange_windows(ctx);
        self.save_macro_dialog(ctx);
        self.save_template_window(ctx);
        self.plugins_windows(ctx);
        self.properties_float_window(ctx);
        self.settings_window(ctx);
        self.command_search_window(ctx);
        self.command_menu_window(ctx);
        self.floating_panes(ctx);
    }

    /// The parts management (database and project parts) as a pane.
    pub(super) fn parts_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let actions = self.parts_ui.manager(ui, &mut self.parts_db, &mut self.session.prj);
        self.parts_actions(&ctx, actions);
    }

    /// The data portal (parts server) as a pane.
    pub(super) fn portal_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let actions = self.parts_ui.portal(ui, &mut self.parts_db);
        self.parts_actions(&ctx, actions);
    }

    /// Page properties.
    pub(super) fn page_properties_window(&mut self, ctx: &egui::Context) {
    if let Some(pi) = self.page_props {
        let (window, esc) = self.dialogs.window(ctx, tr("Page properties"), "page_properties", None);
        let mut open = !esc;
        let mut apply_structure = None;
        let mut document_action = None;
        let mut move_to = None;
        if pi < self.session.prj.pages.len() {
            let tb_names = openecraft_core::forms::names(&self.session.prj);
            let frame_names = openecraft_core::frame::names(&self.session.prj);
            let page_aspects = self.session.prj.structures.page_aspects();
            let structure_entries = self.session.prj.structures.entries.clone();
            let reports = self.report_list();
            let sections = (!self.session.prj.sections.is_empty()).then(|| (self.section_choices(), openecraft_core::sections::of_page(&self.session.prj, pi)));
            window.open(&mut open).resizable(false).show(ctx, |ui| {
                let pg = &mut self.session.prj.pages[pi];
                egui::Grid::new("pgprops").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Page name / number"));
                    ui.text_edit_singleline(&mut pg.name);
                    ui.end_row();
                    move_to = sections.as_ref().and_then(|(choices, current)| sections_ui::section_row(ui, choices, *current, !pg.locked));
                    ui.label(tr("Description"));
                    ui.text_edit_singleline(&mut pg.description);
                    ui.end_row();
                    ui.label(tr("Page type"));
                    egui::ComboBox::from_id_salt("pgkind").selected_text(pg.kind.label()).show_ui(ui, |ui| {
                        ui.selectable_value(&mut pg.kind, PageKind::Schematic, PageKind::Schematic.label());
                        ui.selectable_value(&mut pg.kind, PageKind::Overview, PageKind::Overview.label());
                        ui.selectable_value(&mut pg.kind, PageKind::Graphic, PageKind::Graphic.label());
                        ui.selectable_value(&mut pg.kind, PageKind::Document, PageKind::Document.label());
                        ui.selectable_value(&mut pg.kind, PageKind::Cover, PageKind::Cover.label());
                        for (k, title) in &reports {
                            ui.selectable_value(&mut pg.kind, PageKind::Report(*k), tr_fmt!("Report: {}", title));
                        }
                        for e in &self.session.prj.layout.enclosures {
                            ui.selectable_value(&mut pg.kind, PageKind::Mounting(e.id), tr_fmt!("Mounting layout {}", e.name));
                        }
                    });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Title block"));
                    let shown = if pg.title_block.is_empty() { "(project default)".to_string() } else { pg.title_block.clone() };
                    egui::ComboBox::from_id_salt("pgtb").selected_text(shown).show_ui(ui, |ui| {
                        ui.selectable_value(&mut pg.title_block, String::new(), tr("(project default)"));
                        for n in &tb_names {
                            ui.selectable_value(&mut pg.title_block, n.clone(), n);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Plot frame"));
                    let shown = if pg.frame.is_empty() { tr("(project default)").to_string() } else { openecraft_core::frame::display_name(&pg.frame) };
                    egui::ComboBox::from_id_salt("pgframe").selected_text(shown).show_ui(ui, |ui| {
                        ui.selectable_value(&mut pg.frame, String::new(), tr("(project default)"));
                        for n in &frame_names {
                            ui.selectable_value(&mut pg.frame, n.clone(), openecraft_core::frame::display_name(n));
                        }
                    });
                });
                if pg.kind == PageKind::Document {
                    ui.horizontal(|ui| {
                        match &pg.document {
                            Some(doc) => {
                                ui.label(RichText::new(&doc.name).strong());
                                ui.weak(format!("{}, {}", doc.kind().title(), openecraft_core::document::size_text(doc.size())));
                                if ui.button(tr("Save file…")).clicked() {
                                    document_action = Some(false);
                                }
                            }
                            None => {
                                ui.weak(tr("No document embedded"));
                            }
                        }
                        if ui.button(tr("Choose file…")).on_hover_text(tr("Embeds the file in the project (replaces the current one)")).clicked() {
                            document_action = Some(true);
                        }
                    });
                }
                if matches!(pg.kind, PageKind::Overview | PageKind::Graphic) {
                    ui.weak(tr("Symbols and wires on this page are drawn but not evaluated (no devices, connections or reports)."));
                } else if !pg.kind.is_drawing() && !pg.is_empty() {
                    ui.colored_label(crate::style::lock(), tr("This page contains schematic content which is not evaluated on non-schematic pages."));
                }
                if pg.generated {
                    ui.checkbox(&mut pg.generated, tr("Generated by the configurator"))
                        .on_hover_text(tr("Replaced when the configurator generates again; switch off to keep the page"));
                }
                ui.separator();
                apply_structure = page_structure_ui(ui, pg, pi, &page_aspects, &structure_entries);
            });
        } else {
            open = false;
        }
        match document_action {
            Some(true) => {
                self.pages_ui.replace_document = Some(pi);
                self.io.open(ctx, Purpose::Document);
            }
            Some(false) => self.save_document(ctx, pi),
            None => {}
        }
        if let Some((dt, move_devices)) = apply_structure {
            ops::set_page_structure(&mut self.session.prj, pi, &dt, move_devices);
        }
        if let Some(id) = move_to {
            self.section_action(ctx, sections_ui::SectionAction::MovePage(pi, id));
        }
        if !open {
            self.page_props = None;
        }
    }

    }

    /// Takes the next key press (with modifiers) as the shortcut of the binding being
    /// recorded; Esc cancels. Key presses do not reach the editor meanwhile.
    pub(super) fn record_shortcut(&mut self, ctx: &egui::Context) {
        let Some(id) = self.recording else { return };
        let pressed = ctx.input_mut(|i| {
            let key = i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            });
            i.events.retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)));
            key
        });
        match pressed {
            Some((Key::Escape, m)) if m.is_none() => self.recording = None,
            Some((key, m)) => {
                let s = Shortcut { command: m.command, shift: m.shift, alt: m.alt, key };
                self.editor.shortcuts.set(id, vec![s]);
                self.recording = None;
            }
            None => {}
        }
    }
}

/// The structure section of the page properties: the identifiers per aspect, checked
/// before they can be applied. Returns the structure to apply.
fn page_structure_ui(ui: &mut egui::Ui, pg: &Page, pi: usize, page_aspects: &[Aspect], structure_entries: &[openecraft_core::model::structure::Entry]) -> Option<(Dt, bool)> {
    let mut apply = None;
    ui.label(RichText::new(tr("Structure (IEC 81346)")).strong());
    let key = ui.scope_id().with(("page_structure", pi));
    let (mut dt, mut move_devices): (Dt, bool) = ui.data_mut(|d| d.get_temp(key)).unwrap_or_else(|| (Dt::of_page(pg), true));
    let current = Dt::of_page(pg);
    egui::Grid::new("pgstruct").num_columns(3).show(ui, |ui| {
        for a in Aspect::ALL.into_iter().filter(|a| page_aspects.contains(a) || !current.get(*a).is_empty()) {
            ui.label(format!("{}  {}", a.title(), a.prefix()));
            let hint = match a {
                Aspect::Function => tr("e.g. A1, PUMP.2"),
                Aspect::Installation => tr("e.g. HALL1, PLANT2"),
                Aspect::Location => tr("e.g. CAB1, FIELD"),
                Aspect::Product => tr("e.g. U1, U1-A2"),
            };
            ui.add(egui::TextEdit::singleline(dt.get_mut(a)).hint_text(hint).desired_width(140.0));
            let defined: Vec<_> = structure_entries.iter().filter(|e| e.aspect == a).collect();
            ui.add_enabled_ui(!defined.is_empty(), |ui| {
                let r = icons::button(ui, Icon::ChevronDown, tr("Identifiers of the structure management"));
                egui::Popup::menu(&r).show(|ui| {
                    for e in defined {
                        if ui.button(format!("{}  {}", a.format(&e.id), e.description)).clicked() {
                            *dt.get_mut(a) = e.id.clone();
                            ui.close();
                        }
                    }
                });
            });
            ui.end_row();
        }
    });
    ui.checkbox(&mut move_devices, tr("Move the devices of this page into the new structure"))
        .on_hover_text(tr("Renames the devices (all their functions, also on other pages)"));
    let mut cleaned = Page::default();
    dt.apply_to(&mut cleaned);
    let changed = Dt::of_page(&cleaned) != current;
    let problem = Aspect::ALL.into_iter().find_map(|a| openecraft_core::structure::check_identifier(a, dt.get(a)).err());
    if let Some(p) = &problem {
        ui.colored_label(crate::style::error(), p);
    }
    if ui.add_enabled(changed && problem.is_none(), egui::Button::new(tr("Apply structure"))).clicked() {
        apply = Some((dt.clone(), move_devices));
    }
    ui.weak(tr("Devices placed on the page get its structure; their tags are shown without it."));
    ui.data_mut(|d| d.insert_temp(key, (dt, move_devices)));
    apply
}

#[cfg(test)]
mod tests;
