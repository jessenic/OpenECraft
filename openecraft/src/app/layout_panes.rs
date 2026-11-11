//! The panes of the panel layout (layout spaces, view, properties) and of the wiring and
//! mounting assistants (checklist, workbench). They share `PanelUi`; what the panes ask
//! for (go to the schematic, NC export, mounting page) is handled once per frame.

use super::*;

impl App {
    /// Before the panes are drawn: the assistants mark what is done in the 3D view.
    pub(super) fn layout_frame(&mut self) {
        let workshop = self.pane_shown(Pane::WiringChecklist) || self.pane_shown(Pane::WiringWorkbench);
        if !workshop {
            self.panel_ui.done_wires.clear();
            self.panel_ui.done_items.clear();
        } else if self.mounting_ui.active {
            self.panel_ui.done_wires.clear();
            self.mounting_ui.sync(&self.session.prj, &mut self.panel_ui);
        } else {
            self.panel_ui.selection.clear();
            self.panel_ui.done_items.clear();
            self.wiring_ui.sync(&self.session.prj, &self.session.analysis, &mut self.panel_ui);
        }
    }

    pub(super) fn layout_spaces_pane(&mut self, ui: &mut egui::Ui) {
        self.panel_ui.navigator(ui, &mut self.session.prj, &self.parts_db);
    }

    pub(super) fn layout_properties_pane(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("panel_props").auto_shrink([false, false]).show(ui, |ui| {
            self.panel_ui.properties(ui, &mut self.session.prj, &self.session.analysis, &self.parts_db);
        });
    }

    pub(super) fn layout_view_pane(&mut self, ui: &mut egui::Ui) {
        self.panel_ui.canvas(ui, &mut self.session.prj, &self.session.analysis, &self.parts_db);
    }

    pub(super) fn wiring_checklist_pane(&mut self, ui: &mut egui::Ui) {
        self.mounting_ui.mode_switch(ui);
        if self.mounting_ui.active {
            self.mounting_ui.checklist(ui, &mut self.session.prj, &mut self.panel_ui);
        } else {
            self.wiring_ui.checklist(ui, &mut self.session.prj, &self.session.analysis, &mut self.panel_ui);
        }
    }

    pub(super) fn wiring_workbench_pane(&mut self, ui: &mut egui::Ui) {
        if self.mounting_ui.active {
            self.mounting_ui.workbench(ui, &mut self.session.prj, &self.session.analysis, &mut self.panel_ui);
        } else {
            self.wiring_ui.workbench(ui, &mut self.session.prj, &self.session.analysis, &mut self.panel_ui);
        }
    }

    /// After the panes: the requests of the panel layout and the assistants.
    pub(super) fn layout_requests(&mut self, ctx: &egui::Context, idx: &Index) {
        self.panel_ui.dialogs(ctx, &mut self.session.prj, &self.parts_db);
        if std::mem::take(&mut self.mounting_ui.export) {
            let content = openecraft_core::mounting::csv(&self.session.prj, self.mounting_ui.enclosure);
            self.export(ctx, "mounting.csv".into(), ("CSV", "csv"), content);
        }
        if std::mem::take(&mut self.wiring_ui.export) {
            let (prj, an) = (&self.session.prj, &self.session.analysis);
            let content = openecraft_core::wiring::csv(prj, &an.ends, &an.wiring, self.wiring_ui.order);
            self.export(ctx, "wiring.csv".into(), ("CSV", "csv"), content);
        }
        if let Some(tag) = self.panel_ui.goto_schematic.take() {
            let target = idx.devices.get(&tag).and_then(|d| d.mains.first().or(d.others.first()).or(d.auxs.first()).copied());
            if let Some(loc) = target {
                self.set_workspace(Workspace::Schematic);
                self.goto(loc.page, loc.sym_id);
            }
        }
        if let Some((ei, face, format)) = self.panel_ui.export.take() {
            if let Some(e) = self.session.prj.layout.enclosures.get(ei) {
                let base: String = e.name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
                let what = face.key();
                match format {
                    NcFormat::Dxf => {
                        let content = openecraft_core::production::nc_dxf(&self.session.prj, ei, face);
                        self.export(ctx, format!("{what}{base}.dxf"), (tr("DXF drawing"), "dxf"), content);
                    }
                    NcFormat::Dwg => {
                        let content = openecraft_core::dwg::nc_dwg(&self.session.prj, ei, face);
                        self.export_dwg(ctx, format!("{what}{base}.dwg"), content);
                    }
                    NcFormat::Csv => {
                        let content = openecraft_core::production::nc_csv(&self.session.prj, ei, face);
                        self.export(ctx, format!("{what}{base}_drilling.csv"), ("CSV", "csv"), content);
                    }
                }
            }
        }
        if std::mem::take(&mut self.panel_ui.open_parts) {
            self.run(ctx, crate::commands::Cmd::PartsManagement);
        }
        if let Some(id) = self.panel_ui.add_page.take() {
            self.add_page(PageKind::Mounting(id));
            self.set_workspace(Workspace::Schematic);
        }
    }
}
