use super::*;
use crate::model::{PinPlacement, Wire};
use crate::testkit::Kit;

fn places(kit: &Kit, id: u64) -> Vec<PinLabelPlace> {
    let s = kit.prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap();
    let def = kit.prj.symbol(&s.sym).unwrap();
    pin_label_places(&kit.prj, &Index::build(&kit.prj), 0, &def, s)
}

fn sym(kit: &mut Kit, id: u64) -> &mut SymbolInst {
    kit.prj.pages[0].symbols.iter_mut().find(|s| s.id == id).unwrap()
}

/// Do the lines of the symbol run through the text of a designation?
fn crosses_symbol(kit: &Kit, id: u64, r: &R) -> bool {
    let s = kit.prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap();
    let def = kit.prj.symbol(&s.sym).unwrap();
    let mut o = Obstacles::default();
    let w = |q: P| s.pos + q.transform(s.rot, s.mirror);
    for prim in &def.prims {
        match prim {
            Prim::Line { a, b, .. } => o.segments.push((w(*a), w(*b))),
            Prim::Poly { pts, closed, .. } => {
                o.segments.extend(pts.windows(2).map(|ab| (w(ab[0]), w(ab[1]))));
                if *closed {
                    o.segments.push((w(pts[pts.len() - 1]), w(pts[0])));
                }
            }
            _ => {}
        }
    }
    o.hits(r) > 0
}

#[test]
fn surge_protective_device_designations_keep_clear_of_its_arrow() {
    let mut kit = Kit::new();
    let f = kit.sym("spd", 100.0, 100.0, "-F1", "");
    sym(&mut kit, f).pins = vec!["L1".into(), "PE".into()];
    let l = places(&kit, f);
    assert_eq!(l.len(), 2);
    for x in &l {
        assert!(!crosses_symbol(&kit, f, &x.bounds), "{:?} crosses the symbol", x);
    }
    // PE goes to the right of the lead, away from the arrow at the lower left.
    let pe = &l[1];
    assert_eq!(pe.side, LabelSide::Right);
    assert!(pe.bounds.min.x > 100.0);
}

#[test]
fn designations_with_room_keep_their_usual_place() {
    let mut kit = Kit::new();
    let k = kit.sym("coil", 100.0, 100.0, "-K1", "");
    let s = kit.prj.pages[0].symbols.iter().find(|s| s.id == k).unwrap().clone();
    let def = kit.prj.symbol("coil").unwrap();
    for l in places(&kit, k) {
        let q = def.pin_world(l.pin, s.pos, s.rot, s.mirror);
        assert_eq!((l.side, l.shift), (LabelSide::Left, P::default()));
        assert_eq!((l.anchor, l.v), (Anchor::Right, VAnchor::Middle));
        assert!(l.pos.dist(q + p(-0.6, if l.pin == 0 { 1.1 } else { -1.1 })) < 1e-4);
    }
}

#[test]
fn wires_and_other_designations_push_a_designation_aside() {
    let mut kit = Kit::new();
    let f = kit.sym("spd", 100.0, 100.0, "-F1", "");
    let before = places(&kit, f)[1].clone();
    assert_eq!(before.side, LabelSide::Right);
    // A wire passing there takes that place.
    let id = kit.prj.new_id();
    kit.prj.pages[0].wires.push(Wire { id, a: p(102.0, 103.0), b: p(102.0, 120.0), ..Default::default() });
    let moved = places(&kit, f)[1].clone();
    assert_ne!(moved.bounds, before.bounds);
    assert!(!moved.bounds.expand(CLEARANCE).crosses(p(102.0, 103.0), p(102.0, 120.0)));
    // Two designations never take the same place.
    let l = places(&kit, f);
    assert!(!l[0].bounds.overlaps(&l[1].bounds));
}

#[test]
fn hand_placed_designations_turn_with_the_symbol() {
    let mut kit = Kit::new();
    let k = kit.sym("no", 100.0, 100.0, "-K1", "");
    sym(&mut kit, k).set_pin_placement(0, PinPlacement { side: Some(LabelSide::Right), offset: p(1.0, 0.0) });
    let l = places(&kit, k)[0].clone();
    assert_eq!((l.side, l.anchor), (LabelSide::Right, Anchor::Left));
    let auto = places(&kit, k)[1].clone();
    assert_eq!(auto.side, LabelSide::Left);
    let def = kit.prj.symbol("no").unwrap();
    let q = def.pin_world(0, p(100.0, 100.0), 0, false);
    assert!((l.pos.x - (q.x + 1.6)).abs() < 1e-4);

    // A quarter turn clockwise: the right side of a vertical lead becomes below a
    // horizontal one, the offset turns downwards.
    sym(&mut kit, k).rot = 1;
    let turned = places(&kit, k)[0].clone();
    assert_eq!(turned.side, LabelSide::Right, "stored in the symbol's own orientation");
    assert_eq!(turned.v, VAnchor::Top);
    let q = def.pin_world(0, p(100.0, 100.0), 1, false);
    assert!(turned.pos.y > q.y + 1.0);

    // Mirrored, right becomes left.
    let s = sym(&mut kit, k);
    s.rot = 0;
    s.mirror = true;
    let mirrored = places(&kit, k)[0].clone();
    assert_eq!(mirrored.anchor, Anchor::Right);
    let q = def.pin_world(0, p(100.0, 100.0), 0, true);
    assert!((mirrored.pos.x - (q.x - 1.6)).abs() < 1e-4);
}

#[test]
fn sides_convert_between_page_and_symbol() {
    for side in LabelSide::ALL {
        for rot in 0..4 {
            for mirror in [false, true] {
                assert_eq!(side.on_page(rot, mirror).in_symbol(rot, mirror), side);
            }
        }
    }
    assert_eq!(LabelSide::Left.on_page(1, false), LabelSide::Above);
    assert_eq!(LabelSide::Left.on_page(0, true), LabelSide::Right);
}

#[test]
fn trailing_automatic_designations_are_not_stored() {
    let mut s = SymbolInst::default();
    s.set_pin_placement(2, PinPlacement { side: Some(LabelSide::Above), offset: P::default() });
    assert_eq!(s.pin_placements.len(), 3);
    s.set_pin_placement(2, PinPlacement::default());
    assert!(s.pin_placements.is_empty());
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("pin_placements"), "older readers see nothing new");
}

#[test]
fn designations_of_built_in_symbols_keep_clear_of_their_graphics() {
    let kit = Kit::new();
    let idx = Index::build(&kit.prj);
    let mut crossing = Vec::new();
    for def in crate::symbols::library().iter().filter(|d| d.show_pin_labels) {
        let s = SymbolInst { id: 1, sym: def.key.into(), pos: p(100.0, 100.0), pins: (0..def.pins.len()).map(|i| (i + 11).to_string()).collect(), ..Default::default() };
        let o = Obstacles::around(&kit.prj, &idx, 0, def, &s);
        for l in pin_label_places(&kit.prj, &idx, 0, def, &s) {
            if o.hits(&l.bounds) > 0 {
                crossing.push(format!("{} {}", def.key, s.pins[l.pin]));
            }
        }
    }
    assert!(crossing.is_empty(), "{} designations cross their symbol:
{}", crossing.len(), crossing.join("
"));
}
