use super::*;
use openecraft_core::analysis::Analysis;
use openecraft_core::sample::demo_project;
use egui::{Event, Modifiers, RawInput};

#[test]
fn electrical_items_remain_selectable_inside_page_graphics() {
    use openecraft_core::{symbols::{Func, Prim}, symlib::UserSymbol};
    let mut prj = Project::default();
    prj.symbols.push(UserSymbol {
        key: "user:frame".into(), func: Func::Graphic,
        prims: vec![Prim::Line { a: p(0., 0.), b: p(300., 200.), dashed: false }],
        ..Default::default()
    });
    prj.pages[0].symbols.push(SymbolInst { id: 1, sym: "user:frame".into(), ..Default::default() });
    prj.pages[0].symbols.push(coil(2, p(100., 100.), "-K1"));
    prj.pages[0].wires.push(Wire { id: 3, a: p(80., 100.), b: p(80., 160.), ..Default::default() });
    prj.pages[0].junctions.push(Junction { id: 4, pos: p(80., 120.), ..Default::default() });
    let ed = Editor { zoom: 8., ..Default::default() };
    assert_eq!(ed.hit(&prj, 0, p(80., 145.)), Some(3), "wire inside the frame");
    assert_eq!(ed.hit(&prj, 0, p(80., 120.)), Some(4), "junction inside the frame");
    assert_eq!(ed.hit(&prj, 0, p(100., 100.)), Some(2), "device keeps priority");
    assert_eq!(ed.hit(&prj, 0, p(200., 150.)), Some(1), "artwork remains selectable");
}

/// Runs one editor frame on page `page` with the given input events.
fn frame(ed: &mut Editor, ctx: &egui::Context, prj: &mut Project, page: usize, events: Vec<Event>) -> CanvasOutput {
    let idx = Index::build(prj);
    let an = Analysis::build(prj);
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0))),
        events,
        ..Default::default()
    };
    let mut canvas = CanvasOutput::default();
    let mut out = ctx.run_ui(input, |ui| {
        canvas = ed.ui(ui, prj, page, &idx, &an);
    });
    out.textures_delta.clear();
    canvas
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::default() }
}

/// Replay a drag over the sheet using a local vendor sample without checking it into the repository.
#[test]
#[ignore = "requires OPENECRAFT_DRAG_EDZ pointing to a schematic part archive"]
fn drag_imported_edz_part_onto_sheet() {
    use openecraft_core::symlib::ema;
    let bytes = std::fs::read(std::env::var("OPENECRAFT_DRAG_EDZ").unwrap()).unwrap();
    let part = openecraft_edz::read(&bytes).unwrap().parts.remove(0);
    let variants: Vec<ema::Variant> = part.eplan_variants.iter().map(|v| serde::Deserialize::deserialize(v.as_ref()).unwrap()).collect();
    let mut prj = Project::new_default();
    ema::import_project(&mut prj, &variants);
    let (ctx, mut ed) = (egui::Context::default(), Editor::default());
    let idx = Index::build(&prj);
    let an = Analysis::build(&prj);
    let mut paint = |events| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200., 800.))),
            events,
            ..Default::default()
        };
        let mut drop = None;
        let mut output = ctx.run_ui(input, |ui| drop = ed.ui(ui, &mut prj, 0, &idx, &an).dropped_part);
        output.textures_delta.clear();
        drop
    };
    let start = pos2(240., 200.);
    paint(vec![Event::PointerMoved(start), button(start, true)]);
    egui::DragAndDrop::set_payload(&ctx, PartDrag { number: part.number.clone() });
    let now = std::time::Instant::now();
    for i in 1..=100 {
        assert!(paint(vec![Event::PointerMoved(start + vec2(i as f32, 0.))]).is_none());
    }
    println!("100 schematic pointer frames: {:?}", now.elapsed());
    let (number, at) = paint(vec![button(start + vec2(100., 0.), false)]).expect("part released over schematic");
    assert_eq!(number, part.number);
    let (key, pins) = ema::part_symbol(&mut prj, &part, None).expect("schematic representation");
    assert!(!pins.is_empty());
    assert!(ed.place_part_at(&mut prj, 0, key, pins, &number, at).placed.is_some());
}

#[test]
fn macro_insertion_waits_for_the_click() {
    let mut prj = demo_project();
    let page = 1;
    let sel: BTreeSet<u64> = prj.pages[page].symbols.iter().filter(|s| s.tag == "-K1").map(|s| s.id).collect();
    let clip = ops::copy(&prj.pages[page], &sel);
    let before = prj.pages[page].symbols.len();
    let ctx = egui::Context::default();
    let mut ed = Editor { insert: Some(clip), ..Default::default() };
    ed.set_tool(Tool::Insert);
    // Frames without a click keep the macro (regression: it was taken on the first frame).
    let at = pos2(600.0, 400.0);
    frame(&mut ed, &ctx, &mut prj, page, vec![]);
    frame(&mut ed, &ctx, &mut prj, page, vec![Event::PointerMoved(at)]);
    assert!(ed.insert.is_some());
    assert_eq!(prj.pages[page].symbols.len(), before);
    // A click inserts it once and returns to the select tool.
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![]);
    assert!(ed.insert.is_none());
    assert_eq!(ed.tool, Tool::Select);
    assert_eq!(prj.pages[page].symbols.len(), before + sel.len());
    assert_eq!(ed.selection.len(), sel.len());
}

#[test]
fn released_pages_are_not_edited() {
    let mut prj = demo_project();
    let page = 1;
    prj.pages[page].locked = true;
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ed.set_tool(Tool::Place("coil"));
    let before = prj.pages[page].symbols.len();
    let at = pos2(600.0, 400.0);
    frame(&mut ed, &ctx, &mut prj, page, vec![Event::PointerMoved(at)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert_eq!(prj.pages[page].symbols.len(), before);
    // Unlocked, the same click places the coil.
    prj.pages[page].locked = false;
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert_eq!(prj.pages[page].symbols.len(), before + 1);
}

#[test]
fn pages_locked_by_other_users_are_not_edited() {
    let mut prj = demo_project();
    let page = 1;
    let ctx = egui::Context::default();
    let mut ed = Editor { read_only: true, ..Editor::default() };
    ed.set_tool(Tool::Place("coil"));
    let before = prj.pages[page].symbols.len();
    let at = pos2(600.0, 400.0);
    frame(&mut ed, &ctx, &mut prj, page, vec![Event::PointerMoved(at)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert_eq!(prj.pages[page].symbols.len(), before);
    ed.read_only = false;
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert_eq!(prj.pages[page].symbols.len(), before + 1);
}

#[test]
fn review_tools_work_on_released_pages() {
    let mut prj = demo_project();
    let page = 1;
    prj.pages[page].locked = true;
    let ctx = egui::Context::default();
    let mut ed = Editor { read_only: true, ..Editor::default() };
    ed.set_tool(Tool::Comment);
    let at = pos2(600.0, 400.0);
    frame(&mut ed, &ctx, &mut prj, page, vec![Event::PointerMoved(at)]);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    let out = frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert!(matches!(out.review, Some(ReviewInput::Comment(_))));

    // Markup rectangles take two clicks.
    ed.set_tool(Tool::Markup(MarkupKind::Rect, Pen::Green));
    let b = pos2(700.0, 450.0);
    frame(&mut ed, &ctx, &mut prj, page, vec![button(at, true)]);
    let out = frame(&mut ed, &ctx, &mut prj, page, vec![button(at, false)]);
    assert!(out.review.is_none());
    frame(&mut ed, &ctx, &mut prj, page, vec![Event::PointerMoved(b), button(b, true)]);
    let out = frame(&mut ed, &ctx, &mut prj, page, vec![button(b, false)]);
    match out.review {
        Some(ReviewInput::Markup(MarkupKind::Rect, Pen::Green, a, b)) => assert!(b.x > a.x && b.y > a.y),
        other => panic!("{other:?}"),
    }
}

const SCREEN: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1200.0, 800.0) };

/// Clicks at the page point `q`.
fn click(ed: &mut Editor, ctx: &egui::Context, prj: &mut Project, page: usize, q: P) {
    let at = ed.w2s(SCREEN, q);
    frame(ed, ctx, prj, page, vec![Event::PointerMoved(at)]);
    frame(ed, ctx, prj, page, vec![button(at, true)]);
    frame(ed, ctx, prj, page, vec![button(at, false)]);
    // Moving away avoids double clicks with the next click.
    frame(ed, ctx, prj, page, vec![Event::PointerMoved(at + vec2(300.0, 0.0))]);
}

fn ready(ed: &mut Editor, ctx: &egui::Context, prj: &mut Project) {
    frame(ed, ctx, prj, 0, vec![]);
    frame(ed, ctx, prj, 0, vec![]);
}

#[test]
fn dimension_tool_takes_three_clicks() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Dimension(DimKind::Horizontal));
    click(&mut ed, &ctx, &mut prj, 0, p(40.0, 40.0));
    click(&mut ed, &ctx, &mut prj, 0, p(80.0, 48.0));
    assert!(prj.pages[0].dimensions.is_empty());
    click(&mut ed, &ctx, &mut prj, 0, p(60.0, 28.0));
    let d = &prj.pages[0].dimensions[0];
    assert_eq!((d.a, d.b), (p(40.0, 40.0), p(80.0, 48.0)));
    assert!((d.offset - 12.0).abs() < 1e-4);
    assert_eq!(openecraft_core::graphics::dim_text(d), "40");
}

#[test]
fn cable_definition_tool_makes_cores() {
    let mut prj = Project::default();
    for (i, x) in [40.0, 48.0].into_iter().enumerate() {
        prj.pages[0].wires.push(Wire { id: 100 + i as u64, a: p(x, 40.0), b: p(x, 100.0), ..Default::default() });
    }
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::CableDef);
    click(&mut ed, &ctx, &mut prj, 0, p(36.0, 60.0));
    click(&mut ed, &ctx, &mut prj, 0, p(52.0, 60.0));
    assert_eq!(prj.cables.len(), 1);
    assert_eq!(prj.pages[0].cable_defs.len(), 1);
    assert!(prj.pages[0].wires.iter().all(|w| w.cable == prj.cables[0].tag && !w.core.is_empty()));
    // The tool stays for the next line, which extends the same cable.
    assert_eq!(ed.tool, Tool::CableDef);
    assert_eq!(ed.cable_target, prj.cables[0].tag);
    assert!(ed.hint().contains(&prj.cables[0].tag), "{}", ed.hint());
}

#[test]
fn polygon_ends_with_enter() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Polygon);
    for q in [p(40.0, 40.0), p(80.0, 40.0), p(60.0, 80.0)] {
        click(&mut ed, &ctx, &mut prj, 0, q);
    }
    let enter = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::default() };
    frame(&mut ed, &ctx, &mut prj, 0, vec![enter]);
    let s = &prj.pages[0].shapes[0];
    assert_eq!((s.kind, s.pts.len()), (ShapeKind::Polygon, 3));
}

#[test]
fn tag_block_is_dragged_into_place() {
    let mut prj = Project::default();
    prj.pages[0].symbols.push(SymbolInst { id: 7, sym: "coil".into(), pos: p(100.0, 100.0), tag: "-K1".into(), pins: vec!["A1".into(), "A2".into()], ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.selection.insert(7);
    let def = prj.symbol("coil").unwrap();
    let block = render::tag_block_bounds(&prj, 0, &def, &prj.pages[0].symbols[0]).unwrap();
    let from = ed.w2s(SCREEN, block.center());
    let to = ed.w2s(SCREEN, block.center() + p(-20.0, 10.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![Event::PointerMoved(from)]);
    frame(&mut ed, &ctx, &mut prj, 0, vec![button(from, true)]);
    for i in 1..=5 {
        let t = i as f32 / 5.0;
        frame(&mut ed, &ctx, &mut prj, 0, vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    frame(&mut ed, &ctx, &mut prj, 0, vec![button(to, false)]);
    let s = &prj.pages[0].symbols[0];
    assert_eq!(s.pos, p(100.0, 100.0), "the symbol stays");
    let o = s.tag_offset.expect("tag placed");
    let expect = block.min - s.pos + p(-20.0, 10.0);
    assert!(o.dist(expect) < 0.6, "{o:?} {expect:?}");
}

fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::default() }
}

/// Drags with the primary button from page point `a` to `b`.
fn drag(ed: &mut Editor, ctx: &egui::Context, prj: &mut Project, page: usize, a: P, b: P) {
    let (from, to) = (ed.w2s(SCREEN, a), ed.w2s(SCREEN, b));
    frame(ed, ctx, prj, page, vec![Event::PointerMoved(from)]);
    frame(ed, ctx, prj, page, vec![button(from, true)]);
    for i in 1..=5 {
        let t = i as f32 / 5.0;
        frame(ed, ctx, prj, page, vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    frame(ed, ctx, prj, page, vec![button(to, false)]);
    frame(ed, ctx, prj, page, vec![]);
}

fn coil(id: u64, pos: P, tag: &str) -> SymbolInst {
    SymbolInst { id, sym: "coil".into(), pos, tag: tag.into(), pins: vec!["A1".into(), "A2".into()], ..Default::default() }
}

#[test]
fn window_from_the_right_selects_what_it_crosses() {
    let window = R::from_points(p(10.0, 10.0), p(50.0, 50.0));
    let inside = R::from_points(p(20.0, 20.0), p(30.0, 30.0));
    let across = R::from_points(p(40.0, 20.0), p(90.0, 20.0));
    let outside = R::from_points(p(60.0, 20.0), p(90.0, 30.0));
    assert!(box_selects(window, false, inside) && box_selects(window, true, inside));
    assert!(!box_selects(window, false, across) && box_selects(window, true, across));
    assert!(!box_selects(window, false, outside) && !box_selects(window, true, outside));

    // On the page: a long wire is caught by a crossing window over its middle.
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(40.0, 60.0), b: p(200.0, 60.0), ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    drag(&mut ed, &ctx, &mut prj, 0, p(100.0, 40.0), p(120.0, 80.0));
    assert!(ed.selection.is_empty(), "left to right needs the whole wire");
    drag(&mut ed, &ctx, &mut prj, 0, p(120.0, 40.0), p(100.0, 80.0));
    assert_eq!(ed.selection, BTreeSet::from([5]));
}

#[test]
fn hints_name_the_device_and_the_users_keys() {
    let mut ed = Editor::default();
    ed.set_tool(Tool::Place("coil"));
    assert!(ed.hint().contains("R rotate"), "{}", ed.hint());
    ed.place_tag = Some("-K7".into());
    assert!(ed.hint().contains("-K7"), "{}", ed.hint());
    ed.shortcuts.set("rotate", vec![crate::shortcuts::Shortcut::parse("Ctrl+R").unwrap()]);
    assert!(ed.hint().contains("Ctrl+R rotate"), "{}", ed.hint());
    ed.set_tool(Tool::Wire);
    assert!(ed.hint().contains("Space") && ed.hint().contains("Enter"), "{}", ed.hint());
}

#[test]
fn the_device_and_part_of_a_placement_end_with_the_place_tool() {
    let mut ed = Editor::default();
    ed.set_tool(Tool::Place("coil"));
    ed.place_tag = Some("-K1".into());
    ed.place_part = Some("3RT2016".into());
    // Another symbol keeps the device, but not the part.
    ed.set_tool(Tool::Place("no"));
    assert_eq!(ed.place_tag.as_deref(), Some("-K1"));
    assert_eq!(ed.place_part, None);
    ed.set_tool(Tool::Select);
    assert_eq!(ed.place_tag, None);

    // Esc on the page ends the placement too.
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Place("coil"));
    ed.place_tag = Some("-K1".into());
    frame(&mut ed, &ctx, &mut prj, 0, vec![key(Key::Escape)]);
    assert_eq!((ed.tool.clone(), ed.place_tag.clone()), (Tool::Select, None));
}

#[test]
fn read_only_pages_can_be_selected_but_not_changed() {
    let mut prj = Project::default();
    prj.pages[0].symbols.push(coil(7, p(100.0, 100.0), "-K1"));
    prj.pages[0].locked = true;
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    click(&mut ed, &ctx, &mut prj, 0, p(100.0, 100.0));
    assert_eq!(ed.selection, BTreeSet::from([7]));
    drag(&mut ed, &ctx, &mut prj, 0, p(100.0, 100.0), p(140.0, 100.0));
    assert_eq!(prj.pages[0].symbols[0].pos, p(100.0, 100.0), "no moves");
    frame(&mut ed, &ctx, &mut prj, 0, vec![key(Key::Delete)]);
    assert_eq!(prj.pages[0].symbols.len(), 1, "no deletes");
    // A typed point selects too.
    ed.selection.clear();
    ed.typed = Some(p(100.0, 100.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    assert_eq!(ed.selection, BTreeSet::from([7]));
}

#[test]
fn dragged_symbols_keep_their_wires_square() {
    let mut prj = Project::default();
    prj.pages[0].symbols.push(coil(7, p(100.0, 100.0), "-K1"));
    let top = prj.symbol("coil").unwrap().pin_world(0, p(100.0, 100.0), 0, false);
    prj.pages[0].wires.push(Wire { id: 8, a: p(20.0, 40.0), b: p(300.0, 40.0), ..Default::default() });
    prj.pages[0].wires.push(Wire { id: 9, a: p(100.0, 40.0), b: top, ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    drag(&mut ed, &ctx, &mut prj, 0, p(100.0, 100.0), p(140.0, 108.0));
    assert_eq!(prj.pages[0].symbols[0].pos, p(140.0, 108.0));
    let w = prj.pages[0].wires.iter().find(|w| w.id == 9).unwrap();
    assert_eq!((w.a, w.b), (p(140.0, 40.0), top + p(40.0, 8.0)), "still vertical, still on the rail");
    assert_eq!(prj.pages[0].wires.len(), 2);
}

#[test]
fn dragging_one_of_several_selected_symbols_moves_them_all() {
    let mut prj = Project::default();
    prj.pages[0].symbols.push(coil(7, p(100.0, 100.0), "-K1"));
    prj.pages[0].symbols.push(coil(8, p(200.0, 100.0), "-K2"));
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.selection = BTreeSet::from([7, 8]);
    let def = prj.symbol("coil").unwrap();
    let block = render::tag_block_bounds(&prj, 0, &def, &prj.pages[0].symbols[0]).unwrap();
    drag(&mut ed, &ctx, &mut prj, 0, block.center(), block.center() + p(0.0, 20.0));
    let s = &prj.pages[0].symbols;
    assert_eq!((s[0].pos, s[1].pos), (p(100.0, 120.0), p(200.0, 120.0)));
    assert_eq!(s[0].tag_offset, None);
}

#[test]
fn enter_ends_the_wire() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Wire);
    click(&mut ed, &ctx, &mut prj, 0, p(40.0, 40.0));
    click(&mut ed, &ctx, &mut prj, 0, p(80.0, 40.0));
    assert!(ed.busy(), "the wire goes on from the corner");
    frame(&mut ed, &ctx, &mut prj, 0, vec![key(Key::Enter)]);
    assert!(!ed.busy());
    assert_eq!(ed.tool, Tool::Wire);
    assert_eq!(prj.pages[0].wires.len(), 1);
}

#[test]
fn a_second_click_on_the_start_of_a_markup_line_keeps_it() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Markup(MarkupKind::Line, Pen::Red));
    let mut clicks = Vec::new();
    for q in [p(40.0, 40.0), p(40.0, 40.0), p(80.0, 40.0)] {
        let at = ed.w2s(SCREEN, q);
        frame(&mut ed, &ctx, &mut prj, 0, vec![Event::PointerMoved(at)]);
        frame(&mut ed, &ctx, &mut prj, 0, vec![button(at, true)]);
        clicks.push(frame(&mut ed, &ctx, &mut prj, 0, vec![button(at, false)]).review);
        frame(&mut ed, &ctx, &mut prj, 0, vec![Event::PointerMoved(at + vec2(0.0, 300.0))]);
    }
    assert_eq!(clicks[..2], [None, None]);
    match clicks[2] {
        Some(ReviewInput::Markup(MarkupKind::Line, Pen::Red, a, b)) => assert!(a.dist(p(40.0, 40.0)) < 0.5 && b.dist(p(80.0, 40.0)) < 0.5, "{a:?} {b:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn typed_points_place_connection_definitions() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(40.0, 60.0), b: p(200.0, 60.0), ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::ConnDef);
    ed.typed = Some(p(100.0, 60.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    assert_eq!(prj.pages[0].conn_defs.len(), 1);
    assert_eq!(prj.pages[0].conn_defs[0].pos, p(100.0, 60.0));
    assert_eq!(ed.tool, Tool::ConnDef, "the tool stays for the next point");
}

#[test]
fn texts_are_placed_one_after_the_other() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Text);
    ed.text_proto = Some(TextItem { text: "{page}".into(), ..Default::default() });
    click(&mut ed, &ctx, &mut prj, 0, p(40.0, 40.0));
    click(&mut ed, &ctx, &mut prj, 0, p(80.0, 40.0));
    assert_eq!(ed.tool, Tool::Text);
    assert_eq!(prj.pages[0].texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["{page}", "{page}"]);
    assert_eq!(ed.selection, BTreeSet::from([prj.pages[0].texts[1].id]));
}

#[test]
fn paste_goes_to_the_cursor_then_to_the_last_click() {
    let mut ed = Editor::default();
    let corner = ed.paste_point();
    ed.last_point = Some(p(40.0, 40.0));
    assert_eq!(ed.paste_point(), p(40.0, 40.0));
    ed.hover = Some(p(101.0, 99.0));
    assert_eq!(ed.paste_point(), p(100.0, 100.0), "snapped to the grid");
    ed.last_point = None;
    ed.hover = None;
    assert_eq!(ed.paste_point(), corner);
}

#[test]
fn new_items_never_go_onto_a_hidden_or_locked_layer() {
    let mut prj = Project::default();
    let mut hidden = openecraft_core::graphics::Layer::new("Hidden");
    hidden.visible = false;
    prj.layers.push(hidden);
    prj.layers.push(openecraft_core::graphics::Layer::new("Notes"));
    assert!(!layer_usable(&prj, "Hidden"));
    assert!(layer_usable(&prj, "Notes") && layer_usable(&prj, ""));
    let ctx = egui::Context::default();
    let mut ed = Editor { active_layer: "Hidden".into(), ..Default::default() };
    ready(&mut ed, &ctx, &mut prj);
    assert_eq!(ed.active_layer, "");
    ed.set_tool(Tool::Line);
    click(&mut ed, &ctx, &mut prj, 0, p(40.0, 40.0));
    click(&mut ed, &ctx, &mut prj, 0, p(80.0, 40.0));
    assert_eq!(prj.pages[0].shapes[0].layer, "");
}

#[test]
fn arrow_keys_connect_facing_points_like_a_drag() {
    let mut prj = Project::default();
    // A2 of -K1 at (100, 104) faces A1 of -K2 at (104, 136) once -K2 moves left.
    prj.pages[0].symbols.push(coil(7, p(100.0, 100.0), "-K1"));
    prj.pages[0].symbols.push(coil(8, p(104.0, 140.0), "-K2"));
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.selection = BTreeSet::from([8]);
    frame(&mut ed, &ctx, &mut prj, 0, vec![key(Key::ArrowLeft)]);
    assert_eq!(prj.pages[0].symbols[1].pos, p(100.0, 140.0));
    assert_eq!(prj.pages[0].wires.len(), 1, "connected");
}

#[test]
fn zoom_commands_keep_the_middle_of_the_canvas() {
    let mut ed = Editor { zoom: 2.0, ..Editor::default() };
    let rect = Rect::from_min_size(Pos2::ZERO, ed.canvas_size);
    let middle = ed.s2w(rect, rect.center());
    ed.zoom_center(1.25);
    assert!((ed.zoom - 2.5).abs() < 1e-4);
    let after = ed.s2w(rect, rect.center());
    assert!((after.x - middle.x).abs() < 1e-3 && (after.y - middle.y).abs() < 1e-3);
    ed.zoom_center(ZOOM_ACTUAL / ed.zoom);
    assert!((ed.zoom - ZOOM_ACTUAL).abs() < 1e-4, "Zoom 100 %");
}

#[test]
fn black_is_black_on_paper_and_the_line_colour_on_a_dark_canvas() {
    let black = Ink::Color([20, 20, 20]);
    let light = Palette::new(false);
    assert_eq!(light.ink(black), Color32::from_rgb(20, 20, 20), "a black wire is not drawn blue");
    let dark = Palette::new(true);
    assert_eq!(dark.ink(black), dark.line, "visible on a dark canvas");
    assert_eq!(dark.ink(Ink::Color([200, 0, 0])), Color32::from_rgb(200, 0, 0), "other colours stay");
}

#[test]
fn the_junction_tool_takes_its_direction_from_the_wires() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(40.0, 60.0), b: p(200.0, 60.0), ..Default::default() });
    prj.pages[0].wires.push(Wire { id: 6, a: p(100.0, 60.0), b: p(100.0, 20.0), ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Junction(JunctionKind::Tee));
    // Where the three wires meet the stem is found; elsewhere the tool's direction
    // (turned by R) is used.
    ed.typed = Some(p(100.0, 60.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    ed.symbol_proto.turn();
    ed.typed = Some(p(160.0, 100.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    let js = &prj.pages[0].junctions;
    assert_eq!(js.len(), 2);
    assert_eq!((js[0].pos, js[0].dir, js[0].variant), (p(100.0, 60.0), TeeDir::Up, 3));
    assert_eq!((js[1].pos, js[1].dir), (p(160.0, 100.0), TeeDir::Left));
    assert_eq!(ed.tool, Tool::Junction(JunctionKind::Tee), "the tool stays for the next junction");
    assert_eq!(ed.selection, BTreeSet::from([js[1].id]));
    // The same point takes no second junction.
    ed.typed = Some(p(100.0, 60.0));
    let out = frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    assert_eq!(prj.pages[0].junctions.len(), 2);
    assert!(out.message.is_some());
    // The select tool picks it.
    ed.set_tool(Tool::Select);
    assert_eq!(ed.hit(&prj, 0, p(100.4, 60.0)), Some(prj.pages[0].junctions[0].id));
}

#[test]
fn the_corner_tool_wires_facing_pins_in_with_autoconnect() {
    let mut prj = Project::default();
    prj.pages[0].symbols.push(SymbolInst { id: 1, sym: "no".into(), pos: p(60.0, 40.0), ..Default::default() });
    prj.pages[0].symbols.push(SymbolInst { id: 2, sym: "coil".into(), pos: p(120.0, 80.0), rot: 1, ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Junction(JunctionKind::Corner));
    // R turns the corner: down / right becomes down / left, then up / left.
    assert_eq!(ed.symbol_proto.arms(), [TeeDir::Down, TeeDir::Right]);
    frame(&mut ed, &ctx, &mut prj, 0, vec![Event::Key { key: Key::R, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::default() }]);
    frame(&mut ed, &ctx, &mut prj, 0, vec![Event::Key { key: Key::R, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::default() }]);
    assert_eq!(ed.symbol_proto.arms(), [TeeDir::Up, TeeDir::Left]);
    ed.symbol_proto = Junction::corner(P::default(), TeeDir::Up, TeeDir::Right);
    ed.typed = Some(p(60.0, 80.0));
    let out = frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    assert_eq!(prj.pages[0].junctions.len(), 1);
    assert!(prj.pages[0].junctions[0].is_corner());
    assert_eq!(prj.pages[0].wires.len(), 2, "both facing pins are wired to the corner");
    assert_eq!(out.message.as_deref(), Some("2 connections drawn automatically"));
    assert_eq!(ed.tool, Tool::Junction(JunctionKind::Corner));
}

#[test]
fn the_crossing_tool_connects_crossing_wires_and_turns_with_r() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 5, a: p(100.0, 40.0), b: p(100.0, 80.0), ..Default::default() });
    prj.pages[0].wires.push(Wire { id: 6, a: p(80.0, 60.0), b: p(120.0, 60.0), ..Default::default() });
    prj.fix_next_id();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::Junction(JunctionKind::Crossing));
    assert_eq!((ed.symbol_proto.kind, ed.symbol_proto.dir), (JunctionKind::Crossing, TeeDir::Up));
    frame(&mut ed, &ctx, &mut prj, 0, vec![Event::Key { key: Key::R, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::default() }]);
    assert_eq!(ed.symbol_proto.dir, TeeDir::Left, "R turns the line");
    ed.typed = Some(p(100.0, 60.0));
    frame(&mut ed, &ctx, &mut prj, 0, vec![]);
    let j = &prj.pages[0].junctions[0];
    assert_eq!((j.kind, j.dir, j.pos), (JunctionKind::Crossing, TeeDir::Left, p(100.0, 60.0)));
    let nl = openecraft_core::netlist::Netlist::build(&prj, true);
    assert_eq!(nl.wire_net[&5], nl.wire_net[&6], "the crossing connects the wires");
    // The bridge tool starts from its own prototype.
    ed.set_tool(Tool::Junction(JunctionKind::Bridge));
    assert_eq!((ed.symbol_proto.kind, ed.symbol_proto.dir), (JunctionKind::Bridge, TeeDir::Up));
}

#[test]
fn device_boxes_are_drawn_and_take_their_points_along() {
    let mut prj = Project::default();
    let ctx = egui::Context::default();
    let mut ed = Editor { autoconnect: false, ..Default::default() };
    ready(&mut ed, &ctx, &mut prj);
    ed.set_tool(Tool::DeviceBox);
    click(&mut ed, &ctx, &mut prj, 0, p(80.0, 80.0));
    click(&mut ed, &ctx, &mut prj, 0, p(120.0, 104.0));
    let b = prj.pages[0].symbols[0].clone();
    assert_eq!((b.sym.as_str(), b.tag.as_str(), b.pos, b.size), ("devbox", "-A1", p(80.0, 80.0), Some(p(40.0, 24.0))));
    assert_eq!(ed.selection, BTreeSet::from([b.id]));
    // Points on the top and the right edge lead away from them and are numbered on.
    ed.set_tool(Tool::Place(openecraft_core::symbols::DEVICE_POINT));
    assert!(ed.hint().contains("device box"), "{}", ed.hint());
    click(&mut ed, &ctx, &mut prj, 0, p(88.0, 80.0));
    click(&mut ed, &ctx, &mut prj, 0, p(120.0, 92.0));
    let points: Vec<(String, String, u8)> = prj.pages[0].symbols[1..].iter().map(|s| (s.tag.clone(), s.pins[0].clone(), s.rot)).collect();
    assert_eq!(points, vec![("-A1".into(), "1".into(), 0), ("-A1".into(), "2".into(), 1)]);
    // Inside the box nothing is hit; its outline picks the box.
    ed.set_tool(Tool::Select);
    assert_eq!(ed.hit(&prj, 0, p(100.0, 96.0)), None);
    assert_eq!(ed.hit(&prj, 0, p(100.0, 104.0)), Some(b.id));
    // Dragging the box moves its points with it.
    drag(&mut ed, &ctx, &mut prj, 0, p(100.0, 104.0), p(140.0, 120.0));
    let pg = &prj.pages[0];
    assert_eq!(pg.symbols[0].pos, p(120.0, 96.0));
    assert_eq!((pg.symbols[1].pos, pg.symbols[2].pos), (p(128.0, 96.0), p(160.0, 108.0)));
    // Delete takes them too.
    frame(&mut ed, &ctx, &mut prj, 0, vec![key(Key::Delete)]);
    assert!(prj.pages[0].symbols.is_empty());
}

#[test]
fn the_designation_of_a_device_connection_point_is_dragged_into_place() {
    let mut prj = Project::default();
    let b = openecraft_core::device_boxes::add_box(&mut prj, 0, p(80.0, 80.0), p(120.0, 104.0), "", "").unwrap();
    let id = openecraft_core::device_boxes::place_point(&mut prj, 0, p(88.0, 80.0), 0, "", "PE").unwrap();
    let ctx = egui::Context::default();
    let mut ed = Editor::default();
    ready(&mut ed, &ctx, &mut prj);
    ed.selection.insert(id);
    let s = prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap().clone();
    let def = prj.symbol(&s.sym).unwrap();
    let label = render::pin_label_bounds(&def, &s).unwrap();
    drag(&mut ed, &ctx, &mut prj, 0, label.center(), label.center() + p(6.0, 10.0));
    let s = prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap();
    assert_eq!(s.pos, p(88.0, 80.0), "the point stays");
    let l = s.pin_label.expect("designation placed");
    let (auto, anchor, _) = render::pin_label_at(&def, &SymbolInst { pin_label: None, ..s.clone() }, 0);
    assert!(l.offset.dist(auto - s.pos + p(6.0, 10.0)) < 0.6, "{l:?}");
    assert_eq!(l.anchor, anchor);
    assert_eq!(openecraft_core::device_boxes::box_of(&prj, &prj.pages[0], s), Some(b));
}
