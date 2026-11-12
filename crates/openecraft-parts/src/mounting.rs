//! Typed mounting and placement data of a part (EPLAN: mounting data, drilling
//! pattern): clearances, mounting holes, cutouts (with their place, turn and corner
//! radius, [`PlacedCutout`]), power loss, fastening and the faces accessories fit on. Older parts keep these values as `Key: value` lines in the
//! technical data; [`Part::data_value`] reads either, [`Part::migrate_tech_data`]
//! moves such lines into the typed fields.

use crate::Part;
use serde::{Deserialize, Deserializer, Serialize};

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
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn is_origin(v: &[f32; 2]) -> bool {
    *v == [0.0, 0.0]
}

/// A cutout of a part in the face it is mounted on: its shape, its centre relative to
/// the centre of the part's front view (mm, x to the right, y down), its turn (degrees
/// counter-clockwise, seen from the front) and the radius of the corners of a
/// rectangular one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedCutout {
    pub shape: Cutout,
    #[serde(default, skip_serializing_if = "is_origin")]
    pub offset: [f32; 2],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: f32,
}

impl PlacedCutout {
    /// A cutout in the middle of the part.
    pub fn centered(shape: Cutout) -> PlacedCutout {
        PlacedCutout { shape, offset: [0.0, 0.0], angle: 0.0, radius: 0.0 }
    }

    /// `197 x 141`, `197 x 141 @ 0/-6.5 R3 15°`, `Ø22.5 @ -40/0`: the shape
    /// ([`Cutout::parse`]), then optionally `@ x/y` (offset of the centre), `R` and the
    /// corner radius and the turn in degrees.
    pub fn parse(text: &str) -> Option<PlacedCutout> {
        let num = |t: &str| t.trim().replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite());
        let (shape, place) = match text.split_once('@') {
            Some((shape, place)) => (shape, place),
            None => (text, ""),
        };
        let mut tokens: Vec<&str> = shape.split_whitespace().collect();
        let mut place: Vec<&str> = place.split_whitespace().collect();
        let mut c = PlacedCutout::centered(Cutout::Rect { w: 1.0, h: 1.0 });
        // Radius and angle stand after the shape or after the offset.
        for list in [&mut place, &mut tokens] {
            while let Some(last) = list.last().copied() {
                if let Some(a) = last.strip_suffix('°') {
                    c.angle = num(a)?;
                } else if let Some(r) = last.strip_prefix(['R', 'r']).filter(|r| num(r).is_some()) {
                    c.radius = num(r)?.max(0.0);
                } else {
                    break;
                }
                list.pop();
            }
        }
        c.shape = Cutout::parse(&tokens.join(" "))?;
        if !place.is_empty() {
            let (x, y) = place.join("").split_once('/').map(|(x, y)| (x.to_string(), y.to_string()))?;
            c.offset = [num(&x)?, num(&y)?];
        }
        Some(c)
    }

    pub fn text(&self) -> String {
        let mut t = self.shape.text();
        if self.offset != [0.0, 0.0] {
            t += &format!(" @ {}/{}", fmt(self.offset[0]), fmt(self.offset[1]));
        }
        if self.radius > 0.0 {
            t += &format!(" R{}", fmt(self.radius));
        }
        if self.angle != 0.0 {
            t += &format!(" {}°", fmt(self.angle));
        }
        t
    }

    /// Width and height of the shape (the diameter twice for round ones).
    pub fn size(&self) -> [f32; 2] {
        match self.shape {
            Cutout::Round { d } => [d, d],
            Cutout::Rect { w, h } => [w, h],
        }
    }
}

/// The outline of a cutout around `center` (y down): `size` wide and high, turned by
/// `angle` degrees counter-clockwise, with rounded corners of `radius`; round ones (and
/// the corners) as polygons.
pub fn cutout_outline(center: [f32; 2], size: [f32; 2], round: bool, angle: f32, radius: f32) -> Vec<[f32; 2]> {
    let [cx, cy] = center;
    if round {
        let r = size[0] / 2.0;
        return (0..48).map(|i| (std::f32::consts::TAU * i as f32 / 48.0).sin_cos()).map(|(s, c)| [cx + c * r, cy + s * r]).collect();
    }
    let (s, c) = angle.to_radians().sin_cos();
    // Counter-clockwise on a face whose y points down.
    let (u, v) = ([c, -s], [s, c]);
    let r = radius.clamp(0.0, size[0].min(size[1]) / 2.0);
    let (hw, hh) = (size[0] / 2.0 - r, size[1] / 2.0 - r);
    let steps = if r > 0.0 { 6 } else { 0 };
    let mut pts = Vec::new();
    // The corners from the lower right on, each with its quarter of the radius.
    for (k, [sx, sy]) in [[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]].into_iter().enumerate() {
        for i in 0..=steps {
            let a = std::f32::consts::FRAC_PI_2 * (k as f32 + i as f32 / steps.max(1) as f32);
            let (x, y) = (sx * hw + r * a.cos(), sy * hh + r * a.sin());
            pts.push([cx + u[0] * x + v[0] * y, cy + u[1] * x + v[1] * y]);
        }
    }
    pts
}

impl PlacedCutout {
    /// The outline in the front view of a part `w` wide and `h` high.
    pub fn outline(&self, w: f32, h: f32) -> Vec<[f32; 2]> {
        cutout_outline([w / 2.0 + self.offset[0], h / 2.0 + self.offset[1]], self.size(), matches!(self.shape, Cutout::Round { .. }), self.angle, self.radius)
    }
}

/// Several cutouts in their text form, separated by `;`.
pub fn cutouts_text(list: &[PlacedCutout]) -> String {
    list.iter().map(PlacedCutout::text).collect::<Vec<_>>().join("; ")
}

/// Reads [`cutouts_text`]; `None` if one of them cannot be read.
pub fn parse_cutouts(text: &str) -> Option<Vec<PlacedCutout>> {
    text.split(';').filter(|t| !t.trim().is_empty()).map(PlacedCutout::parse).collect()
}

/// Cutouts as stored: a list of placed cutouts, or (older parts) one cutout without a
/// place, in JSON or as an option in RON (the app's storage).
fn cutout_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PlacedCutout>, D::Error> {
    use serde::de::value::{EnumAccessDeserializer, MapAccessDeserializer};
    use serde::de::{EnumAccess, MapAccess, SeqAccess, Visitor};
    struct List;
    impl<'de> Visitor<'de> for List {
        type Value = Vec<PlacedCutout>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a list of cutouts or one cutout")
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            Cutout::deserialize(d).map(|c| vec![PlacedCutout::centered(c)])
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut list = Vec::new();
            while let Some(c) = seq.next_element::<PlacedCutout>()? {
                list.push(c);
            }
            Ok(list)
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
            Cutout::deserialize(MapAccessDeserializer::new(map)).map(|c| vec![PlacedCutout::centered(c)])
        }
        fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
            Cutout::deserialize(EnumAccessDeserializer::new(data)).map(|c| vec![PlacedCutout::centered(c)])
        }
    }
    d.deserialize_any(List)
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
    /// Cutouts in the door for a door device, when they differ from [`Self::cutouts`].
    #[serde(alias = "door_cutout", deserialize_with = "cutout_list", skip_serializing_if = "Vec::is_empty")]
    pub door_cutouts: Vec<PlacedCutout>,
    /// Cutouts in the face the part is mounted on (fans, glands, lights, panels).
    #[serde(alias = "cutout", deserialize_with = "cutout_list", skip_serializing_if = "Vec::is_empty")]
    pub cutouts: Vec<PlacedCutout>,
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
            "door cutout" if !self.door_cutouts.is_empty() => Some(cutouts_text(&self.door_cutouts)),
            "cutout" if !self.cutouts.is_empty() => Some(cutouts_text(&self.cutouts)),
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
                let Some(list) = parse_cutouts(value) else { return false };
                *if key == "cutout" { &mut self.cutouts } else { &mut self.door_cutouts } = list;
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

    /// The cutouts of the part in the face it is mounted on; `door` for the door, which
    /// takes the door cutouts where the part has them (typed data, otherwise the
    /// technical data of older parts).
    pub fn cutouts(&self, door: bool) -> Vec<PlacedCutout> {
        let md = &self.mounting_data;
        let keys: &[&str] = if door { &["Door cutout", "Cutout"] } else { &["Cutout"] };
        keys.iter()
            .find_map(|k| {
                let typed = match *k {
                    "Door cutout" => &md.door_cutouts,
                    _ => &md.cutouts,
                };
                if !typed.is_empty() {
                    return Some(typed.clone());
                }
                self.tech_value(k).and_then(parse_cutouts).filter(|l| !l.is_empty())
            })
            .unwrap_or_default()
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
