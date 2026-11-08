//! Thermal simulation of the mounting plate: an estimate of the air temperature in
//! front of the plate. The mean comes from the climate calculation (IEC/TR 60890,
//! with the cooling it asks for), stratified from the bottom to the roof (factor c
//! of [`super::PanelRules::stratification`]); every device with power loss data adds
//! a plume of warm air that rises above it and widens, and decays quickly below it.
//! A horizontal cable duct above a device holds its plume back: the warm air gathers
//! and spreads below the duct, only part of it passes. Fans and cooling units mix the
//! air and weaken the plumes. Devices are checked against their highest ambient
//! temperature (part data `Max temperature: 55 °C` or `Operating temperature:
//! -25 … 60 °C`).

use super::checks::data_number;
use super::{part_by_number, Enclosure, Face, ItemKind, PanelItem};
use crate::climate::{self, Cooling};
use crate::geom::{p, P, R};
use crate::model::Project;
use crate::parts::Part;

/// Cell size of the heat map, mm.
pub const CELL: f32 = 10.0;
/// Heat transfer coefficient of the device surfaces (natural convection), W/(m²·K).
const SURFACE_H: f32 = 7.0;
/// Share of the surface temperature rise the air next to a device takes.
const AIR_SHARE: f32 = 0.25;
/// Highest rise of a plume, K.
const PLUME_MAX: f32 = 40.0;
/// Plume: half width next to the device, widening per mm of rise, decay length
/// above and below the device (mm).
const PLUME_SPREAD: f32 = 15.0;
const PLUME_WIDEN: f32 = 0.25;
const PLUME_RISE: f32 = 200.0;
const PLUME_BELOW: f32 = 20.0;
/// Share of a plume that passes a duct above the device; below the duct the plume
/// decays this many times slower and widens as fast.
const DUCT_PASS: f32 = 0.35;
const TRAPPED: f32 = 2.0;
/// Share of the rise of a plume left with forced air circulation.
const FORCED: f32 = 0.5;

/// A device giving off heat.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: u64,
    /// Power loss, W.
    pub loss: f32,
    pub rect: R,
    /// Rise of the air right above it, K.
    pub rise: f32,
    /// Lower edge of the duct right above it, mm.
    pub ceiling: Option<f32>,
}

impl Source {
    /// Temperature rise its plume causes at a point of the plate.
    pub fn rise_at(&self, q: P) -> f32 {
        let r = &self.rect;
        let dx = (r.min.x - q.x).max(q.x - r.max.x).max(0.0);
        let (fy, along) = if q.y < r.min.y {
            let along = r.min.y - q.y;
            match self.ceiling {
                Some(c) if q.y >= c => ((-along / (PLUME_RISE * TRAPPED)).exp(), along * TRAPPED),
                Some(c) => (DUCT_PASS * (-(r.min.y - c) / (PLUME_RISE * TRAPPED) - (c - q.y) / PLUME_RISE).exp(), along + (r.min.y - c) * (TRAPPED - 1.0)),
                None => ((-along / PLUME_RISE).exp(), along),
            }
        } else if q.y > r.max.y {
            ((-(q.y - r.max.y) / PLUME_BELOW).exp(), 0.0)
        } else {
            (1.0, 0.0)
        };
        let spread = PLUME_SPREAD + PLUME_WIDEN * along;
        self.rise * fy * (-(dx / spread).powi(2)).exp()
    }
}

/// Lower edge of the nearest horizontal duct above an area of the plate.
fn ceiling(e: &Enclosure, r: &R) -> Option<f32> {
    e.items
        .iter()
        .filter(|i| i.kind == ItemKind::Duct && i.face == Face::Plate && i.w > i.h)
        .map(|i| i.rect())
        .filter(|d| d.max.y <= r.min.y + 0.5 && d.min.x < r.max.x && d.max.x > r.min.x)
        .map(|d| d.max.y)
        .reduce(f32::max)
}

/// Air temperatures in front of the mounting plate.
#[derive(Clone, Debug, PartialEq)]
pub struct HeatMap {
    pub cols: usize,
    pub rows: usize,
    pub cell: f32,
    /// Row by row from the top left, °C.
    pub temps: Vec<f32>,
    pub min: f32,
    pub max: f32,
    /// Plate coordinates of the hottest cell's centre.
    pub hottest: P,
    /// Mean inside temperature (climate calculation), °C.
    pub mean: f32,
    pub sources: Vec<Source>,
    /// Stratified temperature at the bottom and at the top of the plate, °C.
    pub bottom: f32,
    pub top: f32,
    plate_h: f32,
}

impl HeatMap {
    /// Temperature without the devices' plumes at a height of the plate.
    pub fn base(&self, y: f32) -> f32 {
        let f = if self.plate_h > 0.0 { (y / self.plate_h).clamp(0.0, 1.0) } else { 0.5 };
        self.top + (self.bottom - self.top) * f
    }

    /// Air temperature at a point, without the plume of one device (its own).
    pub fn air(&self, q: P, except: u64) -> f32 {
        self.base(q.y) + self.sources.iter().filter(|s| s.id != except).map(|s| s.rise_at(q)).sum::<f32>()
    }

    /// Temperature of the cell at a point.
    pub fn at(&self, q: P) -> Option<f32> {
        if q.x < 0.0 || q.y < 0.0 {
            return None;
        }
        let (c, r) = ((q.x / self.cell) as usize, (q.y / self.cell) as usize);
        (c < self.cols && r < self.rows).then(|| self.temps[r * self.cols + c])
    }
}

/// Power loss of a device from the parts assigned to it, W; `None` without data.
pub fn device_loss(prj: &Project, tag: &str) -> Option<f32> {
    let data = prj.devices.get(tag)?;
    let mut sum = None;
    for r in &data.parts {
        if let Some(l) = part_by_number(prj, &r.number).and_then(climate::power_loss) {
            *sum.get_or_insert(0.0) += l * r.quantity() as f32;
        }
    }
    sum
}

/// Rise of the air right above a device of this size (mm) and power loss (W).
pub fn plume_rise(it: &PanelItem, loss: f32) -> f32 {
    let (w, h, d) = (it.w / 1000.0, it.h / 1000.0, it.d.max(10.0) / 1000.0);
    let surface = (2.0 * (w * h + w * d + h * d)).max(0.005);
    (AIR_SHARE * loss / (SURFACE_H * surface)).min(PLUME_MAX)
}

/// Heat map of the mounting plate of an enclosure.
pub fn heat_map(prj: &Project, e: &Enclosure) -> HeatMap {
    let c = &e.climate;
    let result = climate::calculate(prj, e);
    let mean = if result.cooling == Cooling::None { result.inside } else { c.inside_max.max(c.ambient_max) };
    let rise = (mean - c.ambient_max).max(0.0);
    let k = prj.layout.checks.stratification.clamp(1.0, 2.0);
    let (pw, ph) = e.plate_size();
    let forced = if result.cooling == Cooling::None { 1.0 } else { FORCED };
    let sources: Vec<Source> = e
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Device && i.face == Face::Plate)
        .filter_map(|i| {
            let loss = device_loss(prj, &i.tag).filter(|l| *l > 0.0)?;
            let rect = i.rect();
            Some(Source { id: i.id, loss, rect, rise: plume_rise(i, loss) * forced, ceiling: ceiling(e, &rect) })
        })
        .collect();
    let (cols, rows) = ((pw / CELL).ceil().max(1.0) as usize, (ph / CELL).ceil().max(1.0) as usize);
    let mut map = HeatMap {
        cols,
        rows,
        cell: CELL,
        temps: Vec::with_capacity(cols * rows),
        min: f32::INFINITY,
        max: f32::NEG_INFINITY,
        hottest: p(0.0, 0.0),
        mean,
        sources,
        bottom: c.ambient_max + rise * (2.0 - k),
        top: c.ambient_max + rise * k,
        plate_h: ph,
    };
    for r in 0..rows {
        for col in 0..cols {
            let q = p((col as f32 + 0.5) * CELL, (r as f32 + 0.5) * CELL);
            let t = map.air(q, 0);
            if t > map.max {
                map.max = t;
                map.hottest = q;
            }
            map.min = map.min.min(t);
            map.temps.push(t);
        }
    }
    map
}

/// Highest ambient temperature of a part (`Max temperature: 55 °C`, the upper end
/// of `Operating temperature: -25 … 60 °C`).
pub fn temperature_limit(part: &Part) -> Option<f32> {
    if let Some(v) = data_number(part, "Max temperature") {
        return Some(v);
    }
    let v = part.tech_value("Operating temperature").or_else(|| part.tech_value("Ambient temperature"))?;
    let mut nums = Vec::new();
    let mut cur = String::new();
    for ch in v.replace("...", " ").replace('…', " ").chars().chain([' ']) {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' || (ch == '-' && cur.is_empty()) || (ch == '+' && cur.is_empty()) {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.replace(',', ".").parse::<f32>() {
                nums.push(n);
            }
            cur.clear();
        }
    }
    nums.into_iter().reduce(f32::max)
}

/// Air temperature at a device on the plate and its limit.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceTemperature {
    pub id: u64,
    pub tag: String,
    /// Hottest air around it without its own plume, °C.
    pub temperature: f32,
    /// From its part, otherwise the project's limit ([`super::PanelRules::device_temperature`]).
    pub limit: Option<f32>,
    /// Its own power loss, W.
    pub loss: Option<f32>,
}

/// Temperatures at the devices on the mounting plate.
pub fn device_temperatures(prj: &Project, e: &Enclosure, map: &HeatMap) -> Vec<DeviceTemperature> {
    let default = Some(prj.layout.checks.device_temperature).filter(|v| *v > 0.0);
    e.items
        .iter()
        .filter(|i| i.kind == ItemKind::Device && i.face == Face::Plate)
        .map(|i| {
            let y = i.y + i.h / 2.0;
            let temperature = [0.2, 0.5, 0.8].into_iter().map(|f| map.air(p(i.x + i.w * f, y), i.id)).fold(f32::NEG_INFINITY, f32::max);
            let limit = part_by_number(prj, &i.part).and_then(temperature_limit).or(default);
            DeviceTemperature { id: i.id, tag: i.tag.clone(), temperature, limit, loss: device_loss(prj, &i.tag) }
        })
        .collect()
}

#[cfg(test)]
mod tests;
