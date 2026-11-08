//! The built-in report tables (device list, BOM, terminal diagram, ...) as registry
//! entries. The derived data they read is [`crate::analysis::Analysis`], the project
//! checks are in [`crate::checks`].

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::ext::Report;
use crate::index::Index;
use crate::panel::{self, OpenWire, RoutedWire};
use crate::model::*;
use crate::netlist::{pin_designation, Connection, Netlist, PinRef};
use crate::params::Text;
use crate::symbols::Func;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

mod diagrams;
mod electrical_reports;
mod fluid_reports;

pub use crate::analysis::Analysis;
pub use crate::issue::{Issue, Severity};
pub use crate::table::Table;

impl Analysis {
    /// The analysis with the built-in reports (hosts take the reports of their
    /// registry instead, see [`Analysis::set_reports`]).
    pub fn with_builtin(prj: &Project) -> Analysis {
        let mut an = Analysis::build(prj);
        an.set_reports(builtin());
        an
    }

    /// The analysis with the built-in reports and the issues of the built-in checks
    /// ([`crate::checks`]). Hosts with plugins fill `issues` from `Plugins::check`
    /// instead, which runs the same checks and those of the plugins.
    pub fn checked(prj: &Project) -> Analysis {
        let mut an = Analysis::with_builtin(prj);
        an.issues = crate::checks::run_builtin(prj, &an);
        an
    }
}

/// The table of a built-in report.
pub fn table(prj: &Project, an: &Analysis, kind: ReportKind) -> Table {
    let (idx, netlist, open, conns) = (&an.index, &an.netlist, &an.open, &an.connections);
    match kind {
        ReportKind::TableOfContents => toc(prj),
        ReportKind::DeviceList => device_list(prj, idx),
        ReportKind::BillOfMaterials => bom(prj, idx, &an.wiring, conns),
        ReportKind::TerminalDiagram => terminal_diagram(prj, netlist, open),
        ReportKind::TerminalLineup => diagrams::terminal_lineup(prj),
        ReportKind::TerminalStripOverview => diagrams::strip_overview(prj, netlist, open),
        ReportKind::TerminalConnections => diagrams::terminal_connections(prj, conns),
        ReportKind::PlugDiagram => plug_diagram(prj, netlist, open),
        ReportKind::PinConnections => diagrams::pin_connections(prj, conns),
        ReportKind::ConnectionList => connection_list(prj, conns),
        ReportKind::DeviceConnections => diagrams::device_connections(prj, idx, conns),
        ReportKind::PlcAssignment => plc_list(prj, netlist),
        ReportKind::RevisionOverview => revision_overview(prj),
        ReportKind::PotentialList => potential_list(prj, netlist),
        ReportKind::PanelWiring => panel_wiring(prj, &an.wiring, &an.open_wires),
        ReportKind::CableOverview => cable_overview(prj, conns),
        ReportKind::CableDiagram => cable_diagram(prj, conns),
        ReportKind::CableConnections => diagrams::cable_connections_report(prj, conns),
        ReportKind::CableSizing => cable_sizing(prj, conns),
        ReportKind::HarnessData => harness_data(prj, an),
        ReportKind::DuctFill => duct_fill_report(prj, &an.duct_fill),
        ReportKind::PanelLegend => diagrams::panel_legend(prj, idx),
        ReportKind::Climate => climate_report(prj),
        ReportKind::Marking => marking_report(&crate::marking::markers(prj, conns)),
        ReportKind::HoseList => hose_list(prj, open),
        ReportKind::PctLoops => pct_loops(prj, idx),
        ReportKind::FluidNetlist => fluid_reports::netlist(prj, open),
        ReportKind::HoseConfiguration => fluid_reports::configuration(prj, open),
        ReportKind::TubingList => fluid_reports::tubing(prj, open),
        ReportKind::FluidDeviceGroups => fluid_reports::groups(prj, idx),
        ReportKind::ElectroFluid => fluid_reports::electro_fluid(prj, idx),
        ReportKind::RoutingList => routing_list(prj, &an.routing),
        ReportKind::BundleList => bundle_list(&an.routing),
        ReportKind::ShortCircuit => electrical_reports::short_circuit(prj, netlist, conns),
        ReportKind::Selectivity => electrical_reports::selectivity(prj, netlist, conns),
        ReportKind::LoadCalculation => electrical_reports::load_calculation(prj, netlist),
        ReportKind::PowerSupplyBudget => electrical_reports::power_supply_budget(prj, netlist),
        ReportKind::SafetyFunctions => electrical_reports::safety_functions(prj),
        ReportKind::Verification => electrical_reports::verification(prj, conns),
        ReportKind::FieldRouting => field_routing(prj, &an.field),
        ReportKind::Bonding => bonding_list(prj),
        ReportKind::RackElevation => rack_elevation(prj),
    }
}

/// The built-in reports as registry entries, in the order of the Reports menu.
pub fn builtin() -> Vec<Report> {
    ReportKind::ALL.into_iter().map(|k| Report { name: k.name().into(), title: Text::Borrowed(k.title_key()), run: Arc::new(move |prj, an| Ok(table(prj, an, k))) }).collect()
}

fn hose_list(prj: &Project, open: &Netlist) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Hose no."), 0.7),
        (tr_const!("From"), 1.4),
        (tr_const!("To"), 1.4),
        (tr_const!("Medium"), 1.1),
        (tr_const!("Nominal size"), 0.8),
        (tr_const!("Colour"), 0.6),
        (tr_const!("Length (m)"), 0.7),
        (tr_const!("Page"), 0.6),
    ]);
    let mut total = 0.0;
    for h in crate::fluid::hoses(prj, open) {
        let length = h.effective_length();
        total += length;
        let s = &prj.pages[h.a.page].symbols[h.a.sym];
        t.push(
            vec![h.number, h.from, h.to, h.medium.title().into(), h.size, h.color, crate::fluid::length_text(length), loc_str(prj, h.a.page, s.pos)],
            Some((h.a.page, s.id)),
        );
    }
    if total > 0.0 {
        let mut last = vec![String::new(); 8];
        last[5] = tr("Total").into();
        last[6] = format!("{total:.2}");
        t.push(last, None);
    }
    t
}

/// Routing list: every routed wire, cable and tube with its ends, class, enclosures,
/// lengths and how it is routed (cut lengths for production).
fn routing_list(prj: &Project, r: &crate::routing3d::Routing) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Kind"), 0.5),
        (tr_const!("Designation"), 1.2),
        (tr_const!("From"), 1.0),
        (tr_const!("To"), 1.0),
        (tr_const!("Class"), 0.6),
        (tr_const!("Enclosures"), 0.9),
        (tr_const!("Length (mm)"), 0.7),
        (tr_const!("Cut length (mm)"), 0.7),
        (tr_const!("Free air (mm)"), 0.6),
        (tr_const!("Bundles"), 0.5),
        (tr_const!("Status"), 1.2),
    ]);
    for (i, it) in r.items.iter().enumerate() {
        let names: Vec<&str> = it.enclosures.iter().filter_map(|&k| prj.layout.enclosures.get(k)).map(|e| e.name.as_str()).collect();
        let routed = it.routed();
        let mm = |v: f32| if routed { format!("{v:.0}") } else { "–".into() };
        t.push(
            vec![
                it.kind.title().into(),
                it.label.clone(),
                it.ends[0].tag.clone(),
                it.ends[1].tag.clone(),
                it.class.title().into(),
                names.join(" / "),
                mm(it.length),
                mm(it.cut),
                mm(it.free),
                r.bundles_of(i).len().to_string(),
                it.status_text(),
            ],
            None,
        );
    }
    t
}

/// Field cables: ends, route through the network, lengths and status.
fn field_routing(prj: &Project, plan: &crate::field::Plan) -> Table {
    let f = &prj.field;
    let mut t = Table::with_widths(&[
        (tr_const!("Cable"), 0.8),
        (tr_const!("From"), 1.0),
        (tr_const!("To"), 1.0),
        (tr_const!("Class"), 0.6),
        (tr_const!("Route"), 2.6),
        (tr_const!("Network (m)"), 0.7),
        (tr_const!("Length (m)"), 0.7),
        (tr_const!("Cut length (m)"), 0.7),
        (tr_const!("Status"), 1.4),
    ]);
    for c in &plan.cables {
        let route: Vec<String> = c.segments.iter().map(|&s| f.segment_name(s)).collect();
        let m = |v: f32| if c.routed() { format!("{v:.1}") } else { "–".into() };
        t.push(
            vec![c.cable.clone(), c.ends[0].clone(), c.ends[1].clone(), c.class.title().into(), route.join(" › "), m(c.network), m(c.length), m(c.cut), c.status_text()],
            None,
        );
    }
    t
}

/// The 19-inch racks from the top unit down: what each part takes, then the free units.
fn rack_elevation(prj: &Project) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Rack"), 0.7),
        (tr_const!("Units"), 0.6),
        (tr_const!("Device"), 0.8),
        (tr_const!("Part"), 1.1),
        (tr_const!("Description"), 2.2),
        (tr_const!("Depth (mm)"), 0.6),
    ]);
    for e in prj.layout.enclosures.iter().filter(|e| panel::is_rack(e)) {
        let taken = panel::rack::occupied(e);
        for &(first, n, id) in &taken {
            let Some(it) = e.item(id) else { continue };
            let units = if n > 1 { format!("{} – {}", first + n - 1, first) } else { first.to_string() };
            let desc = panel::part_by_number(prj, &it.part).map(|p| p.description.clone()).unwrap_or_default();
            t.push(vec![e.name.clone(), units, it.tag.clone(), it.part.clone(), desc, format!("{:.0}", it.d)], None);
        }
        let used: u32 = taken.iter().map(|o| o.1).sum();
        let total = panel::rack_units(e);
        t.push(vec![e.name.clone(), format!("{} / {} U", total.saturating_sub(used), total), String::new(), String::new(), tr("free").to_string(), String::new()], None);
    }
    t
}

/// Bonding conductors, then the bonding points with their connection to the main
/// earthing terminal.
fn bonding_list(prj: &Project) -> Table {
    let mut t = Table::with_widths(&[(tr_const!("From"), 1.2), (tr_const!("To"), 1.2), (tr_const!("Kind"), 1.2), (tr_const!("Cross-section"), 0.7), (tr_const!("Length (m)"), 0.6), (tr_const!("Part"), 1.0)]);
    for b in &prj.field.bonds {
        t.push(vec![b.a.clone(), b.b.clone(), b.kind.title().into(), b.cross_section.clone(), format!("{:.1}", b.length), b.part.clone()], None);
    }
    for p in crate::field::bonding::points(prj) {
        let state = if p.bonded { tr("bonded") } else { tr("not bonded") };
        t.push(vec![p.name, String::new(), p.kind.title().into(), String::new(), String::new(), state.into()], None);
    }
    t
}

/// Bundles: stretches shared by several wires, cables and tubes, with their
/// diameter and the branches at their ends.
fn bundle_list(r: &crate::routing3d::Routing) -> Table {
    let mut t = Table::with_widths(&[(tr_const!("Bundle"), 0.5), (tr_const!("Items"), 0.5), (tr_const!("Diameter (mm)"), 0.7), (tr_const!("Length (mm)"), 0.7), (tr_const!("Classes"), 1.0), (tr_const!("Branches"), 0.6), (tr_const!("Contents"), 3.0)]);
    for (bi, b) in r.bundles.iter().enumerate() {
        let classes: Vec<&str> = b.classes.iter().map(|c| c.title()).collect();
        let branches = r.branches.iter().filter(|x| x.bundles.contains(&bi)).count();
        let labels: Vec<&str> = b.items.iter().map(|&i| r.items[i].label.as_str()).collect();
        t.push(
            vec![
                format!("B{}", bi + 1),
                b.items.len().to_string(),
                format!("{:.1}", b.diameter),
                format!("{:.0}", b.length),
                classes.join(", "),
                branches.to_string(),
                labels.join(", "),
            ],
            None,
        );
    }
    t
}

fn pct_loops(prj: &Project, idx: &Index) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("PCT loop"), 0.6),
        (tr_const!("Description"), 1.6),
        (tr_const!("Functions"), 0.9),
        (tr_const!("Measured variable"), 0.9),
        (tr_const!("Range"), 0.9),
        (tr_const!("Devices"), 1.3),
        (tr_const!("Status"), 0.9),
    ]);
    for l in crate::process::loops(prj, idx) {
        let first = l.functions.first().map(String::as_str).unwrap_or("");
        t.push(
            vec![l.name, l.description, l.functions.join(", "), crate::process::variable(first).into(), l.range, l.devices.join(", "), l.status.title().into()],
            l.target,
        );
    }
    t
}

pub(crate) fn loc_str(prj: &Project, page: usize, pos: crate::geom::P) -> String {
    crate::frame::reference(prj, page, pos)
}

fn toc(prj: &Project) -> Table {
    let mut t = Table::with_widths(&[(tr_const!("Page"), 1.0), (tr_const!("Description"), 4.0), (tr_const!("Type"), 2.0), (tr_const!("Devices"), 1.0)]);
    for (i, pg) in prj.pages.iter().enumerate() {
        t.push(
            vec![pg.name.clone(), pg.description.clone(), pg.kind.label(), pg.symbols.len().to_string()],
            Some((i, 0)),
        );
    }
    t
}

fn device_list(prj: &Project, idx: &Index) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Device tag"), 1.2),
        (tr_const!("Main function"), 2.0),
        (tr_const!("Function text"), 2.0),
        (tr_const!("Location"), 1.0),
        (tr_const!("Aux. functions"), 1.6),
        (tr_const!("Part number"), 1.6),
        (tr_const!("Description"), 3.0),
    ]);
    for tag in idx.sorted_tags() {
        let dev = &idx.devices[tag];
        let first = dev.mains.first().or(dev.others.first()).or(dev.auxs.first()).or(dev.overviews.first()).copied();
        let Some(loc) = first else { continue };
        let inst = prj.pages[loc.page].symbols.iter().find(|s| s.id == loc.sym_id);
        let (name, ftext) = inst
            .map(|s| (prj.symbol(&s.sym).map(|d| tr(d.name)).unwrap_or("?").to_string(), crate::text::function_text(prj, loc.page, s).to_string()))
            .unwrap_or_default();
        let name = if dev.mains.is_empty() && !dev.others.is_empty() {
            format!("{name} ×{}", dev.others.len())
        } else if dev.mains.is_empty() {
            "(missing main function)".into()
        } else {
            name
        };
        let data = prj.devices.get(tag).cloned().unwrap_or_default();
        let desc = if data.description.is_empty() {
            data.parts
                .first()
                .and_then(|r| prj.parts.iter().find(|p| p.number == r.number))
                .map(|p| p.description.clone())
                .unwrap_or_default()
        } else {
            data.description.clone()
        };
        let part_list = data
            .parts
            .iter()
            .map(|r| if r.count > 1 { format!("{}× {}", r.count, r.number) } else { r.number.clone() })
            .collect::<Vec<_>>()
            .join(", ");
        t.push(
            vec![
                tag.clone(),
                name,
                ftext.replace('\n', " "),
                loc_str(prj, loc.page, loc.pos),
                dev.auxs.len().to_string(),
                part_list,
                desc,
            ],
            Some((loc.page, loc.sym_id)),
        );
    }
    t
}

fn bom(prj: &Project, idx: &Index, wiring: &[RoutedWire], conns: &[Connection]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Pos"), 0.5),
        (tr_const!("Qty"), 0.6),
        (tr_const!("Unit"), 0.5),
        (tr_const!("Part number"), 1.8),
        (tr_const!("Type number"), 1.4),
        (tr_const!("Manufacturer"), 1.0),
        (tr_const!("Description"), 3.2),
        (tr_const!("Devices"), 2.0),
        (tr_const!("Unit price"), 0.9),
        (tr_const!("Total"), 0.9),
    ]);
    let mut groups: BTreeMap<String, (f32, Vec<String>)> = BTreeMap::new();
    for tag in idx.sorted_tags() {
        let Some(data) = prj.devices.get(tag) else { continue };
        if data.parts.is_empty() {
            continue;
        }
        let dev = &idx.devices[tag];
        // Terminal strips are ordered per terminal, everything else per device.
        let terminals = dev
            .others
            .iter()
            .filter(|l| {
                prj.pages[l.page]
                    .symbols
                    .iter()
                    .find(|s| s.id == l.sym_id)
                    .and_then(|s| prj.symbol(&s.sym))
                    .map(|d| d.func == Func::Terminal)
                    .unwrap_or(false)
            })
            .count();
        let strip = dev.mains.is_empty() && terminals > 0;
        let layout = if strip { crate::strips::strip_layout(prj, tag) } else { Vec::new() };
        let jumpers = if strip { crate::strips::strip_jumpers(prj, tag) } else { Vec::new() };
        // End brackets, end plates and jumpers of the strip.
        let mut accessories: Vec<&str> = layout
            .iter()
            .filter(|e| matches!(e.kind, crate::strips::ElementKind::EndBracket | crate::strips::ElementKind::EndPlate))
            .map(|e| e.part.as_str())
            .collect();
        accessories.extend(jumpers.iter().filter(|j| !j.part.is_empty()).map(|j| j.part.as_str()));
        for part in &accessories {
            let g = groups.entry(part.to_string()).or_default();
            g.0 += 1.0;
            if !g.1.contains(tag) {
                g.1.push(tag.clone());
            }
        }
        for r in data.parts.iter().filter(|r| !accessories.contains(&r.number.as_str())) {
            // Terminal parts as often as the strip layout uses them (PE terminals their
            // own part), other parts once.
            let used = layout.iter().filter(|e| matches!(e.kind, crate::strips::ElementKind::Terminal(_) | crate::strips::ElementKind::Spare(_)) && e.part == r.number).count();
            let terminal_part = !layout.is_empty() && prj.parts.iter().chain(crate::parts::catalog()).any(|p| p.number == r.number && crate::strips::is_terminal_part(p));
            let per_device = if used > 0 || terminal_part { used } else { 1 };
            if per_device == 0 {
                continue;
            }
            let g = groups.entry(r.number.clone()).or_default();
            g.0 += (per_device * r.count.max(1) as usize) as f32;
            if !g.1.contains(tag) {
                g.1.push(tag.clone());
            }
        }
    }
    // Finished harnesses per piece.
    for h in prj.harnesses.iter().filter(|h| !h.part.is_empty()) {
        let g = groups.entry(h.part.clone()).or_default();
        g.0 += 1.0;
        g.1.push(h.tag.clone());
    }
    // Cables by length (or piece without length).
    for c in prj.cables.iter().filter(|c| !c.part.is_empty()) {
        let g = groups.entry(c.part.clone()).or_default();
        g.0 += if c.length > 0.0 { c.length } else { 1.0 };
        g.1.push(c.tag.clone());
    }
    // Cable carriers, bonding conductors and cable accessories in the field.
    for (part, qty, used) in crate::field::material(prj) {
        let g = groups.entry(part).or_default();
        g.0 += qty;
        g.1.extend(used);
    }
    // Enclosures, rails, ducts and busbars of the panel layout.
    for (part, qty, used) in panel::layout_material(prj) {
        let g = groups.entry(part).or_default();
        g.0 += qty;
        g.1.extend(used);
    }
    // Ferrules of the routed panel wiring.
    for (part, n) in crate::production::ferrule_material(prj, wiring) {
        let g = groups.entry(part.to_string()).or_default();
        g.0 += n as f32;
        g.1.push("wiring".into());
    }
    // Marker material.
    for (part, n) in crate::marking::material(&crate::marking::markers(prj, conns)) {
        let g = groups.entry(part).or_default();
        g.0 += n as f32;
        g.1.push("marking".into());
    }
    // Wiring and mounting material may come straight from the built-in catalog.
    let catalog = crate::parts::catalog();
    let mut sum = 0.0;
    for (i, (part, (qty, tags))) in groups.iter().enumerate() {
        let pd = prj.parts.iter().chain(catalog.iter()).find(|p| &p.number == part).cloned().unwrap_or_else(|| Part {
            description: "(part not in project – synchronize parts)".into(),
            ..Default::default()
        });
        let total = pd.total_price(*qty);
        sum += total;
        let unit_price = if pd.price_unit > 1 { format!("{:.2}/{}", pd.price, pd.price_unit) } else { format!("{:.2}", pd.price) };
        t.push(
            vec![
                (i + 1).to_string(),
                if qty.fract() == 0.0 { format!("{qty:.0}") } else { format!("{qty:.2}") },
                pd.quantity_unit.clone(),
                part.clone(),
                pd.type_number,
                pd.manufacturer,
                pd.description,
                tags.join(", "),
                format!("{unit_price} {}", pd.currency),
                format!("{total:.2}"),
            ],
            None,
        );
        // Components of an assembly below it, included in its price.
        for (k, (sub, n)) in crate::parts::assembly_components(prj, part, *qty).into_iter().enumerate() {
            let sd = prj.parts.iter().chain(catalog.iter()).find(|p| p.number == sub).cloned().unwrap_or_default();
            t.push(
                vec![
                    format!("{}.{}", i + 1, k + 1),
                    if n.fract() == 0.0 { format!("{n:.0}") } else { format!("{n:.2}") },
                    sd.quantity_unit.clone(),
                    sub,
                    sd.type_number,
                    sd.manufacturer,
                    sd.description,
                    tr_fmt!("in {}", part),
                    String::new(),
                    tr("incl.").to_string(),
                ],
                None,
            );
        }
    }
    if !groups.is_empty() {
        let mut last = vec![String::new(); 10];
        last[6] = tr("Sum").into();
        last[9] = format!("{sum:.2}");
        t.push(last, None);
    }
    t
}

fn marking_report(markers: &[crate::marking::Marker]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Material"), 1.2),
        (tr_const!("Category"), 1.0),
        (tr_const!("Text"), 1.6),
        (tr_const!("Count"), 0.5),
        (tr_const!("Enclosure"), 0.8),
        (tr_const!("Belongs to"), 1.6),
    ]);
    for m in markers {
        t.push(vec![m.material.clone(), m.category.title().into(), m.text.clone(), m.count.to_string(), m.enclosure.clone(), m.group.clone()], None);
    }
    t
}

/// Middle of the fullest stretch of a duct, on its centre line (plate coordinates).
pub(crate) fn fill_spot(prj: &Project, f: &panel::DuctFill) -> crate::geom::P {
    let e = &prj.layout.enclosures[f.enclosure];
    let mid = (f.from + f.to) / 2.0;
    match e.item(f.duct) {
        Some(d) if d.horizontal() => crate::geom::p(mid, d.center().y),
        Some(d) => crate::geom::p(d.center().x, mid),
        None => crate::geom::p(0.0, 0.0),
    }
}

fn duct_fill_report(prj: &Project, fill: &[panel::DuctFill]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Enclosure"), 0.8),
        (tr_const!("Cable duct"), 1.2),
        (tr_const!("Position (mm)"), 0.9),
        (tr_const!("Length (mm)"), 0.8),
        (tr_const!("Wires"), 0.6),
        (tr_const!("Fullest stretch (mm)"), 1.0),
        (tr_const!("Fill level (%)"), 0.8),
        (tr_const!("Limit (%)"), 0.6),
    ]);
    for f in fill {
        let e = &prj.layout.enclosures[f.enclosure];
        let Some(d) = e.item(f.duct) else { continue };
        let at = fill_spot(prj, f);
        t.push(
            vec![
                e.name.clone(),
                if d.face == panel::Face::Door { format!("{} ({})", d.part, d.face.title()) } else { d.part.clone() },
                format!("{:.0} / {:.0}", at.x, at.y),
                format!("{:.0}", d.length()),
                f.total_wires.to_string(),
                format!("{:.0} – {:.0}", f.from, f.to),
                format!("{:.0}", f.percent),
                format!("{:.0}", prj.layout.duct_fill_limit),
            ],
            None,
        );
    }
    t
}

fn panel_wiring(prj: &Project, wiring: &[RoutedWire], open: &[OpenWire]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Enclosure"), 0.8),
        (tr_const!("Wire no."), 0.7),
        (tr_const!("From"), 1.4),
        (tr_const!("To"), 1.4),
        (tr_const!("Potential"), 0.9),
        (tr_const!("Colour"), 0.6),
        ("Cross-section", 0.9),
        (tr_const!("Length (mm)"), 0.9),
        (tr_const!("Routing"), 1.2),
    ]);
    let mut total = 0.0;
    for w in wiring {
        total += w.length;
        t.push(
            vec![
                w.enclosure_names(&prj.layout),
                w.number.clone(),
                w.from.clone(),
                w.to.clone(),
                w.net.clone(),
                w.color.clone(),
                w.cross_section.clone(),
                format!("{:.0}", w.length),
                w.routing().into(),
            ],
            None,
        );
    }
    if !wiring.is_empty() {
        let mut last = vec![String::new(); 9];
        last[6] = tr("Total").into();
        last[7] = format!("{total:.0}");
        t.push(last, None);
    }
    for w in open {
        t.push(
            vec![String::new(), String::new(), w.from.clone(), w.to.clone(), w.net.clone(), String::new(), String::new(), "–".into(), w.reason.into()],
            None,
        );
    }
    t
}

fn terminal_diagram(prj: &Project, nl: &Netlist, open: &Netlist) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Strip"), 0.8),
        (tr_const!("Terminal"), 0.7),
        (tr_const!("Level"), 0.4),
        (tr_const!("Jumper"), 0.5),
        (tr_const!("External (top)"), 3.0),
        (tr_const!("Internal (bottom)"), 3.0),
        (tr_const!("Potential"), 1.0),
        (tr_const!("Location"), 0.8),
        (tr_const!("Function text"), 1.6),
    ]);
    use crate::strips::ElementKind;
    for strip in crate::strips::strip_tags(prj) {
        let mut rows = crate::strips::rows(prj, &strip, nl, open);
        // In the order of the strip on the rail, with its end brackets and end plates.
        for e in crate::strips::strip_layout(prj, &strip) {
            let accessory = |title: &str| vec![strip.clone(), title.to_string(), String::new(), String::new(), e.part.clone(), String::new(), String::new(), String::new(), String::new()];
            match &e.kind {
                ElementKind::Terminal(_) | ElementKind::Spare(_) => {
                    // Every level of a multi-level block, the bottom level first.
                    for label in e.designations() {
                        let Some(i) = rows.iter().position(|r| r.label == label) else { continue };
                        let r = rows.remove(i);
                        let target = r.terminal.as_ref().map(|t| (t.page, t.id));
                        let jumper = if r.jumper > 0 { r.jumper.to_string() } else { String::new() };
                        let level = if e.levels.is_empty() { String::new() } else { r.level.to_string() };
                        t.push(vec![strip.clone(), r.label, level, jumper, r.external, r.internal, r.potential, r.location, r.function_text], target);
                    }
                }
                ElementKind::EndBracket => t.push(accessory(tr("End bracket")), None),
                ElementKind::EndPlate => t.push(accessory(tr("End plate")), None),
            }
        }
        // Jumpers: part, length and the teeth to break off.
        for j in crate::strips::strip_jumpers(prj, &strip) {
            let level = if j.level > 1 { j.level.to_string() } else { String::new() };
            let row = vec![strip.clone(), crate::strips::jumper_kind_title(j.kind).into(), level, j.number.to_string(), j.part.clone(), j.length_text(), String::new(), j.broken_text(), j.span_text()];
            t.push(row, None);
        }
    }
    t
}
fn plug_diagram(prj: &Project, nl: &Netlist, open: &Netlist) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Plug"), 0.8),
        (tr_const!("Pin"), 0.6),
        (tr_const!("Socket side"), 3.0),
        (tr_const!("Pin side"), 3.0),
        (tr_const!("Potential"), 1.0),
        (tr_const!("Location"), 0.8),
        (tr_const!("Function text"), 1.6),
    ]);
    for plug in crate::plugs::plug_tags(prj) {
        for r in crate::plugs::rows(prj, &plug, nl, open) {
            let target = r.pins.first().map(|p| (p.page, p.id));
            t.push(vec![plug.clone(), r.label, r.socket, r.pin, r.potential, r.location, r.function_text], target);
        }
    }
    t
}

fn connection_list(prj: &Project, conns: &[Connection]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Wire no."), 0.8),
        (tr_const!("Potential"), 1.0),
        (tr_const!("From"), 1.6),
        (tr_const!("To"), 1.6),
        (tr_const!("Page"), 0.8),
        (tr_const!("Colour"), 0.8),
        ("Cross-section", 1.0),
        (tr_const!("Cable"), 1.0),
        (tr_const!("Core"), 0.6),
        (tr_const!("Wire type"), 1.0),
    ]);
    for c in conns {
        let s = &prj.pages[c.a.page].symbols[c.a.sym];
        t.push(
            vec![
                c.number.clone(),
                c.net.clone(),
                pin_designation(prj, c.a),
                pin_designation(prj, c.b),
                loc_str(prj, c.a.page, s.pos),
                c.color.clone(),
                c.cross_section.clone(),
                c.cable.clone(),
                c.core.clone(),
                c.wire_type.clone(),
            ],
            Some((c.a.page, s.id)),
        );
    }
    t
}

/// Device of a connection end (`-X1:3` → `-X1`).
fn device_of(prj: &Project, pin: PinRef) -> String {
    prj.pages[pin.page].symbols[pin.sym].tag.clone()
}

/// Connections of a cable, oriented alike: the end at the device where the first
/// connection starts comes first.
fn cable_connections(prj: &Project, conns: &[Connection], cable: &str) -> Vec<Connection> {
    let mut v: Vec<Connection> = conns.iter().filter(|c| c.cable == cable).cloned().collect();
    if let Some(first) = v.first() {
        let (from, to) = (device_of(prj, first.a), device_of(prj, first.b));
        for c in &mut v {
            if device_of(prj, c.b) == from || device_of(prj, c.a) == to {
                std::mem::swap(&mut c.a, &mut c.b);
            }
        }
    }
    v
}

fn sorted_cables(prj: &Project) -> Vec<&Cable> {
    let mut v: Vec<&Cable> = prj.cables.iter().collect();
    v.sort_by(|a, b| natural_cmp(&a.tag, &b.tag));
    v
}

fn cable_overview(prj: &Project, conns: &[Connection]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Cable"), 1.0),
        (tr_const!("Type"), 1.6),
        (tr_const!("Cores used"), 0.8),
        ("Cross-section", 0.9),
        (tr_const!("Length (m)"), 0.8),
        (tr_const!("From"), 1.6),
        (tr_const!("To"), 1.6),
        (tr_const!("Part number"), 1.4),
        (tr_const!("Description"), 2.0),
    ]);
    for cable in sorted_cables(prj) {
        let used = cable_connections(prj, conns, &cable.tag);
        let ends = |f: fn(&Connection) -> PinRef| {
            let mut v: Vec<String> = used.iter().map(|c| device_of(prj, f(c))).collect();
            v.sort_by(|a, b| natural_cmp(a, b));
            v.dedup();
            v.join(", ")
        };
        let cores: BTreeSet<&str> = used.iter().map(|c| c.core.as_str()).collect();
        let target = used.first().map(|c| (c.a.page, prj.pages[c.a.page].symbols[c.a.sym].id));
        // Bus cables: all cores, ends at the ports.
        let links = crate::plc_hw::cable_links(prj, &cable.tag);
        let port_ends = |f: fn(&crate::plc_hw::BusConnection) -> &crate::plc_hw::PortRef| {
            links.iter().map(|c| crate::plc_hw::port_label(prj, f(c))).collect::<Vec<_>>().join(", ")
        };
        let (from, to, cores_used) = if used.is_empty() && !links.is_empty() {
            (port_ends(|c| &c.a), port_ends(|c| &c.b), cable.core_list().len())
        } else {
            (ends(|c| c.a), ends(|c| c.b), cores.len())
        };
        t.push(
            vec![
                cable.tag.clone(),
                cable.cable_type.clone(),
                format!("{}/{}", cores_used, cable.core_list().len()),
                cable.cross_section.clone(),
                if cable.length > 0.0 { format!("{}", cable.length) } else { String::new() },
                from,
                to,
                cable.part.clone(),
                cable.description.clone(),
            ],
            target,
        );
    }
    t
}

fn cable_diagram(prj: &Project, conns: &[Connection]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Cable"), 1.0),
        (tr_const!("Core"), 0.6),
        (tr_const!("From"), 1.6),
        (tr_const!("To"), 1.6),
        (tr_const!("Potential"), 1.0),
        (tr_const!("Wire no."), 0.8),
        ("Cross-section", 0.9),
    ]);
    for cable in sorted_cables(prj) {
        let used = cable_connections(prj, conns, &cable.tag);
        let mut cores = cable.core_list();
        // Cores in use that the cable does not list (shown by the project check).
        for c in &used {
            if !cores.contains(&c.core) {
                cores.push(c.core.clone());
            }
        }
        let links = crate::plc_hw::cable_links(prj, &cable.tag);
        if used.is_empty() && !links.is_empty() {
            for link in links {
                for core in &cores {
                    let ends = (crate::plc_hw::port_label(prj, &link.a), crate::plc_hw::port_label(prj, &link.b));
                    t.push(vec![cable.tag.clone(), core.clone(), ends.0, ends.1, link.network.clone(), String::new(), cable.cross_section.clone()], None);
                }
            }
            continue;
        }
        for core in cores {
            let on_core: Vec<&Connection> = used.iter().filter(|c| c.core == core).collect();
            if on_core.is_empty() {
                t.push(vec![cable.tag.clone(), core.clone(), "reserve".into(), String::new(), String::new(), String::new(), cable.cross_section.clone()], None);
            }
            for c in on_core {
                let cs = if c.cross_section.is_empty() { cable.cross_section.clone() } else { c.cross_section.clone() };
                t.push(
                    vec![
                        cable.tag.clone(),
                        core.clone(),
                        pin_designation(prj, c.a),
                        pin_designation(prj, c.b),
                        c.net.clone(),
                        c.number.clone(),
                        cs,
                    ],
                    Some((c.a.page, prj.pages[c.a.page].symbols[c.a.sym].id)),
                );
            }
        }
    }
    t
}

fn plc_list(prj: &Project, nl: &Netlist) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("PLC card"), 1.0),
        (tr_const!("Address"), 1.0),
        (tr_const!("Type"), 0.8),
        (tr_const!("Function text"), 2.5),
        (tr_const!("Connected to"), 3.0),
        (tr_const!("Location"), 0.8),
    ]);
    for p in crate::plc::points(prj, nl) {
        let kind = p.io_kind();
        t.push(vec![p.card, p.address, kind.into(), p.function_text, p.connected, p.location], Some((p.page, p.id)));
    }
    t
}

fn revision_overview(prj: &Project) -> Table {
    let since = crate::revision::last_released(prj).map(|r| r.name.as_str()).unwrap_or("-");
    let mut t = Table::with_widths(&[(tr_const!("Page"), 0.8), (tr_const!("Change"), 0.8), (tr_const!("Item"), 0.8), (if since.is_empty() { tr_const!("Description") } else { tr_const!("Description (since last revision)") }, 3.0)]);
    for (page, kind, what, label, target) in crate::revision::overview(prj) {
        t.push(vec![page, kind, what, label], target);
    }
    t
}

/// Wire list for production (wire processing machines): one row per connection
/// with wire number, ends, colour, cross-section, the routed length if the devices
/// are placed in the panel layout, end treatment (stripping length, ferrule) and
/// cable core. Panel wires come first, sorted by cross-section, colour and length
/// (machine batches).
pub fn production_wire_list(prj: &Project, an: &Analysis) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Wire no."), 0.8),
        (tr_const!("From"), 1.6),
        (tr_const!("To"), 1.6),
        (tr_const!("Colour"), 0.7),
        ("Cross-section", 0.9),
        (tr_const!("Length (mm)"), 0.9),
        (tr_const!("Strip (mm)"), 0.7),
        (tr_const!("Ferrule"), 1.6),
        (tr_const!("Ferrule part"), 1.0),
        (tr_const!("Cable"), 0.8),
        (tr_const!("Core"), 0.6),
        (tr_const!("Potential"), 1.0),
    ]);
    // (routed length, cells, jump target)
    type Row = (Option<f32>, Vec<String>, Option<(usize, u64)>);
    let mut rows: Vec<Row> = Vec::new();
    for c in &an.connections {
        let (from, to) = (pin_designation(prj, c.a), pin_designation(prj, c.b));
        let length = an.wiring.iter().find(|w| (w.from == from && w.to == to) || (w.from == to && w.to == from)).map(|w| w.length);
        let end = crate::production::end_treatment(&c.cross_section, prj.layout.ferrules);
        // Cable cores are processed with their cable, not as single wires.
        let end = if c.cable.is_empty() { end } else { None };
        let ferrule = end.and_then(|e| e.ferrule);
        let target = Some((c.a.page, prj.pages[c.a.page].symbols[c.a.sym].id));
        rows.push((
            length,
            vec![
                c.number.clone(),
                from,
                to,
                c.color.clone(),
                c.cross_section.clone(),
                length.map(|l| format!("{l:.0}")).unwrap_or_default(),
                end.map(|e| format!("{:.0}", e.strip)).unwrap_or_default(),
                ferrule.map(|f| format!("{} mm² {}, {:.0} mm", f.cross_section, f.color, f.length)).unwrap_or_default(),
                ferrule.map(|f| f.part.to_string()).unwrap_or_default(),
                c.cable.clone(),
                c.core.clone(),
                c.net.clone(),
            ],
            target,
        ));
    }
    let cs = |r: &Vec<String>| crate::production::cross_section_mm2(&r[4]).unwrap_or(0.0);
    rows.sort_by(|a, b| {
        b.0.is_some()
            .cmp(&a.0.is_some())
            .then(cs(&a.1).total_cmp(&cs(&b.1)))
            .then(a.1[3].cmp(&b.1[3]))
            .then(a.0.unwrap_or(0.0).total_cmp(&b.0.unwrap_or(0.0)))
    });
    for (_, row, target) in rows {
        t.push(row, target);
    }
    t
}

/// Harness data: per harness its wires with lengths and end treatment, and a
/// summary row (wires, connectors, total length, bundle diameter).
fn harness_data(prj: &Project, an: &Analysis) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Harness"), 0.9),
        (tr_const!("Wire no."), 0.7),
        (tr_const!("From"), 1.4),
        (tr_const!("To"), 1.4),
        (tr_const!("Colour"), 0.6),
        ("Cross-section", 0.9),
        (tr_const!("Length (mm)"), 0.8),
        (tr_const!("End treatment"), 1.6),
    ]);
    for h in &prj.harnesses {
        let ws = crate::harness::wires(prj, an, &h.tag);
        for w in &ws {
            let end = crate::production::end_treatment(&w.cross_section, prj.layout.ferrules)
                .map(|e| match e.ferrule {
                    Some(f) => tr_fmt!("strip {} mm, ferrule {} mm² {}", format!("{:.0}", e.strip), f.cross_section, f.color),
                    None => tr_fmt!("strip {} mm", format!("{:.0}", e.strip)),
                })
                .unwrap_or_default();
            let length = if w.length > 0.0 { format!("{:.0}{}", w.length, if w.routed { "" } else { " *" }) } else { "?".into() };
            let fmt = |e: &(String, String)| if e.1.is_empty() { e.0.clone() } else { format!("{}:{}", e.0, e.1) };
            t.push(vec![h.tag.clone(), w.number.clone(), fmt(&w.from), fmt(&w.to), w.color.clone(), w.cross_section.clone(), length, end], w.target);
        }
        let total: f32 = ws.iter().map(|w| w.length).sum();
        let summary = format!(
            "{} wires, {} connectors, Ø {:.1} mm",
            ws.len(),
            crate::harness::connectors(&ws).len(),
            crate::harness::bundle_diameter(&ws)
        );
        t.push(vec![h.tag.clone(), String::new(), summary, String::new(), String::new(), tr("Total").into(), format!("{total:.0}"), h.description.clone()], None);
    }
    t
}

fn potential_list(prj: &Project, nl: &Netlist) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Potential"), 1.0),
        (tr_const!("Type"), 1.2),
        (tr_const!("Voltage"), 0.9),
        (tr_const!("Colour"), 0.6),
        (tr_const!("Cross-section"), 0.9),
        (tr_const!("Connection points"), 4.0),
        (tr_const!("Pages"), 1.5),
    ]);
    let mut nets: Vec<_> = nl.nets.iter().filter(|n| n.named).collect();
    nets.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for net in nets {
        let mut pins: Vec<String> = net
            .pins
            .iter()
            .filter(|p| {
                let s = &prj.pages[p.page].symbols[p.sym];
                prj.symbol(&s.sym).map(|d| d.func.has_tag()).unwrap_or(false)
            })
            .map(|p| pin_designation(prj, *p))
            .collect();
        pins.sort_by(|a, b| natural_cmp(a, b));
        pins.dedup();
        let pages: BTreeSet<usize> = net.pins.iter().map(|p| p.page).chain(net.wires.iter().map(|w| w.0)).collect();
        let pages: Vec<String> = pages.iter().map(|&p| prj.pages[p].name.clone()).collect();
        let def = prj.potential(&net.name).cloned().unwrap_or_default();
        let kind = if prj.potential(&net.name).is_some() { def.kind.title().to_string() } else { String::new() };
        t.push(vec![net.name.clone(), kind, def.voltage, def.color, def.cross_section, pins.join(", "), pages.join(", ")], None);
    }
    t
}

fn cable_sizing(prj: &Project, conns: &[Connection]) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Cable"), 0.9),
        (tr_const!("Length (m)"), 0.7),
        (tr_const!("Cross-section"), 0.9),
        (tr_const!("Load"), 0.8),
        (tr_const!("Current (A)"), 0.7),
        (tr_const!("Voltage (V)"), 0.7),
        (tr_const!("Voltage drop (V)"), 0.8),
        (tr_const!("Voltage drop (%)"), 0.8),
        (tr_const!("Current capacity (A)"), 0.9),
    ]);
    let num = |v: Option<f32>, digits: usize| v.map(|x| format!("{x:.digits$}")).unwrap_or_else(|| "–".into());
    for d in crate::sizing::cable_drops(prj, conns) {
        t.push(
            vec![
                d.cable,
                format!("{:.1}", d.length),
                d.cross_section,
                d.load,
                num(d.current, 1),
                num(d.voltage, 0),
                num(d.drop.map(|x| x.0), 2),
                num(d.drop.map(|x| x.1), 2),
                num(d.ampacity, 1),
            ],
            None,
        );
    }
    t
}

fn climate_report(prj: &Project) -> Table {
    let mut t = Table::with_widths(&[
        (tr_const!("Enclosure"), 0.9),
        (tr_const!("Surface (m²)"), 0.8),
        (tr_const!("Power loss (W)"), 0.8),
        (tr_const!("Inside (°C)"), 0.7),
        (tr_const!("Highest (°C)"), 0.7),
        (tr_const!("Cooling"), 1.4),
        (tr_const!("Heating (W)"), 0.7),
        (tr_const!("Dew point (°C)"), 0.7),
        (tr_const!("Devices without power loss"), 2.0),
    ]);
    for e in prj.layout.enclosures.iter().filter(|e| !e.is_room()) {
        let r = crate::climate::calculate(prj, e);
        t.push(
            vec![
                e.name.clone(),
                format!("{:.2}", r.surface),
                format!("{:.0}", r.loss),
                format!("{:.1}", r.inside),
                format!("{:.0}", e.climate.inside_max),
                r.cooling.text(),
                if r.heating > 0.0 { format!("{:.0}", r.heating) } else { "–".into() },
                r.dew_point.map(|d| format!("{d:.1}")).unwrap_or_else(|| "–".into()),
                r.without_data.join(", "),
            ],
            None,
        );
    }
    t
}

#[cfg(test)]
mod tests;
