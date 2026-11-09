//! Project-wide find and replace window (Edit › Find, Ctrl+F).

use openecraft_i18n::{tr, tr_fmt};
use super::App;
use crate::dock::{LeftTab, Pane, ToolPane};
use openecraft_core::search::{self, Hit, Kind, Query};
use egui::RichText;

#[derive(Default)]
pub struct FindUi {
    pub open: bool,
    query: Query,
    with: String,
    hits: Vec<Hit>,
    /// Revision of the history the hits were found in.
    rev: u64,
    searched: bool,
}

impl FindUi {
    pub(super) fn clear_results(&mut self) {
        self.hits.clear();
        self.rev = 0;
        self.searched = false;
    }
}

pub fn kind_title(k: Kind) -> &'static str {
    match k {
        Kind::Page => tr("Pages"),
        Kind::Device => tr("Devices"),
        Kind::FunctionText => tr("Function texts"),
        Kind::Text => tr("Texts"),
        Kind::WireNumber => tr("Wire numbers"),
        Kind::Terminal => tr("Terminals and pins"),
        Kind::Cable => tr("Cables"),
        Kind::Part => tr("Parts"),
        Kind::Description => tr("Descriptions"),
    }
}

/// The pane that shows a hit without a page: parts in the parts management, cables
/// in the cables, devices and descriptions in the device navigator.
pub(super) fn pane_of(kind: Kind) -> Option<Pane> {
    match kind {
        Kind::Part => Some(Pane::Tool(ToolPane::Parts)),
        Kind::Cable => Some(Pane::Tool(ToolPane::Cables)),
        Kind::Device | Kind::Description => Some(Pane::Navigator(LeftTab::Devices)),
        _ => None,
    }
}

impl App {
    pub(super) fn find_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut search_now = false;
        let mut replace = false;
        let mut jump = None;
        let mut show = None;
        let rev = self.session.history.rev;
        let st = &mut self.find_ui;
        ui.scope(|ui| {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut st.query.text).hint_text(tr("Text to find")).desired_width(240.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    search_now = true;
                }
                search_now |= ui.button(tr("Find")).clicked();
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut st.query.case_sensitive, tr("Match case"));
                ui.checkbox(&mut st.query.whole_word, tr("Whole words"));
            });
            ui.horizontal_wrapped(|ui| {
                for k in Kind::ALL {
                    let mut on = st.query.kinds.is_empty() || st.query.kinds.contains(&k);
                    if ui.checkbox(&mut on, kind_title(k)).changed() {
                        if st.query.kinds.is_empty() {
                            st.query.kinds = Kind::ALL.to_vec();
                        }
                        st.query.kinds.retain(|x| *x != k);
                        if on {
                            st.query.kinds.push(k);
                        }
                        if st.query.kinds.len() == Kind::ALL.len() {
                            st.query.kinds.clear();
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.with).hint_text(tr("Replace with")).desired_width(240.0));
                replace = ui
                    .add_enabled(!st.query.text.is_empty(), egui::Button::new(tr("Replace all")))
                    .on_hover_text(tr("Replaces in free texts, function texts, page, device and cable descriptions; released pages stay"))
                    .clicked();
            });
            ui.separator();
            if st.searched && st.rev != rev {
                ui.weak(tr("The project changed since the search."));
            }
            if st.searched {
                ui.label(if st.hits.len() == 1 { tr("1 found").to_string() } else { tr_fmt!("{} found", st.hits.len()) });
            }
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                egui::Grid::new("hits").striped(true).num_columns(3).show(ui, |ui| {
                    for h in &st.hits {
                        ui.weak(kind_title(h.kind));
                        let page = h.page.map(|p| openecraft_core::structure::page_full_name(&self.session.prj.pages[p])).unwrap_or_default();
                        ui.label(RichText::new(page).monospace());
                        match (h.page, pane_of(h.kind)) {
                            (Some(p), _) => {
                                if ui.link(&h.text).clicked() {
                                    jump = Some((p, h.item));
                                }
                            }
                            (None, Some(pane)) => {
                                if ui.link(&h.text).on_hover_text(tr("Not on a page: shows it in its pane")).clicked() {
                                    show = Some(pane);
                                }
                            }
                            (None, None) => {
                                ui.label(&h.text).on_hover_text(tr("Not on a page"));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        });
        if replace {
            let q = self.find_ui.query.clone();
            let n = search::replace(&mut self.session.prj, &q, &self.find_ui.with.clone());
            self.notify(ctx, if n == 1 { tr("1 text changed").to_string() } else { tr_fmt!("{} texts changed", n) });
            search_now = true;
        }
        if search_now {
            self.find_ui.hits = search::find(&self.session.prj, &self.find_ui.query);
            self.find_ui.rev = self.session.history.rev;
            self.find_ui.searched = true;
        }
        if let Some((p, id)) = jump {
            self.goto(p, id);
        }
        if let Some(pane) = show {
            self.show_pane(pane);
        }
    }
}

#[cfg(test)]
mod tests;
