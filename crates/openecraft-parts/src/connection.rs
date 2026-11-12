//! Connection points of a part: where its terminals are and in which direction their
//! wires leave, so that the panel layout can wire to the real terminals.
//!
//! Positions are millimetres in the part's front view, the frame of its mounting data
//! and of [`crate::Model2d`]: x to the right from the left edge, y down from the top
//! edge, z the height above the mounting surface (rail, plate or door).

use crate::{FnKind, Part};
use serde::{Deserialize, Serialize};

/// Direction in which the wire leaves a connection point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exit {
    #[default]
    Up,
    Down,
    Left,
    Right,
    /// Away from the mounting surface (rear terminals of door devices).
    Front,
}

impl Exit {
    pub const ALL: [Exit; 5] = [Exit::Up, Exit::Down, Exit::Left, Exit::Right, Exit::Front];

    /// Code in CSV files.
    pub fn code(self) -> &'static str {
        match self {
            Exit::Up => "up",
            Exit::Down => "down",
            Exit::Left => "left",
            Exit::Right => "right",
            Exit::Front => "front",
        }
    }

    pub fn from_code(s: &str) -> Option<Exit> {
        Exit::ALL.into_iter().find(|e| e.code().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionPoint {
    /// Designation as in the function templates (`A1`, `13`); terminals: `1` / `2`
    /// for their two sides.
    pub pin: String,
    /// `[x, y, z]` in mm, see the module documentation.
    pub pos: [f32; 3],
    pub exit: Exit,
    /// Smallest and largest conductor the clamp takes, mm² (0: not known).
    #[serde(skip_serializing_if = "is_zero")]
    pub min_cross_section: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub max_cross_section: f32,
    /// Most wires the clamp takes (0: not known).
    #[serde(skip_serializing_if = "is_zero_count")]
    pub max_wires: u32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_count(v: &u32) -> bool {
    *v == 0
}

impl ConnectionPoint {
    /// The point has limits for the wires it takes.
    pub fn has_wire_limits(&self) -> bool {
        self.min_cross_section > 0.0 || self.max_cross_section > 0.0 || self.max_wires > 0
    }
}

/// Screw terminals sit somewhat behind the front of a device.
const TERMINAL_DEPTH: f32 = 0.75;

/// Height of the wire entries of the screw terminals of DIN rail devices above the rail,
/// as a part of the device's depth: they are in its end faces below the screws, which
/// are reached from the front of its steps (in miniature circuit breakers and contactors
/// a third to two fifths of the depth, in terminal blocks from 0.3 to 0.5).
const RAIL_TERMINAL_DEPTH: f32 = 0.4;

/// Height of the clamps of NH fuse switch disconnectors mounted on busbars above the
/// bars, as a part of the depth: in the terminal compartments at their ends, entered
/// from the end faces below the screws reached through the terminal covers.
const FUSE_SWITCH_TERMINAL_DEPTH: f32 = 0.575;

/// The height of the generated connection points of a part above its mounting surface:
/// DIN rail devices are entered from their end faces ([`RAIL_TERMINAL_DEPTH`]), as are
/// fuse switch disconnectors on busbars ([`FUSE_SWITCH_TERMINAL_DEPTH`]); PLC modules
/// from their front connectors.
fn terminal_depth(part: &Part) -> f32 {
    let plc = part.functions.iter().any(|f| matches!(f.kind, FnKind::PlcInput | FnKind::PlcOutput | FnKind::PlcAnalogInput | FnKind::PlcAnalogOutput));
    if part.mounting.starts_with("Mounting rail") && !plc {
        RAIL_TERMINAL_DEPTH
    } else if part.mounting.starts_with("Busbar") && part.subgroup == "Fuse switch disconnector" {
        FUSE_SWITCH_TERMINAL_DEPTH
    } else {
        TERMINAL_DEPTH
    }
}

/// Devices mounted in the door: their terminals are on the rear.
fn door_kind(kind: FnKind) -> bool {
    matches!(kind, FnKind::PushbuttonNo | FnKind::PushbuttonNc | FnKind::SwitchNo | FnKind::SwitchNc | FnKind::Signal | FnKind::Meter)
}

/// Whether a connection point is on the upper side of a device (IEC convention: odd
/// designations at the top, even at the bottom; supply above, transformer primary
/// `1.x` above the secondary `2.x`).
pub fn upper_side(pin: &str) -> bool {
    let pin = pin.trim();
    if matches!(pin, "L" | "N" | "PE" | "L1" | "L2" | "L3") {
        return true;
    }
    if let Some((winding, _)) = pin.split_once('.').filter(|(a, b)| !a.is_empty() && !b.is_empty() && pin.chars().all(|c| c.is_ascii_digit() || c == '.')) {
        return winding.parse::<u32>().is_ok_and(|w| w % 2 == 1);
    }
    // Power terminals with a leading digit (IEC 60445): 1L1 above, 2T1 below.
    let mut chars = pin.chars();
    if let (Some(d), Some(c)) = (chars.next().and_then(|c| c.to_digit(10)), chars.next()) {
        if c.is_ascii_alphabetic() {
            return d % 2 == 1;
        }
    }
    pin.chars().last().and_then(|c| c.to_digit(10)).is_some_and(|d| d % 2 == 1)
}

/// Connection points following the IEC conventions, for parts without their own:
/// the designations of the function templates in two rows (upper and lower edge,
/// spread over the width). Terminals get one point per side, door devices their
/// points on the rear, PLC modules all points at the bottom; surge arresters have their
/// lines on top and the earth and the remote signalling contact at the bottom. Empty
/// without size.
pub fn default_connection_points(part: &Part) -> Vec<ConnectionPoint> {
    let Some([w, h, d]) = part.size() else { return Vec::new() };
    let depth = d * terminal_depth(part);
    if part.functions.iter().any(|f| f.kind == FnKind::Terminal) {
        let pin = |pin: &str, y, exit| ConnectionPoint { pin: pin.into(), pos: [w / 2.0, y, depth], exit, ..Default::default() };
        return vec![pin("1", 0.0, Exit::Up), pin("2", h, Exit::Down)];
    }
    let door = !part.functions.is_empty() && part.functions.iter().all(|f| door_kind(f.kind));
    let arrester = part.functions.iter().any(|f| f.kind == FnKind::SurgeProtection);
    let (mut upper, mut lower): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for f in &part.functions {
        let plc = matches!(f.kind, FnKind::PlcInput | FnKind::PlcOutput | FnKind::PlcAnalogInput | FnKind::PlcAnalogOutput);
        for pin in f.pin_list() {
            if upper.contains(&pin) || lower.contains(&pin) {
                continue;
            }
            let top = if arrester { f.kind == FnKind::SurgeProtection && !matches!(pin.as_str(), "PE" | "PEN") } else { !plc && upper_side(&pin) };
            if top {
                upper.push(pin);
            } else {
                lower.push(pin);
            }
        }
    }
    let mut out = Vec::new();
    for (row, top) in [(upper, true), (lower, false)] {
        let n = row.len() as f32;
        for (i, pin) in row.into_iter().enumerate() {
            let x = w * (i as f32 + 0.5) / n;
            let (pos, exit) = match (door, top) {
                (true, true) => ([x, h * 0.25, d], Exit::Front),
                (true, false) => ([x, h * 0.75, d], Exit::Front),
                (false, true) => ([x, 0.0, depth], Exit::Up),
                (false, false) => ([x, h, depth], Exit::Down),
            };
            out.push(ConnectionPoint { pin, pos, exit, ..Default::default() });
        }
    }
    out
}

impl Part {
    /// The connection points of the part, or generated ones if it has none. The flag
    /// tells whether they are the part's own (exact) points.
    pub fn connection_points_or_default(&self) -> (Vec<ConnectionPoint>, bool) {
        if self.connection_points.is_empty() {
            (default_connection_points(self), false)
        } else {
            (self.connection_points.clone(), true)
        }
    }
}

/// CSV form: `A1:22.5/0/55/up|A2:22.5/74/55/down`; a point with wire limits adds the
/// smallest and largest cross-section and the most wires: `1:5/0/40/up/0.2/1.5/1`.
pub fn points_to_text(points: &[ConnectionPoint]) -> String {
    points
        .iter()
        .map(|c| {
            let limits = if c.has_wire_limits() { format!("/{}/{}/{}", c.min_cross_section, c.max_cross_section, c.max_wires) } else { String::new() };
            format!("{}:{}/{}/{}/{}{limits}", c.pin, c.pos[0], c.pos[1], c.pos[2], c.exit.code())
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// Reads [`points_to_text`]; decimal commas are accepted, broken entries skipped.
pub fn points_from_text(text: &str) -> Vec<ConnectionPoint> {
    text.split('|')
        .filter_map(|entry| {
            let (pin, rest) = entry.trim().rsplit_once(':')?;
            let fields: Vec<&str> = rest.split('/').map(str::trim).collect();
            let num = |s: &str| s.replace(',', ".").parse::<f32>().ok();
            let (place, limits) = fields.split_at(fields.len().min(4));
            let [x, y, z, exit] = place else { return None };
            let mut c = ConnectionPoint { pin: pin.trim().into(), pos: [num(x)?, num(y)?, num(z)?], exit: Exit::from_code(exit)?, ..Default::default() };
            match limits {
                [] => {}
                [min, max, wires] => {
                    c.min_cross_section = num(min)?.max(0.0);
                    c.max_cross_section = num(max)?.max(0.0);
                    c.max_wires = wires.parse().ok()?;
                }
                _ => return None,
            }
            Some(c)
        })
        .collect()
}

#[cfg(test)]
mod tests;
