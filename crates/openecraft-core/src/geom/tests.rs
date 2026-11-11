use super::*;

#[test]
fn rotate_quarter_turns() {
    let top = p(0.0, -4.0);
    assert!(top.transform(1, false).same(p(4.0, 0.0)));
    assert!(top.transform(2, false).same(p(0.0, 4.0)));
    assert!(top.transform(3, false).same(p(-4.0, 0.0)));
    assert!(top.transform(4, false).same(top));
    assert!(p(2.0, 1.0).transform(0, true).same(p(-2.0, 1.0)));
}

#[test]
fn segment_interior() {
    assert!(on_segment_interior(p(4.0, 0.0), p(0.0, 0.0), p(8.0, 0.0)));
    assert!(!on_segment_interior(p(8.0, 0.0), p(0.0, 0.0), p(8.0, 0.0)));
    assert!(!on_segment_interior(p(4.0, 1.0), p(0.0, 0.0), p(8.0, 0.0)));
}

#[test]
fn untransform_undoes_transform() {
    let q = p(2.0, -3.0);
    for rot in 0..4 {
        for mirror in [false, true] {
            assert!(q.transform(rot, mirror).untransform(rot, mirror).same(q), "rot {rot} mirror {mirror}");
        }
    }
}

#[test]
fn rectangles_overlap_and_are_crossed_by_segments() {
    let r = R::from_points(p(0.0, 0.0), p(2.0, 1.0));
    assert!(r.overlaps(&R::from_points(p(1.0, 0.5), p(3.0, 3.0))));
    assert!(!r.overlaps(&R::from_points(p(2.0, 0.0), p(3.0, 1.0))), "sharing an edge");
    assert!(r.crosses(p(-1.0, 0.5), p(3.0, 0.5)));
    assert!(r.crosses(p(1.0, -1.0), p(1.0, 0.2)), "ending inside");
    assert!(!r.crosses(p(-1.0, -1.0), p(-1.0, 3.0)));
    assert!(!r.crosses(p(3.0, -1.0), p(0.5, -0.1)));
}
