//! Typed mounting and placement data of a part (EPLAN: mounting data, drilling
//! pattern): clearances, mounting holes, cutouts, power loss, fastening and the faces
//! accessories fit on. Older parts keep these values as `Key: value` lines in the
//! technical data; [`Part::data_value`] reads either, [`Part::migrate_tech_data`]
//! moves such lines into the typed fields.

use crate::Part;
use serde::{Deserialize, Serialize};

/// Keys of the typed values (also the column names of the CSV export and the keys of
/// the technical data they replace).
pub const MOUNTING_KEYS: [&str; 18] = [
    "Clearance top",
    "Clearance bottom",
    "Clearance left",
    "Clearance right",
    "Clearance front",
    "EMC clearance",
    "Service space",
    "Outside",
    "Power loss",
    "Mounting holes",
    "Door cutout",
    "Cutout",
    "Fastening",
    "Faces",
    "Collision class",
    "Terminal type",
    "Wall mounting",
    "Floor mounting",
];

/// A round or rectangular cutout.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cutout {
    Round { d: f32 },
    Rect { w: f32, h: f32 },
}

impl Cutout {
    /// `Ø22.5`, `22.5`, `92 x 92` (mm).
    pub fn parse(s: &str) -> Option<Cutout> {
        let s = s.trim().trim_end_matches("mm").trim().replace(',', ".");
        let num = |t: &str| t.trim().parse::<f32>().ok().filter(|v| *v > 0.0);
        if let Some((w, h)) = s.split_once(['x', 'X', '×']) {
            return Some(Cutout::Rect { w: num(w)?, h: num(h)? });
        }
        num(s.trim_start_matches(['Ø', 'D', 'd'])).map(|d| Cutout::Round { d })
    }

    pub fn text(&self) -> String {
        match self {
            Cutout::Round { d } => format!("Ø{}", fmt(*d)),
            Cutout::Rect { w, h } => format!("{} x {}", fmt(*w), fmt(*h)),
        }
    }
}

fn fmt(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The leading number of a value (`7.5 W` → 7.5).
fn leading_number(v: &str) -> Option<f32> {
    let num: String = v.trim().chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+')).collect();
    num.replace(',', ".").parse().ok()
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RailMount {
    /// Positive when the rail centerline is below the center of the device.
    pub center_offset: f32,
    /// Distance from the top surface of the rail to the rear of the device.
    pub clip_on: f32,
}

impl RailMount {
    pub fn is_centered(&self) -> bool { *self == Self::default() }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MountingData {
    #[serde(skip_serializing_if = "RailMount::is_centered")]
    pub rail: RailMount,
    /// Free space for ventilation and wiring around the part, mm (0: none).
    #[serde(skip_serializing_if = "is_zero")]
    pub clearance_top: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub clearance_bottom: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub clearance_left: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub clearance_right: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub clearance_front: f32,
    /// Distance to signal devices, mm.
    #[serde(skip_serializing_if = "is_zero")]
    pub emc_clearance: f32,
    /// Space in front for operation and replacement, mm.
    #[serde(skip_serializing_if = "is_zero")]
    pub service_space: f32,
    /// How far an accessory sticks out of its face, mm.
    #[serde(skip_serializing_if = "is_zero")]
    pub outside: f32,
    /// Power loss at rated load, W (0: unknown).
    #[serde(skip_serializing_if = "is_zero")]
    pub power_loss: f32,
    /// Thread or screw of the mounting holes (`M4`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hole_thread: String,
    /// Mounting holes relative to the top left corner of the front view, mm.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door_cutout: Option<Cutout>,
    /// Cutout in the face the part is mounted on (fans, glands, lights).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cutout: Option<Cutout>,
    /// Fastening key (`tapped`, `through`, `rail`, …).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fastening: String,
    /// Faces an accessory fits on (`Left, Right`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub faces: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub collision_class: String,
    /// Terminal type (`PE`, `N`, …) of terminals.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub terminal_type: String,
    /// The part is screwed directly to the wall or ceiling of a room (inverters,
    /// charge controllers, distribution boxes); other devices need a rail there.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub wall_mount: bool,
    /// The part stands on the floor of a room (pumps, tanks, compressors).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub floor_mount: bool,
}

/// `yes` / `no` of a flag (also `true`, `1`, `x`); `None` for other texts.
fn parse_flag(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "no" | "false" | "0" | "-" => Some(false),
        "yes" | "true" | "1" | "x" => Some(true),
        _ => None,
    }
}

impl MountingData {
    pub fn is_empty(&self) -> bool {
        *self == MountingData::default()
    }

    fn number_mut(&mut self, key: &str) -> Option<&mut f32> {
        Some(match key {
            "clearance top" => &mut self.clearance_top,
            "clearance bottom" => &mut self.clearance_bottom,
            "clearance left" => &mut self.clearance_left,
            "clearance right" => &mut self.clearance_right,
            "clearance front" => &mut self.clearance_front,
            "emc clearance" => &mut self.emc_clearance,
            "service space" => &mut self.service_space,
            "outside" => &mut self.outside,
            "power loss" => &mut self.power_loss,
            _ => return None,
        })
    }

    fn text_mut(&mut self, key: &str) -> Option<&mut String> {
        Some(match key {
            "fastening" => &mut self.fastening,
            "faces" => &mut self.faces,
            "collision class" => &mut self.collision_class,
            "terminal type" => &mut self.terminal_type,
            _ => return None,
        })
    }

    /// The value of a key as text (as it would be written in the technical data);
    /// `None` if it is not set.
    pub fn get(&self, key: &str) -> Option<String> {
        let key = key.trim().to_ascii_lowercase();
        let numbers = [
            ("clearance top", self.clearance_top),
            ("clearance bottom", self.clearance_bottom),
            ("clearance left", self.clearance_left),
            ("clearance right", self.clearance_right),
            ("clearance front", self.clearance_front),
            ("emc clearance", self.emc_clearance),
            ("service space", self.service_space),
            ("outside", self.outside),
            ("power loss", self.power_loss),
        ];
        if let Some((_, v)) = numbers.iter().find(|(k, _)| *k == key) {
            return (*v != 0.0).then(|| fmt(*v));
        }
        let texts = [("fastening", &self.fastening), ("faces", &self.faces), ("collision class", &self.collision_class), ("terminal type", &self.terminal_type)];
        if let Some((_, v)) = texts.iter().find(|(k, _)| *k == key) {
            return (!v.is_empty()).then(|| v.to_string());
        }
        match key.as_str() {
            "mounting holes" if !self.holes.is_empty() => {
                let pts: Vec<String> = self.holes.iter().map(|[x, y]| format!("{},{}", fmt(*x), fmt(*y))).collect();
                Some(format!("{}: {}", self.hole_thread, pts.join("; ")))
            }
            "door cutout" => self.door_cutout.map(|c| c.text()),
            "cutout" => self.cutout.map(|c| c.text()),
            "wall mounting" => self.wall_mount.then(|| "yes".to_string()),
            "floor mounting" => self.floor_mount.then(|| "yes".to_string()),
            _ => None,
        }
    }

    /// Sets a key from its text; an empty text clears it. `Clearance` sets the
    /// clearance above and below. Returns false for unknown keys and values that cannot
    /// be read.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "clearance" {
            return self.set("Clearance top", value) && self.set("Clearance bottom", value);
        }
        if let Some(v) = self.number_mut(&key) {
            if value.is_empty() {
                *v = 0.0;
                return true;
            }
            return match leading_number(value) {
                Some(n) if n >= 0.0 => {
                    *v = n;
                    true
                }
                _ => false,
            };
        }
        if let Some(v) = self.text_mut(&key) {
            *v = value.to_string();
            return true;
        }
        match key.as_str() {
            "mounting holes" => {
                if value.is_empty() {
                    self.holes.clear();
                    self.hole_thread.clear();
                    return true;
                }
                let Some((thread, list)) = value.split_once(':') else { return false };
                let pts: Option<Vec<[f32; 2]>> = list
                    .split(';')
                    .filter(|xy| !xy.trim().is_empty())
                    .map(|xy| {
                        let (x, y) = xy.split_once(',')?;
                        Some([x.trim().parse().ok()?, y.trim().parse().ok()?])
                    })
                    .collect();
                match pts {
                    Some(pts) if !pts.is_empty() => {
                        self.hole_thread = thread.trim().to_string();
                        self.holes = pts;
                        true
                    }
                    _ => false,
                }
            }
            "door cutout" | "cutout" => {
                let c = if value.is_empty() {
                    None
                } else {
                    match Cutout::parse(value) {
                        Some(c) => Some(c),
                        None => return false,
                    }
                };
                if key == "cutout" {
                    self.cutout = c;
                } else {
                    self.door_cutout = c;
                }
                true
            }
            "wall mounting" | "floor mounting" => match parse_flag(value) {
                Some(v) => {
                    *if key == "wall mounting" { &mut self.wall_mount } else { &mut self.floor_mount } = v;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

impl Part {
    /// The part can be mounted directly on the wall or ceiling of a room: the typed
    /// flag, a `Wall mounting: yes` line of the technical data or a mounting text
    /// saying so (EPLAN mounting site "Wall").
    pub fn wall_mountable(&self) -> bool {
        self.mounting_data.wall_mount
            || self.tech_value("Wall mounting").and_then(parse_flag).unwrap_or(false)
            || self.mounting.to_lowercase().contains("wall")
    }

    /// The part stands on the floor of a room: the typed flag, a `Floor mounting: yes`
    /// line of the technical data or a mounting text saying so (EPLAN mounting site
    /// "Floor").
    pub fn floor_standing(&self) -> bool {
        self.mounting_data.floor_mount
            || self.tech_value("Floor mounting").and_then(parse_flag).unwrap_or(false)
            || self.mounting.to_lowercase().contains("floor")
    }

    /// A mounting / placement value: the typed one, otherwise the line of the technical
    /// data with that key (older parts).
    pub fn data_value(&self, key: &str) -> Option<String> {
        self.mounting_data.get(key).or_else(|| self.tech_value(key).map(String::from))
    }

    /// Finite dimension from typed data or a technical-data line, with an optional
    /// mm suffix and a decimal point or comma.
    pub fn dimension_mm(&self, key: &str) -> Option<f32> {
        let value=self.data_value(key)?.trim().to_ascii_lowercase();
        value.trim_end_matches("mm").trim().replace(',', ".").parse::<f32>().ok().filter(|v|v.is_finite())
    }

    /// Moves the lines of the technical data that have typed fields into them. Lines whose
    /// value cannot be read stay. Returns the number of lines moved.
    pub fn migrate_tech_data(&mut self) -> usize {
        let mut moved = 0;
        let mut keep = Vec::new();
        for line in self.tech_data.lines() {
            let typed = line.split_once(':').filter(|(k, _)| {
                let k = k.trim();
                k.eq_ignore_ascii_case("Clearance") || MOUNTING_KEYS.iter().any(|m| m.eq_ignore_ascii_case(k))
            });
            match typed {
                Some((k, v)) if self.mounting_data.get(k).is_none() && self.mounting_data.set(k, v) => moved += 1,
                _ => keep.push(line),
            }
        }
        if moved > 0 {
            self.tech_data = keep.join("\n");
        }
        moved
    }
}

#[cfg(test)]
mod tests;
