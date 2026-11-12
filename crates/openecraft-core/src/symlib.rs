//! Project symbol library: user defined symbols (symbol editor). They are stored in
//! the project, shared as library files and placed like built-in symbols; placed
//! instances refer to them by key; a project's [`SymbolCache`] resolves them for
//! [`Project::symbol`](crate::model::Project::symbol).
//!
//! A placed user symbol can be drawn larger or smaller: its key then carries the
//! factor (`user:ema:x@0.5`, [`scaled_key`]), and the table resolves it to the symbol
//! with its graphics and connection points scaled and its texts at their size
//! ([`scale_def`]), so that imported symbols fit the drawing and their texts stay as
//! large as all others.

use openecraft_i18n::{tr, tr_fmt};
use crate::geom::{dist_to_segment, p, P, GRID};
use crate::model::Project;
use crate::symbols::{self, Anchor, Category, FieldKind, Func, PinDef, Prim, SymbolDef, SymbolRef, SymbolDomain};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
pub use crate::model::symlib::{UserPin, UserSymbol};
pub mod ema;
mod front_view;

/// Keys of user symbols start with this prefix.
pub const PREFIX: &str = "user:";
/// Snap of graphics in the symbol editor (pins snap to the 4 mm grid).
pub const DRAW_SNAP: f32 = 0.5;
/// Separates the scale factor of a placed user symbol from its key.
const SCALE_MARK: char = '@';
/// Smallest and largest scale factor of a placed symbol.
pub const SCALE_RANGE: (f32, f32) = (0.1, 10.0);

/// The key of user symbol `key` drawn `factor` times its size (`user:x@0.5`): the key
/// itself for the factor 1 and for built-in symbols. The factor is limited to
/// [`SCALE_RANGE`] and kept to three decimals.
pub fn scaled_key(key: &str, factor: f32) -> String {
    let (base, _) = scale_of(key);
    let factor = if factor.is_finite() { factor.clamp(SCALE_RANGE.0, SCALE_RANGE.1) } else { 1.0 };
    let text = format!("{factor:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if !base.starts_with(PREFIX) || text == "1" {
        base.to_string()
    } else {
        format!("{base}{SCALE_MARK}{text}")
    }
}

/// The symbol a key refers to and its scale factor (1 for keys without one).
pub fn scale_of(key: &str) -> (&str, f32) {
    match key.rsplit_once(SCALE_MARK) {
        Some((base, f)) if base.starts_with(PREFIX) => match f.parse::<f32>() {
            Ok(f) if f.is_finite() && f > 0.0 => (base, f),
            _ => (key, 1.0),
        },
        _ => (key, 1.0),
    }
}

/// A symbol drawn `factor` times its size under the key `key`: the graphics, the
/// connection points and the places of its texts scale, the texts keep their size.
pub fn scale_def(def: &SymbolDef, factor: f32, key: &str) -> SymbolDef {
    let q = |v: P| v * factor;
    let prims: Vec<Prim> = def
        .prims
        .iter()
        .map(|prim| match prim {
            Prim::Image { name, data, corners } => Prim::Image { name: name.clone(), data: data.clone(), corners: corners.map(q) },
            Prim::Line { a, b, dashed } => Prim::Line { a: q(*a), b: q(*b), dashed: *dashed },
            Prim::Poly { pts, closed, fill } => Prim::Poly { pts: pts.iter().copied().map(q).collect(), closed: *closed, fill: *fill },
            Prim::Path { pts, closed, fill, color, width, dashed } => {
                Prim::Path { pts: pts.iter().copied().map(q).collect(), closed: *closed, fill: *fill, color: *color, width: *width, dashed: *dashed }
            }
            Prim::Circle { c, r, fill, color } => Prim::Circle { c: q(*c), r: r * factor, fill: *fill, color: *color },
            Prim::Text { pos, text, size, anchor, angle, color } => Prim::Text { pos: q(*pos), text: text.clone(), size: *size, anchor: *anchor, angle: *angle, color: *color },
            Prim::Field { kind, pos, size, anchor, angle } => Prim::Field { kind: *kind, pos: q(*pos), size: *size, anchor: *anchor, angle: *angle },
            Prim::PropertyBlock(block) => Prim::PropertyBlock(symbols::PropertyBlock { pos: q(block.pos), ..block.clone() }),
        })
        .collect();
    let pins: Vec<PinDef> = def.pins.iter().map(|x| PinDef { pos: q(x.pos), ..x.clone() }).collect();
    SymbolDef { key: intern(key), bounds: symbols::bounds_of(&prims, pins.iter().map(|x| x.pos)), prims, pins, ..def.clone() }
}

impl Default for UserSymbol {
    fn default() -> Self {
        UserSymbol {
            key: String::new(),
            ema_revision: None,
            source_data: None,
            plc_channel: None,
            plc_channels: Vec::new(),
            name: "New symbol".into(),
            library: "User".into(),
            letter: "A".into(),
            func: Func::Main { mirror: false },
            domain: SymbolDomain::Electrical,
            default_label: String::new(),
            default_function_text: String::new(),
            default_values: Default::default(),
            auto_number: false,
            show_tag: true,
            show_pin_labels: true,
            prims: Vec::new(),
            pins: Vec::new(),
            fluid_ports: Default::default(),
            pin_directions: Default::default(),
            directed_junction: false,
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
        category: match u.domain {
            SymbolDomain::Electrical => Category::User,
            SymbolDomain::Fluid => Category::Fluid,
            SymbolDomain::Process => Category::Process,
        },
        library: intern(&u.library),
        letter: intern(&u.letter),
        func: u.func,
        plc_channel: u.plc_channel.clone(),
        plc_channels: u.plc_channels.clone(),
        prims: u.prims.clone(),
        pins: u.pins.iter().map(|x| PinDef { pos: x.pos, label: intern(&x.label), internal: x.internal }).collect(),
        fluid_ports: u.fluid_ports.iter().filter(|(i, _)| **i < u.pins.len()).map(|(i, kind)| (*i, *kind)).collect(),
        pin_directions: u.pin_directions.iter().filter(|(i,_)|**i<u.pins.len()).map(|(i,mask)|(*i,*mask&15)).collect(),
        directed_junction: u.directed_junction && u.func == Func::Junction,
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
        Prim::Image { name, data, corners } => {
            for q in corners { hash_point(*q, h); }
            name.hash(h);
            data.as_str().hash(h);
        }
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
        Prim::Field { pos, size, anchor, kind, angle } => {
            angle.map(f32::to_bits).hash(h);
            hash_point(*pos, h);
            size.to_bits().hash(h);
            std::mem::discriminant(anchor).hash(h);
            kind.hash(h);
        }
        Prim::PropertyBlock(block) => {
            block.rows.hash(h);
            hash_point(block.pos, h);
            hash_point(block.step, h);
            (block.size.to_bits(), std::mem::discriminant(&block.anchor), block.angle.to_bits(), block.align, block.color).hash(h);
        }
    }
}

/// Fingerprint of a symbol list: changes whenever a symbol is added, removed or
/// edited (so that a project's symbol table is rebuilt exactly then).
pub fn fingerprint(list: &[UserSymbol]) -> u64 {
    hash_symbols(list, std::collections::hash_map::DefaultHasher::new())
}

// Reproduce pre-v1 revisions saved on the other supported pointer width.
pub(super) fn legacy_revision(symbol: &UserSymbol, width: u32) -> u64 {
    struct LegacyHasher(std::collections::hash_map::DefaultHasher, u32);
    impl Hasher for LegacyHasher {
        fn finish(&self) -> u64 { self.0.finish() }
        fn write(&mut self, bytes: &[u8]) { self.0.write(bytes); }
        fn write_usize(&mut self, n: usize) {
            if self.1 == 32 { self.0.write_u32(n as u32); } else { self.0.write_u64(n as u64); }
        }
        fn write_isize(&mut self, n: isize) {
            if self.1 == 32 { self.0.write_i32(n as i32); } else { self.0.write_i64(n as i64); }
        }
    }
    hash_symbols(std::slice::from_ref(symbol), LegacyHasher(Default::default(), width))
}

fn hash_symbols(list: &[UserSymbol], mut h: impl Hasher) -> u64 {
    list.len().hash(&mut h);
    for u in list {
        (&u.key, &u.name, &u.library, &u.letter, &u.default_label, u.auto_number, u.show_tag, u.show_pin_labels).hash(&mut h);
        if !u.default_function_text.is_empty() { ("default_function_text", &u.default_function_text).hash(&mut h); }
        if !u.default_values.is_empty() { ("default_values", &u.default_values).hash(&mut h); }
        if !u.plc_channels.is_empty() { ("plc_channels",&u.plc_channels).hash(&mut h); }
        format!("{:?}", u.func).hash(&mut h);
        std::mem::discriminant(&u.domain).hash(&mut h);
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
            if pin.internal { true.hash(&mut h); }
        }
        u.bridges.hash(&mut h);
        if !u.fluid_ports.is_empty() {
            ("fluid_ports", &u.fluid_ports).hash(&mut h);
        }
        if !u.pin_directions.is_empty() {("pin_directions",&u.pin_directions).hash(&mut h);}
        if u.directed_junction { "directed_junction".hash(&mut h); }
    }
    h.finish()
}

/// User symbol definitions by key.
type Defs = Arc<HashMap<String, Arc<SymbolDef>>>;

/// The user symbols of one project by key, rebuilt when the list changes. A
/// [`Project`] carries one; clones of a project share it until their symbols differ.
#[derive(Clone, Default)]
pub struct SymbolCache(Arc<Mutex<Option<(crate::model::symlib::SymbolList, Defs, Scaled)>>>, front_view::FrontCache);

/// Most scaled symbols kept: dragging a factor looks up a new one in every frame.
const MAX_SCALED: usize = 64;

/// Scaled user symbols by their key, made when they are first looked up.
type Scaled = Arc<Mutex<HashMap<String, Arc<SymbolDef>>>>;

impl SymbolCache {
    pub(crate) fn front_view(&self, part: &openecraft_parts::Part) -> Option<openecraft_parts::Shared<openecraft_parts::Model2d>> {
        part.model_2d.clone().or_else(|| self.1.get(part))
    }
    /// The symbols of a project with the user symbols `list`, as a snapshot.
    pub fn table(&self, list: &crate::model::symlib::SymbolList) -> Symbols {
        if list.is_empty() {
            return Symbols::default();
        }
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if cache.as_ref().is_none_or(|(have, _, _)| !have.same_snapshot(list)) {
            let defs = list.iter().map(|u| (u.key.clone(), Arc::new(to_def(u)))).collect();
            *cache = Some((list.clone(), Arc::new(defs), Scaled::default()));
        }
        Symbols { user: cache.as_ref().map(|(_, defs, _)| defs.clone()), scaled: cache.as_ref().map(|(_, _, s)| s.clone()).unwrap_or_default() }
    }
}

/// The symbols a project can place: the built-in ones and its user symbols. A cheap
/// snapshot without a borrow of the project (for loops that change the project).
#[derive(Clone, Default)]
pub struct Symbols {
    user: Option<Defs>,
    scaled: Scaled,
}

impl Symbols {
    /// Whether both tables share the same immutable definition snapshot.
    pub fn same_snapshot(&self, other: &Self) -> bool {
        match (&self.user, &other.user) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn get(&self, key: &str) -> Option<SymbolRef> {
        if let Some(d) = symbols::builtin(key) {
            return Some(SymbolRef::Builtin(d));
        }
        if !key.starts_with(PREFIX) {
            return None;
        }
        let user = self.user.as_ref()?;
        if let Some(d) = user.get(key) {
            return Some(SymbolRef::User(d.clone()));
        }
        let (base, factor) = scale_of(key);
        let def = user.get(base)?;
        let mut scaled = self.scaled.lock().unwrap_or_else(|e| e.into_inner());
        if scaled.len() >= MAX_SCALED && !scaled.contains_key(key) {
            scaled.clear();
        }
        let d = scaled.entry(key.to_string()).or_insert_with(|| Arc::new(scale_def(def, factor, key)));
        Some(SymbolRef::User(d.clone()))
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
        source_data: None,
        name,
        library: "User".into(),
        letter: def.letter.into(),
        func: def.func,
        domain: match def.category {
            Category::Fluid => SymbolDomain::Fluid,
            Category::Process => SymbolDomain::Process,
            _ => SymbolDomain::Electrical,
        },
        plc_channel: def.plc_channel.clone(),
        plc_channels: def.plc_channels.clone(),
        default_label: def.default_label.into(),
        default_function_text: existing.iter().find(|u| u.key == def.key).map(|u| u.default_function_text.clone()).unwrap_or_default(),
        default_values: existing.iter().find(|u| u.key == def.key).map(|u| u.default_values.clone()).unwrap_or_default(),
        auto_number: def.auto_number,
        show_tag: def.show_tag,
        show_pin_labels: def.show_pin_labels,
        prims: def.prims.clone(),
        pins: def.pins.iter().map(|x| UserPin { pos: x.pos, label: x.label.into(), internal: x.internal }).collect(),
        fluid_ports: def.fluid_ports.clone(),
        pin_directions: def.pin_directions.clone(),
        directed_junction: def.directed_junction,
        bridges: def.bridges.clone(),
    }
}

/// Number of placed instances of a symbol, at any scale.
pub fn usage(prj: &Project, key: &str) -> usize {
    prj.pages.iter().flat_map(|pg| pg.symbols.iter()).filter(|s| scale_of(&s.sym).0 == key).count()
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
    if u.pins.is_empty() && !matches!(u.func, Func::Overview | Func::Graphic) {
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
        Prim::Image { corners, .. } => {
            if inside(corners, q) { 0.0 } else {
                (0..4).map(|i| dist_to_segment(q, corners[i], corners[(i + 1) % 4])).fold(f32::INFINITY, f32::min)
            }
        }
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
        Prim::Field { pos, size, anchor, kind, angle } => {
            let sample = if matches!(kind, FieldKind::Tag | FieldKind::TagName | FieldKind::Product(_)) { "-K1" } else { "1" };
            let (s, c) = angle.unwrap_or_default().to_radians().sin_cos();
            let d = q - *pos;
            text_distance(*pos, sample, *size, *anchor, *pos + p(c * d.x - s * d.y, s * d.x + c * d.y))
        }
        Prim::PropertyBlock(block) => {
            let (sin, cos) = block.angle.to_radians().sin_cos();
            (0..block.rows.len()).map(|i| {
                let pos = block.row_pos(i, block.rows.len());
                let delta = q - pos;
                text_distance(pos, "…", block.size, block.anchor, pos + p(cos * delta.x - sin * delta.y, sin * delta.x + cos * delta.y))
            }).fold(f32::INFINITY, f32::min)
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
        Prim::Image { corners, .. } => {
            for q in corners { *q = *q + d; }
        }
        Prim::Line { a, b, .. } => {
            *a = *a + d;
            *b = *b + d;
        }
        Prim::Poly { pts, .. } | Prim::Path {pts,..} => pts.iter_mut().for_each(|q| *q = *q + d),
        Prim::Circle { c, .. } => *c = *c + d,
        Prim::Text { pos, .. } | Prim::Field { pos, .. } => *pos = *pos + d,
        Prim::PropertyBlock(block) => block.pos = block.pos + d,
    }
}

/// Removes a pin and fixes the bridges that refer to pins after it.
pub fn remove_pin(u: &mut UserSymbol, i: usize) {
    if i >= u.pins.len() {
        return;
    }
    u.pins.remove(i);
    u.fluid_ports = u.fluid_ports.iter().filter(|(pin, _)| **pin != i).map(|(pin, kind)| (if *pin > i { *pin - 1 } else { *pin }, *kind)).collect();
    u.pin_directions = u.pin_directions.iter().filter(|(pin,_)|**pin!=i).map(|(pin,mask)|(if *pin>i {*pin-1}else{*pin},*mask)).collect();
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
