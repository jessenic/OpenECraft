//! Project templates (in the spirit of EPLAN's basic projects): a project whose
//! settings, forms, plot frames, structure, potentials, numbering schemes, symbols,
//! property definitions and standard pages start new projects. A template is an
//! ordinary project file; [`as_template`] removes what belongs to one project only
//! (revisions, baselines, review, wiring marks, accepted check messages, subprojects),
//! [`from_template`] starts a project from it. Built-in templates cover the usual
//! starting points.

use crate::model::{Page, PageKind, Project, ReportKind};
use openecraft_i18n::{tr, tr_const};

/// Built-in templates: (name, description).
pub const BUILTIN: [(&str, &str); 3] = [
    (tr_const!("Empty (IEC 60204-1)"), tr_const!("One schematic page, the potentials of IEC 60204-1")),
    (tr_const!("Machine"), tr_const!("Cover sheet, table of contents, schematic pages with function and location, device list, bill of materials, terminal and cable diagrams")),
    (tr_const!("Distribution board (IEC 61439)"), tr_const!("Cover sheet, single-line overview, schematic pages, bill of materials, enclosure climate and short circuit reports")),
];

/// The project without what belongs to one project only.
pub fn as_template(prj: &Project) -> Project {
    let mut t = prj.clone();
    t.revisions.clear();
    t.baselines.clear();
    t.review = Default::default();
    t.wiring_marks.clear();
    t.mounting_marks.clear();
    t.preplanning.segments.clear();
    t.subprojects = Default::default();
    t.check_settings.accepted.clear();
    t.info.date.clear();
    t.info.revision = "A".into();
    for pg in &mut t.pages {
        pg.locked = false;
    }
    t
}

/// A new project from a template: its name and date set, released pages unlocked.
pub fn from_template(template: &Project, name: &str, date: &str) -> Project {
    let mut prj = as_template(template);
    if !name.trim().is_empty() {
        prj.info.name = name.trim().to_string();
    }
    prj.info.date = date.to_string();
    // A new project counts its ids on, whatever the template was read from.
    prj.id_floor = Default::default();
    prj.normalize();
    prj
}

fn add(prj: &mut Project, name: &str, description: &str, kind: PageKind, function: &str, location: &str) {
    let id = prj.new_id();
    let mut pg = Page::new(id, name, description, kind);
    pg.function = function.into();
    pg.location = location.into();
    prj.pages.push(pg);
}

/// A built-in template by name (English or translated); None for an unknown name.
pub fn builtin(name: &str) -> Option<Project> {
    let name = name.trim();
    let i = BUILTIN.iter().position(|(n, _)| n.eq_ignore_ascii_case(name) || tr(n).to_lowercase() == name.to_lowercase())?;
    let mut prj = Project::new_default();
    match i {
        0 => {}
        1 => {
            prj.pages.clear();
            crate::structure::apply_template(&mut prj.structures, crate::structure::template("Machine (example identifiers)")?);
            add(&mut prj, "1", tr("Cover sheet"), PageKind::Cover, "", "");
            add(&mut prj, "2", tr("Table of contents"), PageKind::Report(ReportKind::TableOfContents.into()), "", "");
            add(&mut prj, "3", tr("Power supply"), PageKind::Schematic, "POWER", "CAB1");
            add(&mut prj, "4", tr("Control"), PageKind::Schematic, "CTRL", "CAB1");
            add(&mut prj, "5", tr("Drives"), PageKind::Schematic, "DRIVE", "CAB1");
            for (n, k) in [("6", ReportKind::DeviceList), ("7", ReportKind::BillOfMaterials), ("8", ReportKind::TerminalDiagram), ("9", ReportKind::CableDiagram)] {
                add(&mut prj, n, k.title(), PageKind::Report(k.into()), "", "");
            }
        }
        _ => {
            prj.pages.clear();
            add(&mut prj, "1", tr("Cover sheet"), PageKind::Cover, "", "");
            add(&mut prj, "2", tr("Single-line diagram"), PageKind::Overview, "", "");
            add(&mut prj, "3", tr("Incoming supply"), PageKind::Schematic, "", "");
            add(&mut prj, "4", tr("Outgoing circuits"), PageKind::Schematic, "", "");
            for (n, k) in [("5", ReportKind::BillOfMaterials), ("6", ReportKind::Climate), ("7", ReportKind::ShortCircuit)] {
                add(&mut prj, n, k.title(), PageKind::Report(k.into()), "", "");
            }
        }
    }
    Some(prj)
}

#[cfg(test)]
mod tests;
