//! Report block and scheme tools: report pages generated per strip, plug, cable or
//! device, named report forms (filter, sort order, grouping) and the output of report
//! pages into other projects.

use super::common::*;
use super::{optional, param, Args, Kind, Tool};
use crate::session::Session;
use openecraft_core::forms::{self, ReportForm, ReportScheme};
use openecraft_core::report_blocks::{self, ReportBlock};
use openecraft_core::analysis::Analysis;
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};
use openecraft_api::builtin::reports::{block_json};

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "set_report_block",
        help: tr_const!("Creates or changes a report block (report pages generated together, e.g. one terminal diagram per strip)"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the block")),
            optional("kind", Kind::Str, tr_const!("Report, e.g. terminal_diagram, terminal_lineup, cable_connections (required for a new block)")),
            optional("scheme", Kind::Str, tr_const!("Report scheme; empty: the form of the report")),
            optional("per_group", Kind::Bool, tr_const!("New pages for every value of the group column (default true)")),
            optional("groups", Kind::Str, tr_const!("Groups to generate, wildcards allowed (-X*, -W1); empty: all")),
            optional("function", Kind::Str, tr_const!("Function (=) of the generated pages")),
            optional("location", Kind::Str, tr_const!("Location (+) of the generated pages")),
        ],
        needs_project: true,
        run: set_report_block,
    },
    Tool {
        name: "generate_report_blocks",
        help: tr_const!("Generates the pages of the report blocks again (replacing the pages they generated before)"),
        params: &[optional("name", Kind::Str, tr_const!("Only this block"))],
        needs_project: true,
        run: generate_report_blocks,
    },
    Tool {
        name: "set_report_scheme",
        help: tr_const!("Creates or replaces a report scheme"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the scheme")),
            param("kind", Kind::Str, tr_const!("Report the scheme belongs to")),
            param("form", Kind::Object, tr_const!("Form: title, columns, rows, filters [{column, op, value}], sort [{column, descending}], group, blank_repeats, widths, text_size, graphic (terminal and cable diagrams as drawings), totals [columns], shade_rows, align {column: Left, Center or Right}")),
        ],
        needs_project: true,
        run: set_report_scheme,
    },
    Tool {
        name: "output_reports",
        help: tr_const!("Outputs report pages as fixed pages into another project file; pages output from this project before are replaced"),
        params: &[
            param("target", Kind::Str, tr_const!("Project file that gets the pages")),
            optional("block", Kind::Str, tr_const!("Only the pages of this report block; default all report pages")),
        ],
        needs_project: true,
        run: output_reports,
    },
];

fn set_report_block(s: &mut Session, a: &Args) -> Result<Value, String> {
    let kind = a.str("kind").map(|k| report_id(&s.plugins, k)).transpose()?;
    let prj = s.project_mut()?;
    let name = a.req("name").trim().to_string();
    let index = match prj.report_blocks.iter().position(|b| b.name == name) {
        Some(i) => i,
        None => {
            let kind = kind.ok_or_else(|| tr_fmt!("New report block {} needs a kind", name))?;
            prj.report_blocks.push(ReportBlock { name: name.clone(), kind, ..Default::default() });
            prj.report_blocks.len() - 1
        }
    };
    let b = &mut prj.report_blocks[index];
    if let Some(k) = kind.filter(|k| *k != b.kind) {
        b.kind = k;
        b.scheme.clear();
    }
    if let Some(v) = a.str("scheme") {
        b.scheme = v.to_string();
    }
    if a.value("per_group").is_some() {
        b.per_group = a.flag("per_group");
    }
    for (field, value) in [("groups", &mut b.groups), ("function", &mut b.function), ("location", &mut b.location)] {
        if let Some(v) = a.str(field) {
            *value = v.trim_start_matches(['=', '+']).to_string();
        }
    }
    let b = prj.report_blocks[index].clone();
    if !b.scheme.is_empty() && !forms::scheme_names(prj, b.kind).contains(&b.scheme) {
        return Err(tr_fmt!("Unknown report scheme: {}", b.scheme));
    }
    Ok(block_json(prj, &b))
}

fn generate_report_blocks(s: &mut Session, a: &Args) -> Result<Value, String> {
    let reports: Vec<openecraft_api::Report> = s.plugins.reports().cloned().collect();
    let prj = s.project_mut()?;
    let pages = match a.str("name") {
        Some(name) => {
            let i = prj.report_blocks.iter().position(|b| b.name == name).ok_or_else(|| tr_fmt!("No report block named {}", name))?;
            let mut an = Analysis::build(prj);
            an.set_reports(reports);
            report_blocks::generate(prj, &an, i)?
        }
        None => report_blocks::generate_all_with(prj, reports)?,
    };
    Ok(json!({ "pages": pages }))
}

fn set_report_scheme(s: &mut Session, a: &Args) -> Result<Value, String> {
    let kind = report_id(&s.plugins, a.req("kind"))?;
    let prj = s.project_mut()?;
    let name = a.req("name").trim().to_string();
    let form: ReportForm = serde_json::from_value(a.value("form").cloned().unwrap_or_default()).map_err(|e| tr_fmt!("Invalid form: {}", e))?;
    let scheme = ReportScheme { name: name.clone(), kind, form };
    match prj.forms.schemes.iter_mut().find(|s| s.kind == kind && s.name == name) {
        Some(s) => *s = scheme,
        None => prj.forms.schemes.push(scheme),
    }
    Ok(json!({ "name": name, "kind": kind.as_str() }))
}

fn output_reports(s: &mut Session, a: &Args) -> Result<Value, String> {
    let prj = s.project()?;
    let target = a.req("target");
    let block = a.str("block");
    if let Some(b) = block.filter(|b| !prj.report_blocks.iter().any(|x| x.name == *b)) {
        return Err(tr_fmt!("No report block named {}", b));
    }
    let pages = report_blocks::report_pages(prj, block);
    let text = std::fs::read_to_string(target).map_err(|e| tr_fmt!("Cannot read {}: {}", target, openecraft_i18n::io_error(&e)))?;
    let mut an = Analysis::build(prj);
    an.set_reports(s.plugins.reports().cloned());
    let (json, n) = report_blocks::output_to_file(prj, &an, &pages, &text)?;
    std::fs::write(target, json).map_err(|e| tr_fmt!("Cannot write {}: {}", target, openecraft_i18n::io_error(&e)))?;
    Ok(json!({ "target": target, "pages": n }))
}

#[cfg(test)]
mod tests;
