//! Structure identifiers (IEC 81346): a device tag such as `=A1++HALL1+CAB1-U1-K1`
//! names the function (`=A1`), the installation site (`++HALL1`), the location
//! (`+CAB1`), the higher-level product (`-U1`) and the device itself (`-K1`).
//!
//! Tags are stored complete. Pages carry a structure; on a page a tag is shown
//! without the aspects that equal the page's, and aspects missing in an entered tag
//! are taken from the page.
//!
//! The structure management of the project ([`Structures`]) holds the defined
//! identifiers with their descriptions, the aspects the pages use (set by a
//! [`Template`]) and the work areas that limit the pages and devices shown.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::geom::{P as Pt, R};
use crate::model::{natural_cmp, Page, PageKind, Project};
use std::collections::BTreeSet;

impl Aspect {
    pub const ALL: [Aspect; 4] = [Aspect::Function, Aspect::Installation, Aspect::Location, Aspect::Product];

    pub fn prefix(self) -> &'static str {
        match self {
            Aspect::Function => "=",
            Aspect::Installation => "++",
            Aspect::Location => "+",
            Aspect::Product => "-",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Aspect::Function => tr("Function"),
            Aspect::Installation => tr("Installation site"),
            Aspect::Location => tr("Location"),
            Aspect::Product => tr("Product"),
        }
    }

    /// Name in files and commands.
    pub fn key(self) -> &'static str {
        match self {
            Aspect::Function => "function",
            Aspect::Installation => "installation",
            Aspect::Location => "location",
            Aspect::Product => "product",
        }
    }

    /// Accepts the key or the prefix sign.
    pub fn from_key(s: &str) -> Option<Aspect> {
        let s = s.trim();
        Aspect::ALL.into_iter().find(|a| a.key() == s.to_lowercase() || a.prefix() == s)
    }

    /// `=A1`, `++HALL1` …
    pub fn format(self, id: &str) -> String {
        format!("{}{id}", self.prefix())
    }
}

impl Dt {
    /// Parses `=A1++HALL1+CAB1-U1-K1`, `+CAB1-K1`, `-K1` …; the product and device
    /// name start at the first `-`, the last `-` level is the device name.
    pub fn parse(tag: &str) -> Dt {
        let tag = tag.trim();
        let (structure, product) = match tag.find('-') {
            Some(i) => (&tag[..i], &tag[i..]),
            None if tag.starts_with(['=', '+']) => (tag, ""),
            None => ("", tag),
        };
        let mut dt = Dt::default();
        match product.rfind('-') {
            Some(i) if i > 0 => {
                dt.product = product[1..i].to_string();
                dt.name = product[i..].to_string();
            }
            _ => dt.name = product.to_string(),
        }
        let mut aspect = None;
        let mut buf = String::new();
        let mut chars = structure.chars().peekable();
        loop {
            let c = chars.next();
            let next = match c {
                Some('=') => Some(Aspect::Function),
                Some('+') if chars.peek() == Some(&'+') => {
                    chars.next();
                    Some(Aspect::Installation)
                }
                Some('+') => Some(Aspect::Location),
                Some(c) => {
                    buf.push(c);
                    continue;
                }
                None => None,
            };
            match aspect {
                Some(a) => *dt.get_mut(a) = std::mem::take(&mut buf),
                None => buf.clear(),
            }
            if c.is_none() {
                break;
            }
            aspect = next;
        }
        dt
    }

    /// Parses a structure without device name (`=A1+CAB1-U1-A2`): all `-` levels
    /// belong to the product.
    pub fn parse_structure(s: &str) -> Dt {
        let mut dt = Dt::parse(s);
        if let Some(level) = std::mem::take(&mut dt.name).strip_prefix('-') {
            dt.product = if dt.product.is_empty() { level.to_string() } else { format!("{}-{level}", dt.product) };
        }
        dt
    }

    /// Structure of a page (no device name).
    pub fn of_page(page: &Page) -> Dt {
        Dt {
            function: page.function.clone(),
            installation: page.installation.clone(),
            location: page.location.clone(),
            product: page.product.clone(),
            name: String::new(),
        }
    }

    /// Sets the structure of a page (cleaned, see [`clean_aspect`]).
    pub fn apply_to(&self, page: &mut Page) {
        page.function = clean_aspect(&self.function);
        page.installation = clean_aspect(&self.installation);
        page.location = clean_aspect(&self.location);
        page.product = clean_aspect(&self.product);
    }

    pub fn get(&self, a: Aspect) -> &str {
        match a {
            Aspect::Function => &self.function,
            Aspect::Installation => &self.installation,
            Aspect::Location => &self.location,
            Aspect::Product => &self.product,
        }
    }

    pub fn get_mut(&mut self, a: Aspect) -> &mut String {
        match a {
            Aspect::Function => &mut self.function,
            Aspect::Installation => &mut self.installation,
            Aspect::Location => &mut self.location,
            Aspect::Product => &mut self.product,
        }
    }

    /// `=A1++HALL1+CAB1-U1` (empty without structure).
    pub fn structure(&self) -> String {
        Aspect::ALL.into_iter().filter(|a| !self.get(*a).is_empty()).map(|a| a.format(self.get(a))).collect()
    }

    pub fn tag(&self) -> String {
        format!("{}{}", self.structure(), self.name)
    }

    /// Complete tag of a tag entered in this structure: missing aspects come from it.
    pub fn complete(&self, entered: &str) -> String {
        let mut dt = Dt::parse(entered);
        for a in Aspect::ALL {
            if dt.get(a).is_empty() {
                *dt.get_mut(a) = self.get(a).to_string();
            }
        }
        dt.tag()
    }

    /// Shortest form of a complete tag in this structure: equal aspects are left out.
    pub fn relative(&self, tag: &str) -> String {
        let mut dt = Dt::parse(tag);
        for a in Aspect::ALL {
            if dt.get(a) == self.get(a) {
                dt.get_mut(a).clear();
            }
        }
        dt.tag()
    }
}

/// Removes prefix signs and blanks from an entered aspect (`"=A1 "` → `"A1"`).
pub fn clean_aspect(s: &str) -> String {
    s.trim().trim_start_matches(['=', '+', '-']).replace(' ', "")
}

/// A structure identifier of `aspect` as entered (`CAB1`, `+CAB1`), without its
/// prefix. Blanks and the signs that separate the aspects of a tag are refused
/// instead of dropped; a product may have levels (`U1-A2`).
pub fn check_identifier(aspect: Aspect, s: &str) -> Result<String, String> {
    let id = s.trim().trim_start_matches(['=', '+', '-']);
    let bad = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '=' | '+' | '/' | ':') || (c == '-' && aspect != Aspect::Product);
    if id.chars().any(bad) {
        return Err(tr_fmt!("Structure identifier {} must not contain blanks or the signs = + - / :", s.trim()));
    }
    Ok(id.to_string())
}

/// Structure of a page, e.g. `=A1+CAB1`.
pub fn page_structure(page: &Page) -> String {
    Dt::of_page(page).structure()
}

/// Splits the device name of a tag (`-K12`, `-1CR2`) into the identifying letters
/// (`K`, `CR`); None if it has none.
pub fn letters_of(name: &str) -> Option<String> {
    let s = name.trim_start_matches('-').trim_start_matches(|c: char| c.is_ascii_digit());
    let l: String = s.chars().take_while(|c| c.is_alphabetic()).collect();
    (!l.is_empty()).then_some(l)
}

/// Page name with structure, e.g. `=A1+CAB1/3` (just `3` without structure).
pub fn page_full_name(page: &Page) -> String {
    let s = page_structure(page);
    if s.is_empty() {
        page.name.clone()
    } else {
        format!("{s}/{}", page.name)
    }
}

/// Complete tag of a tag entered on a page: missing aspects come from the page.
pub fn complete(page: &Page, entered: &str) -> String {
    Dt::of_page(page).complete(entered)
}

/// Shortest form of a complete tag on a page: aspects equal to the page's are left out.
pub fn relative(page: &Page, tag: &str) -> String {
    Dt::of_page(page).relative(tag)
}

impl StructureBox {
    pub fn rect(&self) -> R {
        R::from_points(self.a, self.b)
    }

    /// Membership rule: a symbol belongs to the box when its insertion point lies in
    /// the rectangle (edges included).
    pub fn contains(&self, q: Pt) -> bool {
        self.rect().contains(q)
    }

    /// The identifiers the box sets, e.g. `+FIELD` (empty if it sets none).
    pub fn identifiers(&self) -> String {
        self.structure.structure()
    }

    fn area(&self) -> f32 {
        let r = self.rect();
        r.width() * r.height()
    }
}

/// The structure boxes of a page that contain `q`, outermost (largest) first.
pub fn boxes_at(page: &Page, q: Pt) -> Vec<&StructureBox> {
    let mut v: Vec<&StructureBox> = page.structure_boxes.iter().filter(|b| b.contains(q)).collect();
    v.sort_by(|a, b| b.area().total_cmp(&a.area()));
    v
}

/// Structure in effect at `q`: the page's, with the aspects set by the structure
/// boxes containing `q`; of nested boxes the innermost wins.
pub fn scope(page: &Page, q: Pt) -> Dt {
    let mut dt = Dt::of_page(page);
    for b in boxes_at(page, q) {
        for a in Aspect::ALL {
            if !b.structure.get(a).is_empty() {
                *dt.get_mut(a) = b.structure.get(a).to_string();
            }
        }
    }
    dt
}

/// Structure inside a box (not counting the boxes nested in it): the page's with the
/// aspects of the boxes enclosing it and its own.
pub fn box_scope(page: &Page, b: &StructureBox) -> Dt {
    let mut around = page.clone();
    around.structure_boxes.retain(|o| o.id == b.id || (o.rect().contains_rect(&b.rect()) && o.area() > b.area()));
    scope(&around, b.rect().center())
}

/// Structure around a box: [`box_scope`] without the box's own aspects.
pub fn around_box(page: &Page, b: &StructureBox) -> Dt {
    let mut around = page.clone();
    around.structure_boxes.retain(|o| o.id != b.id && o.rect().contains_rect(&b.rect()) && o.area() > b.area());
    scope(&around, b.rect().center())
}

/// The innermost structure box at `q` that sets `aspect`.
pub fn box_for(page: &Page, q: Pt, aspect: Aspect) -> Option<&StructureBox> {
    boxes_at(page, q).into_iter().rev().find(|b| !b.structure.get(aspect).is_empty())
}

/// Complete tag of a tag entered at `q` on a page (see [`scope`]).
pub fn complete_at(page: &Page, q: Pt, entered: &str) -> String {
    scope(page, q).complete(entered)
}

/// Shortest form of a complete tag at `q` on a page: aspects equal to the structure
/// in effect there are left out.
pub fn relative_at(page: &Page, q: Pt, tag: &str) -> String {
    scope(page, q).relative(tag)
}

/// Where an aspect of a device tag comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// Inherited from the page structure.
    Page,
    /// Inherited from a structure box (its id).
    Box(u64),
    /// Set on the device itself, differing from the structure around it.
    Device,
}

/// The aspects of a tag placed at `q` with their values and where they come from:
/// an aspect equal to the structure in effect there is inherited, any other is the
/// device's own.
pub fn sources(page: &Page, q: Pt, tag: &str) -> Vec<(Aspect, String, Source)> {
    let (dt, here) = (Dt::parse(tag), scope(page, q));
    Aspect::ALL
        .into_iter()
        .filter(|a| !dt.get(*a).is_empty())
        .map(|a| {
            let source = if dt.get(a) != here.get(a) {
                Source::Device
            } else {
                box_for(page, q, a).map_or(Source::Page, |b| Source::Box(b.id))
            };
            (a, dt.get(a).to_string(), source)
        })
        .collect()
}

/// Pairs of structure boxes of a page that overlap without one lying inside the
/// other: their common area has no clear structure.
pub fn overlapping(page: &Page) -> Vec<(u64, u64)> {
    let boxes = &page.structure_boxes;
    let mut v = Vec::new();
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            let (ra, rb) = (a.rect(), b.rect());
            let apart = ra.max.x <= rb.min.x || rb.max.x <= ra.min.x || ra.max.y <= rb.min.y || rb.max.y <= ra.min.y;
            if !apart && !ra.contains_rect(&rb) && !rb.contains_rect(&ra) {
                v.push((a.id, b.id));
            }
        }
    }
    v
}

impl Structures {
    pub fn is_default(&self) -> bool {
        *self == Structures::default()
    }

    /// Aspects shown for pages, in tag order.
    pub fn page_aspects(&self) -> Vec<Aspect> {
        if self.aspects.is_empty() {
            vec![Aspect::Function, Aspect::Location]
        } else {
            Aspect::ALL.into_iter().filter(|a| self.aspects.contains(a)).collect()
        }
    }

    pub fn entry(&self, aspect: Aspect, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.aspect == aspect && e.id == id)
    }

    /// Defines an identifier or changes its description.
    pub fn define(&mut self, aspect: Aspect, id: &str, description: &str) {
        let id = clean_aspect(id);
        match self.entries.iter_mut().find(|e| e.aspect == aspect && e.id == id) {
            Some(e) => e.description = description.to_string(),
            None => self.entries.push(Entry { aspect, id, description: description.to_string() }),
        }
        self.sort();
    }

    pub fn remove(&mut self, aspect: Aspect, id: &str) -> bool {
        let n = self.entries.len();
        self.entries.retain(|e| !(e.aspect == aspect && e.id == id));
        self.entries.len() != n
    }

    fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.aspect.cmp(&b.aspect).then_with(|| natural_cmp(&a.id, &b.id)));
    }

    pub fn work_area(&self, name: &str) -> Option<&WorkArea> {
        self.work_areas.iter().find(|w| w.name == name)
    }

    /// Adds or replaces a work area.
    pub fn set_work_area(&mut self, area: WorkArea) {
        match self.work_areas.iter_mut().find(|w| w.name == area.name) {
            Some(w) => *w = area,
            None => self.work_areas.push(area),
        }
    }

    /// File of a user template: aspects, identifiers and work areas.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Structures, String> {
        serde_json::from_str(text).map_err(|e| tr_fmt!("Not a structure template: {}", e))
    }

    /// Takes the aspects of a template file (if it has any), adds its identifiers
    /// (descriptions of defined ones stay) and work areas. Returns the number of
    /// identifiers added.
    pub fn merge(&mut self, other: &Structures) -> usize {
        if !other.aspects.is_empty() {
            self.aspects = other.aspects.clone();
        }
        let mut n = 0;
        for e in &other.entries {
            if self.entry(e.aspect, &e.id).is_none() {
                self.define(e.aspect, &e.id, &e.description);
                n += 1;
            }
        }
        for w in &other.work_areas {
            if self.work_area(&w.name).is_none() {
                self.work_areas.push(w.clone());
            }
        }
        n
    }
}

impl WorkArea {
    pub fn contains(&self, dt: &Dt) -> bool {
        Aspect::ALL.into_iter().all(|a| matches(self.filter.get(a), dt.get(a)))
    }

    /// Schematic pages in the work area; other pages (cover, reports, mounting
    /// layouts) are always shown.
    pub fn shows_page(&self, page: &Page) -> bool {
        page.kind != PageKind::Schematic || self.contains(&Dt::of_page(page))
    }

    pub fn shows_device(&self, tag: &str) -> bool {
        self.contains(&Dt::parse(tag))
    }
}

/// Identifier pattern (see [`WorkArea::filter`]).
pub fn matches(pattern: &str, id: &str) -> bool {
    let pattern = clean_aspect(pattern);
    if pattern.is_empty() {
        return true;
    }
    if !pattern.contains('*') {
        return id == pattern || id.strip_prefix(pattern.as_str()).is_some_and(|rest| rest.starts_with(['.', '-']));
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    let Some(mut rest) = id.strip_prefix(first) else { return false };
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// Built-in structure template: the aspects the pages use and identifiers to start with.
pub struct Template {
    pub name: &'static str,
    pub aspects: &'static [Aspect],
    /// Aspect, identifier, description (translated when applied).
    pub entries: &'static [(Aspect, &'static str, &'static str)],
}

use Aspect::{Function as F, Installation as I, Location as L, Product as P};
pub use crate::model::structure::{Aspect, BoxCorner, BoxLine, Dt, Entry, StructureBox, Structures, WorkArea};

pub static TEMPLATES: &[Template] = &[
    Template { name: tr_const!("Function and location"), aspects: &[F, L], entries: &[] },
    Template { name: tr_const!("Installation site and location"), aspects: &[I, L], entries: &[] },
    Template { name: tr_const!("Function, installation site and location"), aspects: &[F, I, L], entries: &[] },
    Template { name: tr_const!("All aspects (with product)"), aspects: &[F, I, L, P], entries: &[] },
    Template {
        name: tr_const!("Machine (example identifiers)"),
        aspects: &[F, L],
        entries: &[
            (F, "POWER", tr_const!("Power supply")),
            (F, "CTRL", tr_const!("Control")),
            (F, "DRIVE", tr_const!("Drives")),
            (F, "SAFETY", tr_const!("Safety")),
            (L, "CAB1", tr_const!("Control cabinet")),
            (L, "DESK", tr_const!("Operator desk")),
            (L, "FIELD", tr_const!("Field devices")),
        ],
    },
    Template {
        name: tr_const!("Process plant (example identifiers)"),
        aspects: &[F, I, L],
        entries: &[
            (F, "SUPPLY", tr_const!("Supply")),
            (F, "PUMP", tr_const!("Pumps")),
            (F, "TANK", tr_const!("Tanks")),
            (I, "HALL1", tr_const!("Hall 1")),
            (I, "OUTDOOR", tr_const!("Outdoor area")),
            (L, "CAB1", tr_const!("Control cabinet")),
            (L, "FIELD", tr_const!("Field devices")),
        ],
    },
];

pub fn template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name || tr(t.name) == name)
}

/// Sets the aspects of a template and adds its identifiers. Returns the number of
/// identifiers added.
pub fn apply_template(s: &mut Structures, t: &Template) -> usize {
    let other = Structures {
        aspects: t.aspects.to_vec(),
        entries: t.entries.iter().map(|(aspect, id, d)| Entry { aspect: *aspect, id: id.to_string(), description: tr(d).to_string() }).collect(),
        work_areas: Vec::new(),
    };
    s.merge(&other)
}

/// Every device tag of the project (symbols, cables, harnesses, panel layout, device data).
pub fn device_tags(prj: &Project) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = prj.all_tags().into_iter().collect();
    tags.extend(prj.cables.iter().map(|c| c.tag.clone()));
    tags.extend(prj.harnesses.iter().map(|h| h.tag.clone()));
    tags.extend(prj.layout.enclosures.iter().flat_map(|e| e.items.iter()).map(|i| i.tag.clone()));
    tags.extend(prj.devices.keys().cloned());
    tags.remove("");
    tags
}

/// An identifier of the structure management: defined, used or both.
#[derive(Clone, Debug, PartialEq)]
pub struct Usage {
    pub aspect: Aspect,
    pub id: String,
    pub description: String,
    pub defined: bool,
    /// Pages and structure boxes with this identifier.
    pub pages: usize,
    /// Devices with this identifier.
    pub devices: usize,
}

/// Defined and used identifiers, sorted by aspect and identifier.
pub fn overview(prj: &Project) -> Vec<Usage> {
    let mut rows: Vec<Usage> = prj
        .structures
        .entries
        .iter()
        .map(|e| Usage { aspect: e.aspect, id: e.id.clone(), description: e.description.clone(), defined: true, pages: 0, devices: 0 })
        .collect();
    let mut row = |a: Aspect, id: &str| -> usize {
        match rows.iter().position(|r| r.aspect == a && r.id == id) {
            Some(i) => i,
            None => {
                rows.push(Usage { aspect: a, id: id.to_string(), description: String::new(), defined: false, pages: 0, devices: 0 });
                rows.len() - 1
            }
        }
    };
    let mut counts = Vec::new();
    for pg in &prj.pages {
        for dt in std::iter::once(Dt::of_page(pg)).chain(pg.structure_boxes.iter().map(|b| b.structure.clone())) {
            for a in Aspect::ALL.into_iter().filter(|a| !dt.get(*a).is_empty()) {
                counts.push((row(a, dt.get(a)), true));
            }
        }
    }
    for tag in device_tags(prj) {
        let dt = Dt::parse(&tag);
        for a in Aspect::ALL.into_iter().filter(|a| !dt.get(*a).is_empty()) {
            counts.push((row(a, dt.get(a)), false));
        }
    }
    for (i, page) in counts {
        if page {
            rows[i].pages += 1;
        } else {
            rows[i].devices += 1;
        }
    }
    rows.sort_by(|a, b| a.aspect.cmp(&b.aspect).then_with(|| natural_cmp(&a.id, &b.id)));
    rows
}

/// Defines every identifier in use that is not defined yet. Returns their number.
pub fn define_used(prj: &mut Project) -> usize {
    let new: Vec<Usage> = overview(prj).into_iter().filter(|u| !u.defined).collect();
    for u in &new {
        prj.structures.define(u.aspect, &u.id, "");
    }
    new.len()
}

/// Removes the definitions of identifiers no page or device uses. Returns their number.
pub fn remove_unused(prj: &mut Project) -> usize {
    let unused: Vec<Usage> = overview(prj).into_iter().filter(|u| u.defined && u.pages == 0 && u.devices == 0).collect();
    for u in &unused {
        prj.structures.remove(u.aspect, &u.id);
    }
    unused.len()
}

/// Renames an identifier in the whole project: pages, structure boxes, device tags (with all their
/// data, see [`crate::ops::rename_device`]), cables, harnesses, definition and work
/// areas. Returns the number of renamed pages and devices; fails without changes
/// when a renamed device tag exists already.
pub fn rename(prj: &mut Project, aspect: Aspect, old: &str, new: &str) -> Result<usize, String> {
    let (old, new) = (clean_aspect(old), clean_aspect(new));
    if old.is_empty() || new.is_empty() {
        return Err(tr("The identifier must not be empty").into());
    }
    if old == new {
        return Ok(0);
    }
    let existing = device_tags(prj);
    let mut renames = Vec::new();
    for tag in &existing {
        let mut dt = Dt::parse(tag);
        if dt.get(aspect) == old {
            *dt.get_mut(aspect) = new.clone();
            let to = dt.tag();
            if existing.contains(&to) {
                return Err(tr_fmt!("{} exists already", to));
            }
            renames.push((tag.clone(), to));
        }
    }
    let mut n = renames.len();
    for pg in &mut prj.pages {
        let mut dt = Dt::of_page(pg);
        if dt.get(aspect) == old {
            *dt.get_mut(aspect) = new.clone();
            dt.apply_to(pg);
            n += 1;
        }
        for b in pg.structure_boxes.iter_mut().filter(|b| b.structure.get(aspect) == old) {
            *b.structure.get_mut(aspect) = new.clone();
        }
    }
    for (from, to) in &renames {
        crate::ops::rename_device(prj, from, to);
        if let Some(c) = prj.cables.iter_mut().find(|c| c.tag == *from) {
            c.tag = to.clone();
            crate::ops::rename_cable(prj, from, to);
        }
        for h in prj.harnesses.iter_mut().filter(|h| h.tag == *from) {
            h.tag = to.clone();
        }
    }
    let s = &mut prj.structures;
    if let Some(e) = s.entries.iter().position(|e| e.aspect == aspect && e.id == old) {
        let e = s.entries.remove(e);
        if s.entry(aspect, &new).is_none() {
            s.define(aspect, &new, &e.description);
        }
    }
    for w in &mut s.work_areas {
        if w.filter.get(aspect) == old {
            *w.filter.get_mut(aspect) = new.clone();
        }
    }
    Ok(n)
}

/// Identifiers in use that the structure management does not define (only checked
/// once it defines any).
pub fn problems(prj: &Project) -> Vec<String> {
    if prj.structures.entries.is_empty() {
        return Vec::new();
    }
    overview(prj)
        .into_iter()
        .filter(|u| !u.defined)
        .map(|u| tr_fmt!("Structure identifier {} is not defined in the structure management", u.aspect.format(&u.id)))
        .collect()
}

#[cfg(test)]
mod tests;
