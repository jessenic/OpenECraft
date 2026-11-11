use super::*;
use crate::sample::demo_project;

/// A DXF file from `code value` pairs (one per line, separated by a blank).
fn dxf(header: &str, blocks: &str, entities: &str) -> String {
    let body = |s: &str| s.trim().lines().flat_map(|l| l.trim().splitn(2, ' ').map(str::to_string).collect::<Vec<_>>()).collect::<Vec<_>>().join("\n");
    let mut out = String::new();
    for (name, content) in [("HEADER", header), ("BLOCKS", blocks), ("ENTITIES", entities)] {
        out.push_str(&format!("0\nSECTION\n2\n{name}\n"));
        if !content.trim().is_empty() {
            out.push_str(&body(content));
            out.push('\n');
        }
        out.push_str("0\nENDSEC\n");
    }
    out.push_str("0\nEOF\n");
    out
}

fn import(text: &str, fit: bool) -> (Project, DxfImport) {
    let mut prj = Project::default();
    let r = import_page(&mut prj, text, "Old drawing", 0, DxfOptions { fit }).unwrap();
    (prj, r)
}

#[test]
fn entities_become_lines_and_texts() {
    let text = dxf(
        "",
        "",
        "0 LINE\n8 0\n10 0\n20 0\n11 100\n21 0
         0 LWPOLYLINE\n90 3\n70 1\n10 0\n20 10\n10 10\n20 10\n10 10\n20 20
         0 CIRCLE\n10 50\n20 50\n40 5
         0 ARC\n10 50\n20 50\n40 10\n50 0\n51 90
         0 POLYLINE\n66 1\n70 0\n0 VERTEX\n10 0\n20 30\n0 VERTEX\n10 20\n20 30\n0 SEQEND
         0 TEXT\n10 0\n20 40\n40 3.5\n1 Motor %%c 20
         0 MTEXT\n10 0\n20 60\n40 2.5\n71 1\n1 {\\fArial;Line 1}\\PLine 2
         0 HATCH\n2 SOLID",
    );
    let (prj, r) = import(&text, false);
    let page = &prj.pages[r.page];
    assert_eq!((page.kind, page.description.as_str()), (PageKind::Schematic, "Old drawing"));
    assert_eq!(r.texts, 2);
    // Line + closed triangle (3) + circle + arc + 1 polyline segment.
    assert!(r.lines > 1 + 3 + 1, "{}", r.lines);
    assert_eq!(r.skipped, vec!["HATCH".to_string()]);
    let texts: Vec<&str> = page.texts.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"Motor Ø 20") && texts.contains(&"Line 1\nLine 2"), "{texts:?}");
    // 1:1, y flipped: the drawing's top (y = 60 + text) is at the top of the area.
    let line = page.shapes.iter().find(|s| (s.a.x - s.b.x).abs() > 99.0).unwrap();
    assert!((line.b.x - line.a.x).abs() - 100.0 < 0.01);
    let area = crate::frame::PlotFrame::default().area();
    assert!(page.shapes.iter().all(|s| s.a.y >= area.min.y - 0.01 && s.b.y >= area.min.y - 0.01));
}

#[test]
fn blocks_are_placed_scaled_and_rotated() {
    let text = dxf(
        "",
        "0 BLOCK\n2 BOX\n10 5\n20 5\n0 LINE\n10 5\n20 5\n11 15\n21 5\n0 ENDBLK",
        "0 INSERT\n2 BOX\n10 100\n20 100\n41 2\n42 2\n50 90
         0 LINE\n10 0\n20 0\n11 1\n21 0",
    );
    let (prj, r) = import(&text, false);
    let page = &prj.pages[r.page];
    assert_eq!(r.lines, 2);
    // The block line (10 long, scaled 2, turned 90°) runs vertically, 20 long.
    let l = page.shapes.iter().find(|s| (s.a.y - s.b.y).abs() > 1.0).unwrap();
    assert!(((l.a.y - l.b.y).abs() - 20.0).abs() < 0.01 && (l.a.x - l.b.x).abs() < 0.01, "{l:?}");
}

#[test]
fn large_drawings_are_scaled_to_the_page_and_units_are_respected() {
    let text = dxf("", "", "0 LINE\n10 0\n20 0\n11 4000\n21 1000");
    let (prj, r) = import(&text, true);
    assert!(r.scale < 0.1);
    let area = crate::frame::PlotFrame::default().area();
    for s in &prj.pages[r.page].shapes {
        for q in [s.a, s.b] {
            assert!(area.expand(0.01).contains(q), "{q:?}");
        }
    }
    // Inches: a 1" line is 25.4 mm.
    let text = dxf("9 $INSUNITS\n70 1", "", "0 LINE\n10 0\n20 0\n11 1\n21 0");
    let (prj, r) = import(&text, false);
    let s = &prj.pages[r.page].shapes[0];
    assert!(((s.b.x - s.a.x).abs() - 25.4).abs() < 0.01);
}

#[test]
fn source_coordinates_survive_units_fitting_and_page_translation() {
    let text = dxf("9 $INSUNITS\n70 5", "", "0 LINE\n10 -20\n20 30\n11 400\n21 100");
    for fit in [false, true] {
        let (prj, r) = import(&text, fit);
        assert_eq!(r.source_min, p(-200.0, 300.0));
        assert_eq!(r.source_max, p(4000.0, 1000.0));
        let line = &prj.pages[r.page].shapes[0];
        let recover = |q: P| p((q.x - r.page_origin.x) / r.scale + r.source_min.x,
                              r.source_max.y - (q.y - r.page_origin.y) / r.scale);
        for (actual, expected) in [(recover(line.a), p(-200.0, 300.0)), (recover(line.b), p(4000.0, 1000.0))] {
            assert!((actual.x - expected.x).abs() < 0.001 && (actual.y - expected.y).abs() < 0.001);
        }
        assert_eq!(r.page_origin, crate::frame::PlotFrame::default().area().min);
    }
}

#[test]
fn exported_pages_come_back() {
    let demo = demo_project();
    let page = demo.pages.iter().position(|p| p.symbols.iter().any(|s| s.tag == "-K1")).unwrap();
    let text = crate::dxf::pages_dxf(&demo, &[page]);
    let (prj, r) = import(&text, true);
    assert!(r.lines > 50 && r.texts > 5, "{r:?}");
    assert!(prj.pages[r.page].texts.iter().any(|t| t.text.contains("-K1")));
}

#[test]
fn text_codes() {
    assert_eq!(clean_text("%%d90 \\U+00B1x"), "°90 ±x");
    assert_eq!(clean_text("{\\H2.5;\\C1;Red}\\Pnext"), "Red\nnext");
    assert_eq!(clean_text("a\\\\b"), "a\\b");
}

#[test]
fn files_that_are_no_dxf_are_refused() {
    let mut prj = Project::default();
    assert!(import_page(&mut prj, "hello", "x", 0, DxfOptions::default()).is_err());
    assert!(import_page(&mut prj, &dxf("", "", ""), "x", 0, DxfOptions::default()).unwrap_err().contains("nothing"));
    assert_eq!(prj.pages.len(), 1);
}

#[test]
fn texts_turn_with_their_block() {
    let text = dxf(
        "",
        "0 BLOCK\n2 LBL\n10 0\n20 0\n0 TEXT\n10 0\n20 0\n40 2.5\n50 10\n1 HELLO\n0 ENDBLK",
        "0 INSERT\n2 LBL\n10 100\n20 100\n41 2\n42 2\n50 90",
    );
    let (prj, r) = import(&text, false);
    let t = &prj.pages[r.page].texts[0];
    assert!((t.angle - 100.0).abs() < 0.01, "{t:?}");
    assert!((t.size - 5.0).abs() < 0.01, "{t:?}");
}

#[test]
fn blocks_in_a_mirrored_block_turn_the_other_way() {
    // IN: a line along x, turned 90° inside OUT; OUT is inserted mirrored in x.
    let blocks = "0 BLOCK\n2 IN\n10 0\n20 0\n0 LINE\n10 0\n20 0\n11 10\n21 0\n0 ENDBLK
                  0 BLOCK\n2 OUT\n10 0\n20 0\n0 INSERT\n2 IN\n10 0\n20 0\n50 90\n0 ENDBLK";
    let d = parse(&dxf("", blocks, "0 INSERT\n2 OUT\n10 0\n20 0\n41 -1\n42 1")).unwrap();
    let mut geo = Vec::new();
    entities(&d, &d.entities, Tf::ID, 0, &mut geo, &mut BTreeSet::new());
    // Turned up to (0, 10); mirroring in x leaves it there.
    let Geo::Line(a, b, _) = &geo[0] else { panic!("{geo:?}") };
    assert!(a.x.abs() < 0.01 && a.y.abs() < 0.01 && b.x.abs() < 0.01 && (b.y - 10.0).abs() < 0.01, "{a:?} {b:?}");
}
