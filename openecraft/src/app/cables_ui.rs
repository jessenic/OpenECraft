//! Cables window (cable list with type, cores, part and usage) and the wire
//! numbering window.

use openecraft_ui::icons::{self, Icon};
use openecraft_i18n::{tr, tr_fmt};
use super::App;
use openecraft_core::model::*;
use openecraft_core::{ops, parts, structure, wirenum};
use egui::RichText;

/// Example format of the wire numbering (a format, not a text to translate).
const FORMAT_HINT: &str = "{page}.{column}.{n}";

fn cable_tag_input(ui:&mut egui::Ui,cable:&Cable)->Option<String> {
    let key=ui.make_persistent_id(("cable_tag",cable.id,cable.tag.as_str()));
    let mut text=ui.data(|data|data.get_temp::<String>(key)).unwrap_or_else(||cable.tag.clone());
    let response=ui.add_sized([100.,20.],egui::TextEdit::singleline(&mut text).id(key).font(egui::TextStyle::Monospace))
        .on_hover_text(tr("Cable tag, e.g. -W20"));
    if (response.has_focus() || response.lost_focus()) && ui.input(|input|input.key_pressed(egui::Key::Escape)) {
        ui.data_mut(|data|data.remove_temp::<String>(key));response.surrender_focus();return None
    }
    if response.lost_focus() || response.has_focus() && ui.input(|input|input.key_pressed(egui::Key::Enter)) {
        ui.data_mut(|data|data.remove_temp::<String>(key));response.surrender_focus();
        return (text.trim()!=cable.tag).then(||text.trim().to_string())
    }
    if response.has_focus() {ui.data_mut(|data|data.insert_temp(key,text));}
    None
}

impl App {
    pub(super) fn cables_pane(&mut self, ui: &mut egui::Ui) {
        let mut renamed: Vec<(u64, String)> = Vec::new();
        let mut remove = None;
        let mut picked: Vec<String> = Vec::new();
        let mut article_changes=Vec::new();
        let mut overview = false;
        let mut used: std::collections::HashMap<String, std::collections::BTreeSet<String>> = Default::default();
        for w in self.session.prj.pages.iter().flat_map(|pg| pg.wires.iter()).filter(|w| !w.cable.is_empty()) {
            used.entry(w.cable.clone()).or_default().insert(w.core.clone());
        }
        let mut bus_links: std::collections::HashMap<String, usize> = Default::default();
        for l in self.session.prj.plc.connections.iter().filter(|l| !l.cable.is_empty()) {
            *bus_links.entry(l.cable.clone()).or_default() += 1;
        }
        let structure = structure::page_structure(&self.session.prj.pages[self.session.page]);
        ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                if icons::text_button(ui, Icon::Plus, tr("New cable")).on_hover_text(tr("In the structure of the current page")).clicked() {
                    ops::add_cable(&mut self.session.prj, &structure);
                }
                if icons::text_button(ui, Icon::Report, tr("Cable overview")).clicked() {
                    overview = true;
                }
                ui.weak(tr("Assign wires: select them in the schematic › Assign to cable."));
            });
            ui.separator();
            let cable_parts = parts::group_choices(&self.session.prj.parts, Some(&self.parts_db), "Cables");
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("cables").striped(true).num_columns(10).spacing([8.0, 4.0]).show(ui, |ui| {
                    for h in [tr("Cable"), tr("Type"), tr("Bus"), tr("Cores"), tr("Cross-section"), tr("Length"), tr("Part"), tr("Description"), tr("Used"), ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, c) in self.session.prj.cables.iter_mut().enumerate() {
                        let old = c.tag.clone();
                        let old_part=c.part.clone();
                        if let Some(tag)=cable_tag_input(ui,c) {renamed.push((c.id,tag));}
                        ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut c.cable_type));
                        egui::ComboBox::from_id_salt(("cablebus", i)).width(90.0).selected_text(c.bus.map(|b| b.title()).unwrap_or("–")).show_ui(ui, |ui| {
                            ui.selectable_value(&mut c.bus, None, "–").on_hover_text(tr("Ordinary cable"));
                            for b in openecraft_core::plc_hw::BusKind::ALL {
                                ui.selectable_value(&mut c.bus, Some(b), b.title());
                            }
                        });
                        ui.add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut c.cores)).on_hover_text(tr("Core designations separated by blanks"));
                        ui.add_sized([70.0, 20.0], egui::TextEdit::singleline(&mut c.cross_section));
                        ui.add(egui::DragValue::new(&mut c.length).speed(0.5).range(0.0..=100000.0).suffix(" m"));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut c.part).desired_width(120.0));
                            ui.menu_button("…", |ui| {
                                for p in &cable_parts {
                                    if ui.button(format!("{}  {}", p.number, p.description)).clicked() {
                                        ops::apply_cable_part(c, p);
                                        picked.push(p.number.clone());
                                    }
                                }
                                if cable_parts.is_empty() {
                                    ui.weak(tr("No parts of the product group Cables in the parts database."));
                                }
                            });
                        });
                        ui.add_sized([160.0, 20.0], egui::TextEdit::singleline(&mut c.description));
                        let links = bus_links.get(&old).copied().unwrap_or(0);
                        if links > 0 {
                            ui.label(tr_fmt!("{} bus connections", links));
                        } else {
                            let n = used.get(&old).map(|s| s.len()).unwrap_or(0);
                            ui.label(format!("{n}/{}", c.core_list().len()));
                        }
                        if icons::button(ui, Icon::Delete, tr("Delete cable (its wires are removed from it)")).clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                        if c.part!=old_part {article_changes.push((old,old_part,c.part.clone()));}
                    }
                });
                if self.session.prj.cables.is_empty() {
                    ui.weak(tr("No cables defined."));
                }
            });
        });
        for (tag,old,new) in article_changes {
            ops::retarget_cable_part_references(&mut self.session.prj,&tag,&old,&new);
        }
        for (id, new) in renamed {
            if let Err(error)=ops::rename_project_cable(&mut self.session.prj,id,&new) {self.notify_error(ui.ctx(),error);}
        }
        if let Some(i) = remove {
            ops::remove_cable(&mut self.session.prj, i);
        }
        for n in picked {
            parts::ensure_in_project(&mut self.session.prj, &self.parts_db, &n);
        }
        if overview {
            self.report_kind = ReportKind::CableOverview.into();
            self.show_reports = true;
        }
    }

    pub(super) fn wire_numbering_body(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let (mut number, mut remove) = (false, false);
        let nets: Vec<&openecraft_core::netlist::Net> = self.session.analysis.open.nets.iter().filter(|n| !n.wires.is_empty() && !n.pins.is_empty()).collect();
        let numbered = nets.iter().filter(|n| !wirenum::number_of(&self.session.prj, n).is_empty()).count();
        let total = nets.len();
        ui.scope(|ui| {
            let s = &mut self.session.prj.wire_numbering;
            ui.label(tr("Every connection gets one number for all its wires; the two sides of a terminal are separate connections."));
            ui.add_space(4.0);
            for scheme in NumberScheme::ALL {
                ui.radio_value(&mut s.scheme, scheme, scheme.title());
            }
            egui::Grid::new("numbering").num_columns(2).show(ui, |ui| {
                ui.label(tr("Prefix"));
                ui.add(egui::TextEdit::singleline(&mut s.prefix).desired_width(60.0).hint_text(tr("e.g. W")));
                ui.end_row();
                ui.label(tr("Start at"));
                ui.add(egui::DragValue::new(&mut s.start).range(0..=1_000_000));
                ui.end_row();
                ui.label(tr("Step"));
                ui.add(egui::DragValue::new(&mut s.step).range(1..=1000));
                ui.end_row();
                ui.label(tr("Digits"));
                ui.add(egui::DragValue::new(&mut s.digits).range(0..=9)).on_hover_text(tr("Minimum digits of the counter (leading zeros)"));
                ui.end_row();
                if s.scheme == NumberScheme::Format {
                    ui.label(tr("Format"));
                    ui.add(egui::TextEdit::singleline(&mut s.format).desired_width(160.0).hint_text(FORMAT_HINT).font(egui::TextStyle::Monospace));
                    ui.end_row();
                }
            });
            if s.scheme == NumberScheme::Format {
                ui.label(RichText::new(tr("Placeholders: {n} counter (counted per value of the rest), {page}, {column}, {row}, {potential}, {kind}, {color}, {cross_section}. Without {n}, connections with the same text share the number.")).weak());
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("No numbers for"));
                for kind in openecraft_core::potentials::PotentialKind::ALL {
                    let mut on = s.skip_kinds.contains(&kind);
                    if ui.checkbox(&mut on, kind.title()).changed() {
                        s.skip_kinds.retain(|k| *k != kind);
                        if on {
                            s.skip_kinds.push(kind);
                        }
                    }
                }
            });
            ui.checkbox(&mut self.numbering_overwrite, tr("Include manually set numbers"));
            ui.label(tr_fmt!("{} of {} connections numbered", numbered, total));
            ui.horizontal(|ui| {
                number = ui.button(tr("Number wires")).clicked();
                remove = ui.button(tr("Remove numbers")).clicked();
            });
        });
        if number {
            let n = wirenum::renumber(&mut self.session.prj, &self.session.analysis.open, &self.session.analysis.netlist, self.numbering_overwrite);
            self.notify(ctx, tr_fmt!("{} connections numbered", n));
        }
        if remove {
            let n = wirenum::clear(&mut self.session.prj, self.numbering_overwrite);
            self.notify(ctx, tr_fmt!("Numbers removed from {} wires", n));
        }
    }
}
