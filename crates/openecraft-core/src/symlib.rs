//! Project symbol library: user defined symbols (symbol editor). They are stored in
//! the project, shared as library files and placed like built-in symbols; placed
//! instances refer to them by key; a project's [`SymbolCache`] resolves them for
//! [`Project::symbol`](crate::model::Project::symbol).

use openecraft_i18n::{tr, tr_fmt};
use crate::geom::{dist_to_segment, p, P, GRID};
use crate::model::Project;
use crate::symbols::{self, Anchor, Category, FieldKind, Func, PinDef, Prim, SymbolDef, SymbolRef};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
pub use crate::model::symlib::{UserPin, UserSymbol};
pub mod ema;

/// Keys of user symbols start with this prefix.
pub const PREFIX: &str = "user:";
/// Snap of graphics in the symbol editor (pins snap to the 4 mm grid).
pub const DRAW_SNAP: f32 = 0.5;

impl Default for UserSymbol {
    fn default() -> Self {
        UserSymbol {
            key: String::new(),
            ema_revision: None,
            plc_channel: None,
            name: "New symbol".into(),
            library: "User".into(),
            letter: "A".into(),
            func: Func::Main { mirror: false },
            default_label: String::new(),
            auto_number: false,
            show_tag: true,
            show_pin_labels: true,
            prims: Vec::new(),
            pins: Vec::new(),
            bridges: Vec::new(),
        }
    }
}

/// File format for exchanging symbols between projects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolLibrary {
    pub symbols: Vec<UserSymbol>,
}

impl SymbolLibrary {
    /// A symbol library file, or a file with a single symbol. (Every object would parse
    /// as an empty library because of the defaults, so the key decides.)
    pub fn from_json(text: &str) -> Result<SymbolLibrary, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value.get("symbols").is_some() {
            serde_json::from_value(value).map_err(|e| e.to_string())
        } else {
            serde_json::from_value::<UserSymbol>(value).map(|u| SymbolLibrary { symbols: vec![u] }).map_err(|e| e.to_string())
        }
    }
}

/// Interned copy of a string: user symbols need `&'static str` like the built-in
/// definitions; equal strings share one allocation, so editing leaks nothing new.
fn intern(s: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    match names.get(s) {
        Some(n) => n,
        None => {
            let n: &'static str = Box::leak(s.to_string().into_boxed_str());
            names.insert(n);
            n
        }
    }
}

/// Definition of a user symbol as the editor and the netlist see it.
pub(crate) fn to_def(u: &UserSymbol) -> SymbolDef {
    SymbolDef {
        key: intern(&u.key),
        name: intern(&u.name),
        category: Category::User,
        library: intern(&u.library),
        letter: intern(&u.letter),
        func: u.func,
        plc_channel: u.plc_channel.clone(),
        prims: u.prims.clone(),
        pins: u.pins.iter().map(|x| PinDef { pos: x.pos, label: intern(&x.label) }).collect(),
        bridges: u.bridges.iter().copied().filter(|(a, b)| *a < u.pins.len() && *b < u.pins.len()).collect(),
        default_label: intern(&u.default_label),
        auto_number: u.auto_number,
        show_tag: u.show_tag && u.func.has_tag(),
        show_pin_labels: u.show_pin_labels,
        bounds: symbols::bounds_of(&u.prims, u.pins.iter().map(|x| x.pos)),
    }
}

fn hash_point(q: P, h: &mut impl Hasher) {
    q.x.to_bits().hash(h);
    q.y.to_bits().hash(h);
}

fn hash_prim(prim: &Prim, h: &mut impl Hasher) {
    std::mem::discriminant(prim).hash(h);
    match prim {
        Prim::Line { a, b, dashed } => {
            hash_point(*a, h);
            hash_point(*b, h);
            dashed.hash(h);
        }
        Prim::Poly { pts, closed, fill } => {
            for q in pts {
                hash_point(*q, h);
            }
            (closed, fill).hash(h);
        }
        Prim::Circle { c, r, fill, color } => {
            color.hash(h);
            hash_point(*c, h);
            r.to_bits().hash(h);
            fill.hash(h);
        }
        Prim::Path {pts,closed,fill,color,width,dashed} => {
            for point in pts {hash_point(*point,h);}
            (closed,fill,color,width.to_bits(),dashed).hash(h);
        }
        Prim::Text { pos, text, size, anchor, angle, color } => {
            color.hash(h);
            angle.map(f32::to_bits).hash(h);
            hash_point(*pos, h);
            text.hash(h);
            size.to_bits().hash(h);
            std::mem::discriminant(anchor).hash(h);
        }
        Prim::Field { pos, size, anchor, kind } => {
            hash_point(*pos, h);
            size.to_bits().hash(h);
            std::mem::discriminant(anchor).hash(h);
            std::mem::discriminant(kind).hash(h);
        }
    }
}

/// Fingerprint of a symbol list: changes whenever a symbol is added, removed or
/// edited (so that a project's symbol table is rebuilt exactly then).
pub fn fingerprint(list: &[UserSymbol]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    list.len().hash(&mut h);
    for u in list {
        (&u.key, &u.name, &u.library, &u.letter, &u.default_label, u.auto_number, u.show_tag, u.show_pin_labels).hash(&mut h);
        format!("{:?}", u.func).hash(&mut h);
        if let Some(channel) = &u.plc_channel {
            // Keep revisions of digital definitions imported by earlier versions.
            (&channel.family, channel.input, channel.index, &channel.designation, &channel.address, &channel.function_text).hash(&mut h);
            if let Some(pin) = &channel.digital_terminal { pin.hash(&mut h); }
            if let Some(signal) = &channel.analog {
                signal.hash(&mut h);
            }
        }
        for p in &u.prims {
            hash_prim(p, &mut h);
        }
        for pin in &u.pins {
            hash_point(pin.pos, &mut h);
            pin.label.hash(&mut h);
        }
        u.bridges.hash(&mut h);
    }
    h.finish()
}

/// User symbol definitions by key.
type Defs = Arc<HashMap<String, Arc<SymbolDef>>>;

/// The user symbols of one project by key, rebuilt when the list changes. A
/// [`Project`] carries one; clones of a project share it until their symbols differ.
#[derive(Clone, Default)]
pub struct SymbolCache(Arc<Mutex<Option<(crate::model::symlib::SymbolList, Defs)>>>);

impl SymbolCache {
    /// The symbols of a project with the user symbols `list`, as a snapshot.
    pub fn table(&self, list: &crate::model::symlib::SymbolList) -> Symbols {
        if list.is_empty() {
            return Symbols::default();
        }
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if cache.as_ref().is_none_or(|(have, _)| !have.same_snapshot(list)) {
            let defs = list.iter().map(|u| (u.key.clone(), Arc::new(to_def(u)))).collect();
            *cache = Some((list.clone(), Arc::new(defs)));
        }
        Symbols { user: cache.as_ref().map(|(_, defs)| defs.clone()) }
    }
}

/// The symbols a project can place: the built-in ones and its user symbols. A cheap
/// snapshot without a borrow of the project (for loops that change the project).
#[derive(Clone, Default)]
pub struct Symbols {
    user: Option<Defs>,
}

impl Symbols {
    pub fn get(&self, key: &str) -> Option<SymbolRef> {
        if let Some(d) = symbols::builtin(key) {
            return Some(SymbolRef::Builtin(d));
        }
        if !key.starts_with(PREFIX) {
            return None;
        }
        self.user.as_ref()?.get(key).cloned().map(SymbolRef::User)
    }
}

impl PartialEq for SymbolCache {
    /// Caches never make projects differ.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for SymbolCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SymbolCache")
    }
}

/// A new unique key for a symbol name, e.g. `user:motor_starter_2`.
pub fn new_key(existing: &[UserSymbol], name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let base = format!("{PREFIX}{}", if slug.is_empty() { "symbol" } else { &slug });
    let mut key = base.clone();
    let mut n = 2;
    while existing.iter().any(|u| u.key == key) {
        key = format!("{base}_{n}");
        n += 1;
    }
    key
}

/// Editable copy of a built-in (or other) symbol.
pub fn from_def(def: &SymbolDef, existing: &[UserSymbol]) -> UserSymbol {
    let name = format!("{} (copy)", def.name);
    UserSymbol {
        key: new_key(existing, &name),
        ema_revision: None,
        name,
        library: "User".into(),
        letter: def.letter.into(),
        func: def.func,
        plc_channel: def.plc_channel.clone(),
        default_label: def.default_label.into(),
        auto_number: def.auto_number,
        show_tag: def.show_tag,
        show_pin_labels: def.show_pin_labels,
        prims: def.prims.clone(),
        pins: def.pins.iter().map(|x| UserPin { pos: x.pos, label: x.label.into() }).collect(),
        bridges: def.bridges.clone(),
    }
}

/// Number of placed instances of a symbol.
pub fn usage(prj: &Project, key: &str) -> usize {
    prj.pages.iter().flat_map(|pg| pg.symbols.iter()).filter(|s| s.sym == key).count()
}

/// Adds or replaces symbols of a library file; returns (added, replaced).
pub fn merge(list: &mut Vec<UserSymbol>, lib: SymbolLibrary) -> (usize, usize) {
    let (mut added, mut replaced) = (0, 0);
    for mut u in lib.symbols {
        if !u.key.starts_with(PREFIX) {
            u.key = new_key(list, &u.name);
        }
        match list.iter_mut().find(|x| x.key == u.key) {
            Some(x) => {
                *x = u;
                replaced += 1;
            }
            None => {
                list.push(u);
                added += 1;
            }
        }
    }
    (added, replaced)
}

/// Problems that make a symbol unusable or awkward to connect.
pub fn problems(u: &UserSymbol) -> Vec<String> {
    let mut v = Vec::new();
    if u.name.trim().is_empty() {
        v.push(tr("The symbol has no name.").into());
    }
    if u.pins.is_empty() && !matches!(u.func, Func::Overview) {
        v.push(tr("The symbol has no connection points.").into());
    }
    for (i, a) in u.pins.iter().enumerate() {
        if a.pos.x % GRID != 0.0 || a.pos.y % GRID != 0.0 {
            v.push(tr_fmt!("Connection point {} is not on the {} mm grid.", i + 1, GRID));
        }
        if u.pins[..i].iter().any(|b| b.pos.same(a.pos)) {
            v.push(tr_fmt!("Connection point {} lies on another one.", i + 1));
        }
    }
    if u.func.has_tag() && u.letter.trim().is_empty() {
        v.push(tr("No identifying letter for the device tag.").into());
    }
    v
}

/// Item of a symbol under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Prim(usize),
    Pin(usize),
}

/// Distance from `q` to a graphic primitive.
pub fn prim_distance(prim: &Prim, q: P) -> f32 {
    match prim {
        Prim::Line { a, b, .. } => dist_to_segment(q, *a, *b),
        Prim::Poly { pts, closed, fill } | Prim::Path {pts,closed,fill,..} => {
            let mut d = pts.windows(2).map(|s| dist_to_segment(q, s[0], s[1])).fold(f32::INFINITY, f32::min);
            if *closed && pts.len() > 2 {
                d = d.min(dist_to_segment(q, pts[pts.len() - 1], pts[0]));
                if *fill && inside(pts, q) {
                    d = 0.0;
                }
            }
            d
        }
        Prim::Circle { c, r, fill, .. } => {
            let d = (q.dist(*c) - r).abs();
            if *fill && q.dist(*c) <= *r { 0.0 } else { d }
        }
        Prim::Text { pos, text, size, anchor, angle, .. } => {
            let (s,c) = angle.unwrap_or_default().to_radians().sin_cos();
            let d = q - *pos;
            text_distance(*pos,text,*size,*anchor,*pos + p(c*d.x-s*d.y,s*d.x+c*d.y))
        }
        Prim::Field { pos, size, anchor, kind } => {
            let sample = if *kind == FieldKind::Tag { "-K1" } else { "1" };
            text_distance(*pos, sample, *size, *anchor, q)
        }
    }
}

fn text_distance(pos: P, text: &str, size: f32, anchor: Anchor, q: P) -> f32 {
    let w = text.chars().count().max(1) as f32 * size * 0.62;
    let x0 = match anchor {
        Anchor::Left => pos.x,
        Anchor::Center => pos.x - w / 2.0,
        Anchor::Right => pos.x - w,
    };
    let dx = (x0 - q.x).max(q.x - (x0 + w)).max(0.0);
    let dy = ((q.y - pos.y).abs() - size * 0.6).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn inside(pts: &[P], q: P) -> bool {
    let mut c = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// Pins first (they are small), then the nearest graphic within `tol`.
pub fn hit(u: &UserSymbol, q: P, tol: f32) -> Option<Hit> {
    if let Some(i) = u.pins.iter().position(|x| x.pos.dist(q) <= tol.max(1.0)) {
        return Some(Hit::Pin(i));
    }
    u.prims
        .iter()
        .enumerate()
        .map(|(i, prim)| (i, prim_distance(prim, q)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| Hit::Prim(i))
}

/// Moves a primitive by `d`.
pub fn translate(prim: &mut Prim, d: P) {
    match prim {
        Prim::Line { a, b, .. } => {
            *a = *a + d;
            *b = *b + d;
        }
        Prim::Poly { pts, .. } | Prim::Path {pts,..} => pts.iter_mut().for_each(|q| *q = *q + d),
        Prim::Circle { c, .. } => *c = *c + d,
        Prim::Text { pos, .. } | Prim::Field { pos, .. } => *pos = *pos + d,
    }
}

/// Removes a pin and fixes the bridges that refer to pins after it.
pub fn remove_pin(u: &mut UserSymbol, i: usize) {
    if i >= u.pins.len() {
        return;
    }
    u.pins.remove(i);
    u.bridges.retain(|(a, b)| *a != i && *b != i);
    for (a, b) in &mut u.bridges {
        if *a > i {
            *a -= 1;
        }
        if *b > i {
            *b -= 1;
        }
    }
}

/// Next free numeric pin designation ("1", "2", …).
pub fn next_pin_label(u: &UserSymbol) -> String {
    let max = u.pins.iter().filter_map(|x| x.label.parse::<u32>().ok()).max().unwrap_or(0);
    (max + 1).to_string()
}

pub fn snap(q: P, step: f32) -> P {
    p((q.x / step).round() * step, (q.y / step).round() * step)
}

#[cfg(test)]
mod tests;
