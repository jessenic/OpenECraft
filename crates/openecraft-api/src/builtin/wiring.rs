//! Wiring assistant commands: the routed wires of the panel layout as a checklist with
//! the wiring state, marking wires as wired or as a problem.

use crate::builtin::common::{check_placeholders, int_in};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::analysis::Analysis;
use openecraft_core::model::potentials::PotentialKind;
use openecraft_core::model::{NumberScheme, Project};
use openecraft_core::production::{self, EndTreatment, Termination, Twin};
use openecraft_core::wiring::{self, Mark, Order, Status};
use openecraft_core::marking;
use openecraft_core::wirenum;
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Wiring;

impl Plugin for Wiring {
    fn info(&self) -> Info {
        info("wiring", tr_const!("Wiring"), tr_const!("Wire numbering and wiring marks"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "renumber_wires",
        help: tr_const!("Numbers the wires by the numbering scheme of the project"),
        params: &[optional("overwrite_fixed", Kind::Bool, tr_const!("Also renumber wires with a fixed number"))],
        run: renumber_wires,
    },
    Spec {
        name: "set_wire_numbering",
        help: tr_const!("Sets the numbering scheme of the wires (only the given settings) and shows it"),
        params: &[
            optional("scheme", Kind::Str, tr_const!("sequential, page_column, potential or format")),
            optional("format", Kind::Str, tr_const!("Format of the scheme format: {n} (counter), {page}, {column}, {row}, {potential}, {kind}, {color}, {cross_section}")),
            optional("prefix", Kind::Str, tr_const!("Put in front of the numbers, e.g. W")),
            optional("start", Kind::Int, tr_const!("First counter value (default 1)")),
            optional("step", Kind::Int, tr_const!("Counting step (default 1)")),
            optional("digits", Kind::Int, tr_const!("Minimum digits of the counter (leading zeros)")),
            optional("skip_kinds", Kind::Str, tr_const!("Potential kinds whose connections get no number, separated by commas (l, n, pe, dc_plus, dc_minus, signal)")),
        ],
        run: set_wire_numbering,
    },
    Spec {
        name: "wiring_list",
        help: tr_const!("Lists the routed wires of the panel layout as a wiring checklist with end treatment, state and progress"),
        params: &[
            optional("enclosure", Kind::Str, tr_const!("Enclosure name (all if missing)")),
            optional("order", Kind::Str, tr_const!("device (default), position, cross_section or number")),
            optional("status", Kind::Str, tr_const!("Only wires with this state: open, wired, changed or problem")),
        ],
        run: wiring_list,
    },
    Spec {
        name: "mark_wire",
        help: tr_const!("Marks a routed wire as wired or as a problem, or as open again; the wire is given by its two ends"),
        params: &[
            param("from", Kind::Str, tr_const!("One end, e.g. -K1:A1")),
            param("to", Kind::Str, tr_const!("Other end, e.g. -X1:3")),
            param("state", Kind::Str, tr_const!("wired, problem or open")),
            optional("note", Kind::Str, tr_const!("Note, e.g. what the problem is")),
        ],
        run: mark_wire,
    },
    Spec {
        name: "harness_nailboard",
        help: tr_const!("Nailboard layout of a harness: the branches from the common node to the connectors with length, direction, bundle diameter and pins, and the drawing scale"),
        params: &[param("harness", Kind::Str, tr_const!("Harness tag, e.g. -WH1"))],
        run: harness_nailboard,
    },
    Spec {
        name: "marking_schemes",
        help: tr_const!("Lists the labeling schemes for marking programs (built-in and of the project) and the selected one"),
        params: &[],
        run: marking_schemes,
    },
    Spec {
        name: "set_marking_scheme",
        help: tr_const!("Adds or changes a labeling scheme of the project, removes it, or selects the scheme used for the marking export"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the scheme")),
            optional("columns", Kind::Str, tr_const!("Columns, comma separated: Material, Category, Text, Count, Enclosure, Group, No")),
            optional("separator", Kind::Str, tr_const!("Separator: semicolon, comma or tab")),
            optional("header", Kind::Bool, tr_const!("First row with the column names")),
            optional("one_row_per_label", Kind::Bool, tr_const!("One row per printed label instead of a count")),
            optional("categories", Kind::Str, tr_const!("Markers, comma separated: device, terminal, wire, cable (empty: all)")),
            optional("select", Kind::Bool, tr_const!("Use this scheme for the marking export")),
            optional("remove", Kind::Bool, tr_const!("Remove the scheme from the project")),
        ],
        run: set_marking_scheme,
    },
    Spec {
        name: "mounting_list",
        help: tr_const!("Lists the items of the panel layout in mounting order with place, size, part and mounting state"),
        params: &[optional("enclosure", Kind::Str, tr_const!("Enclosure name (all if missing)"))],
        run: mounting_list,
    },
    Spec {
        name: "mark_mounted",
        help: tr_const!("Marks a panel item as mounted or as a problem, or as open again"),
        params: &[
            param("item", Kind::Int, tr_const!("Id of the panel item (from mounting_list)")),
            param("state", Kind::Str, tr_const!("mounted, problem or open")),
            optional("note", Kind::Str, tr_const!("Note, e.g. what the problem is")),
        ],
        run: mark_mounted,
    },
    Spec {
        name: "set_termination",
        help: tr_const!("Chooses how wire ends are terminated: bare, with a ferrule or a crimped terminal; at a connection point, at all connection points of a device or for one wire only"),
        params: &[
            param("at", Kind::Str, tr_const!("Connection point, e.g. -K1:13, or a device, e.g. -K1 (all its connection points)")),
            param("kind", Kind::Str, tr_const!("bare, ferrule, twin_ferrule, ring, fork, pin, blade, receptacle, or auto (as the layout sets ferrules)")),
            optional("to", Kind::Str, tr_const!("Other end of the wire, e.g. -X1:3: only this wire")),
        ],
        run: set_termination,
    },
    Spec {
        name: "set_ferrules",
        help: tr_const!("Sets how wire ends without a chosen termination are terminated: with ferrules or bare, and with twin ferrules where two wires share a connection point"),
        params: &[
            optional("ferrules", Kind::Bool, tr_const!("Wire ends get ferrules")),
            optional("twin_ferrules", Kind::Bool, tr_const!("Two wires at a connection point share a twin ferrule")),
        ],
        run: set_ferrules,
    },
    Spec {
        name: "terminations",
        help: tr_const!("Lists the terminations chosen for wire ends and the kinds there are"),
        params: &[],
        run: terminations,
    },
    Spec {
        name: "remove_old_wiring_marks",
        help: tr_const!("Removes the wiring marks of wires that no longer exist"),
        params: &[],
        run: remove_old_marks,
    },
];

fn renumber_wires(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let an = Analysis::build(prj);
    let n = wirenum::renumber(prj, &an.open, &an.netlist, a.flag("overwrite_fixed"));
    Ok(json!({ "renumbered": n }))
}

fn scheme_code(s: NumberScheme) -> &'static str {
    match s {
        NumberScheme::Sequential => "sequential",
        NumberScheme::PageColumn => "page_column",
        NumberScheme::Potential => "potential",
        NumberScheme::Format => "format",
    }
}

fn kind_code(k: PotentialKind) -> &'static str {
    match k {
        PotentialKind::L => "l",
        PotentialKind::N => "n",
        PotentialKind::Pe => "pe",
        PotentialKind::DcPlus => "dc_plus",
        PotentialKind::DcMinus => "dc_minus",
        PotentialKind::Signal => "signal",
        PotentialKind::Other => "other",
    }
}

fn set_wire_numbering(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let s = &mut prj.wire_numbering;
    if let Some(v) = a.str("scheme") {
        let v = v.trim().to_ascii_lowercase();
        s.scheme = NumberScheme::ALL.into_iter().find(|x| scheme_code(*x) == v).ok_or_else(|| tr_fmt!("Unknown numbering scheme: {}", v))?;
    }
    if let Some(v) = a.str("format") {
        check_placeholders(v, &["n", "page", "column", "row", "potential", "kind", "color", "cross_section"])?;
        s.format = v.trim().to_string();
    }
    if let Some(v) = a.str("prefix") {
        s.prefix = v.trim().to_string();
    }
    if let Some(v) = int_in(a, "start", 0, u32::MAX.into())? {
        s.start = v as u32;
    }
    if let Some(v) = int_in(a, "step", 1, u32::MAX.into())? {
        s.step = v as u32;
    }
    if let Some(v) = int_in(a, "digits", 0, 9)? {
        s.digits = v as u8;
    }
    if let Some(v) = a.str("skip_kinds") {
        let mut kinds = Vec::new();
        for code in crate::builtin::common::words(v).into_iter().map(str::to_ascii_lowercase) {
            kinds.push(PotentialKind::ALL.into_iter().find(|k| kind_code(*k) == code).ok_or_else(|| tr_fmt!("Unknown potential kind: {}", code))?);
        }
        s.skip_kinds = kinds;
    }
    Ok(json!({
        "scheme": scheme_code(s.scheme), "format": s.format, "prefix": s.prefix, "start": s.start, "step": s.step, "digits": s.digits,
        "skip_kinds": s.skip_kinds.iter().map(|k| kind_code(*k)).collect::<Vec<_>>()
    }))
}

fn harness_nailboard(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag = a.req("harness");
    let an = Analysis::checked(prj);
    let nb = openecraft_core::nailboard::layout(prj, &an, tag).ok_or_else(|| tr_fmt!("Harness {} has no wires", tag))?;
    let branches: Vec<Value> = nb
        .branches
        .iter()
        .map(|b| {
            let pins: Vec<Value> = b.pins.iter().map(|p| json!({ "pin": p[0], "wire": p[1], "color": p[2], "cross_section": p[3], "to": p[4] })).collect();
            json!({ "connector": b.device, "length": b.length.round(), "assumed": b.assumed, "angle": b.angle.round(), "diameter": (b.diameter * 10.0).round() / 10.0, "pins": pins })
        })
        .collect();
    Ok(json!({ "harness": nb.tag, "wires": nb.wires, "diameter": (nb.diameter * 10.0).round() / 10.0, "scale": format!("1:{}", openecraft_core::nailboard::scale(&nb)), "branches": branches }))
}

fn scheme_json(s: &marking::Scheme) -> Value {
    let separator = match s.separator.as_str() {
        "\t" => "tab",
        "," => "comma",
        _ => "semicolon",
    };
    json!({
        "name": s.name, "builtin": marking::is_builtin(&s.name), "columns": s.columns.iter().map(|c| c.code()).collect::<Vec<_>>(), "separator": separator,
        "header": s.header, "one_row_per_label": s.one_row_per_label, "categories": s.categories.iter().map(|c| c.code()).collect::<Vec<_>>()
    })
}

fn marking_schemes(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let selected = marking::scheme(&prj.marking, None).map(|s| s.name).unwrap_or_default();
    Ok(json!({ "selected": selected, "schemes": marking::schemes(&prj.marking).iter().map(scheme_json).collect::<Vec<_>>() }))
}

fn set_marking_scheme(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if name.is_empty() {
        return Err(tr_fmt!("Parameter {} has the wrong type", "name"));
    }
    if a.flag("remove") {
        let before = prj.marking.schemes.len();
        prj.marking.schemes.retain(|s| s.name != name);
        if prj.marking.schemes.len() == before {
            return Err(tr_fmt!("No labeling scheme named {}", name));
        }
        if prj.marking.scheme == name && !marking::is_builtin(&name) {
            prj.marking.scheme.clear();
        }
        return marking_schemes(prj, a);
    }
    let mut s = marking::scheme(&prj.marking, Some(&name)).unwrap_or_else(|| marking::Scheme { name: name.clone(), ..Default::default() });
    let changes = ["columns", "separator", "header", "one_row_per_label", "categories"].iter().any(|k| a.value(k).is_some());
    if let Some(cols) = a.str("columns") {
        s.columns = crate::builtin::common::words(cols).into_iter().map(|c| marking::Column::parse(c).ok_or_else(|| tr_fmt!("Unknown column: {}", c))).collect::<Result<_, _>>()?;
        if s.columns.is_empty() {
            return Err(tr_fmt!("Unknown column: {}", cols));
        }
    }
    if let Some(sep) = a.str("separator") {
        s.separator = match sep.trim() {
            "semicolon" | ";" => ";",
            "comma" | "," => ",",
            "tab" => "\t",
            other => return Err(tr_fmt!("Unknown separator: {}", other)),
        }
        .into();
    }
    if a.value("header").is_some() {
        s.header = a.flag("header");
    }
    if a.value("one_row_per_label").is_some() {
        s.one_row_per_label = a.flag("one_row_per_label");
    }
    if let Some(cats) = a.str("categories") {
        s.categories = crate::builtin::common::words(cats).into_iter().map(|c| marking::Category::parse(c).ok_or_else(|| tr_fmt!("Unknown marker category: {}", c))).collect::<Result<_, _>>()?;
    }
    if changes || !marking::is_builtin(&name) {
        match prj.marking.schemes.iter_mut().find(|x| x.name == name) {
            Some(x) => *x = s,
            None => prj.marking.schemes.push(s),
        }
    }
    if a.flag("select") {
        prj.marking.scheme = name;
    }
    marking_schemes(prj, a)
}

fn mounting_list(prj: &mut Project, a: &Args) -> Result<Value, String> {
    use openecraft_core::mounting;
    let enclosure = match a.str("enclosure") {
        Some(name) => Some(prj.layout.enclosures.iter().position(|e| e.name == name).ok_or_else(|| tr_fmt!("No enclosure named {}", name))?),
        None => None,
    };
    let code = |s: mounting::Status| match s {
        mounting::Status::Open => "open",
        mounting::Status::Done => "mounted",
        mounting::Status::Changed => "changed",
        mounting::Status::Problem => "problem",
    };
    let steps: Vec<Value> = mounting::steps(prj, enclosure)
        .into_iter()
        .map(|s| {
            let e = &prj.layout.enclosures[s.enclosure];
            let it = &e.items[s.item];
            json!({
                "enclosure": e.name, "item": it.id, "title": mounting::item_title(it), "phase": s.phase.title(), "part": it.part,
                "face": format!("{:?}", it.face).to_lowercase(), "x": it.x, "y": it.y, "width": it.w, "height": it.h, "state": code(s.status), "note": s.note
            })
        })
        .collect();
    let p = mounting::progress(prj, enclosure);
    Ok(json!({ "items": steps, "total": p.total, "mounted": p.done - p.changed, "changed": p.changed, "problems": p.problems }))
}

fn mark_mounted(prj: &mut Project, a: &Args) -> Result<Value, String> {
    use openecraft_core::mounting::{self, Mark};
    let id = a.int("item").unwrap_or_default() as u64;
    let it = prj.layout.enclosures.iter().flat_map(|e| e.items.iter()).find(|i| i.id == id).cloned().ok_or_else(|| tr_fmt!("No panel item with id {}", id))?;
    let mark = match a.req("state").trim() {
        "mounted" => Some(Mark::Done),
        "problem" => Some(Mark::Problem),
        "open" => None,
        other => return Err(tr_fmt!("Unknown state: {}", other)),
    };
    mounting::set_mark(prj, &it, mark, a.str("note").unwrap_or_default());
    Ok(json!({ "item": id, "state": mounting::status(prj, &it).title() }))
}

fn status_code(s: Status) -> &'static str {
    match s {
        Status::Open => "open",
        Status::Done => "wired",
        Status::Changed => "changed",
        Status::Problem => "problem",
    }
}

fn wiring_list(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let an = Analysis::build(prj);
    let enclosure = match a.str("enclosure") {
        Some(name) => Some(prj.layout.enclosures.iter().position(|e| e.name == name).ok_or_else(|| tr_fmt!("No enclosure named {}", name))?),
        None => None,
    };
    let order = match a.str("order") {
        Some(o) => Order::from_code(o).ok_or_else(|| tr_fmt!("Unknown order: {}", o))?,
        None => Order::Device,
    };
    let filter = a.str("status").map(|s| s.trim().to_ascii_lowercase());
    let steps: Vec<Value> = wiring::steps(prj, &an.ends, &an.wiring, enclosure, order)
        .into_iter()
        .filter(|st| filter.as_deref().is_none_or(|f| status_code(st.status) == f))
        .map(|st| {
            let w = &an.wiring[st.wire];
            let ends = [end_json(&w.from, st.ends[0], st.twins[0].as_ref()), end_json(&w.to, st.ends[1], st.twins[1].as_ref())];
            json!({
                "enclosure": w.enclosure_names(&prj.layout),
                "from": w.from,
                "to": w.to,
                "number": w.number,
                "potential": w.net,
                "color": w.color,
                "cross_section": w.cross_section,
                "length_mm": w.length.round(),
                "strip_mm": st.ends[0].map(|e| e.strip),
                "ferrule": st.ends[0].and_then(|e| e.ferrule).map(|f| f.part),
                "ends": ends,
                "status": status_code(st.status),
                "note": st.note,
            })
        })
        .collect();
    let p = wiring::progress(prj, &an.wiring, enclosure);
    Ok(json!({
        "wires": steps,
        "progress": {
            "total": p.total,
            "wired": p.done - p.changed,
            "changed": p.changed,
            "problems": p.problems,
            "percent": (p.fraction() * 1000.0).round() / 10.0,
            "length_wired_m": (p.length_done / 10.0).round() / 100.0,
            "length_total_m": (p.length_total / 10.0).round() / 100.0,
        },
        "old_marks": wiring::stale(prj, &an.wiring).len(),
    }))
}

fn mark_wire(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let mark = match a.req("state").trim().to_ascii_lowercase().as_str() {
        "wired" | "done" => Some(Mark::Done),
        "problem" => Some(Mark::Problem),
        "open" => None,
        x => return Err(tr_fmt!("Unknown state: {}", x)),
    };
    let an = Analysis::build(prj);
    let (from, to) = (a.req("from").trim(), a.req("to").trim());
    let w = an
        .wiring
        .iter()
        .find(|w| (w.from == from && w.to == to) || (w.from == to && w.to == from))
        .ok_or_else(|| tr_fmt!("No routed wire {} – {}", from, to))?;
    wiring::set_mark(prj, w, mark, a.str("note").unwrap_or_default());
    let p = wiring::progress(prj, &an.wiring, None);
    Ok(json!({ "from": w.from, "to": w.to, "status": status_code(wiring::status(prj, w)), "wired": p.done - p.changed, "total": p.total }))
}

/// A wire end with its termination, stripping length and part, and the other wire
/// sharing its twin ferrule (the part is given at the first of them).
fn end_json(at: &str, e: Option<EndTreatment>, twin: Option<&Twin>) -> Value {
    json!({
        "at": at,
        "termination": e.map(|e| e.kind.key()),
        "strip_mm": e.map(|e| e.strip),
        "part": e.filter(|e| !e.shared).map(|e| e.part()).filter(|p| !p.is_empty()),
        "description": e.map(|e| e.describe()),
        "twin_with": twin.map(|t| t.with.clone()),
    })
}

fn set_termination(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let at = a.req("at").trim().to_string();
    let to = a.str("to").unwrap_or_default().trim().to_string();
    if at.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", "at"));
    }
    let kind = match a.req("kind").trim() {
        "auto" | "" => None,
        k => Some(Termination::from_key(k).ok_or_else(|| tr_fmt!("Unknown termination: {}", k))?),
    };
    let an = Analysis::build(prj);
    let wire_ends = &an.ends;
    production::choose_termination(prj, wire_ends, &at, &to, kind);
    // The wire ends it applies to now.
    let on = |end: &str, other: &str| (end == at || end.rsplit_once(':').is_some_and(|(d, _)| d == at)) && (to.is_empty() || other == to);
    let ends: Vec<Value> = an
        .wiring
        .iter()
        .flat_map(|w| [(&w.from, &w.to), (&w.to, &w.from)].map(|(end, other)| (end.clone(), other.clone(), w.cross_section.clone())))
        .filter(|(end, other, _)| on(end, other))
        .map(|(end, other, cs)| end_json(&end, wire_ends.end(prj, &end, &other, &cs), wire_ends.twin(prj, &end, &other).as_ref()))
        .collect();
    Ok(json!({ "at": at, "to": to, "termination": kind.map(|k| k.key()).unwrap_or("auto"), "ends": ends }))
}

fn terminations(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let chosen: Vec<Value> = prj.layout.terminations.iter().map(|t| json!({ "at": t.at, "to": t.to, "termination": t.kind.key() })).collect();
    let kinds: Vec<Value> = Termination::ALL.iter().map(|k| json!({ "key": k.key(), "name": k.title() })).collect();
    Ok(json!({ "ferrules": prj.layout.ferrules, "twin_ferrules": prj.layout.twin_ferrules, "chosen": chosen, "kinds": kinds }))
}

fn set_ferrules(prj: &mut Project, a: &Args) -> Result<Value, String> {
    if let Some(on) = a.value("ferrules").and_then(Value::as_bool) {
        prj.layout.ferrules = on;
    }
    if let Some(on) = a.value("twin_ferrules").and_then(Value::as_bool) {
        prj.layout.twin_ferrules = on;
    }
    Ok(json!({ "ferrules": prj.layout.ferrules, "twin_ferrules": prj.layout.twin_ferrules }))
}

fn remove_old_marks(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let an = Analysis::build(prj);
    let removed = wiring::remove_stale(prj, &an.wiring);
    Ok(json!({ "removed": removed }))
}

#[cfg(test)]
mod tests;
