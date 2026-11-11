use super::*;

fn point(rot: u8, mirror: bool) -> SymbolInst {
    SymbolInst { id: 1, sym: "intr".into(), pos: p(100.0, 100.0), rot, mirror, label: "L1_OUT".into(), ..Default::default() }
}

fn overlap(a: &R, b: &R) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// Areas of the name and of a cross reference to two pages.
fn areas(def: &SymbolDef, s: &SymbolInst, (label, xref): (TextPlace, TextPlace)) -> (R, R) {
    (text_area(s.pos + label.offset, &s.label, label_size(def), label.anchor), text_area(s.pos + xref.offset, "/12.3 /14.1", SMALL_SIZE, xref.anchor))
}

#[test]
fn texts_keep_clear_of_the_arrow_the_conductor_and_each_other() {
    let prj = Project::default();
    let def = prj.symbol("intr").unwrap();
    for rot in 0..4 {
        for mirror in [false, true] {
            let s = point(rot, mirror);
            let arrow = def.world_bounds(s.pos, s.rot, s.mirror).expand(-0.5);
            let pin = def.pin_world(0, s.pos, s.rot, s.mirror);
            let back = pin - (arrow.center() - pin) * 10.0;
            let conductor = R::from_points(pin, back).expand(0.1);
            for side in TextSide::ALL {
                let (label, xref) = areas(&def, &s, side_places(&def, &s, side));
                let at = format!("rot {rot}, mirror {mirror}, {side:?}");
                assert!(!overlap(&label, &arrow), "name on the arrow: {at}");
                assert!(!overlap(&xref, &arrow), "cross reference on the arrow: {at}");
                assert!(!overlap(&label, &xref), "texts on each other: {at}");
                assert!(!overlap(&label, &conductor) && !overlap(&xref, &conductor), "texts on the conductor: {at}");
            }
        }
    }
}

#[test]
fn horizontal_points_show_name_and_cross_reference_along_the_conductor() {
    let prj = Project::default();
    let def = prj.symbol("intr").unwrap();
    // Three quarter turns: the arrow points to the right, the conductor comes from the left.
    let (label, xref) = interruption_places(&def, &point(3, false));
    assert_eq!((label.anchor, xref.anchor), (Anchor::Right, Anchor::Right));
    assert_eq!((label.offset.x, xref.offset.x), (0.0, 0.0), "ending at the connection point");
    assert!(label.offset.y < -2.0 && xref.offset.y > 1.0, "the name above, the cross reference below");
    // One quarter turn: to the left, starting at the connection point.
    let (label, xref) = interruption_places(&def, &point(1, false));
    assert_eq!((label.anchor, xref.anchor), (Anchor::Left, Anchor::Left));
    assert_eq!((label.offset.x, xref.offset.x), (0.0, 0.0));
    // Right of a right arrow: one line after the tip, for conductors 4 mm apart.
    let s = SymbolInst { text_side: TextSide::Right, ..point(3, false) };
    let (label, xref) = interruption_places(&def, &s);
    assert_eq!((label.offset.y, xref.offset.y), (0.0, 0.0), "on the axis of the conductor");
    assert!(label.offset.x > 2.7 && xref.offset.x > label.offset.x + name_width("L1_OUT", 2.5));
    // Vertical: the name beside the arrow, the cross reference below it.
    let (label, xref) = interruption_places(&def, &point(0, false));
    assert_eq!(label.anchor, Anchor::Left);
    assert!(label.offset.x > 1.5 && xref.offset.x == label.offset.x && xref.offset.y > label.offset.y);
}

#[test]
fn texts_placed_by_hand_win_until_a_side_is_chosen() {
    let prj = Project::default();
    let def = prj.symbol("intr").unwrap();
    let mut s = point(3, false);
    assert!(!s.texts_placed());
    let hand = TextPlace { offset: p(4.0, -6.0), anchor: Anchor::Center };
    s.xref_place = Some(hand);
    let (label, xref) = interruption_places(&def, &s);
    assert_eq!(xref, hand);
    assert_eq!(label, side_places(&def, &s, TextSide::Auto).0, "the name stays where it was");
    assert!(s.texts_placed());

    s.set_text_side(TextSide::Below);
    assert_eq!(s.xref_place, None);
    let (label, xref) = interruption_places(&def, &s);
    assert_eq!((label.anchor, xref.anchor), (Anchor::Right, Anchor::Left));
    assert!(label.offset.y > 2.0 && xref.offset.y == label.offset.y, "below the arrow in one line");
    s.set_text_side(TextSide::Auto);
    assert!(!s.texts_placed());
}

#[test]
fn placed_texts_are_saved() {
    let mut s = point(3, false);
    let plain = serde_json::to_value(&s).unwrap();
    assert!(plain.get("text_side").is_none() && plain.get("label_place").is_none(), "nothing stored for automatic placement");
    s.text_side = TextSide::Above;
    s.label_place = Some(TextPlace { offset: p(1.5, -4.0), anchor: Anchor::Left });
    s.xref_place = Some(TextPlace { offset: p(8.0, -4.0), anchor: Anchor::Right });
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["text_side"], "above");
    let back: SymbolInst = serde_json::from_value(json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn hit_areas_follow_the_texts() {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages.push(Page::new(id, "2", "", PageKind::Schematic));
    for page in [0, 1] {
        let id = prj.new_id();
        prj.pages[page].symbols.push(SymbolInst { id, ..point(3, false) });
    }
    let idx = Index::build(&prj);
    let def = prj.symbol("intr").unwrap();
    let s = &mut prj.pages[0].symbols[0];
    s.label_place = Some(TextPlace { offset: p(0.0, -10.0), anchor: Anchor::Center });
    let s = prj.pages[0].symbols[0].clone();
    let areas = interruption_text_bounds(&prj, &idx, 0, &def, &s);
    assert_eq!(areas.len(), 2);
    assert_eq!(areas[0].0, IntrText::Label);
    assert!(areas[0].1.contains(p(100.0, 90.0)));
    let (_, xref) = interruption_places(&def, &s);
    assert_eq!(areas[1].0, IntrText::Xref);
    assert!(areas[1].1.contains(s.pos + xref.offset));
}

#[test]
fn names_are_not_estimated_narrower_than_drawn() {
    // Widths of the SVG export (Arial at 1.25 × size), measured in a browser.
    for (text, size, drawn) in [("L1_OUT", 2.5, 11.82), ("PE_OUT", 2.5, 12.51), ("N_OUT", 2.5, 10.6), ("/2.2", SMALL_SIZE, 3.93), ("X9", 2.5, 4.02)] {
        assert!(name_width(text, size) >= drawn, "{text}: {} < {drawn}", name_width(text, size));
        assert!(name_width(text, size) < drawn * 1.3, "{text}: {} far wider than {drawn}", name_width(text, size));
    }
}

#[test]
fn parallel_points_keep_clear_of_their_neighbours() {
    let prj = Project::default();
    let def = prj.symbol("intr").unwrap();
    for (rot, pitch, sides) in [(3, 8.0, &TextSide::ALL[..]), (1, 8.0, &TextSide::ALL[..]), (3, 4.0, &[TextSide::Right][..]), (1, 4.0, &[TextSide::Left][..])] {
        for side in sides {
            // Five conductors one below the other, ending in points with the texts on `side`.
            let points: Vec<SymbolInst> = (0..5).map(|i| SymbolInst { text_side: *side, pos: p(100.0, 100.0 + pitch * i as f32), ..point(rot, false) }).collect();
            let mut taken: Vec<(usize, R)> = Vec::new();
            for (i, s) in points.iter().enumerate() {
                let arrow = def.world_bounds(s.pos, s.rot, s.mirror).expand(-0.5);
                let pin = def.pin_world(0, s.pos, s.rot, s.mirror);
                let conductor = R::from_points(pin, pin - (arrow.center() - pin) * 20.0).expand(0.1);
                taken.extend([(i, arrow), (i, conductor)]);
            }
            for (i, s) in points.iter().enumerate() {
                let (label, xref) = areas(&def, s, interruption_places(&def, s));
                for (j, r) in &taken {
                    assert!(*j == i || (!overlap(&label, r) && !overlap(&xref, r)), "rot {rot}, {pitch} mm, {side:?}: texts of point {i} on point {j}");
                }
                taken.extend([(i, label), (i, xref)]);
            }
            for (k, (i, a)) in taken.iter().enumerate() {
                for (j, b) in &taken[k + 1..] {
                    assert!(i == j || !overlap(a, b), "rot {rot}, {pitch} mm, {side:?}: point {i} and {j}");
                }
            }
        }
    }
}

#[test]
fn automatic_texts_of_horizontal_points_stay_behind_the_tip() {
    // Points at the edge of the page point at the frame: nothing may run past the tip.
    let prj = Project::default();
    let def = prj.symbol("intr").unwrap();
    for (rot, mirror) in [(1, false), (1, true), (3, false), (3, true)] {
        let s = point(rot, mirror);
        let tip = s.pos + p(0.0, 2.7).transform(rot, mirror);
        let (label, xref) = areas(&def, &s, interruption_places(&def, &s));
        for r in [label, xref] {
            if tip.x > s.pos.x {
                assert!(r.max.x <= tip.x, "rot {rot}: {r:?} past the tip {tip:?}");
            } else {
                assert!(r.min.x >= tip.x, "rot {rot}: {r:?} past the tip {tip:?}");
            }
        }
    }
}
