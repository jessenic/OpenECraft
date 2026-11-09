//! Page commands: list, show, add, change and delete pages and page sections.

use crate::builtin::common::*;
use crate::params::{optional, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::model::*;
use openecraft_core::netlist::Netlist;
use openecraft_core::structure::{Aspect, Dt};
use openecraft_core::numbering::{self, PageNumbering};
use openecraft_core::{frame, ops, potentials, sections};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Pages;

impl Plugin for Pages {
    fn info(&self) -> Info {
        info("pages", tr_const!("Pages"), tr_const!("Lists, changes and deletes pages and page sections"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_pages",
        help: tr_const!("Lists the pages of the project"),
        params: &[optional("work_area", Kind::Str, tr_const!("Only the pages of this work area (list_structure)"))],
        run: list_pages,
    },
    Spec {
        name: "get_page",
        help: tr_const!("Shows the content of a page: symbols with their connection point positions, wires and texts"),
        params: &[PAGE],
        run: get_page,
    },
    Spec {
        name: "set_page",
        help: tr_const!("Changes name, description or structure of a page"),
        params: &[
            PAGE,
            optional("name", Kind::Str, tr_const!("New page name")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("frame", Kind::Str, tr_const!("Plot frame (list_frames); empty = the project default")),
            optional("function", Kind::Str, tr_const!("Function aspect (=) of the page; the devices on the page move along")),
            optional("installation", Kind::Str, tr_const!("Installation site aspect (++) of the page; the devices on the page move along")),
            optional("location", Kind::Str, tr_const!("Location aspect (+) of the page; the devices on the page move along")),
            optional("product", Kind::Str, tr_const!("Higher-level product aspect (-) of the page; the devices on the page move along")),
            optional("section", Kind::Str, tr_const!("Moves the page into this section (list_sections; empty = top level); its name gets the section number")),
            optional("before", Kind::Str, tr_const!("Moves the page before this page of its section (or of the section given)")),
        ],
        run: set_page,
    },
    Spec {
        name: "delete_page",
        help: tr_const!("Deletes a page"),
        params: &[PAGE],
        run: delete_page,
    },
    Spec {
        name: "set_cross_references",
        help: tr_const!("Sets how cross references are shown (only the given settings) and shows the settings"),
        params: &[
            optional("prefix", Kind::Str, tr_const!("In front of the page name (default /)")),
            optional("separator", Kind::Str, tr_const!("Between page and column (default .)")),
            optional("row", Kind::Bool, tr_const!("Show the row after the column")),
            optional("structure", Kind::Str, tr_const!("Structure of the target page: different (default), always or never")),
            optional("interruptions", Kind::Str, tr_const!("Interruption points refer to: all (default), star or pairs")),
            optional("contact_image", Kind::Bool, tr_const!("Draw the contact image below coils (default true)")),
        ],
        run: set_cross_references,
    },
    Spec {
        name: "list_cross_references",
        help: tr_const!("Cross references"),
        params: &[
            optional("id", Kind::Int, tr_const!("Symbol ID")),
            optional("page", Kind::Str, tr_const!("Page")),
        ],
        run: list_cross_references,
    },
    Spec {
        name: "renumber_pages",
        help: tr_const!("Numbers the pages with a start value and step; released pages keep their name"),
        params: &[
            optional("start", Kind::Int, tr_const!("First page number (default 1)")),
            optional("step", Kind::Int, tr_const!("Counting step (default 1)")),
            optional("per_structure", Kind::Bool, tr_const!("Start again on every page whose structure differs from the page before")),
            optional("subpages", Kind::Bool, tr_const!("Pages with a suffix (3.a) stay sub-pages of the page before (default true)")),
            optional("sections", Kind::Bool, tr_const!("Number the sections 1, 2, 3 … on every level first")),
            optional("pages", Kind::Str, tr_const!("Only these pages (names separated by commas)")),
        ],
        run: renumber_pages,
    },
    Spec {
        name: "list_sections",
        help: tr_const!("Lists the page sections in tree order; the page names start with the section number"),
        params: &[],
        run: list_sections,
    },
    Spec {
        name: "add_section",
        help: tr_const!("Adds a page section with the next free number"),
        params: &[
            optional("parent", Kind::Str, tr_const!("Section to add a subsection to (default: top level)")),
            optional("number", Kind::Str, tr_const!("Number within the parent (default: the next free one)")),
            optional("description", Kind::Str, tr_const!("Description")),
        ],
        run: add_section,
    },
    Spec {
        name: "set_section",
        help: tr_const!("Changes number, description or parent of a page section; the names of its pages follow"),
        params: &[
            SECTION,
            optional("number", Kind::Str, tr_const!("New number within the parent")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("parent", Kind::Str, tr_const!("Moves the section into this one (empty = top level)")),
            optional("move", Kind::Str, tr_const!("up or down: swaps place and number with the neighbouring section")),
            optional("before", Kind::Str, tr_const!("Moves the section before this section of its parent (or of the parent given); the numbers keep counting in order")),
        ],
        run: set_section,
    },
    Spec {
        name: "delete_section",
        help: tr_const!("Deletes a page section; its pages and subsections go to the section above"),
        params: &[SECTION],
        run: delete_section,
    },
];

fn section_json(prj: &Project, id: u64) -> Value {
    let s = sections::get(prj, id).cloned().unwrap_or_default();
    json!({
        "number": sections::full_number(prj, id),
        "description": s.description,
        "parent": sections::full_number(prj, s.parent),
        "pages": prj.pages.iter().filter(|p| p.section == id).map(|p| p.name.clone()).collect::<Vec<_>>(),
    })
}

fn list_sections(prj: &mut Project, _: &Args) -> Result<Value, String> {
    Ok(sections::tree(prj).into_iter().map(|(id, _)| section_json(prj, id)).collect())
}

fn add_section(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let parent = section_id(prj, a.str("parent").unwrap_or_default())?;
    let id = sections::add(prj, parent, a.str("description").unwrap_or_default());
    if let Some(n) = a.str("number") {
        if let Err(e) = sections::set_number(prj, id, n) {
            prj.sections.retain(|s| s.id != id);
            return Err(tr(e).into());
        }
    }
    Ok(section_json(prj, id))
}

fn set_section(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let id = section_id(prj, a.req("section"))?;
    if id == 0 {
        return Err(tr("Give a section").into());
    }
    let before = match a.str("before").map(str::trim) {
        Some(b) if !b.is_empty() => Some((b, section_id(prj, b)?)),
        _ => None,
    };
    let parent_of = |prj: &Project, s: u64| sections::get(prj, s).map_or(0, |s| s.parent);
    match (a.str("parent"), before) {
        (p, Some((name, b))) => {
            let parent = match p {
                Some(p) => section_id(prj, p)?,
                None => parent_of(prj, b),
            };
            if parent_of(prj, b) != parent {
                return Err(tr_fmt!("{} is not in that section", name));
            }
            sections::move_section_to(prj, id, parent, Some(b)).map_err(tr)?;
        }
        (Some(p), None) => {
            let parent = section_id(prj, p)?;
            sections::set_parent(prj, id, parent).map_err(tr)?;
        }
        (None, None) => {}
    }
    if let Some(n) = a.str("number") {
        sections::set_number(prj, id, n).map_err(tr)?;
    }
    if let Some(d) = a.str("description") {
        if let Some(s) = prj.sections.iter_mut().find(|s| s.id == id) {
            s.description = d.into();
        }
    }
    match a.str("move").map(|m| m.trim().to_ascii_lowercase()) {
        Some(m) if m == "up" || m == "down" => {
            sections::move_section(prj, id, m == "up");
        }
        Some(m) => return Err(tr_fmt!("Unknown setting: {}", m)),
        None => {}
    }
    Ok(section_json(prj, id))
}

fn delete_section(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let id = section_id(prj, a.req("section"))?;
    if id == 0 {
        return Err(tr("Give a section").into());
    }
    let number = sections::full_number(prj, id);
    sections::delete(prj, id).map_err(tr)?;
    Ok(json!({ "deleted": number }))
}

fn list_pages(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let area = match a.str("work_area") {
        Some(name) => Some(prj.structures.work_area(name).ok_or_else(|| tr_fmt!("No work area named {}", name))?),
        None => None,
    };
    let pages = prj.pages.iter().enumerate().filter(|(_, pg)| area.is_none_or(|w| w.shows_page(pg))).map(|(i, pg)| {
        let mut v = page_json(pg);
        v["section"] = sections::full_number(prj, sections::of_page(prj, i)).into();
        v["symbols"] = pg.symbols.len().into();
        v["wires"] = pg.wires.len().into();
        v["texts"] = pg.texts.len().into();
        v
    });
    Ok(Value::Array(pages.collect()))
}

fn get_page(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let pg = &prj.pages[page_index(prj, a.req("page"))?];
    let (nl, open) = (Netlist::build(prj, true), Netlist::build(prj, false));
    let mut v = page_json(pg);
    let symbols = prj.symbol_table();
    v["symbols"] = pg.symbols.iter().map(|s| symbol_json(&symbols, s)).collect();
    v["wires"] = pg
        .wires
        .iter()
        .map(|w| {
            let auto = potentials::props_of_wire(prj, &nl, &open, w.id);
            json!({
                "id": w.id,
                "from": xy(w.a),
                "to": xy(w.b),
                "number": w.number,
                "color": w.color,
                "cross_section": w.cross_section,
                "cable": w.cable,
                "core": w.core,
                "potential": auto.potential,
                "resulting_color": auto.color,
                "resulting_cross_section": auto.cross_section,
            })
        })
        .collect();
    v["texts"] = pg.texts.iter().map(|t| json!({ "id": t.id, "x": t.pos.x, "y": t.pos.y, "text": t.text, "size": t.size })).collect();
    v["shapes"] = serde_json::to_value(&pg.shapes).unwrap_or_default();
    Ok(v)
}

fn set_page(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = unlocked(prj, a.req("page"))?;
    // Leading and trailing blanks are no part of a name.
    if let Some(name) = a.str("name").map(str::trim) {
        if name.is_empty() {
            return Err(tr("A page needs a name").to_string());
        }
        if prj.pages.iter().enumerate().any(|(j, pg)| j != i && pg.name == name) {
            return Err(tr_fmt!("A page named {} already exists", name));
        }
        prj.pages[i].name = name.into();
    }
    if let Some(d) = a.str("description") {
        prj.pages[i].description = d.into();
    }
    if let Some(f) = a.str("frame") {
        if !f.is_empty() && !frame::names(prj).iter().any(|n| n == f) {
            return Err(tr_fmt!("No plot frame named {}", f));
        }
        prj.pages[i].frame = f.into();
    }
    if Aspect::ALL.iter().any(|asp| a.str(asp.key()).is_some()) {
        let mut dt = Dt::of_page(&prj.pages[i]);
        for asp in Aspect::ALL {
            if let Some(v) = a.str(asp.key()) {
                *dt.get_mut(asp) = openecraft_core::structure::check_identifier(asp, v)?;
            }
        }
        ops::set_page_structure(prj, i, &dt, true);
    }
    let mut i = i;
    let before = match a.str("before").map(str::trim) {
        Some(b) if !b.is_empty() => Some((b, page_index(prj, b)?)),
        _ => None,
    };
    match (a.str("section"), before) {
        (s, Some((name, b))) => {
            let section = match s {
                Some(s) => section_id(prj, s)?,
                None => sections::of_page(prj, b),
            };
            if sections::of_page(prj, b) != section {
                return Err(tr_fmt!("{} is not in that section", name));
            }
            i = sections::move_page_to(prj, i, section, Some(b)).map_err(tr)?;
        }
        (Some(s), None) => {
            let section = section_id(prj, s)?;
            i = sections::move_page(prj, i, section).map_err(tr)?;
        }
        (None, None) => {}
    }
    let mut v = page_json(&prj.pages[i]);
    v["section"] = sections::full_number(prj, sections::of_page(prj, i)).into();
    Ok(v)
}

fn set_cross_references(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let x = &mut prj.xref;
    if let Some(v) = a.str("prefix") {
        x.prefix = v.to_string();
    }
    if let Some(v) = a.str("separator") {
        x.separator = v.to_string();
    }
    if let Some(v) = a.value("row").and_then(Value::as_bool) {
        x.row = v;
    }
    if let Some(v) = a.value("contact_image").and_then(Value::as_bool) {
        x.hide_contact_image = !v;
    }
    if let Some(v) = a.str("structure") {
        x.structure = match v.trim().to_ascii_lowercase().as_str() {
            "different" => XrefStructure::Different,
            "always" => XrefStructure::Always,
            "never" => XrefStructure::Never,
            other => return Err(tr_fmt!("Unknown setting: {}", other)),
        };
    }
    if let Some(v) = a.str("interruptions") {
        x.interruptions = match v.trim().to_ascii_lowercase().as_str() {
            "all" => InterruptionMode::All,
            "star" => InterruptionMode::Star,
            "pairs" => InterruptionMode::Pairs,
            other => return Err(tr_fmt!("Unknown setting: {}", other)),
        };
    }
    let structure = match x.structure {
        XrefStructure::Different => "different",
        XrefStructure::Always => "always",
        XrefStructure::Never => "never",
    };
    let interruptions = match x.interruptions {
        InterruptionMode::All => "all",
        InterruptionMode::Star => "star",
        InterruptionMode::Pairs => "pairs",
    };
    Ok(json!({ "prefix": x.prefix, "separator": x.separator, "row": x.row, "structure": structure, "interruptions": interruptions, "contact_image": !x.hide_contact_image }))
}

fn list_cross_references(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let selected_page = a.str("page").map(|name| page_index(prj, name)).transpose()?;
    let selected_id = a.int("id").map(|id| u64::try_from(id).map_err(|_| tr_fmt!("Not a symbol id: {}", id))).transpose()?;
    let idx = openecraft_core::index::Index::build(prj);
    let mut rows = Vec::new();
    for (pi, page) in prj.pages.iter().enumerate().filter(|(pi, _)| selected_page.is_none_or(|p| p == *pi)) {
        for symbol in page.symbols.iter().filter(|s| selected_id.is_none_or(|id| id == s.id)) {
            let Some(def) = prj.symbol(&symbol.sym) else { continue };
            let refs = openecraft_core::references::of_symbol(prj, &idx, pi, &def, symbol);
            rows.push(json!({"id":symbol.id,"page":page.name,"tag":symbol.tag,
                "text":openecraft_core::references::text(&refs),
                "references":refs.iter().map(|r|json!({"text":r.text,"page":r.page.map(|p|prj.pages[p].name.as_str())})).collect::<Vec<_>>() }));
        }
    }
    Ok(rows.into())
}

fn renumber_pages(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let defaults = PageNumbering::default();
    let start = a.int("start").unwrap_or(defaults.start.into());
    let step = a.int("step").unwrap_or(defaults.step.into());
    if !(0..=u32::MAX as i64).contains(&start) {
        return Err(tr("The first page number must not be negative").to_string());
    }
    if !(1..=u32::MAX as i64).contains(&step) {
        return Err(tr("The counting step must be at least 1").to_string());
    }
    let opts = PageNumbering {
        start: start as u32,
        step: step as u32,
        per_structure: a.flag("per_structure"),
        subpages: a.value("subpages").and_then(Value::as_bool).unwrap_or(defaults.subpages),
        sections: a.flag("sections"),
        pages: crate::builtin::devices::page_list(prj, a.str("pages"))?,
    };
    let n = numbering::renumber_pages(prj, &opts);
    let mut out = json!({ "renamed": n, "pages": prj.pages.iter().map(|pg| pg.name.clone()).collect::<Vec<_>>() });
    // Released pages keep their names: the numbers of the others count their places.
    let kept: Vec<String> = prj.pages.iter().enumerate().filter(|(i, pg)| pg.locked && (opts.pages.is_empty() || opts.pages.contains(i))).map(|(_, pg)| pg.name.clone()).collect();
    if !kept.is_empty() {
        out["kept"] = json!(kept);
        out["note"] = json!(tr_fmt!("Released pages keep their names: {}", kept.join(", ")));
    }
    Ok(out)
}

fn delete_page(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = page_index(prj, a.req("page"))?;
    if out_in_subproject(prj, i).is_some() {
        return Err(locked_reason(prj, i));
    }
    let pg = ops::delete_page(prj, i).map_err(tr)?;
    Ok(json!({ "deleted": pg.name }))
}
