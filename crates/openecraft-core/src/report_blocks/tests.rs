use super::*;
use crate::forms::{FilterOp, ReportScheme, RowFilter, SortKey};
use crate::sample::demo_project;

fn block(name: &str, kind: ReportKind) -> ReportBlock {
    ReportBlock { name: name.into(), kind: kind.into(), ..Default::default() }
}

fn block_pages_of(prj: &Project, name: &str) -> Vec<usize> {
    report_pages(prj, Some(name))
}

#[test]
fn wildcards() {
    assert!(matches("", "-X1"));
    assert!(matches("-X*", "-X12"));
    assert!(matches("-x?", "-X1"));
    assert!(!matches("-X?", "-X12"));
    assert!(matches("-A1, -X*", "-X3"));
    assert!(!matches("-A1; -B*", "-X3"));
}

#[test]
fn schemes_filter_and_sort_the_rows() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let table = &an.report(&prj, ReportKind::ConnectionList).unwrap();
    let form = ReportForm {
        filters: vec![RowFilter { column: "Cable".into(), op: FilterOp::Equals, value: "-w1".into() }],
        sort: vec![SortKey { column: "Core".into(), descending: true }],
        ..Default::default()
    };
    let t = scheme_table(table, Some(&form));
    let cores: Vec<&str> = t.rows.iter().map(|r| r[8].as_str()).collect();
    assert_eq!(cores, vec!["GY", "GNYE", "BN", "BK"]);
    assert_eq!(t.rows.len(), t.targets.len());
    // Unknown columns are ignored.
    let form = ReportForm { filters: vec![RowFilter { column: "Nope".into(), op: FilterOp::Empty, value: String::new() }], ..Default::default() };
    assert_eq!(scheme_table(table, Some(&form)).rows.len(), table.rows.len());
    assert!(RowFilter { op: FilterOp::NotEmpty, ..Default::default() }.passes("x"));
    assert!(RowFilter { op: FilterOp::StartsWith, value: "-K".into(), ..Default::default() }.passes("-k1:A1"));
}

#[test]
fn named_schemes_are_found_by_report_and_name() {
    let mut prj = Project::default();
    prj.forms.reports.insert(ReportKind::DeviceList.into(), ReportForm { title: "Default".into(), ..Default::default() });
    let form = ReportForm { title: "Workshop".into(), ..Default::default() };
    prj.forms.schemes.push(ReportScheme { name: "Workshop".into(), kind: ReportKind::DeviceList.into(), form });
    assert_eq!(forms::report_form(&prj, ReportKind::DeviceList, "Workshop").unwrap().title, "Workshop");
    assert_eq!(forms::report_form(&prj, ReportKind::DeviceList, "").unwrap().title, "Default");
    assert_eq!(forms::report_form(&prj, ReportKind::DeviceList, "Other").unwrap().title, "Default");
    assert!(forms::report_form(&prj, ReportKind::BillOfMaterials, "Workshop").is_none());
    assert_eq!(forms::scheme_names(&prj, ReportKind::DeviceList), vec!["Workshop".to_string()]);
}

#[test]
fn blocks_generate_one_page_set_per_group_and_replace_them() {
    let mut prj = demo_project();
    prj.forms.schemes.push(ReportScheme { name: "Short".into(), kind: ReportKind::CableConnections.into(), form: ReportForm { rows: 3, ..Default::default() } });
    prj.report_blocks.push(ReportBlock { scheme: "Short".into(), function: "A1".into(), ..block("Cables", ReportKind::CableConnections) });
    let an = Analysis::checked(&prj);
    let before = prj.pages.len();
    // -W1 has four cores: two pages with three rows each.
    assert_eq!(generate(&mut prj, &an, 0), Ok(2));
    let pages = block_pages_of(&prj, "Cables");
    assert_eq!(pages.len(), 2);
    assert_eq!(prj.pages.len(), before + 2);
    let names: Vec<String> = pages.iter().map(|&i| prj.pages[i].name.clone()).collect();
    assert_eq!(prj.pages[pages[0]].function, "A1");
    let an = Analysis::checked(&prj);
    let first = page_table(&prj, &an, pages[0]).unwrap();
    let second = page_table(&prj, &an, pages[1]).unwrap();
    assert_eq!((first.rows.len(), second.rows.len()), (3, 1));
    assert!(first.continued && !second.continued);
    assert_eq!(first.title, "Cable connection diagram -W1");
    assert_eq!(second.rows[0][4], "GNYE");
    // Generating again keeps place and page names.
    assert_eq!(generate(&mut prj, &an, 0), Ok(2));
    let again = block_pages_of(&prj, "Cables");
    assert_eq!(again, pages);
    assert_eq!(again.iter().map(|&i| prj.pages[i].name.clone()).collect::<Vec<_>>(), names);
    // Released pages stop the block.
    prj.pages[pages[0]].locked = true;
    assert!(generate(&mut prj, &an, 0).is_err());
    assert_eq!(prj.pages.len(), before + 2);
}

#[test]
fn group_filter_and_whole_report_blocks() {
    let mut prj = demo_project();
    prj.report_blocks.push(ReportBlock { groups: "-Y*".into(), ..block("None", ReportKind::TerminalDiagram) });
    prj.report_blocks.push(ReportBlock { per_group: false, ..block("All", ReportKind::DeviceList) });
    let pages = generate_all(&mut prj).unwrap();
    assert_eq!(pages, 1, "no strip matches -Y*, the device list fits one page");
    let all = block_pages_of(&prj, "All");
    let an = Analysis::checked(&prj);
    let t = page_table(&prj, &an, all[0]).unwrap();
    assert_eq!(t.rows.len(), an.report(&prj, ReportKind::DeviceList).unwrap().rows.len());
    // The block is saved with the project.
    let back = Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(back.report_blocks, prj.report_blocks);
    assert_eq!(back.pages[all[0]].report, prj.pages[all[0]].report);
}

#[test]
fn line_up_pages_draw_the_strip() {
    let mut prj = demo_project();
    prj.report_blocks.push(block("Line-up", ReportKind::TerminalLineup));
    generate_all(&mut prj).unwrap();
    let page = block_pages_of(&prj, "Line-up")[0];
    let an = Analysis::checked(&prj);
    let t = page_table(&prj, &an, page).unwrap();
    let labels: Vec<&str> = t.lineup.iter().filter(|b| !b.accessory).map(|b| b.label.as_str()).collect();
    assert_eq!(labels, vec!["1", "2", "3", "PE"]);
    assert_eq!(t.lineup.len(), t.rows.len());
    let svg = crate::svg::page_svg(&prj, page);
    assert!(svg.contains(">PE<"));
}

#[test]
fn report_pages_are_output_into_other_projects() {
    let mut src = demo_project();
    src.info.name = "Conveyor".into();
    src.report_blocks.push(block("Terminals", ReportKind::TerminalDiagram));
    generate_all(&mut src).unwrap();
    let an = Analysis::checked(&src);
    let pages = report_pages(&src, Some("Terminals"));
    let mut target = Project::default();
    target.info.name = "Plant".into();
    assert_eq!(output_to_project(&src, &an, &pages, &mut target), Ok(1));
    assert_eq!(target.pages.len(), 2);
    let out = target.pages.len() - 1;
    let frozen = target.pages[out].report.as_ref().and_then(|r| r.frozen.clone()).unwrap();
    assert_eq!(frozen.source, "Conveyor");
    assert!(frozen.rows.iter().any(|r| r[1] == "PE"));
    // The fixed page does not depend on the target's schematic.
    let an_target = Analysis::checked(&target);
    assert_eq!(page_table(&target, &an_target, out).unwrap().rows, frozen.rows);
    let svg = crate::svg::page_svg(&target, out);
    assert!(svg.contains("Output from project Conveyor"));
    // Output again: replaced, not added; through the file as well.
    let all = report_pages(&src, None);
    let (json, n) = output_to_file(&src, &an, &all, &target.to_json()).unwrap();
    assert_eq!(n, all.len());
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.pages.len(), 1 + all.len());
    assert!(back.pages[1..].iter().all(|p| p.report.as_ref().is_some_and(|r| r.frozen.is_some())));
    let mut ids: Vec<u64> = back.pages.iter().map(|p| p.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), back.pages.len());
}

#[test]
fn renaming_keeps_blocks_schemes_and_pages_together() {
    let mut prj = demo_project();
    prj.forms.schemes.push(ReportScheme { name: "A".into(), kind: ReportKind::TerminalDiagram.into(), form: ReportForm::default() });
    prj.report_blocks.push(ReportBlock { scheme: "A".into(), ..block("Strips", ReportKind::TerminalDiagram) });
    prj.report_blocks.push(block("Other", ReportKind::DeviceList));
    generate_all(&mut prj).unwrap();
    assert!(!rename_block(&mut prj, "Strips", "Other"), "name in use");
    assert!(!rename_block(&mut prj, "Strips", " "));
    assert!(rename_block(&mut prj, "Strips", "Terminals"));
    assert_eq!(report_pages(&prj, Some("Terminals")).len(), 1);
    assert!(report_pages(&prj, Some("Strips")).is_empty());
    assert!(forms::rename_scheme(&mut prj, ReportKind::TerminalDiagram, "A", "Workshop"));
    assert!(!forms::rename_scheme(&mut prj, ReportKind::DeviceList, "Workshop", "B"), "scheme of another report");
    assert_eq!(prj.report_blocks[0].scheme, "Workshop");
    let page = report_pages(&prj, Some("Terminals"))[0];
    assert_eq!(prj.pages[page].report.as_ref().unwrap().scheme, "Workshop");
    // Generating again finds the renamed pages instead of adding new ones.
    let n = prj.pages.len();
    generate_all(&mut prj).unwrap();
    assert_eq!(prj.pages.len(), n);
}

#[test]
fn report_tables_get_totals_shading_and_alignment() {
    let mut prj = demo_project();
    let page = prj.pages.iter().position(|p| p.kind == PageKind::Report(ReportKind::BillOfMaterials.into())).unwrap();
    let an = Analysis::checked(&prj);
    let table = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    let qty = column(&table, "Qty").unwrap();
    let sum: f64 = table.rows.iter().filter_map(|r| cell_number(&r[qty]).map(|n| n.0)).sum();
    let form = ReportForm {
        totals: vec!["Qty".into()],
        // All rows on the page: the total row follows the last one.
        rows: 80,
        shade_rows: true,
        align: [("Qty".to_string(), crate::symbols::Anchor::Right)].into(),
        ..Default::default()
    };
    prj.forms.reports.insert(ReportKind::BillOfMaterials.into(), form);
    let t = page_table(&prj, &an, page).unwrap();
    assert_eq!(t.more, 0);
    assert!(t.shade_rows);
    assert_eq!(t.align[qty], crate::symbols::Anchor::Right);
    assert_eq!(t.align[0], crate::symbols::Anchor::Left);
    assert_eq!(t.totals[0], "Total");
    assert_eq!(t.totals[qty], format!("{sum}"));
    assert!(crate::svg::page_svg(&prj, page).contains(">Total<"));
}

#[test]
fn totals_sum_numbers_with_their_unit() {
    assert_eq!(cell_number(" 2,5 m"), Some((2.5, "m")));
    assert_eq!(cell_number("-3"), Some((-3.0, "")));
    assert_eq!(cell_number("m 2"), None);
    let table = Table {
        headers: ["Tag", "Length", "Note"].map(crate::params::Text::Borrowed).to_vec(),
        widths: vec![1.0; 3],
        rows: vec![vec!["-W1".into(), "1.5 m".into(), "x".into()], vec!["-W2".into(), "2,25 m".into(), "".into()], vec!["-W3".into(), "".into(), "".into()]],
        targets: vec![None; 3],
    };
    let totals = total_row(&table, &[0, 1, 2], &[0, 1, 2], &["Length".into()]);
    assert_eq!(totals, vec!["Total", "3.75 m", ""]);
    // Only the rows given (e.g. those of one group).
    assert_eq!(total_row(&table, &[1], &[1], &["Length".into()]), vec!["2.25 m"]);
    assert_eq!(total_row(&table, &[0], &[0], &["Nothing".into()]), vec![""]);
}
