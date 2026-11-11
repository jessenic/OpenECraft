//! Choosing how wire ends are terminated (bare, ferrule, ring terminal, …) in the
//! properties of a wire or a cable ([`openecraft_core::production::termination_of`]).

use super::PanelUi;
use egui::RichText;
use openecraft_core::analysis::Analysis;
use openecraft_core::model::Project;
use openecraft_core::production::{self, Termination};
use openecraft_i18n::{tr, tr_fmt};

/// The termination picked in a combo box (`None`: the default) showing `current`
/// (`None`: several different ones); twin ferrules only offered where `twin` (two wires
/// at the connection point).
fn choose(ui: &mut egui::Ui, id: impl egui::AsIdSalt, current: Option<Option<Termination>>, default: Termination, shown: String, twin: bool) -> Option<Option<Termination>> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(150.0)
        .show_ui(ui, |ui| {
            if ui.selectable_label(current == Some(None), tr_fmt!("Default ({})", default.title())).clicked() {
                picked = Some(None);
            }
            for k in Termination::ALL.into_iter().filter(|k| twin || *k != Termination::TwinFerrule || current == Some(Some(*k))) {
                if ui.selectable_label(current == Some(Some(k)), k.title()).clicked() {
                    picked = Some(Some(k));
                }
            }
        })
        .response
        .on_hover_text(tr("How the wire is terminated at this end; by default as chosen for the connection point or its device, otherwise as the layout sets ferrules"));
    picked
}

/// What a wire end gets: the stripping length, the part and the wire sharing its twin
/// ferrule.
fn gets(prj: &Project, ends: &production::Ends, at: &str, to: &str, cross_section: &str) -> String {
    let Some(e) = ends.end(prj, at, to, cross_section) else { return String::new() };
    let what = if e.part().is_empty() { e.describe() } else { format!("{} ({})", e.describe(), e.part()) };
    let what = match ends.twin(prj, at, to) {
        Some(t) => tr_fmt!("{}, together with the wire to {}", what, t.with),
        None => what,
    };
    tr_fmt!("strip {} mm, {}", format!("{:.0}", e.strip), what)
}

/// The ends of the cores of `cable`, each with its other end and cross-section, both
/// ways round. Cables are taken from the connections: the panel wiring only holds the
/// wires inside the enclosures.
pub(super) fn core_ends(prj: &Project, an: &Analysis, cable: &str) -> Vec<(String, String, String)> {
    an.connections
        .iter()
        .filter(|c| c.cable == cable)
        .flat_map(|c| {
            let (a, b) = (openecraft_core::netlist::pin_designation(prj, c.a), openecraft_core::netlist::pin_designation(prj, c.b));
            [(a.clone(), b.clone(), c.cross_section.clone()), (b, a, c.cross_section.clone())]
        })
        .collect()
}

/// The termination of all wires (or cores) at a connection point or device without a
/// choice of their own.
fn default_at(prj: &Project, at: &str) -> Termination {
    production::termination_of(prj, at, "").unwrap_or(if prj.layout.ferrules { Termination::Ferrule } else { Termination::Bare })
}

impl PanelUi {
    /// The two ends of a wire of `cross_section` from `from` to `to`, each with the
    /// choice of its termination and what it gets; a twin ferrule is chosen for both
    /// wires at the connection point ([`production::choose_termination`]).
    pub(super) fn wire_ends(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, from: &str, to: &str, cross_section: &str) {
        ui.label(RichText::new(tr("Wire ends")).strong());
        egui::Grid::new("pwireends").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            for (at, other) in [(from, to), (to, from)] {
                ui.label(RichText::new(at).monospace());
                let current = prj.layout.terminations.iter().find(|t| t.at == at && t.to == other).map(|t| t.kind);
                let default = an.ends.default_kind(prj, at, other);
                let shown = current.map_or_else(|| tr_fmt!("Default ({})", default.title()), |k| k.title().to_string());
                let twin = an.ends.wires_at(at).len() == 2;
                if let Some(kind) = choose(ui, ("wire_end", at, other), Some(current), default, shown, twin) {
                    production::choose_termination(prj, &an.ends, at, other, kind);
                }
                ui.end_row();
                ui.label("");
                ui.label(RichText::new(gets(prj, &an.ends, at, other, cross_section)).weak());
                ui.end_row();
            }
        });
    }

    /// The ends of a cable at its devices: one choice sets the termination of all its
    /// cores there.
    pub(super) fn cable_ends(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, cable: &str, devices: [&str; 2]) {
        // Each core's end at a device and its other end.
        let cores = core_ends(prj, an, cable);
        if cores.is_empty() {
            return;
        }
        ui.label(RichText::new(tr("Cable ends")).strong());
        egui::Grid::new("rcableends").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            for device in devices {
                let at: Vec<&(String, String, String)> = cores.iter().filter(|(end, _, _)| end.rsplit_once(':').is_some_and(|(d, _)| d == device)).collect();
                if at.is_empty() {
                    continue;
                }
                ui.label(RichText::new(device).monospace());
                let chosen: Vec<Option<Termination>> = at.iter().map(|(end, other, _)| prj.layout.terminations.iter().find(|t| &t.at == end && &t.to == other).map(|t| t.kind)).collect();
                let current = chosen.iter().all(|c| *c == chosen[0]).then_some(chosen[0]);
                let default = default_at(prj, device);
                let shown = match current {
                    Some(Some(k)) => k.title().to_string(),
                    Some(None) => tr_fmt!("Default ({})", default.title()),
                    None => tr("Mixed").to_string(),
                };
                if let Some(kind) = choose(ui, ("cable_end", cable, device), current, default, shown, false) {
                    for (end, other, _) in &at {
                        production::set_termination(prj, end, other, kind);
                    }
                }
                ui.end_row();
            }
        });
    }
}
