use super::*;
use crate::geom::p;
use crate::model::{Page, SymbolInst};

#[test]
fn interruption_references_and_checks_respect_the_connection_scope() {
    let mut prj = project();
    for page in &mut prj.pages {
        for s in &mut page.symbols {
            s.values.insert("function.connection_scope".into(), if s.id % 2 == 0 { "CC1" } else { "CC2" }.into());
        }
    }
    let idx = Index::build(&prj);
    for page in &prj.pages {
        for s in &page.symbols {
            let targets = idx.interruption_targets(&prj, &s.interruption_key(), s.id);
            assert_eq!(targets.len(), 1);
            let other = prj.pages[targets[0].page].symbols.iter().find(|other| other.id == targets[0].sym_id).unwrap();
            assert_eq!(s.interruption_key(), other.interruption_key());
        }
    }
    prj.pages[1].symbols.pop();
    let an = crate::analysis::Analysis::checked(&prj);
    assert!(an.issues.iter().any(|i| i.message() == "Interruption point 'A' has no counterpart"));
}

/// Four interruption points named `A` on two pages, and a coil on page 2.
fn project() -> Project {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages.push(Page::new(id, "2", "", PageKind::Schematic));
    prj.pages[1].function = "A2".into();
    for (page, x) in [(0, 300.0), (0, 60.0), (1, 60.0), (1, 200.0)] {
        let id = prj.new_id();
        prj.pages[page].symbols.push(SymbolInst { id, sym: "intr".into(), pos: p(x, 100.0), label: "A".into(), ..Default::default() });
    }
    prj
}

fn targets(prj: &Project, idx: &Index, page: usize, n: usize) -> Vec<String> {
    let me = prj.pages[page].symbols[n].id;
    idx.interruption_targets(prj, "A", me).iter().map(|l| l.reference(prj)).collect()
}

#[test]
fn interruption_points_all_star_and_pairs() {
    let mut prj = project();
    let idx = Index::build(&prj);
    // In schematic order: page 1 x=60, page 1 x=300, page 2 x=60, page 2 x=200.
    assert_eq!(targets(&prj, &idx, 0, 1).len(), 3, "all others");

    prj.xref.interruptions = InterruptionMode::Star;
    let source = targets(&prj, &idx, 0, 1);
    assert_eq!(source.len(), 3, "the first point is the source");
    let leaf = targets(&prj, &idx, 1, 1);
    assert_eq!(leaf.len(), 1);
    assert!(leaf[0].starts_with("/1."), "{leaf:?}");

    prj.xref.interruptions = InterruptionMode::Pairs;
    let first = targets(&prj, &idx, 0, 1);
    let second = targets(&prj, &idx, 0, 0);
    assert_eq!((first.len(), second.len()), (1, 1));
    assert_eq!(first[0], idx.interruptions["A"].iter().find(|l| l.sym_id == prj.pages[0].symbols[0].id).unwrap().reference(&prj));
    let third = targets(&prj, &idx, 1, 0);
    assert!(third[0].starts_with("=A2/2."), "{third:?}");
    assert!(idx.interruption_targets(&prj, "nope", 1).is_empty());

    // An odd number of points: the last has no partner, the check says so.
    prj.pages[1].symbols.pop();
    let an = crate::analysis::Analysis::checked(&prj);
    assert!(an.issues.iter().any(|i| i.message() == "Interruption point 'A' has no counterpart"));
}

#[test]
fn reference_format_and_structure() {
    let mut prj = project();
    let idx = Index::build(&prj);
    let loc = idx.interruptions["A"].iter().find(|l| l.page == 1).copied().unwrap();
    assert!(loc.reference_from(&prj, 0).starts_with("=A2/2."), "another structure is shown");
    assert!(loc.reference_from(&prj, 1).starts_with("/2."));
    prj.xref.structure = XrefStructure::Never;
    assert!(loc.reference_from(&prj, 0).starts_with("/2."));
    prj.xref.structure = XrefStructure::Always;
    assert!(loc.reference_from(&prj, 1).starts_with("=A2/2."));

    prj.xref = crate::model::CrossRefs { prefix: String::new(), separator: "-".into(), row: true, ..Default::default() };
    let r = crate::frame::reference(&prj, 1, loc.pos);
    let frame = crate::frame::of_page(&prj, 1);
    assert_eq!(r, format!("2-{}-{}", frame.column_of(loc.pos), frame.row_of(loc.pos)));
    assert!(!prj.xref.is_default());
}
