//! Wiring assistant workspace for the workshop (tablet friendly): checklist of the
//! routed wires, the current wire with its end treatment, highlighted in 3D, and the
//! wiring progress.

use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::icons::{self, Icon};
use openecraft_core::model::Project;
use openecraft_core::panel::RoutedWire;
use crate::panel_ui::PanelUi;
use openecraft_core::potentials::color_parts;
use openecraft_core::analysis::Analysis;
use openecraft_core::wiring::{self, Mark, Order, Status, Step};
use egui::{vec2, Color32, RichText, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    /// Open, changed and problem wires.
    #[default]
    Todo,
    Wired,
    All,
}

impl Filter {
    const ALL: [Filter; 3] = [Filter::Todo, Filter::Wired, Filter::All];

    fn title(self) -> &'static str {
        match self {
            Filter::Todo => tr("To do"),
            Filter::Wired => tr("Wired"),
            Filter::All => tr("All"),
        }
    }

    fn shows(self, s: Status) -> bool {
        match self {
            Filter::Todo => s != Status::Done,
            Filter::Wired => matches!(s, Status::Done | Status::Changed),
            Filter::All => true,
        }
    }
}

#[derive(Default)]
pub struct WiringUi {
    pub order: Order,
    pub filter: Filter,
    /// Enclosure shown; None = all.
    pub enclosure: Option<usize>,
    /// Current wire (from, to).
    pub current: Option<(String, String)>,
    pub show_others: bool,
    note: String,
    /// Export of the checklist as CSV requested.
    pub export: bool,
}

/// Larger controls for touch screens.
fn touch_spacing(ui: &mut egui::Ui) {
    let s = ui.spacing_mut();
    s.interact_size.y = 40.0;
    s.button_padding = vec2(12.0, 8.0);
    s.item_spacing = vec2(8.0, 8.0);
}

fn icon(s: Status) -> (&'static str, Color32) {
    match s {
        Status::Open => ("☐", Color32::GRAY),
        Status::Done => ("✔", Color32::from_rgb(50, 170, 80)),
        Status::Changed => ("⟳", crate::style::warning()),
        Status::Problem => ("⚠", crate::style::error()),
    }
}

/// Colour swatch of a wire colour code (two halves for two-colour codes).
fn swatch(ui: &mut egui::Ui, code: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    let parts = color_parts(code);
    let painter = ui.painter();
    match parts.as_slice() {
        [c] => {
            painter.rect_filled(rect, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
        }
        [a, b] => {
            let (l, r) = rect.split_left_right_at_fraction(0.5);
            painter.rect_filled(l, 0.0, Color32::from_rgb(a[0], a[1], a[2]));
            painter.rect_filled(r, 0.0, Color32::from_rgb(b[0], b[1], b[2]));
        }
        _ => {}
    }
    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside);
}

impl WiringUi {
    fn steps(&self, prj: &Project, an: &Analysis) -> Vec<Step> {
        wiring::steps(prj, &an.ends, &an.wiring, self.enclosure, self.order)
    }

    fn current_index(&self, an: &Analysis) -> Option<usize> {
        let key = self.current.as_ref()?;
        an.wiring.iter().position(|w| &w.key() == key)
    }

    fn select(&mut self, prj: &Project, panel: &mut PanelUi, w: &RoutedWire) {
        self.current = Some(w.key());
        self.note = wiring::mark_of(prj, w).map(|m| m.note.clone()).unwrap_or_default();
        panel.focus_wire(prj, w);
    }

    /// Goes on with the next wire that needs work after `wire`.
    fn advance(&mut self, prj: &Project, an: &Analysis, panel: &mut PanelUi, wire: usize) {
        let steps = self.steps(prj, an);
        let at = steps.iter().position(|s| s.wire == wire);
        match wiring::next_open(&steps, at) {
            Some(i) => self.select(prj, panel, &an.wiring[steps[i].wire]),
            None => {
                self.current = None;
                panel.selected_wire = None;
            }
        }
    }

    /// Keeps the current wire valid and in sync with picking in the 3D view.
    pub fn sync(&mut self, prj: &Project, an: &Analysis, panel: &mut PanelUi) {
        if panel.selected_wire.is_some() && panel.selected_wire != self.current {
            self.current = panel.selected_wire.clone();
            if let Some(i) = self.current_index(an) {
                self.note = wiring::mark_of(prj, &an.wiring[i]).map(|m| m.note.clone()).unwrap_or_default();
            }
        }
        if self.current.is_some() && self.current_index(an).is_none() {
            self.current = None;
        }
        if self.current.is_none() {
            let steps = self.steps(prj, an);
            if let Some(i) = wiring::next_open(&steps, None) {
                self.select(prj, panel, &an.wiring[steps[i].wire]);
            }
        }
        self.enclosure = self.enclosure.filter(|e| *e < prj.layout.enclosures.len());
        panel.done_wires = an.wiring.iter().filter(|w| matches!(wiring::status(prj, w), Status::Done | Status::Changed)).map(|w| w.key()).collect();
    }

    /// Progress, filters and the checklist (left side).
    pub fn checklist(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, panel: &mut PanelUi) {
        touch_spacing(ui);
        let prog = wiring::progress(prj, &an.wiring, self.enclosure);
        ui.add_space(4.0);
        ui.heading(tr("Wiring"));
        ui.add(
            egui::ProgressBar::new(prog.fraction())
                .desired_height(28.0)
                .text(RichText::new(tr_fmt!("{} of {} wires", prog.done - prog.changed, prog.total)).strong()),
        );
        ui.label(tr_fmt!("{} m of {} m wired", format!("{:.1}", prog.length_done / 1000.0), format!("{:.1}", prog.length_total / 1000.0)));
        if prog.changed > 0 || prog.problems > 0 {
            ui.colored_label(crate::style::warning(), tr_fmt!("{} changed since wired · {} problems", prog.changed, prog.problems));
        }
        ui.separator();
        let all = tr("All enclosures");
        let name = |e: Option<usize>| e.and_then(|e| prj.layout.enclosures.get(e)).map(|e| e.name.clone()).unwrap_or_else(|| all.to_string());
        egui::ComboBox::from_id_salt("wiring_enc").width(ui.available_width()).selected_text(name(self.enclosure)).show_ui(ui, |ui| {
            ui.selectable_value(&mut self.enclosure, None, all);
            for (i, e) in prj.layout.enclosures.iter().enumerate() {
                ui.selectable_value(&mut self.enclosure, Some(i), &e.name);
            }
        });
        egui::ComboBox::from_id_salt("wiring_order").width(ui.available_width()).selected_text(self.order.title()).show_ui(ui, |ui| {
            for o in Order::ALL {
                ui.selectable_value(&mut self.order, o, o.title());
            }
        });
        ui.horizontal(|ui| {
            for f in Filter::ALL {
                ui.selectable_value(&mut self.filter, f, f.title());
            }
        });
        ui.separator();
        let steps = self.steps(prj, an);
        let current = self.current_index(an);
        let mut clicked: Option<usize> = None;
        let mut toggled: Option<usize> = None;
        egui::ScrollArea::vertical().id_salt("wiring_steps").auto_shrink([false, false]).show(ui, |ui| {
            let shown: Vec<&Step> = steps.iter().filter(|s| self.filter.shows(s.status)).collect();
            if shown.is_empty() {
                ui.weak(if an.wiring.is_empty() { tr("No routed wires: place the devices in the panel layout.") } else { tr("Nothing left in this list.") });
            }
            for s in shown {
                let w = &an.wiring[s.wire];
                let (sym, color) = icon(s.status);
                ui.horizontal(|ui| {
                    let tip = if s.status == Status::Open { tr("Mark as wired") } else { tr("Mark as open") };
                    if ui.add_sized([44.0, 44.0], egui::Button::new(RichText::new(sym).size(22.0).color(color))).on_hover_text(tip).clicked() {
                        toggled = Some(s.wire);
                    }
                    swatch(ui, &w.color, 14.0);
                    let mut text = egui::text::LayoutJob::default();
                    let strong = egui::TextFormat { font_id: egui::FontId::proportional(16.0), color: ui.visuals().strong_text_color(), ..Default::default() };
                    let weak = egui::TextFormat { font_id: egui::FontId::proportional(13.0), color: ui.visuals().weak_text_color(), ..Default::default() };
                    text.append(&format!("{} – {}\n", w.from, w.to), 0.0, strong);
                    let number = if w.number.is_empty() { String::new() } else { format!("{} · ", w.number) };
                    text.append(&format!("{number}{} · {} · {:.0} mm", w.color, w.cross_section, w.length), 0.0, weak);
                    if ui.selectable_label(Some(s.wire) == current, text).clicked() {
                        clicked = Some(s.wire);
                    }
                });
            }
        });
        if let Some(i) = toggled {
            let w = &an.wiring[i];
            let mark = if wiring::status(prj, w) == Status::Open { Some(Mark::Done) } else { None };
            wiring::set_mark(prj, w, mark, "");
        }
        if let Some(i) = clicked {
            self.select(prj, panel, &an.wiring[i]);
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Export CSV")).on_hover_text(tr("Checklist with the wiring state")).clicked() {
                self.export = true;
            }
            let stale = wiring::stale(prj, &an.wiring).len();
            if stale > 0 && ui.button(tr_fmt!("Remove {} old marks", stale)).on_hover_text(tr("Marks of wires that no longer exist")).clicked() {
                wiring::remove_stale(prj, &an.wiring);
            }
        });
    }

    /// The current wire in large print with its actions, above the 3D view.
    pub fn workbench(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, panel: &mut PanelUi) {
        touch_spacing(ui);
        let Some(wi) = self.current_index(an) else {
            ui.add_space(8.0);
            let done = !an.wiring.is_empty() && wiring::progress(prj, &an.wiring, self.enclosure).fraction() >= 1.0;
            ui.heading(if done { tr("✔ All wires are wired.") } else { tr("Select a wire in the list or in the 3D view.") });
            ui.separator();
            self.view(ui, prj, an, panel);
            return;
        };
        let w = an.wiring[wi].clone();
        let status = wiring::status(prj, &w);
        let wire_ends = &an.ends;
        let ends = [(&w.from, &w.to), (&w.to, &w.from)].map(|(at, other)| wire_ends.end(prj, at, other, &w.cross_section));
        let twins = [(&w.from, &w.to), (&w.to, &w.from)].map(|(at, other)| wire_ends.twin(prj, at, other));
        egui::Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                swatch(ui, &w.color, 28.0);
                ui.label(RichText::new(format!("{}  –  {}", w.from, w.to)).size(26.0).strong());
                let (sym, color) = icon(status);
                ui.label(RichText::new(format!("{sym} {}", status.title())).size(18.0).color(color));
            });
            ui.horizontal_wrapped(|ui| {
                let big = |t: String| RichText::new(t).size(18.0);
                if !w.number.is_empty() {
                    ui.label(big(tr_fmt!("Wire no. {}", w.number)));
                    ui.separator();
                }
                ui.label(big(format!("{} · {}", w.color, w.cross_section)));
                ui.separator();
                ui.label(big(tr_fmt!("Length {} mm", format!("{:.0}", w.length))));
                ui.separator();
                // What the ends get: once where both get the same.
                let what = |e: &openecraft_core::production::EndTreatment| if e.part().is_empty() { e.describe() } else { format!("{} ({})", e.describe(), e.part()) };
                match ends {
                    [Some(a), Some(b)] if a == b => {
                        ui.label(big(tr_fmt!("Strip {} mm", format!("{:.0}", a.strip))));
                        ui.separator();
                        ui.label(big(what(&a)));
                        ui.separator();
                    }
                    _ => {
                        for (at, e) in [(&w.from, ends[0]), (&w.to, ends[1])] {
                            if let Some(e) = e {
                                ui.label(big(tr_fmt!("{}: strip {} mm, {}", at, format!("{:.0}", e.strip), what(&e))));
                                ui.separator();
                            }
                        }
                    }
                }
                // Wires sharing a twin ferrule are crimped together.
                for (at, t) in [&w.from, &w.to].into_iter().zip(&twins) {
                    if let Some(t) = t {
                        ui.label(big(tr_fmt!("{}: crimp together with the wire to {}", at, t.with)).strong());
                        ui.separator();
                    }
                }
                ui.label(big(w.routing().to_string()));
                if w.net != w.number && !w.net.is_empty() {
                    ui.separator();
                    ui.label(big(tr_fmt!("Potential {}", w.net)));
                }
            });
            if status == Status::Changed {
                if let Some(m) = wiring::mark_of(prj, &w) {
                    ui.colored_label(
                        crate::style::warning(),
                        tr_fmt!("Changed since wired: was {} · {} · {}", m.color, m.cross_section, if m.number.is_empty() { "–" } else { &m.number }),
                    );
                }
            }
            ui.horizontal_wrapped(|ui| {
                let big = |t: &str| RichText::new(t).size(20.0);
                if ui.add(egui::Button::new(big(tr("✔ Wired"))).fill(crate::style::ok()).min_size(vec2(160.0, 52.0))).clicked() {
                    wiring::set_mark(prj, &w, Some(Mark::Done), &self.note);
                    self.advance(prj, an, panel, wi);
                }
                if ui.add(egui::Button::new(big(tr("⚠ Problem"))).min_size(vec2(130.0, 52.0))).on_hover_text(tr("Mark as not wirable; describe the problem in the note")).clicked() {
                    wiring::set_mark(prj, &w, Some(Mark::Problem), &self.note);
                    self.advance(prj, an, panel, wi);
                }
                if status != Status::Open && ui.add(egui::Button::new(big(tr("Reset"))).min_size(vec2(100.0, 52.0))).on_hover_text(tr("Mark as open")).clicked() {
                    wiring::set_mark(prj, &w, None, "");
                }
                let steps = self.steps(prj, an);
                let at = steps.iter().position(|s| s.wire == wi);
                let n = steps.len();
                if let Some(at) = at.filter(|_| n > 1) {
                    if ui.add(egui::Button::new(big("◀")).min_size(vec2(60.0, 52.0))).on_hover_text(tr("Previous wire")).clicked() {
                        self.select(prj, panel, &an.wiring[steps[(at + n - 1) % n].wire]);
                    }
                    if ui.add(egui::Button::new(big("▶")).min_size(vec2(60.0, 52.0))).on_hover_text(tr("Next wire")).clicked() {
                        self.select(prj, panel, &an.wiring[steps[(at + 1) % n].wire]);
                    }
                    ui.label(RichText::new(format!("{} / {n}", at + 1)).size(18.0));
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Note"));
                ui.add(egui::TextEdit::singleline(&mut self.note).desired_width(f32::INFINITY).hint_text(tr("e.g. wire too short, terminal missing")));
            });
        });
        self.view(ui, prj, an, panel);
    }

    fn view(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, panel: &mut PanelUi) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_others, tr("Show the other wires"));
            panel.door_button(ui);
            if icons::text_button(ui, Icon::Fit, tr("Fit")).clicked() {
                panel.reset_camera();
                if let Some(w) = self.current_index(an).map(|i| an.wiring[i].clone()) {
                    panel.focus_wire(prj, &w);
                }
            }
        });
        let show = panel.show_wires;
        panel.show_wires = self.show_others;
        panel.view_3d_current(ui, prj, an);
        panel.show_wires = show;
    }
}

#[cfg(test)]
mod tests;
