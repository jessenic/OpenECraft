use super::*;
use openecraft_core::analysis::Analysis;
use openecraft_core::sample::demo_project;
use egui::{Event, Modifiers, RawInput};

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
