//! Drawing commands: symbols, wires, texts; moving and deleting items. Positions are in
//! mm on the A3 page (420 x 297, y downwards) and snap to the 4 mm grid like in the
//! editor.

use crate::builtin::common::*;
use crate::params::{optional, param, Args, Kind, Param};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::fluid::Medium;
use openecraft_core::geom::{p, P, GRID};
use openecraft_core::model::*;
use openecraft_core::ops::{self, PlaceCtx};
use openecraft_core::parts;
use openecraft_core::symbols::{self, Anchor, Func, SymbolRef};
use openecraft_core::symlib::Symbols;
use openecraft_core::arrange::{self, Align};
use openecraft_core::netlist::junctions;
use openecraft_core::{cabledef, conndef, routing};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const JUNCTION_POLICY:Param=optional("junction_policy",Kind::Str,tr_const!("Wire junction policy: inherit, explicit or implicit"));

pub struct Drawing;

impl Plugin for Drawing {
    fn info(&self) -> Info {
        info("drawing", tr_const!("Drawing"), tr_const!("Symbols, wires, texts, cable and connection definitions on schematic pages"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name:"set_pin_connection_directions",
        help:tr_const!("Sets allowed side contacts of a user symbol connection point"),
        params:&[
            param("symbol",Kind::Str,tr_const!("Symbol key")),
            param("pin",Kind::Int,tr_const!("Zero-based connection point index")),
            param("directions",Kind::Str,tr_const!("Comma-separated up, right, down, left, all, or none")),
        ],
        run:set_pin_connection_directions,
    },
    Spec {
        name: "set_symbol_junction_paths",
        help: tr_const!("Sets physical target paths of a user junction symbol"),
        params: &[
            param("symbol",Kind::Str,tr_const!("Symbol key")),
            param("paths",Kind::Object,tr_const!("Junction paths with links (zero-based pin pairs) and optional directed flag")),
        ],
        run: set_symbol_junction_paths,
    },
    Spec {
        name: "insert_macro",
        help: tr_const!("Inserts a native macro with its symbol definitions on a page"),
        params: &[
            PAGE,
            param("macro", Kind::Str, tr_const!("Native macro JSON")),
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
        ],
        run: insert_macro,
    },
    Spec {
        name: "import_ema",
        help: tr_const!("Imports 2D EPLAN window macro variants into the project symbol library"),
        params: &[param("xml", Kind::Str, tr_const!("EPLAN window macro XML"))],
        run: import_ema,
    },
    Spec {
        name: "list_symbols",
        help: tr_const!("Lists the symbols that can be placed, with their connection points relative to the insertion point"),
        params: &[
            optional("query", Kind::Str, tr_const!("Text in the key or name")),
            optional("category", Kind::Str, tr_const!("Category, e.g. coils, contacts, terminals, plc")),
        ],
        run: list_symbols,
    },
    Spec {
        name: "place_symbol",
        help: tr_const!("Places a symbol on a page. Without a tag the next free one is used; auxiliary contacts get the next free contact numbers of their device"),
        params: &[
            PAGE,
            param("symbol", Kind::Str, tr_const!("Symbol key from list_symbols")),
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            ROTATION,
            MIRROR,
            TAG,
            LABEL,
            FUNCTION_TEXT,
            PINS,
        ],
        run: place_symbol,
    },
    Spec {
        name: "place_part",
        help: tr_const!("Places a part on a page: the symbol of its main function with its connection point designations, as a new device the part is assigned to (project parts and the example catalog)"),
        params: &[
            PAGE,
            param("part", Kind::Str, tr_const!("Part number, e.g. GEN.LMP-GN-24")),
            optional("variant", Kind::Str, tr_const!("Imported EPLAN symbol key")),
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            ROTATION,
            MIRROR,
            TAG,
        ],
        run: place_part,
    },
    Spec {
        name: "update_symbol",
        help: tr_const!("Changes a placed symbol; when it moves, connected wire ends follow"),
        params: &[
            PAGE,
            param("id", Kind::Int, tr_const!("Item id")),
            optional("x", Kind::Num, tr_const!("X position in mm")),
            optional("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            ROTATION,
            MIRROR,
            TAG,
            LABEL,
            FUNCTION_TEXT,
            PINS,
        ],
        run: update_symbol,
    },
    Spec {
        name: "add_wire",
        help: tr_const!("Draws a wire through two or more points"),
        params: &[PAGE, param("points", Kind::Points, tr_const!("Points in mm: [[x, y], …]")),JUNCTION_POLICY],
        run: add_wire,
    },
    Spec {
        name: "connect",
        help: tr_const!("Draws a wire between two connection points, e.g. -K1:A1 to -S1:14 (#id:pin for ambiguous ones, the name for potentials, 1, 2… for points without designation)"),
        params: &[
            PAGE,
            param("from", Kind::Str, tr_const!("Start: device tag and connection point, e.g. -K1:A1")),
            param("to", Kind::Str, tr_const!("End: device tag and connection point, e.g. -S1:14")),
            optional("via", Kind::Points, tr_const!("Points in between: [[x, y], …]")),
            JUNCTION_POLICY,
        ],
        run: connect,
    },
    Spec {
        name: "update_wire",
        help: tr_const!("Changes the properties of a wire"),
        params: &[
            PAGE,
            param("id", Kind::Int, tr_const!("Item id")),
            optional("number", Kind::Str, tr_const!("Wire number (kept when renumbering)")),
            optional("hide_labels", Kind::Bool, tr_const!("Hide automatic wire and cable labels")),
            JUNCTION_POLICY,
            optional("color", Kind::Str, tr_const!("Wire colour, e.g. BU (empty = from the potential definition)")),
            optional("cross_section", Kind::Str, tr_const!("Cross section, e.g. 1.5 mm² (empty = from the potential definition)")),
            optional("wire_type", Kind::Str, tr_const!("Wire type")),
            optional("cable", Kind::Str, tr_const!("Cable tag")),
            optional("core", Kind::Str, tr_const!("Cable core")),
            optional("medium", Kind::Str, tr_const!("Hoses / pipes: medium (compressed_air, vacuum, hydraulic_oil, water, coolant, lubricant, gas, steam, product; empty = project default)")),
            optional("length", Kind::Num, tr_const!("Hoses / pipes: length in m")),
        ],
        run: update_wire,
    },
    Spec {
        name: "add_text",
        help: tr_const!("Adds a text to a page"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            param("text", Kind::Str, tr_const!("Text; placeholders such as {page} or {prop:Name} are filled in when drawn")),
            optional("size", Kind::Num, tr_const!("Text height in mm (default 3.5)")),
            optional("angle", Kind::Num, tr_const!("Rotation in degrees, counter-clockwise (90 reads from bottom to top)")),
            optional("align", Kind::Str, tr_const!("Horizontal alignment: left, center or right")),
            optional("valign", Kind::Str, tr_const!("Which line the position is on: top (first), middle or bottom (last)")),
            optional("width", Kind::Num, tr_const!("Wrap width in mm (0: no wrapping)")),
            optional("bold", Kind::Bool, tr_const!("Bold")),
            optional("italic", Kind::Bool, tr_const!("Italic")),
            optional("underline", Kind::Bool, tr_const!("Underline")),
            optional("frame", Kind::Bool, tr_const!("Border around the text")),
            optional("background", Kind::Bool, tr_const!("Opaque background in the paper colour")),
            optional("link", Kind::Str, tr_const!("Hyperlink (URL)")),
            optional("path_function", Kind::Bool, tr_const!("Path function text: function text of the functions in its column that have none")),
        ],
        run: add_text,
    },
    Spec {
        name: "update_text",
        help: tr_const!("Changes a text (only the given properties)"),
        params: &[
            PAGE,
            param("id", Kind::Int, tr_const!("Id of the text")),
            optional("text", Kind::Str, tr_const!("Text; placeholders such as {page} or {prop:Name} are filled in when drawn")),
            optional("x", Kind::Num, tr_const!("X position in mm")),
            optional("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            optional("size", Kind::Num, tr_const!("Text height in mm (default 3.5)")),
            optional("angle", Kind::Num, tr_const!("Rotation in degrees, counter-clockwise (90 reads from bottom to top)")),
            optional("align", Kind::Str, tr_const!("Horizontal alignment: left, center or right")),
            optional("valign", Kind::Str, tr_const!("Which line the position is on: top (first), middle or bottom (last)")),
            optional("width", Kind::Num, tr_const!("Wrap width in mm (0: no wrapping)")),
            optional("bold", Kind::Bool, tr_const!("Bold")),
            optional("italic", Kind::Bool, tr_const!("Italic")),
            optional("underline", Kind::Bool, tr_const!("Underline")),
            optional("frame", Kind::Bool, tr_const!("Border around the text")),
            optional("background", Kind::Bool, tr_const!("Opaque background in the paper colour")),
            optional("link", Kind::Str, tr_const!("Hyperlink (URL)")),
            optional("path_function", Kind::Bool, tr_const!("Path function text: function text of the functions in its column that have none")),
        ],
        run: update_text,
    },
    Spec {
        name: "move_items",
        help: tr_const!("Moves symbols, wires and texts; connected wire ends follow"),
        params: &[
            PAGE,
            param("ids", Kind::Ids, tr_const!("Item ids")),
            param("dx", Kind::Num, tr_const!("Distance in X (mm)")),
            param("dy", Kind::Num, tr_const!("Distance in Y (mm)")),
        ],
        run: move_items,
    },
    Spec {
        name: "align_items",
        help: tr_const!("Aligns symbols, texts and graphics to a common edge or centre; connected wire ends follow"),
        params: &[
            PAGE,
            param("ids", Kind::Ids, tr_const!("Item ids")),
            param("align", Kind::Str, tr_const!("left, center, right, top, middle or bottom")),
        ],
        run: align_items,
    },
    Spec {
        name: "distribute_items",
        help: tr_const!("Distributes symbols, texts and graphics evenly between the first and the last one"),
        params: &[PAGE, param("ids", Kind::Ids, tr_const!("Item ids")), optional("vertical", Kind::Bool, tr_const!("Distribute vertically (default horizontally)"))],
        run: distribute_items,
    },
    Spec {
        name: "copy_items",
        help: tr_const!("Copies items several times at a fixed distance; devices get new tags, terminals new numbers"),
        params: &[
            PAGE,
            param("ids", Kind::Ids, tr_const!("Item ids")),
            optional("count", Kind::Int, tr_const!("Number of copies (default 1)")),
            param("dx", Kind::Num, tr_const!("Distance in X (mm)")),
            param("dy", Kind::Num, tr_const!("Distance in Y (mm)")),
        ],
        run: copy_items,
    },
    Spec {
        name: "delete_items",
        help: tr_const!("Deletes symbols, wires, texts and shapes"),
        params: &[PAGE, param("ids", Kind::Ids, tr_const!("Item ids"))],
        run: delete_items,
    },
    Spec {
        name: "route",
        help: tr_const!("Connects two connection points with wires routed around symbols and other wires"),
        params: &[
            PAGE,
            param("from", Kind::Str, tr_const!("Connection point TAG:PIN, e.g. -K1:A1")),
            param("to", Kind::Str, tr_const!("Connection point TAG:PIN, e.g. -K1:A1")),
            JUNCTION_POLICY,
        ],
        run: route,
    },
    Spec {
        name: "autoconnect",
        help: tr_const!("Draws wires between free connection points of symbols that face each other, and to the free arms of T-junctions and corners"),
        params: &[PAGE, optional("ids", Kind::Ids, tr_const!("Ids of symbols and connection symbols (default: all symbols of the page)"))],
        run: autoconnect,
    },
    Spec {
        name: "add_cable_definition",
        help: tr_const!("Draws a cable definition line; the wires it crosses become cores of the cable (a new cable when none is given)"),
        params: &[
            PAGE,
            param("points", Kind::Points, tr_const!("Start and end point of the line")),
            optional("cable", Kind::Str, tr_const!("Cable tag, e.g. -W1")),
        ],
        run: add_cable_definition,
    },
    Spec {
        name: "add_connection_definition",
        help: tr_const!("Places a connection definition point on a wire; the connection takes its number, colour and cross-section"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            optional("number", Kind::Str, tr_const!("Wire number")),
            optional("color", Kind::Str, tr_const!("Wire colour, e.g. BU")),
            optional("cross_section", Kind::Str, tr_const!("Cross-section, e.g. 1.5 mm²")),
        ],
        run: add_connection_definition,
    },
    Spec {
        name: "add_t_junction",
        help: tr_const!("Places a directed T-junction where three wires meet: it names the device the two wires of the connection start from (the source) and the order of their targets, instead of the schematic order"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            optional("direction", Kind::Str, tr_const!("Direction of the stem: left, right, up or down (default: from the wires there)")),
            VARIANT,
            DOT,
            AUTOCONNECT,
        ],
        run: add_t_junction,
    },
    Spec {
        name: "add_corner",
        help: tr_const!("Places a corner: two wires meet there at a right angle, and connection points facing its arms are wired to it"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            ARMS,
            AUTOCONNECT,
        ],
        run: add_corner,
    },
    Spec {
        name: "add_crossing",
        help: tr_const!("Places a directed crossing where two wires cross: it connects them, and each end of its line is wired straight on first and to one arm across second"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            LINE,
            optional("variant", Kind::Int, tr_const!("1: the upper / left end of the line goes across to the right / lower arm · 2: to the left / upper arm (default 2)")),
            DOT,
            AUTOCONNECT,
        ],
        run: add_crossing,
    },
    Spec {
        name: "add_bridge",
        help: tr_const!("Places a bridge where four wires meet: the connection comes in at one side and goes to the three others, the opposite side first, then the left / upper, then the right / lower"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position in mm")),
            param("y", Kind::Num, tr_const!("Y position in mm (downwards)")),
            optional("direction", Kind::Str, tr_const!("Side the connection comes from: left, right, up or down (default up)")),
            DOT,
            AUTOCONNECT,
        ],
        run: add_bridge,
    },
    Spec {
        name: "update_connection_symbol",
        help: tr_const!("Changes a T-junction (direction, variant, representation), a corner (arms), a crossing (line, variant, representation) or a bridge (direction, representation)"),
        params: &[
            PAGE,
            param("id", Kind::Int, tr_const!("Item id")),
            optional("direction", Kind::Str, tr_const!("T-junction: direction of the stem; bridge: side the connection comes from (left, right, up or down)")),
            VARIANT,
            DOT,
            ARMS,
            LINE,
        ],
        run: update_connection_symbol,
    },
    Spec {
        name: "list_connection_symbols",
        help: tr_const!("Lists the T-junctions, corners, crossings and bridges with their routing and the wires of their connection"),
        params: &[optional("page", Kind::Str, tr_const!("Page name (default: all pages)"))],
        run: list_connection_symbols,
    },
];

const ARMS: Param = optional("arms", Kind::Str, tr_const!("The two arms of a corner, vertical and horizontal, e.g. down left (default: from the wires there, else down right)"));

const AUTOCONNECT: Param = optional("autoconnect", Kind::Bool, tr_const!("Draw wires to the connection points facing its free arms"));

const LINE: Param = optional("line", Kind::Str, tr_const!("The straight line of a crossing: vertical or horizontal (default vertical)"));

const VARIANT: Param = optional(
    "variant",
    Kind::Int,
    tr_const!("1: from the left / upper arm straight to the other arm, then to the stem · 2: the same from the right / lower arm · 3: from the stem to the left / upper arm, then to the other · 4: from the stem to the right / lower arm first (default 3)"),
);

const DOT: Param = optional("dot", Kind::Bool, tr_const!("Draw the junction as a connection dot; its routing stays"));

const ROTATION: Param = optional("rotation", Kind::Int, tr_const!("Rotation in degrees clockwise: 0, 90, 180 or 270"));

const MIRROR: Param = optional("mirror", Kind::Bool, tr_const!("Mirror the symbol"));

const TAG: Param = optional("tag", Kind::Str, tr_const!("Device tag, e.g. -K1"));

const LABEL: Param = optional("label", Kind::Str, tr_const!("Label: terminal number, PLC address or potential name"));

const FUNCTION_TEXT: Param = optional("function_text", Kind::Str, tr_const!("Function text"));

const PINS: Param = optional("pins", Kind::Str, tr_const!("Connection point designations, comma separated, e.g. 13,14"));

fn func_name(f: Func) -> &'static str {
    match f {
        Func::Main { .. } => "main",
        Func::Aux(_) => "auxiliary",
        Func::Terminal => "terminal",
        Func::Plc => "plc",
        Func::Plug(_) => "plug",
        Func::Potential => "potential",
        Func::Interruption => "interruption",
        Func::Overview => "overview",
        Func::Auxiliary => "auxiliary",
        Func::Junction => "junction",
        Func::Graphic => "graphic",
    }
}

fn symbol_def(prj: &Project, key: &str) -> Result<SymbolRef, String> {
    prj.symbol(key).ok_or_else(|| tr_fmt!("Unknown symbol: {}", key))
}

fn rotation(a: &Args) -> Result<Option<u8>, String> {
    match a.int("rotation") {
        None => Ok(None),
        Some(deg) if deg % 90 == 0 => Ok(Some((deg / 90).rem_euclid(4) as u8)),
        Some(_) => Err(tr("The rotation must be 0, 90, 180 or 270").to_string()),
    }
}

fn snapped(x: f32, y: f32) -> P {
    p(x, y).snap(GRID)
}

/// A coordinate for messages: huge values in exponent form.
fn mm(v: f64) -> String {
    if v.is_finite() && v.abs() < 1e6 {
        format!("{}", v as f32)
    } else {
        format!("{v:e}")
    }
}

/// The position `x` / `y` as given (mm) on the paper of the page, snapped to the grid.
fn on_page(prj: &Project, page: usize, x: f64, y: f64) -> Result<P, String> {
    let (w, h) = openecraft_core::frame::of_page(prj, page).size();
    let inside = |v: f64, max: f32| v.is_finite() && (0.0..=f64::from(max)).contains(&v);
    if inside(x, w) && inside(y, h) {
        Ok(snapped(x as f32, y as f32))
    } else {
        Err(tr_fmt!("Position {} / {} mm lies outside the page ({} x {} mm)", mm(x), mm(y), w, h))
    }
}

/// A number parameter as sent (not rounded to `f32`).
fn num(a: &Args, name: &str) -> Option<f64> {
    a.value(name).and_then(Value::as_f64)
}

/// The points of a parameter as sent.
fn raw_points(a: &Args, name: &str) -> Vec<[f64; 2]> {
    let pt = |v: &Value| Some([v.get(0)?.as_f64()?, v.get(1)?.as_f64()?]);
    a.value(name).and_then(Value::as_array).map(|list| list.iter().filter_map(pt).collect()).unwrap_or_default()
}

/// Everything on the page still lies on its paper (after moving or copying).
fn inside_page(prj: &Project, page: usize) -> bool {
    let (w, h) = openecraft_core::frame::of_page(prj, page).size();
    let ok = |q: P| q.x.is_finite() && q.y.is_finite() && (0.0..=w).contains(&q.x) && (0.0..=h).contains(&q.y);
    let pg = &prj.pages[page];
    pg.symbols.iter().all(|s| ok(s.pos)) && pg.texts.iter().all(|t| ok(t.pos)) && pg.wires.iter().all(|wi| ok(wi.a) && ok(wi.b))
}

fn pin_list(text: &str) -> Vec<String> {
    text.split(',').map(|s| s.trim().to_string()).collect()
}

/// Applies the optional symbol parameters shared by `place_symbol` and `update_symbol`.
fn apply_symbol_params(s: &mut SymbolInst, a: &Args) -> Result<(), String> {
    if let Some(rot) = rotation(a)? {
        s.rot = rot;
    }
    if let Some(m) = a.value("mirror").and_then(Value::as_bool) {
        s.mirror = m;
    }
    if let Some(l) = a.str("label") {
        s.label = l.into();
    }
    if let Some(t) = a.str("function_text") {
        s.function_text = t.into();
    }
    if let Some(pins) = a.str("pins") {
        s.pins = pin_list(pins);
    }
    Ok(())
}

fn wire_policy(value:&str)->Result<Option<bool>,String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inherit"=>Ok(None),"explicit"=>Ok(Some(true)),"implicit"=>Ok(Some(false)),
        _=>Err(tr_fmt!("Unknown wire junction policy: {}",value)),
    }
}

fn push_path(prj: &mut Project, page: usize, pts: &[P],explicit_junctions:Option<bool>) -> Vec<u64> {
    let mut ids = Vec::new();
    for seg in pts.windows(2).filter(|s| !s[0].same(s[1])) {
        let id = prj.new_id();
        prj.pages[page].wires.push(Wire { id, a: seg[0], b: seg[1],explicit_junctions, ..Default::default() });
        ids.push(id);
    }
    ops::cleanup_wires(&mut prj.pages[page]);
    ids
}

/// A connection point found on a page: its position, the symbol and its number of
/// connection points.
#[derive(Clone, Copy)]
struct PinAt {
    pos: P,
    symbol: u64,
    pins: usize,
}

/// Position of a connection point `TAG:PIN` / `#ID:PIN` / `POTENTIAL:PIN` on a page.
fn pin_position(symbols: &Symbols, pg: &Page, spec: &str) -> Result<PinAt, String> {
    let (dev, pin) = spec.rsplit_once(':').ok_or_else(|| tr_fmt!("{} is not a connection point (TAG:PIN)", spec))?;
    let id = dev.strip_prefix('#').and_then(|n| n.parse::<u64>().ok());
    let mut found: Vec<PinAt> = Vec::new();
    for s in &pg.symbols {
        let named = |f: Func| matches!(f, Func::Potential | Func::Interruption) && s.label == dev;
        let hit = match id {
            Some(id) => s.id == id,
            None => s.tag == dev || symbols.get(&s.sym).is_some_and(|d| named(d.func)),
        };
        if hit {
            let pins = pins_of(symbols, s);
            for (label, q) in &pins {
                if *label == pin && !found.iter().any(|f| f.pos.same(*q)) {
                    found.push(PinAt { pos: *q, symbol: s.id, pins: pins.len() });
                }
            }
        }
    }
    match found.as_slice() {
        [q] => Ok(*q),
        [] => Err(tr_fmt!("No connection point {} on this page", spec)),
        _ => Err(tr_fmt!("{} is ambiguous: use #id:pin", spec)),
    }
}

/// The two ends of a wire between connection points: not the same point and not both
/// ends of a function with two connection points (a coil, contact or lamp would be
/// short-circuited; bridges between poles of a multi-pole symbol stay possible).
fn wire_ends(symbols: &Symbols, pg: &Page, from: &str, to: &str) -> Result<(P, P), String> {
    let (a, b) = (pin_position(symbols, pg, from)?, pin_position(symbols, pg, to)?);
    if a.pos.same(b.pos) {
        return Err(tr("Start and end are the same connection point").to_string());
    }
    if a.symbol == b.symbol && a.pins == 2 {
        return Err(tr_fmt!("{} and {} are both ends of one function: the wire would short-circuit it", from, to));
    }
    Ok((a.pos, b.pos))
}

/// Orthogonal path through the points: vertical first, then horizontal.
fn orthogonal(pts: &[P]) -> Vec<P> {
    let mut out = vec![pts[0]];
    for &q in &pts[1..] {
        let last = *out.last().unwrap();
        if last.x != q.x && last.y != q.y {
            out.push(p(last.x, q.y));
        }
        out.push(q);
    }
    out
}

/// The ids of `a` that exist on the page.
fn selection(pg: &Page, a: &Args) -> Result<BTreeSet<u64>, String> {
    let all: BTreeSet<u64> = ops::all_ids(pg).into_iter().collect();
    let sel: BTreeSet<u64> = a.ids("ids").into_iter().collect();
    match sel.difference(&all).next() {
        Some(id) => Err(tr_fmt!("No item with id {}", id)),
        None => Ok(sel),
    }
}

fn list_symbols(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let query = a.str("query").unwrap_or_default().to_lowercase();
    let category = a.str("category").map(|c| c.trim().to_ascii_lowercase());
    let function_texts: std::collections::HashMap<&str, &str> = prj.symbols.iter().map(|u| (u.key.as_str(), u.default_function_text.as_str())).collect();
    let defaults: std::collections::HashMap<_, _> = prj.symbols.iter().map(|u| (u.key.as_str(), &u.default_values)).collect();
    let user: Vec<SymbolRef> = prj.symbols.iter().filter_map(|u| prj.symbol(&u.key)).collect();
    let list = symbols::library()
        .iter()
        .map(SymbolRef::Builtin)
        .chain(user)
        .filter(|d| query.is_empty() || d.key.to_lowercase().contains(&query) || d.name.to_lowercase().contains(&query))
        .filter(|d| category.as_ref().is_none_or(|c| snake(d.category) == *c))
        .map(|d| {
            let pins: Vec<Value> = d.pins.iter().map(|pd| json!({ "pin": pd.label, "x": pd.pos.x, "y": pd.pos.y })).collect();
            json!({
                "key": d.key,
                "name": d.name,
                "category": snake(d.category),
                "function": func_name(d.func),
                "letter": d.letter,
                "default_label": d.default_label,
                "default_function_text": function_texts.get(d.key).copied().unwrap_or_default(),
                "default_values": defaults.get(d.key),
                "plc_channels": d.plc_channels,
                "property_blocks": d.prims.iter().filter_map(|p| if let symbols::Prim::PropertyBlock(block) = p { Some(block) } else { None }).collect::<Vec<_>>(),
                "pins": pins,
                "fluid_ports": d.fluid_ports,
                "pin_directions": d.pin_directions,
                "directed_junction": d.directed_junction,
                "bridges": d.bridges,
            })
        });
    Ok(Value::Array(list.collect()))
}

fn place_symbol(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let symbols = prj.symbol_table();
    let i = editable(prj, a.req("page"))?;
    let def = symbol_def(prj, a.req("symbol"))?;
    // A given tag decides the device the new function belongs to, like the last
    // placed main function in the editor.
    let mut ctx = PlaceCtx::default();
    // An empty tag means the next free one, as without the parameter.
    let tag = a.str("tag").filter(|t| !t.trim().is_empty());
    if let Some(tag) = tag {
        ops::check_tag(tag)?;
        match def.func {
            Func::Aux(_) | Func::Overview => {
                ctx.last_main.insert(def.letter.into(), tag.into());
            }
            Func::Terminal => ctx.last_strip = Some(tag.into()),
            Func::Plug(_) => ctx.last_plug = Some(tag.into()),
            Func::Plc => {
                ctx.last_plc.insert(def.key.into(), tag.into());
            }
            _ => {}
        }
    }
    let pos = on_page(prj, i, num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default())?;
    let mut sym = ops::new_symbol(prj, i, &ctx, &def, pos, rotation(a)?.unwrap_or(0), a.flag("mirror"));
    if let Some(tag) = tag.filter(|_| def.func.has_tag()) {
        sym.tag = tag.into();
    }
    apply_symbol_params(&mut sym, a)?;
    let out = symbol_json(&symbols, &sym);
    let plc_tag = def.plc_channel.as_ref().map(|_| sym.tag.clone());
    prj.pages[i].symbols.push(sym);
    if let Some(tag) = plc_tag {
        parts::link_unassigned_plc_parts(prj, &parts::PartsDb { parts: Vec::new() }, Some(&tag));
    }
    Ok(out)
}

fn import_ema(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let variants = openecraft_core::symlib::ema::decode_with_library(a.req("xml"), &prj.symbols)?;
    let added = openecraft_core::symlib::ema::import_project(prj, &variants);
    let windows: Vec<_> = variants.iter().filter_map(openecraft_core::macros::eplan_window).collect();
    Ok(json!({"added":added,"variants":variants,"macros":windows}))
}

fn insert_macro(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let page = editable(prj, a.req("page"))?;
    let m: openecraft_core::macros::Macro = serde_json::from_str(a.req("macro")).map_err(|e| e.to_string())?;
    let ids = ops::paste(prj, page, &m.clip, p(a.num("x").unwrap_or_default(), a.num("y").unwrap_or_default()));
    Ok(json!({ "ids": ids, "symbol_definitions": m.clip.symbol_defs.len() }))
}

fn place_part(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let number = a.req("part").trim().to_string();
    let db = parts::PartsDb::default();
    let part = parts::lookup(prj, None, &number).or_else(|| parts::catalog().iter().find(|p| p.number == number)).cloned();
    let part = part.ok_or_else(|| tr_fmt!("Unknown part: {}", number))?;
    let i = editable(prj, a.req("page"))?;
    let pos = on_page(prj, i, num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default())?;
    let key = a.str("variant");
    let rot = rotation(a)?.unwrap_or(0);
    let tag = a.str("tag").filter(|t| !t.trim().is_empty());
    if let Some(tag) = tag {
        ops::check_tag(tag)?;
    }
    let selected = part.eplan_variants.iter().find(|v| match key {
        Some(key) => v["symbol"]["key"].as_str() == Some(key),
        None => v["function_type"].as_u64() == Some(1),
    });
    if let Some(value) = selected.filter(|v| v["devices"].is_array()) {
        let variant = serde::Deserialize::deserialize(value.as_ref()).map_err(|e: serde_json::Error| e.to_string())?;
        if let Some(mut m) = openecraft_core::macros::eplan_window(&variant) {
            if tag.is_some() {
                return Err(tr("A multi-device window carries its own device tags").into());
            }
            let mirror = a.flag("mirror");
            for s in &mut m.clip.symbols {
                s.pos = s.pos.transform(rot, mirror);
                s.rot = rot;
                s.mirror = mirror;
            }
            openecraft_core::symlib::ema::install(&mut prj.symbols, &[variant]);
            let ids = openecraft_core::macro_boxes::place(prj, i, &m, &Default::default(), pos);
            return Ok(json!({"part":number,"ids":ids,"macro":m.name}));
        }
    }
    let choice = openecraft_core::symlib::ema::part_symbol_for_card(prj, &part, key, tag);
    let (symbol, pins) = choice
        .or_else(|| if key.is_none() { openecraft_core::unplaced::part_symbol(&part) } else { None })
        .ok_or_else(|| tr_fmt!("Part {} has no function with a symbol", number))?;
    let def = symbol_def(prj, symbol)?;
    if let (Some(tag), Some(c)) = (tag, &def.plc_channel) {
        if prj.channel_placed(tag, &c.family, c.input, c.index) {
            return Err(tr_fmt!("{} is already placed on PLC card {}", c.designation, tag));
        }
    }
    let mut ctx = PlaceCtx { plc_article: Some(number.clone()), ..Default::default() };
    if let Some(tag) = tag {
        ctx.last_plc.insert(def.key.into(), tag.into());
    }
    let mut sym = ops::new_symbol(prj, i, &ctx, &def, pos, rot, a.flag("mirror"));
    if let Some(tag) = tag {
        sym.tag = tag.into();
    }
    if pins.len() == sym.pins.len() {
        sym.pins = pins;
    }
    let id = sym.id;
    let mut out = symbol_json(&prj.symbol_table(), &sym);
    out["part"] = json!(number);
    prj.pages[i].symbols.push(sym);
    parts::assign_placed_function_part(prj, &db, i, id, &number);
    Ok(out)
}

fn update_symbol(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let symbols = prj.symbol_table();
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let old = prj.pages[i].symbols.iter().find(|x| x.id == id).ok_or_else(|| tr_fmt!("No symbol with id {}", id))?.pos;
    let to = on_page(prj, i, num(a, "x").unwrap_or(old.x.into()), num(a, "y").unwrap_or(old.y.into()))?;
    if let Some(tag) = a.str("tag") {
        ops::check_tag(tag)?;
    }
    if !to.same(old) {
        routing::move_selection(prj, i, &BTreeSet::from([id]), to - old);
    }
    let sym = prj.pages[i].symbols.iter_mut().find(|x| x.id == id).ok_or_else(|| tr_fmt!("No symbol with id {}", id))?;
    if let Some(tag) = a.str("tag") {
        sym.tag = tag.into();
    }
    apply_symbol_params(sym, a)?;
    Ok(symbol_json(&symbols, sym))
}

fn set_pin_connection_directions(prj:&mut Project,a:&Args)->Result<Value,String> {
    let key=a.req("symbol");
    let pin=int_in(a,"pin",0,u32::MAX.into())?.unwrap_or(0) as usize;
    let mut mask=0;
    for direction in a.req("directions").split(',').map(str::trim) {
        mask|=match direction {"up"=>1,"right"=>2,"down"=>4,"left"=>8,"all"=>15,"none"=>0,_=>return Err(tr_fmt!("Unknown connection direction: {}",direction))};
    }
    let symbol=prj.symbols.iter_mut().find(|s|s.key==key).ok_or_else(||tr_fmt!("Unknown user symbol: {}",key))?;
    if pin>=symbol.pins.len() {return Err(tr_fmt!("Connection point index {} is out of range",pin));}
    if mask==15 {symbol.pin_directions.remove(&pin);} else {symbol.pin_directions.insert(pin,mask);}
    Ok(json!({"symbol":key,"pin":pin,"directions":mask}))
}

fn set_symbol_junction_paths(prj:&mut Project,a:&Args)->Result<Value,String> {
    let key=a.req("symbol");
    let value=a.value("paths").expect("required object");
    for name in value.as_object().expect("checked object").keys() {
        if !["links","directed"].contains(&name.as_str()) {return Err(tr_fmt!("Unknown parameter: {}",name));}
    }
    let invalid=||tr("Junction paths need distinct valid connection point indexes").to_string();
    let links:Vec<(usize,usize)>=serde_json::from_value(value["links"].clone()).map_err(|_|invalid())?;
    let directed=match value.get("directed") {Some(v)=>v.as_bool().ok_or_else(invalid)?,None=>true};
    let symbol=prj.symbols.iter_mut().find(|s|s.key==key).ok_or_else(||tr_fmt!("Unknown user symbol: {}",key))?;
    if symbol.func!=Func::Junction {return Err(tr_fmt!("Symbol {} is not a junction",key));}
    if links.iter().any(|&(a,b)| a==b || a>=symbol.pins.len() || b>=symbol.pins.len()) {return Err(invalid());}
    symbol.bridges=links;
    symbol.directed_junction=directed;
    Ok(json!({"symbol":key,"directed":directed,"links":symbol.bridges}))
}

fn add_wire(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let pts = raw_points(a, "points").into_iter().map(|[x, y]| on_page(prj, i, x, y)).collect::<Result<Vec<P>, String>>()?;
    if pts.len() < 2 {
        return Err(tr("A wire needs at least two points").to_string());
    }
    let ids = push_path(prj, i, &pts,wire_policy(a.str("junction_policy").unwrap_or("inherit"))?);
    if ids.is_empty() {
        return Err(tr("The wire has no length").to_string());
    }
    Ok(json!({ "ids": ids }))
}

fn connect(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let symbols = prj.symbol_table();
    let i = editable(prj, a.req("page"))?;
    let (from, to) = wire_ends(&symbols, &prj.pages[i], a.req("from"), a.req("to"))?;
    let mut pts = vec![from];
    for [x, y] in raw_points(a, "via") {
        pts.push(on_page(prj, i, x, y)?);
    }
    pts.push(to);
    let path = orthogonal(&pts);
    let ids = push_path(prj, i, &path,wire_policy(a.str("junction_policy").unwrap_or("inherit"))?);
    Ok(json!({ "ids": ids, "points": path.into_iter().map(xy).collect::<Vec<_>>() }))
}

fn update_wire(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let policy=a.str("junction_policy").map(wire_policy).transpose()?;
    let id = a.int("id").unwrap_or_default() as u64;
    let w = prj.pages[i].wires.iter_mut().find(|w| w.id == id).ok_or_else(|| tr_fmt!("No wire with id {}", id))?;
    if let Some(policy)=policy {w.explicit_junctions=policy;}
    if let Some(n) = a.str("number") {
        w.number = n.into();
        w.number_fixed = !n.is_empty();
    }
    if let Some(hide) = a.value("hide_labels").and_then(Value::as_bool) {
        w.hide_labels = hide;
    }
    let color = a.str("color").map(wire_color).transpose()?;
    let cross = a.str("cross_section").map(cross_section).transpose()?;
    for (name, field) in [("cable", &mut w.cable), ("core", &mut w.core), ("wire_type", &mut w.wire_type)] {
        if let Some(v) = a.str(name) {
            *field = v.into();
        }
    }
    if let Some(c) = color {
        w.color = c;
    }
    if let Some(c) = cross {
        w.cross_section = c;
    }
    if let Some(m) = a.str("medium") {
        w.medium = if m.trim().is_empty() { None } else { Some(Medium::from_code(m).ok_or_else(|| tr_fmt!("Unknown medium: {}", m))?) };
    }
    if let Some(l) = a.num("length") {
        if !(l >= 0.0 && l.is_finite()) {
            return Err(tr("The length must not be negative").to_string());
        }
        w.length = l;
    }
    let medium = w.medium.map(|m| m.code()).unwrap_or_default();
    Ok(json!({ "id": w.id, "number": w.number, "hide_labels": w.hide_labels,"junction_policy":w.junction_policy(), "color": w.color, "cross_section": w.cross_section, "wire_type": w.wire_type, "cable": w.cable, "core": w.core, "medium": medium, "length": w.length }))
}

/// Takes the style parameters of `add_text` / `update_text` that are given.
fn text_style_args(t: &mut TextItem, a: &Args) -> Result<(), String> {
    if let Some(s) = a.num("size") {
        if !(s >= 0.5 && s.is_finite()) {
            return Err(tr("The text height must be at least 0.5 mm").to_string());
        }
        t.size = s;
    }
    if let Some(v) = a.num("angle") {
        t.angle = v.rem_euclid(360.0);
    }
    if let Some(v) = a.str("align") {
        t.align = match v.trim().to_ascii_lowercase().as_str() {
            "left" => Anchor::Left,
            "center" | "centre" => Anchor::Center,
            "right" => Anchor::Right,
            _ => return Err(tr_fmt!("Unknown alignment: {}", v)),
        };
    }
    if let Some(v) = a.str("valign") {
        t.valign = match v.trim().to_ascii_lowercase().as_str() {
            "top" => VAnchor::Top,
            "middle" => VAnchor::Middle,
            "bottom" => VAnchor::Bottom,
            _ => return Err(tr_fmt!("Unknown vertical anchor: {}", v)),
        };
    }
    if let Some(w) = a.num("width") {
        if !(w >= 0.0 && w.is_finite()) {
            return Err(tr("The wrap width must not be negative").to_string());
        }
        t.width = w;
    }
    for (name, field) in [
        ("bold", &mut t.bold),
        ("italic", &mut t.italic),
        ("underline", &mut t.underline),
        ("frame", &mut t.frame),
        ("background", &mut t.background),
        ("path_function", &mut t.path_function),
    ] {
        if let Some(v) = a.value(name).and_then(Value::as_bool) {
            *field = v;
        }
    }
    if let Some(l) = a.str("link") {
        t.link = l.trim().to_string();
    }
    Ok(())
}

fn text_json(t: &TextItem) -> Value {
    let align = match t.align {
        Anchor::Left => "left",
        Anchor::Center => "center",
        Anchor::Right => "right",
    };
    let valign = match t.valign {
        VAnchor::Top => "top",
        VAnchor::Middle => "middle",
        VAnchor::Bottom => "bottom",
    };
    json!({
        "id": t.id, "x": t.pos.x, "y": t.pos.y, "text": t.text, "size": t.size, "angle": t.angle, "align": align, "valign": valign,
        "width": t.width, "bold": t.bold, "italic": t.italic, "underline": t.underline, "frame": t.frame, "background": t.background,
        "link": t.link, "path_function": t.path_function
    })
}

fn add_text(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let pos = on_page(prj, i, num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default())?;
    let id = prj.new_id();
    let mut t = TextItem { id, pos, text: a.req("text").into(), size: 3.5, ..Default::default() };
    text_style_args(&mut t, a)?;
    let out = text_json(&t);
    prj.pages[i].texts.push(t);
    Ok(out)
}

fn update_text(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let old = prj.pages[i].texts.iter().find(|t| t.id == id).ok_or_else(|| tr_fmt!("No text with id {}", id))?.pos;
    let pos = match (num(a, "x"), num(a, "y")) {
        (None, None) => old,
        (x, y) => on_page(prj, i, x.unwrap_or(old.x.into()), y.unwrap_or(old.y.into()))?,
    };
    let t = prj.pages[i].texts.iter_mut().find(|t| t.id == id).ok_or_else(|| tr_fmt!("No text with id {}", id))?;
    if let Some(s) = a.str("text") {
        t.text = s.into();
    }
    t.pos = pos;
    text_style_args(t, a)?;
    Ok(text_json(t))
}

fn move_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let before = prj.pages[i].clone();
    let sel = selection(&prj.pages[i], a)?;
    let d = snapped(a.num("dx").unwrap_or_default(), a.num("dy").unwrap_or_default());
    routing::move_selection(prj, i, &sel, d);
    if !inside_page(prj, i) {
        prj.pages[i] = before;
        return Err(tr("The items would leave the page").to_string());
    }
    Ok(json!({ "moved": sel.len(), "dx": d.x, "dy": d.y }))
}

fn align_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let how = Align::from_code(a.req("align")).ok_or_else(|| tr_fmt!("Unknown alignment: {}", a.req("align")))?;
    let sel = selection(&prj.pages[i], a)?;
    let n = arrange::align(prj, i, &sel, how);
    Ok(json!({ "moved": n }))
}

fn distribute_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let sel = selection(&prj.pages[i], a)?;
    let n = arrange::distribute(prj, i, &sel, !a.flag("vertical"));
    Ok(json!({ "moved": n }))
}

fn copy_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let sel = selection(&prj.pages[i], a)?;
    let count = a.int("count").unwrap_or(1);
    if !(1..=100).contains(&count) {
        return Err(tr("The number of copies must be between 1 and 100").to_string());
    }
    let d = p(a.num("dx").unwrap_or_default(), a.num("dy").unwrap_or_default());
    let before = prj.clone();
    let ids = arrange::multiple_copy(prj, i, &sel, count as usize, d);
    if !inside_page(prj, i) {
        *prj = before;
        return Err(tr("The copies would leave the page").to_string());
    }
    Ok(json!({ "ids": ids }))
}

fn delete_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let sel = selection(&prj.pages[i], a)?;
    ops::delete(&mut prj.pages[i], &sel);
    Ok(json!({ "deleted": sel.len() }))
}

fn route(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let symbols = prj.symbol_table();
    let i = editable(prj, a.req("page"))?;
    let (from, to) = wire_ends(&symbols, &prj.pages[i], a.req("from"), a.req("to"))?;
    let area = openecraft_core::frame::of_page(prj, i).area();
    let path = routing::route(&prj.symbol_table(), &prj.pages[i], area, from, to);
    let ids = push_path(prj, i, &path,wire_policy(a.str("junction_policy").unwrap_or("inherit"))?);
    Ok(json!({ "ids": ids, "points": path.into_iter().map(xy).collect::<Vec<_>>() }))
}

fn autoconnect(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let ids: BTreeSet<u64> = match a.ids("ids") {
        v if v.is_empty() => prj.pages[i].symbols.iter().map(|s| s.id).collect(),
        v => v.into_iter().collect(),
    };
    let before = prj.pages[i].wires.len();
    routing::apply_autoconnect(prj, i, &ids);
    let added: Vec<u64> = prj.pages[i].wires[before..].iter().map(|w| w.id).collect();
    Ok(json!({ "ids": added }))
}

fn add_cable_definition(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let pts = raw_points(a, "points");
    let [[x0, y0], [x1, y1]] = pts.as_slice() else {
        return Err(tr("A cable definition needs a start and an end point").to_string());
    };
    let (from, to) = (on_page(prj, i, *x0, *y0)?, on_page(prj, i, *x1, *y1)?);
    if from.same(to) {
        return Err(tr("The cable definition line has no length").to_string());
    }
    if cabledef::crossed_wires(&prj.pages[i], from, to).is_empty() {
        return Err(tr("The cable definition line crosses no wire").to_string());
    }
    let cable = a.str("cable").unwrap_or_default();
    if !cable.is_empty() && prj.cable(cable).is_none() {
        return Err(tr_fmt!("No cable {}", cable));
    }
    let id = cabledef::add(prj, i, from, to, cable);
    let def = prj.pages[i].cable_defs.iter().find(|c| c.id == id).cloned().unwrap_or_default();
    let cores: Vec<Value> = prj.pages[i].wires.iter().filter(|w| w.cable == def.cable).map(|w| json!({ "wire": w.id, "core": w.core })).collect();
    Ok(json!({ "id": id, "cable": def.cable, "cores": cores }))
}

fn add_connection_definition(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let (x, y) = (num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default());
    let pos = p(x as f32, y as f32);
    if conndef::wire_at(&prj.pages[i], pos).is_none() {
        return Err(tr_fmt!("No wire at {} / {} mm", mm(x), mm(y)));
    }
    a.str("color").map(wire_color).transpose()?;
    a.str("cross_section").map(cross_section).transpose()?;
    let open = openecraft_core::netlist::Netlist::build(prj, false);
    let mut cd = conndef::adopt(prj, &open, i, pos);
    for (field, name) in [(&mut cd.number, "number"), (&mut cd.color, "color"), (&mut cd.cross_section, "cross_section")] {
        if let Some(v) = a.str(name) {
            *field = v.to_string();
        }
    }
    cd.id = prj.new_id();
    let id = cd.id;
    prj.pages[i].conn_defs.push(cd);
    let changed = conndef::apply(prj, &open, i, id);
    Ok(json!({ "id": id, "wires_changed": changed }))
}

/// A connection symbol with its routing and the wires of its connection.
fn tee_json(prj: &Project, page: usize, j: &Junction) -> Value {
    let nl = openecraft_core::netlist::Netlist::build(prj, true);
    let open = openecraft_core::netlist::Netlist::build(prj, false);
    let conns = openecraft_core::netlist::connections(prj, &nl, &open);
    let pin = |p| openecraft_core::netlist::pin_designation(prj, p);
    let wires: Vec<Value> = junctions::connections_of(prj, &open, &conns, page, j).into_iter().map(|i| json!({ "from": pin(conns[i].a), "to": pin(conns[i].b) })).collect();
    let kind = match j.kind {
        JunctionKind::Tee => "t_junction",
        JunctionKind::Corner => "corner",
        JunctionKind::Crossing => "crossing",
        JunctionKind::Bridge => "bridge",
    };
    let mut v = json!({
        "id": j.id,
        "page": prj.pages[page].name,
        "x": j.pos.x,
        "y": j.pos.y,
        "kind": kind,
        "fits": junctions::fits(prj, &prj.pages[page], j),
        "routing": junctions::describe(j),
        "links": j.links().iter().map(|(a, b)| json!([a.name(), b.name()])).collect::<Vec<_>>(),
        "wires": wires,
    });
    match j.kind {
        JunctionKind::Corner => v["arms"] = json!(j.arms().iter().map(|d| d.name()).collect::<Vec<_>>()),
        JunctionKind::Crossing => {
            v["line"] = json!(if j.line() == TeeDir::Up { "vertical" } else { "horizontal" });
            v["variant"] = json!(j.variant);
            v["dot"] = json!(j.dot);
        }
        JunctionKind::Bridge => {
            v["direction"] = json!(j.dir.name());
            v["dot"] = json!(j.dot);
        }
        JunctionKind::Tee => {
            v["direction"] = json!(j.dir.name());
            v["variant"] = json!(j.variant);
            v["dot"] = json!(j.dot);
        }
    }
    v
}

/// The line of a crossing as given.
fn crossing_line(a: &Args) -> Result<Option<TeeDir>, String> {
    match a.str("line").map(|s| s.trim().to_ascii_lowercase()) {
        None => Ok(None),
        Some(s) if s == "vertical" => Ok(Some(TeeDir::Up)),
        Some(s) if s == "horizontal" => Ok(Some(TeeDir::Left)),
        Some(s) => Err(tr_fmt!("The line of a crossing is vertical or horizontal, not {}", s)),
    }
}

fn add_crossing(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let (x, y) = (num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default());
    let pos = on_page(prj, i, x, y)?;
    let variant = match a.int("variant") {
        None => 2,
        Some(v @ 1..=2) => v as u8,
        Some(v) => return Err(tr_fmt!("Variant {} is not one of 1 to 2", v)),
    };
    let j = Junction { dir: crossing_line(a)?.unwrap_or(TeeDir::Up), variant, dot: a.flag("dot"), ..Junction::new(JunctionKind::Crossing, pos) };
    add_symbol(prj, a, i, j)
}

fn add_bridge(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let (x, y) = (num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default());
    let pos = on_page(prj, i, x, y)?;
    let j = Junction { dir: tee_direction(a)?.unwrap_or(TeeDir::Up), dot: a.flag("dot"), ..Junction::new(JunctionKind::Bridge, pos) };
    add_symbol(prj, a, i, j)
}

/// The arms of a corner as given: a vertical and a horizontal direction.
fn corner_arms(a: &Args) -> Result<Option<(TeeDir, TeeDir)>, String> {
    let Some(text) = a.str("arms") else { return Ok(None) };
    let dirs: Vec<TeeDir> = text.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(|s| TeeDir::parse(s).ok_or_else(|| tr_fmt!("Unknown direction {}: left, right, up or down", s))).collect::<Result<_, _>>()?;
    junctions::corner_of(&dirs).map(Some).ok_or_else(|| tr_fmt!("The arms of a corner are one vertical and one horizontal direction, e.g. down left, not {}", text))
}

/// Adds a connection symbol and, when asked, the automatic connections to its arms.
fn add_symbol(prj: &mut Project, a: &Args, page: usize, mut j: Junction) -> Result<Value, String> {
    if prj.pages[page].junctions.iter().any(|o| o.pos.same(j.pos)) {
        return Err(tr("There is a connection symbol at this point already").to_string());
    }
    j.id = prj.new_id();
    let id = j.id;
    prj.pages[page].junctions.push(j.clone());
    let drawn = if a.flag("autoconnect") { routing::apply_autoconnect(prj, page, &BTreeSet::from([id])) } else { 0 };
    let mut v = tee_json(prj, page, &j);
    v["wires_drawn"] = json!(drawn);
    Ok(v)
}

fn add_corner(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let (x, y) = (num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default());
    let pos = on_page(prj, i, x, y)?;
    let (v, h) = match corner_arms(a)? {
        Some(arms) => arms,
        None => junctions::arms(&prj.pages[i], pos).as_deref().and_then(junctions::corner_of).unwrap_or((TeeDir::Down, TeeDir::Right)),
    };
    add_symbol(prj, a, i, Junction::corner(pos, v, h))
}

fn tee_direction(a: &Args) -> Result<Option<TeeDir>, String> {
    match a.str("direction") {
        None => Ok(None),
        Some(d) => TeeDir::parse(d).map(Some).ok_or_else(|| tr_fmt!("Unknown direction {}: left, right, up or down", d)),
    }
}

fn tee_variant(a: &Args) -> Result<Option<u8>, String> {
    match a.int("variant") {
        None => Ok(None),
        Some(v @ 1..=4) => Ok(Some(v as u8)),
        Some(v) => Err(tr_fmt!("Variant {} is not one of 1 to 4", v)),
    }
}

fn add_t_junction(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let (x, y) = (num(a, "x").unwrap_or_default(), num(a, "y").unwrap_or_default());
    let pos = on_page(prj, i, x, y)?;
    let dir = match tee_direction(a)? {
        Some(d) => d,
        None => junctions::arms(&prj.pages[i], pos)
            .as_deref()
            .and_then(junctions::stem_of)
            .ok_or_else(|| tr_fmt!("No three wires meet at {} / {} mm: give the direction of the stem", mm(x), mm(y)))?,
    };
    let j = Junction { pos, dir, variant: tee_variant(a)?.unwrap_or(3), dot: a.flag("dot"), ..Default::default() };
    add_symbol(prj, a, i, j)
}

fn update_connection_symbol(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let (dir, arms, line) = (tee_direction(a)?, corner_arms(a)?, crossing_line(a)?);
    let dot = a.value("dot").and_then(Value::as_bool);
    let Some(j) = prj.pages[i].junctions.iter_mut().find(|j| j.id == id) else { return Err(tr_fmt!("No connection symbol with id {}", id)) };
    // The parameters a kind has: the others are refused.
    let allowed: &[&str] = match j.kind {
        JunctionKind::Tee => &["direction", "variant", "dot"],
        JunctionKind::Corner => &["arms"],
        JunctionKind::Crossing => &["line", "variant", "dot"],
        JunctionKind::Bridge => &["direction", "dot"],
    };
    for name in ["direction", "variant", "dot", "arms", "line"] {
        if a.value(name).is_some() && !allowed.contains(&name) {
            return Err(tr_fmt!("A {} has no {}; it takes {}", junctions::kind_name(j.kind), name, allowed.join(", ")));
        }
    }
    let variant = match (j.kind, a.int("variant")) {
        (_, None) => None,
        (JunctionKind::Crossing, Some(v @ 1..=2)) => Some(v as u8),
        (JunctionKind::Crossing, Some(v)) => return Err(tr_fmt!("Variant {} is not one of 1 to 2", v)),
        _ => tee_variant(a)?,
    };
    if let Some((v, h)) = arms {
        *j = Junction::corner(j.pos, v, h);
        j.id = id;
    }
    if let Some(d) = dir.or(line) {
        j.dir = d;
    }
    if let Some(v) = variant {
        j.variant = v;
    }
    if let Some(d) = dot {
        j.dot = d;
    }
    let j = j.clone();
    Ok(tee_json(prj, i, &j))
}

fn list_connection_symbols(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let page = a.str("page").map(|name| page_index(prj, name)).transpose()?;
    let list: Vec<Value> = prj
        .pages
        .iter()
        .enumerate()
        .filter(|(i, _)| page.is_none_or(|p| p == *i))
        .flat_map(|(i, pg)| pg.junctions.iter().map(move |j| (i, j)))
        .map(|(i, j)| tee_json(prj, i, j))
        .collect();
    Ok(Value::Array(list))
}

#[cfg(test)]
mod tests;
