use super::*;

#[test]
fn terminal_layout_uses_each_functions_explicitly_selected_part() {
    let mut prj=crate::sample::demo_project();
    let terms=terminals(&prj,"-X1");let term=&terms[0];
    let mut part=terminal_part(&prj,"-X1",false).unwrap().clone();
    part.number="CUSTOM.TERMINAL".into();part.width=12.;prj.parts.push(part);
    prj.devices.get_mut("-X1").unwrap().parts.push(crate::parts::PartRef {number:"CUSTOM.TERMINAL".into(),count:1, total:false });
    prj.pages[term.page].symbols[term.sym].values.insert("function.part_number".into(),"CUSTOM.TERMINAL".into());
    let layout=strip_layout(&prj,"-X1");
    let chosen=layout.iter().find(|e|e.kind==ElementKind::Terminal(term.label.clone())).unwrap();
    assert_eq!((&*chosen.part,chosen.width),("CUSTOM.TERMINAL",12.));
    assert!(layout.iter().any(|e|matches!(e.kind,ElementKind::Terminal(_)) && e.part!="CUSTOM.TERMINAL"));
}
use crate::sample::demo_project;

fn labels(prj: &Project) -> Vec<String> {
    terminals(prj, "-X1").into_iter().map(|t| t.label).collect()
}

#[test]
fn order_moves_and_renumbering() {
    let mut prj = demo_project();
    assert_eq!(strip_tags(&prj), vec!["-X1".to_string()]);
    assert_eq!(labels(&prj), ["1", "2", "3", "PE"]);
    move_terminal(&mut prj, "-X1", 2, true);
    assert_eq!(labels(&prj), ["1", "3", "2", "PE"]);
    move_terminal(&mut prj, "-X1", 0, true); // already first
    assert_eq!(labels(&prj), ["1", "3", "2", "PE"]);
    // Renumbering follows the strip order, PE keeps its designation; jumpers move along.
    prj.strips.entry("-X1".into()).or_default().jumpers.insert("3".into(), 1);
    assert_eq!(renumber(&mut prj, "-X1"), 2);
    assert_eq!(labels(&prj), ["1", "2", "3", "PE"]);
    assert_eq!(prj.strips["-X1"].jumpers.get("2"), Some(&1));
}

#[test]
fn rows_include_jumpers_and_spares() {
    let mut prj = demo_project();
    let data = prj.strips.entry("-X1".into()).or_default();
    data.spares = 2;
    data.jumpers.insert("1".into(), 4);
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    let rows = rows(&prj, "-X1", &nl, &open);
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].jumper, 4);
    assert!(rows[0].internal.contains("-M1:U1"), "{}", rows[0].internal);
    assert_eq!((rows[4].label.as_str(), rows[5].label.as_str()), ("4", "5"));
    assert!(rows[5].terminal.is_none());
    // Spares are in the bill of materials (as feed-through terminals, PE apart).
    let an = crate::analysis::Analysis::checked(&prj);
    let qty = |part: &str| an.report(&prj, ReportKind::BillOfMaterials).unwrap().rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap();
    assert_eq!((qty("GEN.TB-2.5").as_str(), qty("GEN.TB-PE-2.5").as_str()), ("5", "1"));
}

#[test]
fn jumpers_join_potentials() {
    let mut prj = demo_project();
    let pot = |prj: &Project, label: &str| {
        let nl = Netlist::build(prj, true);
        let s = prj.pages.iter().flat_map(|p| &p.symbols).find(|s| s.tag == "-X1" && s.label == label).unwrap();
        nl.nets[nl.pin_net[&(s.id, 0)]].name.clone()
    };
    assert_ne!(pot(&prj, "1"), pot(&prj, "2"));
    let data = prj.strips.entry("-X1".into()).or_default();
    data.jumpers.insert("1".into(), 1);
    data.jumpers.insert("2".into(), 1);
    assert_eq!(pot(&prj, "1"), pot(&prj, "2"));
    assert_eq!(jumper_groups(&prj), vec![("-X1".to_string(), vec!["1".to_string(), "2".to_string()])]);
    // The physical connections (wire numbers) stay apart.
    let open = Netlist::build(&prj, false);
    let id = |l: &str| prj.pages.iter().flat_map(|p| &p.symbols).find(|s| s.tag == "-X1" && s.label == l).unwrap().id;
    assert_ne!(open.pin_net[&(id("1"), 0)], open.pin_net[&(id("2"), 0)]);
}

#[test]
fn every_terminal_gets_its_part() {
    let prj = demo_project();
    assert_eq!(terminal_part(&prj, "-X1", false).unwrap().number, "GEN.TB-2.5");
    assert_eq!(terminal_part(&prj, "-X1", true).unwrap().number, "GEN.TB-PE-2.5");
    // Without a PE part, PE terminals fall back to the strip's terminal part.
    let mut only_grey = prj.clone();
    only_grey.devices.get_mut("-X1").unwrap().parts.retain(|r| r.number == "GEN.TB-2.5");
    assert_eq!(terminal_part(&only_grey, "-X1", true).unwrap().number, "GEN.TB-2.5");
    // Unknown strips have none.
    assert!(terminal_part(&prj, "-X9", false).is_none());
}

#[test]
fn strip_layout_lists_terminals_and_spares() {
    let mut prj = demo_project();
    prj.strips.entry("-X1".into()).or_default().spares = 1;
    prj.strips.entry("-X1".into()).or_default().manual_accessories = true;
    let layout = strip_layout(&prj, "-X1");
    let kinds: Vec<(ElementKind, &str)> = layout.iter().map(|e| (e.kind.clone(), e.part.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (ElementKind::Terminal("1".into()), "GEN.TB-2.5"),
            (ElementKind::Terminal("2".into()), "GEN.TB-2.5"),
            (ElementKind::Terminal("3".into()), "GEN.TB-2.5"),
            (ElementKind::Terminal("PE".into()), "GEN.TB-PE-2.5"),
            (ElementKind::Spare("4".into()), "GEN.TB-2.5"),
        ]
    );
    assert!(layout.iter().all(|e| e.width == 5.2));
    // A strip without parts still has its terminals, in the default width.
    prj.devices.remove("-X1");
    let bare = strip_layout(&prj, "-X1");
    assert_eq!(bare.len(), 5);
    assert!(bare.iter().all(|e| e.part.is_empty() && e.width == TERMINAL_WIDTH));
}

#[test]
fn strips_are_as_long_as_their_layout() {
    let mut prj = demo_project();
    let terminal = |prj: &Project| crate::panel::device_size(prj, "-X1").unwrap().w;
    // 4 terminals, end plates after the grey ones and after PE, 2 end brackets.
    assert!((terminal(&prj) - (4.0 * 5.2 + 2.0 * 2.2 + 2.0 * 9.5)).abs() < 0.01);
    // A wider PE terminal and a spare (with its own end plate) make the strip longer.
    prj.parts.iter_mut().find(|p| p.number == "GEN.TB-PE-2.5").unwrap().width = 10.4;
    prj.strips.entry("-X1".into()).or_default().spares = 1;
    assert!((terminal(&prj) - (4.0 * 5.2 + 10.4 + 3.0 * 2.2 + 2.0 * 9.5)).abs() < 0.01);
}

fn kinds(prj: &Project) -> Vec<ElementKind> {
    strip_layout(prj, "-X1").into_iter().map(|e| e.kind).collect()
}

#[test]
fn accessories_follow_the_terminal_parts() {
    let mut prj = demo_project();
    use ElementKind::*;
    assert_eq!(
        kinds(&prj),
        [EndBracket, Terminal("1".into()), Terminal("2".into()), Terminal("3".into()), EndPlate, Terminal("PE".into()), EndPlate, EndBracket]
    );
    // Same part throughout: one end plate at the end.
    prj.devices.get_mut("-X1").unwrap().parts.retain(|r| r.number == "GEN.TB-2.5");
    assert_eq!(kinds(&prj).iter().filter(|k| **k == EndPlate).count(), 1);
    assert_eq!(kinds(&prj).last(), Some(&EndBracket));
    // Parts without accessory data: none.
    prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap().tech_data.clear();
    assert!(kinds(&prj).iter().all(|k| matches!(k, Terminal(_))));
    // Unknown accessory parts still get placed in their default width.
    prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap().tech_data = "End bracket: OWN.BRACKET".into();
    let layout = strip_layout(&prj, "-X1");
    assert_eq!((layout[0].part.as_str(), layout[0].width), ("OWN.BRACKET", END_BRACKET_WIDTH));
}

#[test]
fn accessories_are_in_the_bill_of_materials() {
    let prj = demo_project();
    let an = crate::analysis::Analysis::checked(&prj);
    let qty = |part: &str| an.report(&prj, ReportKind::BillOfMaterials).unwrap().rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap_or_default();
    assert_eq!((qty("GEN.EB-35").as_str(), qty("GEN.TB-END").as_str()), ("2", "2"));
    // Switched off: none, and nothing counted twice when listed by hand.
    let mut manual = prj.clone();
    manual.strips.entry("-X1".into()).or_default().manual_accessories = true;
    manual.devices.get_mut("-X1").unwrap().parts.push(crate::parts::PartRef { number: "GEN.TB-END".into(), count: 1, total:false });
    let an = crate::analysis::Analysis::checked(&manual);
    let qty = |part: &str| an.report(&manual, ReportKind::BillOfMaterials).unwrap().rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap_or_default();
    assert_eq!((qty("GEN.EB-35").as_str(), qty("GEN.TB-END").as_str()), ("", "1"));
}

fn jumper(prj: &mut Project, number: u32, labels: &[&str]) {
    let data = prj.strips.entry("-X1".into()).or_default();
    for l in labels {
        data.jumpers.insert(l.to_string(), number);
    }
}

#[test]
fn jumpers_get_the_shortest_fitting_part() {
    let mut prj = demo_project();
    jumper(&mut prj, 1, &["1", "2"]);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.part.as_str(), j.poles, j.span.len(), j.problem.clone()), ("GEN.JMP-2.5-2", 2, 2, None));
    // 1 and 3: a 3-pole jumper with the tooth at 2 broken off.
    let mut prj = demo_project();
    jumper(&mut prj, 1, &["1", "3"]);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.part.as_str(), j.broken.clone()), ("GEN.JMP-2.5-3", vec!["2".to_string()]));
    // Four spares in a row: no 4-pole part, the 10-pole one is cut.
    let mut prj = demo_project();
    prj.strips.entry("-X1".into()).or_default().spares = 4;
    jumper(&mut prj, 2, &["4", "7"]);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.part.as_str(), j.poles, j.span.len()), ("GEN.JMP-2.5-10", 10, 4));
    // In the bill of materials.
    let an = crate::analysis::Analysis::checked(&prj);
    assert!(an.report(&prj, ReportKind::BillOfMaterials).unwrap().rows.iter().any(|r| r[3] == "GEN.JMP-2.5-10" && r[1] == "1"));
}

#[test]
fn jumper_problems() {
    let problem = |labels: &[&str], setup: &dyn Fn(&mut Project)| {
        let mut prj = demo_project();
        setup(&mut prj);
        jumper(&mut prj, 1, labels);
        strip_jumpers(&prj, "-X1")[0].problem.clone()
    };
    let none = |_: &mut Project| {};
    // Across the end plate between the grey terminals and PE.
    assert_eq!(problem(&["3", "PE"], &none), Some(JumperProblem::Blocked));
    let manual = |prj: &mut Project| prj.strips.entry("-X1".into()).or_default().manual_accessories = true;
    assert_eq!(problem(&["3", "PE"], &manual), Some(JumperProblem::MixedParts));
    let no_jumpers = |prj: &mut Project| {
        let p = prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap();
        p.tech_data = p.tech_data.lines().filter(|l| !l.starts_with("Jumper")).collect::<Vec<_>>().join("\n");
    };
    assert_eq!(problem(&["1", "2"], &no_jumpers), Some(JumperProblem::NoPart));
    let short = |prj: &mut Project| {
        let p = prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap();
        p.tech_data = p.tech_data.replace(", GEN.JMP-2.5-3, GEN.JMP-2.5-10", "");
    };
    assert_eq!(problem(&["1", "3"], &short), Some(JumperProblem::TooShort { needed: 3, longest: 2 }));
    // Reported by the project check.
    let mut prj = demo_project();
    jumper(&mut prj, 1, &["3", "PE"]);
    let an = crate::analysis::Analysis::checked(&prj);
    assert!(an.issues.iter().any(|i| i.message() == "-X1: jumper 1 crosses an end plate or end bracket"));
}

#[test]
fn terminal_diagram_shows_accessories_and_jumpers() {
    let mut prj = demo_project();
    jumper(&mut prj, 1, &["1", "3"]);
    let an = crate::analysis::Analysis::checked(&prj);
    let rows = &an.report(&prj, ReportKind::TerminalDiagram).unwrap().rows;
    let column: Vec<&str> = rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(column, ["End bracket", "1", "2", "3", "End plate", "PE", "End plate", "End bracket", "Plug-in jumper"]);
    let j = rows.last().unwrap();
    assert_eq!((j[4].as_str(), j[5].as_str(), j[7].as_str(), j[8].as_str()), ("GEN.JMP-2.5-3", "3 poles", "break off at 2", "terminals 1 – 3"));
}

#[test]
fn texts_for_lists() {
    assert_eq!(ElementKind::EndBracket.title(), "End bracket");
    assert_eq!(ElementKind::Spare("7".into()).title(), "7");
    let mut prj = demo_project();
    jumper(&mut prj, 1, &["1", "2"]);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.length_text().as_str(), j.broken_text().as_str(), j.span_text().as_str()), ("2 poles", "", "terminals 1 – 2"));
    assert_eq!(j.problem_text("-X1"), None);
    let cut = Jumper { poles: 10, ..j.clone() };
    assert_eq!(cut.length_text(), "cut from 10 to 2 poles");
}


/// The demo strip with terminal 2 on level 2 of terminal 1's block, 3 on level 1 of the
/// next block, and the terminal part declared as a double-level terminal.
fn two_levels(levels: u8) -> Project {
    let mut prj = demo_project();
    prj.strips.entry("-X1".into()).or_default().levels.insert("2".into(), 2);
    let p = prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap();
    p.tech_data.push_str(&format!("\nLevels: {levels}"));
    prj
}

#[test]
fn multi_level_terminals_share_a_block() {
    let prj = two_levels(2);
    let layout = strip_layout(&prj, "-X1");
    let blocks: Vec<Vec<&str>> = layout.iter().filter(|e| matches!(e.kind, ElementKind::Terminal(_))).map(|e| e.designations()).collect();
    assert_eq!(blocks, [vec!["1", "2"], vec!["3"], vec!["PE"]]);
    let first = layout.iter().find(|e| e.kind == ElementKind::Terminal("1".into())).unwrap();
    assert_eq!((first.at_level(1), first.at_level(2), first.at_level(3)), (Some("1"), Some("2"), None));
    assert_eq!(level(&prj, "-X1", "2"), 2);
    assert_eq!(level(&prj, "-X1", "1"), 1);
    assert!(level_problems(&prj, "-X1").is_empty());

    // One terminal part less in the bill of materials than before.
    let single = demo_project();
    let count = |prj: &Project| strip_layout(prj, "-X1").iter().filter(|e| e.part == "GEN.TB-2.5").count();
    assert_eq!(count(&prj) + 1, count(&single));

    // The terminal diagram lists both levels of the block.
    let an = crate::analysis::Analysis::checked(&prj);
    let rows = &an.report(&prj, ReportKind::TerminalDiagram).unwrap().rows;
    let at = |label: &str| rows.iter().find(|r| r[1] == label).map(|r| r[2].clone()).unwrap();
    assert_eq!((at("1"), at("2"), at("3")), ("1".to_string(), "2".to_string(), String::new()));

    // A single-level part cannot carry two levels.
    let wrong = two_levels(1);
    let problems = level_problems(&wrong, "-X1");
    assert_eq!(problems, ["-X1: terminal 1 uses 2 levels, its part GEN.TB-2.5 has 1"]);
    let an = crate::analysis::Analysis::checked(&wrong);
    assert!(an.issues.iter().any(|i| i.message() == problems[0]));

    // Renumbering keeps the levels with their terminals.
    let mut prj = two_levels(2);
    move_terminal(&mut prj, "-X1", 2, true);
    renumber(&mut prj, "-X1");
    assert_eq!(prj.strips["-X1"].levels.get("3"), Some(&2));
}

#[test]
fn jumper_kinds() {
    let mut prj = two_levels(2);
    let data = prj.strips.get_mut("-X1").unwrap();
    data.jumpers.insert("1".into(), 1);
    data.jumpers.insert("3".into(), 1);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.kind, j.level), (JumperKind::PlugIn, 1));
    assert_eq!(j.span, ["1", "3"], "the plug-in jumper bridges level 1 of both blocks");
    assert_eq!(j.problem, None);

    // Across levels only a wire jumper fits.
    let data = prj.strips.get_mut("-X1").unwrap();
    data.jumpers.insert("2".into(), 1);
    assert_eq!(strip_jumpers(&prj, "-X1")[0].problem, Some(JumperProblem::DifferentLevels));
    prj.strips.get_mut("-X1").unwrap().jumper_kinds.insert(1, JumperKind::Wire);
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.problem.as_ref(), j.part.as_str(), j.length_text()), (None, "", "2 wires".to_string()));
    assert!(j.broken.is_empty());

    // Saddle jumpers take the parts of the tech data `Saddle jumper`.
    let mut prj = demo_project();
    let data = prj.strips.entry("-X1".into()).or_default();
    data.jumpers.insert("1".into(), 2);
    data.jumpers.insert("2".into(), 2);
    data.jumper_kinds.insert(2, JumperKind::Saddle);
    assert_eq!(strip_jumpers(&prj, "-X1")[0].problem, Some(JumperProblem::NoPart));
    let p = prj.parts.iter_mut().find(|p| p.number == "GEN.TB-2.5").unwrap();
    p.tech_data.push_str("\nSaddle jumper: GEN.JMP-2.5-2");
    let j = &strip_jumpers(&prj, "-X1")[0];
    assert_eq!((j.problem.as_ref(), j.part.as_str()), (None, "GEN.JMP-2.5-2"));
    assert_eq!(jumper_kind_title(JumperKind::Saddle), "Saddle jumper");
}
