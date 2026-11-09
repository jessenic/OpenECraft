//! Assemblies and 3D macros of the panel layout: items grouped into an assembly move
//! and are selected together; a 3D macro stores a group of items (on one or several
//! faces) to insert it again: items without an anchor relative to the insertion
//! point, anchored items at their place on their face. Device items of a macro take
//! an unplaced device with the same part, otherwise they become accessories.

use super::{apply_one_anchor, part_by_number, snap_to_rail, Align, Anchor, Face, ItemKind, PanelItem, DUCT_PART, DUCT_PROFILE, RAIL_PART, RAIL_PROFILE};
use crate::geom::P;
use crate::model::Project;
use openecraft_i18n::{tr, tr_const};
use std::collections::HashSet;
pub use crate::model::panel::{ItemGroup, PanelMacro};

/// Names of the built-in macros (English; translated where shown).
const BUILTIN: [&str; 5] = [
    tr_const!("Duct frame with rails"),
    tr_const!("Filter fan with outlet filter"),
    tr_const!("Cable entry 5 x M25"),
    tr_const!("Door handle and document holder"),
    tr_const!("Enclosure light"),
];

impl PanelMacro {
    /// Name to show: built-in macros are translated.
    pub fn title(&self) -> &str {
        BUILTIN.iter().find(|n| **n == self.name).map(|n| tr(n)).unwrap_or(&self.name)
    }

    pub fn is_builtin(&self) -> bool {
        BUILTIN.contains(&self.name.as_str())
    }
}

fn anchored(kind: ItemKind, part: &str, face: Face, (w, h, d): (f32, f32, f32), z: f32, x: Align, y: Align) -> PanelItem {
    PanelItem { kind, part: part.into(), w, h, d, z, face, anchor: Anchor { x, y }, ..Default::default() }
}

fn part_size(prj: &Project, part: &str) -> (f32, f32, f32, f32) {
    let pd = part_by_number(prj, part);
    let [w, h, d] = pd.and_then(|p| p.size()).unwrap_or([40.0, 40.0, 40.0]);
    let outside = pd.and_then(|p| super::tech_number(p, "Outside")).unwrap_or(0.0);
    (w, h, d, -outside)
}

fn accessory(prj: &Project, part: &str, face: Face, x: Align, y: Align) -> PanelItem {
    let (w, h, d, z) = part_size(prj, part);
    anchored(ItemKind::Accessory, part, face, (w, h, d), z, x, y)
}

/// The macros that come with the program.
pub fn builtin_macros(prj: &Project) -> Vec<PanelMacro> {
    use Align::*;
    let (dw, dd) = DUCT_PROFILE;
    let (rw, rd) = RAIL_PROFILE;
    let duct = |x, y, horizontal: bool| {
        let size = if horizontal { (200.0, dw, dd) } else { (dw, 200.0, dd) };
        anchored(ItemKind::Duct, DUCT_PART, Face::Plate, size, 0.0, x, y)
    };
    let rail = |y| anchored(ItemKind::Rail, RAIL_PART, Face::Plate, (200.0, rw, rd), 0.0, Stretch(dw + 20.0, dw + 20.0), y);
    let glands = (0..5)
        .map(|k| {
            let (w, h, d, z) = part_size(prj, "GEN.GLAND-M25");
            PanelItem { kind: ItemKind::Accessory, part: "GEN.GLAND-M25".into(), x: 50.0 * k as f32, w, h, d, z, face: Face::Bottom, ..Default::default() }
        })
        .collect();
    let m = |name: &str, description: &str, face, items| PanelMacro { name: name.into(), description: description.into(), face, items };
    vec![
        m(
            BUILTIN[0],
            "Cable ducts along the plate edges and across the middle, two mounting rails",
            Face::Plate,
            vec![
                duct(Stretch(0.0, 0.0), Start(0.0), true),
                duct(Stretch(0.0, 0.0), End(0.0), true),
                duct(Start(0.0), Stretch(dw, dw), false),
                duct(End(0.0), Stretch(dw, dw), false),
                duct(Stretch(dw, dw), Center(0.0), true),
                rail(Start(dw + 90.0)),
                rail(End(dw + 90.0)),
            ],
        ),
        m(
            BUILTIN[1],
            "Filter fan low on the left side, outlet filter high on the right side",
            Face::Left,
            vec![accessory(prj, "GEN.FAN-FILTER-120", Face::Left, Center(0.0), End(150.0)), accessory(prj, "GEN.FAN-OUTLET-120", Face::Right, Center(0.0), Start(150.0))],
        ),
        m(BUILTIN[2], "Five cable glands M25 in a row, 50 mm apart", Face::Bottom, glands),
        m(
            BUILTIN[3],
            "Swing handle on the opening side, document holder on the inside of the door",
            Face::Door,
            vec![accessory(prj, "GEN.HANDLE-SWING", Face::Door, End(30.0), Center(0.0)), accessory(prj, "GEN.DOC-HOLDER-A4", Face::Door, Center(0.0), End(60.0))],
        ),
        m(BUILTIN[4], "LED light centred under the roof", Face::Roof, vec![accessory(prj, "GEN.LIGHT-LED-350", Face::Roof, Center(0.0), Center(0.0))]),
    ]
}

/// All macros: the project's own, then the built-in ones.
pub fn all_macros(prj: &Project) -> Vec<PanelMacro> {
    let mut v = prj.layout.macros.clone();
    v.extend(builtin_macros(prj));
    v
}

/// The ids of the items of a group.
pub fn group_members(e: &super::Enclosure, group: u64) -> Vec<u64> {
    if group == 0 {
        return Vec::new();
    }
    e.items.iter().filter(|i| i.group == group).map(|i| i.id).collect()
}

/// A macro from items of an enclosure: the items on the face of the first one
/// relative to their common top left corner, items on other faces anchored where
/// they are. Device items keep their part, not their tag.
pub fn macro_from_items(e: &super::Enclosure, ids: &[u64], name: &str) -> Option<PanelMacro> {
    let items: Vec<&PanelItem> = e.items.iter().filter(|i| ids.contains(&i.id)).collect();
    let face = items.first()?.face;
    let min_x = items.iter().filter(|i| i.face == face && i.anchor.x == Align::Free).map(|i| i.x).fold(f32::INFINITY, f32::min);
    let min_y = items.iter().filter(|i| i.face == face && i.anchor.y == Align::Free).map(|i| i.y).fold(f32::INFINITY, f32::min);
    let (min_x, min_y) = (if min_x.is_finite() { min_x } else { 0.0 }, if min_y.is_finite() { min_y } else { 0.0 });
    let local:std::collections::HashMap<_,_>=items.iter().enumerate().map(|(n,item)|(item.id,n as u64+1)).collect();
    let has_mounts=items.iter().any(|item|item.mounted_on!=0);
    let out = items
        .into_iter()
        .map(|i| {
            let mut it = PanelItem { id: if has_mounts {local[&i.id]} else {0}, tag: String::new(), owner_device:String::new(),group: 0, ..i.clone() };
            if it.mounted_on!=0 {it.mounted_on=local.get(&it.mounted_on).copied().unwrap_or(0);it.mounting_explicit=true;}
            if it.face == face {
                it.x -= if it.anchor.x == Align::Free { min_x } else { 0.0 };
                it.y -= if it.anchor.y == Align::Free { min_y } else { 0.0 };
            } else {
                // On another face: kept where it is.
                let fix = |a: Align| if a == Align::Free { Align::Start(0.0) } else { a };
                let anchor = Anchor { x: fix(it.anchor.x), y: fix(it.anchor.y) };
                it.anchor = anchor.capture(&it, e.face_size(it.face));
            }
            it
        })
        .collect();
    Some(PanelMacro { name: name.into(), description: String::new(), face, items: out })
}

/// Inserts a macro into an enclosure with the top left corner of its relative items
/// at `at` on `face` (anchored items go to their place). The items become one group.
/// Returns the ids of the new items.
pub fn insert_macro(prj: &mut Project, enc: usize, m: &PanelMacro, face: Face, at: P) -> Vec<u64> {
    if enc >= prj.layout.enclosures.len() {
        return Vec::new();
    }
    let group = prj.new_id();
    prj.layout.enclosures[enc].groups.push(ItemGroup { id: group, name: m.title().to_string(), source: m.name.clone() });
    let mut taken: HashSet<String> = HashSet::new();
    let mut ids = Vec::new();
    let new_ids:Vec<_>=m.items.iter().map(|_|prj.new_id()).collect();
    let mapping:std::collections::HashMap<_,_>=m.items.iter().zip(&new_ids).filter(|(src,_)|src.id!=0).map(|(src,id)|(src.id,*id)).collect();
    for (src,id) in m.items.iter().zip(new_ids) {
        let mut it = PanelItem { id, group, ..src.clone() };
        if it.mounted_on!=0 {it.mounted_on=mapping.get(&it.mounted_on).copied().unwrap_or(0);it.mounting_explicit=true;}
        if it.face == m.face {
            it.face = face;
            if it.anchor.x == Align::Free {
                it.x += at.x;
            }
            if it.anchor.y == Align::Free {
                it.y += at.y;
            }
        }
        if it.kind == ItemKind::Device {
            let free = super::devices_to_place(prj).into_iter().find(|d| d.part == it.part && !taken.contains(&d.tag));
            match free {
                Some(d) => {
                    taken.insert(d.tag.clone());
                    it.tag = d.tag;
                    if !it.rail_mount_override {it.rail_mount = part_by_number(prj, &d.part).map(|p| p.effective_rail_mount()).unwrap_or_default();}
                }
                None => {
                    it.kind = ItemKind::Accessory;
                    it.tag.clear();
                }
            }
        }
        let e = &mut prj.layout.enclosures[enc];
        apply_one_anchor(e, &mut it);
        if !it.mounting_explicit {snap_to_rail(e, &mut it);}
        ids.push(it.id);
        e.items.push(it);
    }
    ids
}

#[cfg(test)]
mod tests;
