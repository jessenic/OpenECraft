//! Built-in plugins, compiled into every front end. New features that add commands,
//! checks, reports or exports register them here (see AGENTS.md, extensibility).

use crate::params::{Args, Param};
use crate::{Registry, Table};
use crate::{Info, Plugin, Text};
use openecraft_core::model::Project;
use openecraft_core::analysis::Analysis;
use openecraft_core::issue::Severity;
use openecraft_i18n::tr_const;
use serde_json::Value;

pub mod buildings;
pub mod check_settings;
pub mod common;
pub mod config;
pub mod device_boxes;
pub mod devices;
pub mod drawing;
pub mod electrical;
pub mod engineering;
pub mod field;
pub mod harnesses;
pub mod integration;
pub mod languages;
pub mod management;
pub mod pages;
pub mod panel;
pub mod plc;
pub mod plugs;
pub mod potentials;
pub mod process;
pub mod properties;
pub mod racks;
pub mod reports;
pub mod rooms;
pub mod routing;
pub mod sheets;
pub mod singleline;
pub mod structure;
pub mod tables;
pub mod terminals;
pub mod wiring;
mod lifecycle;
mod preplanning;
#[cfg(test)]
pub mod testing;
mod manufacturing;
mod panel_checks;
pub use lifecycle::PartLifecycle;
pub use manufacturing::PanelManufacturing;
pub use panel_checks::PanelChecks;

/// Parameters renamed so that all commands name alike (`tag` / `new_tag`, `name` /
/// `new_name`, `find` / `replace`): command, old name, current name. The command line
/// and the MCP server still take the old names.
pub const RENAMED_PARAMS: &[(&str, &str, &str)] = &[
    ("rename_device", "old", "tag"),
    ("rename_device", "new", "new_tag"),
    ("set_potential", "rename", "new_name"),
    ("set_fluid_group", "rename", "new_name"),
    ("set_pct_loop", "rename", "new_name"),
    ("replace_text", "text", "find"),
    ("replace_text", "with", "replace"),
];

/// The current name of the parameter `name` of `command` if it was renamed.
pub fn renamed_param(command: &str, name: &str) -> Option<&'static str> {
    RENAMED_PARAMS.iter().find(|(c, old, _)| *c == command && *old == name).map(|(_, _, new)| *new)
}

/// The arguments of `command` with renamed parameters under their current names
/// (an argument given under both names keeps the current one).
pub fn current_names(command: &str, args: &serde_json::Map<String, serde_json::Value>) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for (k, v) in args {
        let name = renamed_param(command, k).filter(|new| !args.contains_key(*new)).unwrap_or(k);
        out.insert(name.to_string(), v.clone());
    }
    out
}

/// Names of the checks of the built-in plugins.
pub fn check_names() -> Vec<String> {
    let mut reg = Registry::default();
    for p in plugins() {
        let _ = p.register(&mut reg);
    }
    reg.checks.into_iter().map(|c| c.name).collect()
}

pub fn plugins() -> Vec<Box<dyn Plugin>> {
    vec![Box::new(ProjectChecks), Box::new(ProjectReports), Box::new(ProjectExports), Box::new(Statistics), Box::new(PanelChecks), Box::new(PanelManufacturing), Box::new(PartLifecycle), Box::new(buildings::Buildings), Box::new(check_settings::CheckSettings), Box::new(config::Configurator), Box::new(device_boxes::DeviceBoxes), Box::new(devices::Devices), Box::new(drawing::Drawing), Box::new(electrical::Electrical), Box::new(engineering::Engineering), Box::new(field::FieldWiring), Box::new(harnesses::Harnesses), Box::new(integration::Integration), Box::new(languages::Languages), Box::new(management::Management), Box::new(pages::Pages), Box::new(panel::Panel), Box::new(plc::Plc), Box::new(plugs::Plugs), Box::new(potentials::Potentials), Box::new(preplanning::Preplanning), Box::new(process::Process), Box::new(properties::Properties), Box::new(racks::Racks), Box::new(reports::ReportBlocks), Box::new(rooms::Rooms), Box::new(routing::Routing), Box::new(sheets::Sheets), Box::new(singleline::SingleLine), Box::new(structure::Structure), Box::new(tables::Tables), Box::new(terminals::Terminals), Box::new(wiring::Wiring)]
}

/// The built-in reports of `openecraft_core::reports` as a plugin, so that the Reports
/// menu, report pages and the `report` tool list them next to the reports of plugins.
pub struct ProjectReports;

impl Plugin for ProjectReports {
    fn info(&self) -> Info {
        Info {
            id: "project_reports".into(),
            name: Text::Borrowed(tr_const!("Project reports")),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Text::Borrowed(tr_const!("Device list, bill of materials, terminal, plug, cable and connection diagrams, PLC, potentials, panel, fluid, electrical and field reports")),
        }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.reports.extend(openecraft_core::reports::builtin());
        Ok(())
    }
}

/// The built-in exports of `openecraft_core::exporters` as a plugin, so that File ›
/// Export and the `export` tool list them next to the exports of plugins.
pub struct ProjectExports;

impl Plugin for ProjectExports {
    fn info(&self) -> Info {
        Info {
            id: "project_exports".into(),
            name: Text::Borrowed(tr_const!("Project exports")),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Text::Borrowed(tr_const!("SVG, HTML, PDF, DXF and DWG of the pages, production and PLC lists, harness and PLC exchange files, the panel layout in 3D and the translation dictionary")),
        }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.exporters.extend(openecraft_core::exporters::builtin());
        Ok(())
    }
}

/// The built-in project checks of `openecraft_core::checks` as a plugin, so that the
/// front ends list and run them like every other check.
pub struct ProjectChecks;

impl Plugin for ProjectChecks {
    fn info(&self) -> Info {
        Info {
            id: "project_checks".into(),
            name: Text::Borrowed(tr_const!("Project checks")),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Text::Borrowed(tr_const!("Devices, wires, potentials, cables, terminal strips, PLC, panel, fluid, electrical calculations, structure and translations")),
        }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.checks.extend(openecraft_core::checks::builtin());
        Ok(())
    }
}

/// Key figures of the project: as command, report and text export of the device tags.
pub struct Statistics;

impl Plugin for Statistics {
    fn info(&self) -> Info {
        Info {
            id: "statistics".into(),
            name: Text::Borrowed(tr_const!("Project statistics")),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Text::Borrowed(tr_const!("Key figures of the project and a list of its device tags")),
        }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.command("project_statistics", tr_const!("Key figures of the project: pages, symbols, wires, devices, parts, cables and check results"), vec![], |prj, _| {
            Ok(figures(prj, &Analysis::checked(prj)).into_iter().map(|(key, _, n)| (key.to_string(), Value::from(n))).collect::<serde_json::Map<_, _>>().into())
        })
        .report("project_statistics", tr_const!("Project statistics"), |prj, an| {
            let mut table = Table::new([tr_const!("Figure"), tr_const!("Count")]);
            for (_, title, n) in figures(prj, an) {
                table.push(vec![openecraft_i18n::tr(title).to_string(), n.to_string()], None);
            }
            Ok(table)
        })
        .exporter("device_tags", tr_const!("Device tags (text)"), "txt", |prj| Ok(openecraft_core::structure::device_tags(prj).into_iter().map(|t| t + "\n").collect()));
        Ok(())
    }
}

/// (JSON key, title, value) of each figure.
fn figures(prj: &Project, an: &Analysis) -> Vec<(&'static str, &'static str, usize)> {
    let count = |s: Severity| an.issues.iter().filter(|i| i.severity == s).count();
    vec![
        ("pages", tr_const!("Pages"), prj.pages.len()),
        ("symbols", tr_const!("Symbols"), prj.pages.iter().map(|p| p.symbols.len()).sum()),
        ("wires", tr_const!("Wires"), prj.pages.iter().map(|p| p.wires.len()).sum()),
        ("devices", tr_const!("Devices"), openecraft_core::structure::device_tags(prj).len()),
        ("parts", tr_const!("Parts"), prj.parts.len()),
        ("cables", tr_const!("Cables"), prj.cables.len()),
        ("errors", tr_const!("Errors"), count(Severity::Error)),
        ("warnings", tr_const!("Warnings"), count(Severity::Warning)),
    ]
}

/// A built-in command: the data of one `Registry::command` call, so that an area lists
/// its commands in a table (`COMMANDS`) like the tools of the command line.
pub struct Spec {
    pub name: &'static str,
    pub help: &'static str,
    pub params: &'static [Param],
    pub run: fn(&mut Project, &Args) -> Result<Value, String>,
}

/// Registers the commands of a table.
pub fn add_commands(reg: &mut Registry, specs: &[Spec]) -> Result<(), String> {
    for s in specs {
        reg.command(s.name, s.help, s.params.to_vec(), s.run);
    }
    Ok(())
}

/// Info of a built-in plugin, with the version of this crate.
pub fn info(id: &str, name: &'static str, description: &'static str) -> Info {
    Info { id: id.into(), name: Text::Borrowed(name), version: env!("CARGO_PKG_VERSION").into(), description: Text::Borrowed(description) }
}

#[cfg(test)]
mod tests;
