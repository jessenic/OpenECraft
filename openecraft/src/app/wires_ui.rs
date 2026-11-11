//! Wire crossing settings: how wires jump over the wires they cross, how T-connections
//! are marked, and whether moved items route their connections anew.

use openecraft_i18n::tr;
use super::App;
use openecraft_core::geom::{p, P};
use openecraft_core::model::{JumpStyle, JumpWire, TeeStyle, Wire, WireSettings};
use openecraft_core::render::jumps;
use egui::RichText;

pub fn jump_title(s: JumpStyle) -> &'static str {
    match s {
        JumpStyle::None => tr("No jump: plain crossing, connections have dots"),
        JumpStyle::Arc => tr("Arc"),
        JumpStyle::Bridge => tr("Bridge"),
        JumpStyle::Gap => tr("Gap"),
    }
}

pub fn jump_wire_title(w: JumpWire) -> &'static str {
    match w {
        JumpWire::Vertical => tr("The vertical wire"),
        JumpWire::Horizontal => tr("The horizontal wire"),
    }
}

pub fn tee_title(s: TeeStyle) -> &'static str {
    match s {
        TeeStyle::Dot => tr("Dot"),
        TeeStyle::LargeDot => tr("Large dot"),
        TeeStyle::None => tr("No dot: plain T"),
    }
}

impl App {
    pub(super) fn wires_body(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.session.prj.wire_settings;
        ui.label(RichText::new(tr("Wires crossing without connection")).strong());
        for v in JumpStyle::ALL {
            ui.radio_value(&mut s.jump, *v, jump_title(*v));
        }
        ui.add_enabled_ui(s.jump != JumpStyle::None, |ui| {
            ui.label(tr("Jumps over the other:"));
            ui.horizontal(|ui| {
                for v in JumpWire::ALL {
                    ui.radio_value(&mut s.jump_wire, *v, jump_wire_title(*v));
                }
            });
        });
        ui.separator();
        ui.label(RichText::new(tr("T-connections")).strong());
        ui.horizontal(|ui| {
            for v in TeeStyle::ALL {
                ui.radio_value(&mut s.tee, *v, tee_title(*v));
            }
        });
        ui.weak(tr("Connections of four or more wires always have a dot."));
        ui.separator();
        ui.label(RichText::new(tr("Preview")).strong());
        preview(ui, s);
        ui.separator();
        ui.label(RichText::new(tr("Moving")).strong());
        ui.checkbox(&mut s.reroute, tr("Route the connections of moved items anew"));
        ui.weak(tr("Takes the way with the least weight: its length in grid steps, 1.2 per bend and 1.5 per wire crossed. Off: the wires stretch and keep their corners."));
    }
}

/// A crossing and a T-connection drawn in the chosen styles.
fn preview(ui: &mut egui::Ui, s: &WireSettings) {
    const SCALE: f32 = 5.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0 * SCALE, 20.0 * SCALE), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let color = ui.visuals().text_color();
    let at = |q: P| rect.min + egui::vec2((q.x + 2.0) * SCALE, (q.y + 2.0) * SCALE);
    let wire = |id: u64, a: P, b: P| Wire { id, a, b, ..Default::default() };
    let level = wire(1, p(0.0, 8.0), p(40.0, 8.0));
    let upright = wire(2, p(12.0, 0.0), p(12.0, 16.0));
    let tee = wire(3, p(28.0, 8.0), p(28.0, 16.0));
    let crossing = [p(12.0, 8.0)];
    let jumping = if s.jump_wire == JumpWire::Horizontal { level.id } else { upright.id };
    for w in [&level, &upright, &tee] {
        let at_jumps: &[P] = if w.id == jumping { &crossing } else { &[] };
        for line in jumps::strokes(w, at_jumps, s.jump) {
            painter.add(egui::Shape::line(line.into_iter().map(at).collect(), egui::Stroke::new(1.5, color)));
        }
    }
    let dot = match s.tee {
        TeeStyle::Dot => Some(jumps::DOT_R),
        TeeStyle::LargeDot => Some(jumps::LARGE_DOT_R),
        TeeStyle::None => None,
    };
    if let Some(r) = dot {
        painter.circle_filled(at(p(28.0, 8.0)), r * SCALE, color);
    }
}
