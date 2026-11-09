//! Panel validation (in the spirit of EPLAN Pro Panel's layout checks): mounting,
//! ventilation and EMC clearances of the devices from their part data, service space
//! in front of them, heights above the floor (IEC 60204-1 10.1.2, 11.2.1), distances
//! between configurable collision classes and the hot spots of the thermal simulation
//! ([`super::thermal`]). Manufacturability: [`super::manufacturing`].

use super::{collisions, item_box, outside, part_by_number, Enclosure, Face, ItemKind, PanelItem, PLATE_MARGIN};
use crate::model::Project;
use crate::parts::{kind_of_symbol, FnKind, Part};
use crate::issue::Severity;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use std::collections::{BTreeSet, HashMap};
pub use crate::model::panel::{ClassRule, PanelRules};

impl Default for PanelRules {
    fn default() -> Self {
        PanelRules {
            device_gap: 0.0,
            duct_gap: 0.0,
            edge_distance: 0.0,
            service_space: 0.0,
            operating_min: 400.0,
            operating_max: 2000.0,
            actuator_min: 600.0,
            actuator_max: 1700.0,
            terminal_min: 200.0,
            device_temperature: 55.0,
            stratification: 1.4,
            hole_web: 3.0,
            hole_edge: 3.0,
            cutout_web: 5.0,
            min_profile: 50.0,
            stock_length: 2000.0,
            classes: vec![ClassRule::distance("drive", "signal", 100.0)],
        }
    }
}

impl PanelRules {
    pub fn is_default(&self) -> bool {
        *self == PanelRules::default()
    }

    /// The rule for two classes: the first that names both.
    pub fn rule(&self, a: &str, b: &str) -> Option<&ClassRule> {
        self.classes.iter().find(|r| r.matches(a, b))
    }
}

impl ClassRule {
    pub fn distance(a: &str, b: &str, distance: f32) -> ClassRule {
        ClassRule { a: a.into(), b: b.into(), distance, ignore: false }
    }

    pub fn matches(&self, a: &str, b: &str) -> bool {
        let m = |rule: &str, c: &str| rule.trim() == "*" || rule.trim().eq_ignore_ascii_case(c);
        (m(&self.a, a) && m(&self.b, b)) || (m(&self.a, b) && m(&self.b, a))
    }
}

/// The built-in collision classes: key and title.
pub const CLASSES: [(&str, &str); 10] = [
    ("power", tr_const!("Power")),
    ("drive", tr_const!("Drive")),
    ("control", tr_const!("Control")),
    ("signal", tr_const!("Signal")),
    ("terminal", tr_const!("Terminal")),
    ("rail", tr_const!("Mounting rail")),
    ("duct", tr_const!("Cable duct")),
    ("busbar", tr_const!("Busbar system")),
    ("cutout", tr_const!("Cutout")),
    ("accessory", tr_const!("Accessory")),
];

/// Title of a class: translated for the built-in ones, the key of others.
pub fn class_title(key: &str) -> String {
    match CLASSES.iter().find(|c| c.0 == key) {
        Some(c) => tr(c.1).to_string(),
        None if key.trim() == "*" => tr("(any)").to_string(),
        None => key.to_string(),
    }
}

/// Function kinds of the devices, by tag.
pub fn device_kinds(prj: &Project) -> HashMap<&str, BTreeSet<FnKind>> {
    let mut m: HashMap<&str, BTreeSet<FnKind>> = HashMap::new();
    for s in prj.pages.iter().flat_map(|p| p.symbols.iter()).filter(|s| !s.tag.is_empty()) {
        if let Some(k) = kind_of_symbol(&s.sym) {
            m.entry(s.tag.as_str()).or_default().insert(k);
        }
    }
    m
}

/// Class of a device by its functions.
pub fn device_class(kinds: &BTreeSet<FnKind>) -> &'static str {
    use FnKind::*;
    let any = |ks: &[FnKind]| kinds.iter().any(|k| ks.contains(k));
    if any(&[FrequencyConverter, SoftStarter]) {
        "drive"
    } else if any(&[PlcAnalogInput, PlcAnalogOutput, Transmitter, Thermistor]) {
        "signal"
    } else if !kinds.is_empty() && kinds.iter().all(|k| *k == Terminal) {
        "terminal"
    } else if any(&[
        MainContacts3, MainContacts4, MainContact, Breaker1, Breaker2, Breaker3, Breaker4, ResidualCurrent, MotorProtection, Overload, Fuse, Fuse3,
        Disconnector3, SurgeProtection, PowerSupply, Transformer,
    ]) {
        "power"
    } else {
        "control"
    }
}

/// Collision class of an item: set on the item, from its part (`Collision class:
/// signal`), otherwise by kind and, for devices, by their functions.
pub fn item_class(prj: &Project, kinds: &HashMap<&str, BTreeSet<FnKind>>, it: &PanelItem) -> String {
    if !it.class.trim().is_empty() {
        return it.class.trim().to_ascii_lowercase();
    }
    if let Some(c) = part_by_number(prj, &it.part).and_then(|p| p.data_value("Collision class")).filter(|c| !c.trim().is_empty()) {
        return c.trim().to_ascii_lowercase();
    }
    match it.kind {
        ItemKind::Rail => "rail",
        ItemKind::Duct => "duct",
        ItemKind::Busbar => "busbar",
        ItemKind::Cutout => "cutout",
        ItemKind::Accessory => "accessory",
        ItemKind::MountingPlate => "accessory",
        ItemKind::Device => kinds.get(it.tag.as_str()).map(device_class).unwrap_or("control"),
    }
    .into()
}

/// First number of a part data value ("25 mm (above and below)" → 25).
pub fn data_number(part: &Part, name: &str) -> Option<f32> {
    let v = part.data_value(name)?;
    let num: String = v.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+')).collect();
    num.replace(',', ".").parse().ok()
}

/// Free space an item needs around it, from its part data, mm.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clearances {
    /// Ventilation above, below and in front (`Clearance: 50` for above and below,
    /// `Clearance top / bottom / front: 50`).
    pub top: f32,
    pub bottom: f32,
    pub front: f32,
    /// Mounting clearance to the neighbours (`Clearance left / right: 10`).
    pub left: f32,
    pub right: f32,
    /// Distance to signal devices (`EMC clearance: 100`).
    pub emc: f32,
    /// Space in front for operation and replacement (`Service space: 100`).
    pub service: f32,
}

pub fn clearances(part: &Part) -> Clearances {
    let n = |name: &str| data_number(part, name).filter(|v| *v > 0.0);
    let both = n("Clearance").unwrap_or(0.0);
    Clearances {
        top: n("Clearance top").unwrap_or(both),
        bottom: n("Clearance bottom").unwrap_or(both),
        front: n("Clearance front").unwrap_or(0.0),
        left: n("Clearance left").unwrap_or(0.0),
        right: n("Clearance right").unwrap_or(0.0),
        emc: n("EMC clearance").unwrap_or(0.0),
        service: n("Service space").unwrap_or(0.0),
    }
}

/// What a check is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckKind {
    Collision,
    Outside,
    Ventilation,
    Mounting,
    Emc,
    ClassDistance,
    Service,
    Height,
    Thermal,
    Manufacturing,
}

impl CheckKind {
    pub fn title(self) -> &'static str {
        match self {
            CheckKind::Collision => tr("Collision"),
            CheckKind::Outside => tr("Outside"),
            CheckKind::Ventilation => tr("Ventilation clearance"),
            CheckKind::Mounting => tr("Mounting clearance"),
            CheckKind::Emc => tr("EMC clearance"),
            CheckKind::ClassDistance => tr("Collision class"),
            CheckKind::Service => tr("Service space"),
            CheckKind::Height => tr("Height above floor"),
            CheckKind::Thermal => tr("Thermal"),
            CheckKind::Manufacturing => tr("Manufacturing"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            CheckKind::Collision => "collision",
            CheckKind::Outside => "outside",
            CheckKind::Ventilation => "ventilation",
            CheckKind::Mounting => "mounting",
            CheckKind::Emc => "emc",
            CheckKind::ClassDistance => "class_distance",
            CheckKind::Service => "service",
            CheckKind::Height => "height",
            CheckKind::Thermal => "thermal",
            CheckKind::Manufacturing => "manufacturing",
        }
    }
}

/// A problem of the panel layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub kind: CheckKind,
    pub severity: Severity,
    /// Items involved (highlighted).
    pub items: Vec<u64>,
    pub message: String,
}

/// Device tag or what the item is.
pub fn item_name(e: &Enclosure, id: u64) -> String {
    e.item(id)
        .map(|i| match i.kind {
            ItemKind::Device if !i.tag.is_empty() => i.tag.clone(),
            ItemKind::Cutout if !i.opening.is_cutout() => i.opening.title().to_string(),
            k => k.title().to_string(),
        })
        .unwrap_or_default()
}

/// Gap between two boxes (0 when they touch or overlap).
pub fn box_gap(a: &([f32; 3], [f32; 3]), b: &([f32; 3], [f32; 3])) -> f32 {
    let d: Vec<f32> = (0..3).map(|k| (b.0[k] - a.1[k]).max(a.0[k] - b.1[k]).max(0.0)).collect();
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn boxes_overlap(a: &([f32; 3], [f32; 3]), b: &([f32; 3], [f32; 3])) -> bool {
    let eps = 0.01;
    (0..3).all(|k| a.0[k] < b.1[k] - eps && b.0[k] < a.1[k] - eps)
}

/// Box of a zone given in the coordinates of an item's face.
/// Zone of x, y, width and height on an item's face and of base and depth.
fn zone_box(e: &Enclosure, it: &PanelItem, [x, y, w, h]: [f32; 4], (z, d): (f32, f32)) -> ([f32; 3], [f32; 3]) {
    item_box(e, &PanelItem { x, y, w, h, z, d, face: it.face, ..Default::default() })
}

fn same_group(a: &PanelItem, b: &PanelItem) -> bool {
    a.group != 0 && a.group == b.group
}

/// Pairs of colliding items, without those whose collision classes may overlap.
pub fn class_collisions(prj: &Project, e: &Enclosure) -> Vec<(u64, u64)> {
    let rules = &prj.layout.checks;
    let all = collisions(e, &prj.parts);
    if !rules.classes.iter().any(|r| r.ignore) {
        return all;
    }
    let kinds = device_kinds(prj);
    let class = |id: u64| e.item(id).map(|i| item_class(prj, &kinds, i)).unwrap_or_default();
    all.into_iter().filter(|(a, b)| !rules.rule(&class(*a), &class(*b)).is_some_and(|r| r.ignore)).collect()
}

/// Height of the enclosure's bottom above the floor: set, or 0 on a plinth; unknown
/// for wall mounted enclosures.
pub fn floor_height(e: &Enclosure) -> Option<f32> {
    e.elevation.or((e.build.plinth > 0.0).then_some(0.0)).map(|h| h + e.build.plinth)
}

/// Bottom and top edge of an item above the floor (vertical faces only).
pub fn heights(e: &Enclosure, it: &PanelItem) -> Option<(f32, f32)> {
    let base = floor_height(e)?;
    let top = match it.face {
        Face::Plate => e.height - PLATE_MARGIN,
        Face::Door | Face::Left | Face::Right | Face::Rear | Face::Wall(_) | Face::Inner(..) | Face::Outside(_) => e.height,
        Face::Roof | Face::Bottom | Face::RoofTop => return None,
    };
    Some((base + top - it.y - it.h, base + top - it.y))
}

/// All checks of an enclosure: collisions (by class), items outside, clearances,
/// service space, heights, thermal and manufacturability.
#[allow(clippy::too_many_lines)]
pub fn validate(prj: &Project, e: &Enclosure) -> Vec<Finding> {
    let rules = &prj.layout.checks;
    let kinds = device_kinds(prj);
    let name = |id: u64| item_name(e, id);
    let mut out = Vec::new();
    let mut push = |kind: CheckKind, severity: Severity, items: Vec<u64>, message: String| out.push(Finding { kind, severity, items, message });
    for (a, b) in class_collisions(prj, e) {
        push(CheckKind::Collision, Severity::Warning, vec![a, b], tr_fmt!("{}: {} collides with {}", e.name, name(a), name(b)));
    }
    for id in outside(e) {
        push(CheckKind::Outside, Severity::Warning, vec![id], tr_fmt!("{}: {} outside the mounting plate / enclosure", e.name, name(id)));
    }
    let boxes: Vec<_> = e.items.iter().map(|i| item_box(e, i)).collect();
    let classes: Vec<String> = e.items.iter().map(|i| item_class(prj, &kinds, i)).collect();
    let mm = |v: f32| format!("{v:.0}");
    for (ai, a) in e.items.iter().enumerate() {
        if !matches!(a.kind, ItemKind::Device | ItemKind::Accessory) {
            continue;
        }
        let c = part_by_number(prj, &a.part).map(clearances).unwrap_or_default();
        // Ventilation: above, below and in front; rails and cutouts do not block the air.
        let mut zones = Vec::new();
        if c.top > 0.0 {
            zones.push(zone_box(e, a, [a.x, a.y - c.top, a.w, c.top], (a.z, a.d)));
        }
        if c.bottom > 0.0 {
            zones.push(zone_box(e, a, [a.x, a.y + a.h, a.w, c.bottom], (a.z, a.d)));
        }
        if c.front > 0.0 {
            zones.push(zone_box(e, a, [a.x, a.y, a.w, a.h], (a.z + a.d, c.front)));
        }
        let gap_side = |v: f32| v.max(if a.kind == ItemKind::Device { rules.device_gap } else { 0.0 });
        let (left, right) = (gap_side(c.left), gap_side(c.right));
        let sides = [
            (left > 0.0).then(|| zone_box(e, a, [a.x - left, a.y, left, a.h], (a.z, a.d))),
            (right > 0.0).then(|| zone_box(e, a, [a.x + a.w, a.y, right, a.h], (a.z, a.d))),
        ];
        let emc = (c.emc > 0.0).then(|| {
            let (mut lo, mut hi) = boxes[ai];
            for k in 0..3 {
                lo[k] -= c.emc;
                hi[k] += c.emc;
            }
            (lo, hi)
        });
        for (bi, b) in e.items.iter().enumerate() {
            if bi == ai || same_group(a, b) {
                continue;
            }
            let passive = matches!(b.kind, ItemKind::Rail | ItemKind::Cutout);
            if !passive && zones.iter().any(|z| boxes_overlap(z, &boxes[bi])) {
                push(CheckKind::Ventilation, Severity::Warning, vec![a.id, b.id], tr_fmt!("{}: {} is inside the ventilation clearance of {}", e.name, name(b.id), name(a.id)));
            }
            // Mounting clearance: the neighbours side by side (each pair once when both need it).
            let neighbour = matches!(b.kind, ItemKind::Device | ItemKind::Accessory) && b.face == a.face;
            if neighbour && sides.iter().flatten().any(|z| boxes_overlap(z, &boxes[bi])) && !boxes_overlap(&boxes[ai], &boxes[bi]) {
                push(
                    CheckKind::Mounting,
                    Severity::Warning,
                    vec![a.id, b.id],
                    tr_fmt!("{}: {} is closer to {} than its mounting clearance of {} mm", e.name, name(b.id), name(a.id), mm(left.max(right))),
                );
            }
            if let Some(z) = &emc {
                if b.kind == ItemKind::Device && classes[bi] == "signal" && boxes_overlap(z, &boxes[bi]) {
                    push(CheckKind::Emc, Severity::Warning, vec![a.id, b.id], tr_fmt!("{}: {} is inside the EMC clearance of {} ({} mm)", e.name, name(b.id), name(a.id), mm(c.emc)));
                }
            }
        }
        // Wiring space to the cable ducts.
        if a.kind == ItemKind::Device && rules.duct_gap > 0.0 {
            let r = a.rect().expand(rules.duct_gap - 0.01);
            for d in e.items.iter().filter(|d| d.kind == ItemKind::Duct && d.face == a.face) {
                let dr = d.rect();
                if r.min.x < dr.max.x && dr.min.x < r.max.x && r.min.y < dr.max.y && dr.min.y < r.max.y {
                    push(CheckKind::Mounting, Severity::Warning, vec![a.id, d.id], tr_fmt!("{}: {} is closer than {} mm to a cable duct", e.name, name(a.id), mm(rules.duct_gap)));
                }
            }
        }
        if a.kind == ItemKind::Device && a.face == Face::Plate && rules.edge_distance > 0.0 {
            let plate = e.plate_rect();
            let inside = plate.expand(0.01).contains_rect(&a.rect());
            let margin = (a.x).min(a.y).min(plate.max.x - a.x - a.w).min(plate.max.y - a.y - a.h);
            if inside && margin < rules.edge_distance - 0.01 {
                push(CheckKind::Mounting, Severity::Warning, vec![a.id], tr_fmt!("{}: {} is closer than {} mm to the edge of the mounting plate", e.name, name(a.id), mm(rules.edge_distance)));
            }
        }
        // Service space in front of devices on the plate: up to the door, free of items.
        let service = if a.kind == ItemKind::Device && a.face == Face::Plate { c.service.max(rules.service_space) } else { 0.0 };
        if service > 0.0 {
            let zone = zone_box(e, a, [a.x, a.y, a.w, a.h], (a.z + a.d, service));
            let door = e.depth - e.wall();
            let blocker = e.items.iter().enumerate().find(|(bi, b)| *bi != ai && !same_group(a, b) && b.kind != ItemKind::Cutout && boxes_overlap(&zone, &boxes[*bi]));
            if let Some((_, b)) = blocker {
                push(CheckKind::Service, Severity::Warning, vec![a.id, b.id], tr_fmt!("{}: {} blocks the service space in front of {}", e.name, name(b.id), name(a.id)));
            } else if zone.1[2] > door + 0.01 {
                push(CheckKind::Service, Severity::Warning, vec![a.id], tr_fmt!("{}: no service space of {} mm in front of {} (the door is too close)", e.name, mm(service), name(a.id)));
            }
        }
    }
    // Distances between collision classes.
    for (ai, a) in e.items.iter().enumerate() {
        for (bi, b) in e.items.iter().enumerate().skip(ai + 1) {
            let Some(rule) = rules.rule(&classes[ai], &classes[bi]).filter(|r| !r.ignore && r.distance > 0.0) else { continue };
            if same_group(a, b) {
                continue;
            }
            let gap = box_gap(&boxes[ai], &boxes[bi]);
            if gap < rule.distance - 0.01 {
                push(
                    CheckKind::ClassDistance,
                    Severity::Warning,
                    vec![a.id, b.id],
                    tr_fmt!(
                        "{}: {} ({}) and {} ({}) are {} mm apart, at least {} mm required",
                        e.name,
                        name(a.id),
                        class_title(&classes[ai]),
                        name(b.id),
                        class_title(&classes[bi]),
                        mm(gap),
                        mm(rule.distance)
                    ),
                );
            }
        }
    }
    if e.is_room() {
        // No heights of machinery, no climate and no sheet metal work in a room.
        super::rooms::check(prj, e, &mut push);
        return out;
    }
    super::rack::check(e, &mut push);
    height_checks(e, &kinds, rules, &mut push);
    let map = super::thermal::heat_map(prj, e);
    for t in super::thermal::device_temperatures(prj, e, &map) {
        if let Some(limit) = t.limit.filter(|l| t.temperature > *l) {
            push(
                CheckKind::Thermal,
                Severity::Warning,
                vec![t.id],
                tr_fmt!("{}: {} reaches about {} °C at its place, allowed {} °C", e.name, name(t.id), format!("{:.0}", t.temperature), format!("{limit:.0}")),
            );
        }
    }
    if map.max > e.climate.inside_max + 0.5 && !map.temps.is_empty() {
        push(
            CheckKind::Thermal,
            Severity::Info,
            Vec::new(),
            tr_fmt!(
                "{}: hot spot of about {} °C at x = {} mm, y = {} mm of the mounting plate (inside temperature at most {} °C)",
                e.name,
                format!("{:.0}", map.max),
                mm(map.hottest.x),
                mm(map.hottest.y),
                format!("{:.0}", e.climate.inside_max)
            ),
        );
    }
    super::manufacturing::check(prj, e, &mut push);
    // Pairs that need a mounting clearance from each other are reported once.
    let mut seen = BTreeSet::new();
    out.retain(|f| {
        let mut ids = f.items.clone();
        ids.sort_unstable();
        f.kind != CheckKind::Mounting || seen.insert(ids)
    });
    out
}

/// Heights above the floor of terminals, devices and the actuators on the door.
fn height_checks(e: &Enclosure, kinds: &HashMap<&str, BTreeSet<FnKind>>, rules: &PanelRules, push: &mut impl FnMut(CheckKind, Severity, Vec<u64>, String)) {
    use FnKind::*;
    if floor_height(e).is_none() {
        return;
    }
    let mm = |v: f32| format!("{v:.0}");
    for it in e.items.iter().filter(|i| i.kind == ItemKind::Device) {
        let Some((bottom, top)) = heights(e, it) else { continue };
        let ks = kinds.get(it.tag.as_str()).cloned().unwrap_or_default();
        if device_class(&ks) == "terminal" {
            if rules.terminal_min > 0.0 && bottom < rules.terminal_min - 0.01 {
                push(
                    CheckKind::Height,
                    Severity::Warning,
                    vec![it.id],
                    tr_fmt!("{}: terminals {} are {} mm above the floor, at least {} mm required", e.name, it.tag, mm(bottom), mm(rules.terminal_min)),
                );
            }
            continue;
        }
        let actuator = it.face == Face::Door && ks.iter().any(|k| matches!(k, PushbuttonNo | PushbuttonNc | SwitchNo | SwitchNc | Disconnector3));
        if actuator && rules.actuator_max > 0.0 {
            let c = (bottom + top) / 2.0;
            if c < rules.actuator_min - 0.01 || c > rules.actuator_max + 0.01 {
                push(
                    CheckKind::Height,
                    Severity::Info,
                    vec![it.id],
                    tr_fmt!("{}: actuator {} is {} mm above the floor, recommended {} … {} mm", e.name, it.tag, mm(c), mm(rules.actuator_min), mm(rules.actuator_max)),
                );
            }
        }
        if rules.operating_max > 0.0 && (bottom < rules.operating_min - 0.01 || top > rules.operating_max + 0.01) {
            push(
                CheckKind::Height,
                Severity::Warning,
                vec![it.id],
                tr_fmt!("{}: {} is {} … {} mm above the floor, allowed {} … {} mm", e.name, it.tag, mm(bottom), mm(top), mm(rules.operating_min), mm(rules.operating_max)),
            );
        }
    }
}

#[cfg(test)]
mod tests;
