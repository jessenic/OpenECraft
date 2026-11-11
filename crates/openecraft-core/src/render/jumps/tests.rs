use super::*;
use crate::analysis::Analysis;
use crate::model::Project;
use crate::render::{draw_page, Draw, Ink, Opts, VAnchor};
use crate::symbols::Anchor;

fn wire(id: u64, a: P, b: P) -> Wire {
    Wire { id, a, b, ..Default::default() }
}

/// A horizontal wire crossed by a vertical one at (40, 40).
fn cross() -> Page {
    let mut page = Project::default().pages.remove(0);
    page.wires = vec![wire(1, p(20.0, 40.0), p(60.0, 40.0)), wire(2, p(40.0, 20.0), p(40.0, 60.0))];
    page
}

#[test]
fn wires_crossing_without_connection_are_found() {
    assert_eq!(crossings(&cross(), &[]), vec![(p(40.0, 40.0), 1, 2)]);
    // A wire ending at the crossing connects all of them.
    let mut joined = cross();
    joined.wires.push(wire(3, p(40.0, 40.0), p(40.0, 80.0)));
    joined.wires[1].b = p(40.0, 40.0);
    assert!(crossings(&joined, &[]).is_empty());
    // A T-connection is no crossing; neither is a connected point.
    let mut tee = cross();
    tee.wires[1].b = p(40.0, 40.0);
    assert!(crossings(&tee, &[]).is_empty());
    assert!(crossings(&cross(), &[p(40.0, 40.0)]).is_empty());
}

#[test]
fn the_chosen_wire_jumps() {
    let page = cross();
    assert!(jumps(&page, &[], JumpStyle::None, JumpWire::Vertical).is_empty());
    assert_eq!(jumps(&page, &[], JumpStyle::Arc, JumpWire::Vertical).into_iter().collect::<Vec<_>>(), vec![(2, vec![p(40.0, 40.0)])]);
    assert_eq!(jumps(&page, &[], JumpStyle::Gap, JumpWire::Horizontal).into_iter().collect::<Vec<_>>(), vec![(1, vec![p(40.0, 40.0)])]);
}

#[test]
fn strokes_jump_with_an_arc_a_bridge_or_a_gap() {
    let w = wire(1, p(20.0, 40.0), p(60.0, 40.0));
    let at = [p(40.0, 40.0)];
    assert_eq!(strokes(&w, &at, JumpStyle::None), vec![vec![w.a, w.b]]);
    assert_eq!(strokes(&w, &[], JumpStyle::Arc), vec![vec![w.a, w.b]]);

    let arc = strokes(&w, &at, JumpStyle::Arc);
    assert_eq!(arc.len(), 1);
    let line = &arc[0];
    assert!(line[0].same(w.a) && line[line.len() - 1].same(w.b));
    // The top of the arc is above the crossing, one jump radius high.
    let top = line.iter().map(|q| q.y).fold(f32::MAX, f32::min);
    assert!((top - (40.0 - JUMP_R)).abs() < 0.05, "{top}");
    assert!(line.iter().all(|q| q.x >= 20.0 && q.x <= 60.0 && q.y <= 40.0));

    let bridge = strokes(&w, &at, JumpStyle::Bridge);
    assert_eq!(bridge, vec![vec![w.a, p(39.0, 40.0), p(39.0, 39.0), p(41.0, 39.0), p(41.0, 40.0), w.b]]);

    let gap = strokes(&w, &at, JumpStyle::Gap);
    assert_eq!(gap, vec![vec![w.a, p(39.0, 40.0)], vec![p(41.0, 40.0), w.b]]);

    // A vertical wire jumps to the right.
    let v = wire(2, p(40.0, 20.0), p(40.0, 60.0));
    let right = strokes(&v, &at, JumpStyle::Bridge);
    assert_eq!(right, vec![vec![v.a, p(40.0, 39.0), p(41.0, 39.0), p(41.0, 41.0), p(40.0, 41.0), v.b]]);
}

#[test]
fn close_jumps_merge_into_one() {
    let w = wire(1, p(20.0, 40.0), p(60.0, 40.0));
    let gap = strokes(&w, &[p(41.5, 40.0), p(40.0, 40.0)], JumpStyle::Gap);
    assert_eq!(gap, vec![vec![w.a, p(39.0, 40.0)], vec![p(42.5, 40.0), w.b]]);
}

#[test]
fn tee_dots_follow_the_style_and_four_way_connections_keep_theirs() {
    let mut tee = cross();
    tee.wires[1].b = p(40.0, 40.0);
    let q = p(40.0, 40.0);
    assert_eq!(dot_radius(&tee, q, TeeStyle::Dot), Some(DOT_R));
    assert_eq!(dot_radius(&tee, q, TeeStyle::LargeDot), Some(LARGE_DOT_R));
    assert_eq!(dot_radius(&tee, q, TeeStyle::None), None);
    let mut four = cross();
    four.wires = vec![wire(1, p(20.0, 40.0), p(40.0, 40.0)), wire(2, p(40.0, 40.0), p(60.0, 40.0)), wire(3, p(40.0, 20.0), p(40.0, 60.0))];
    assert_eq!(dot_radius(&four, q, TeeStyle::None), Some(DOT_R));
}

/// Wire strokes and filled dots drawn.
#[derive(Default)]
struct Wires {
    lines: Vec<(P, P)>,
    dots: Vec<(P, f32)>,
}

impl Draw for Wires {
    fn line(&mut self, a: P, b: P, _: f32, ink: Ink, _: bool) {
        if ink == Ink::Wire {
            self.lines.push((a, b));
        }
    }
    fn poly(&mut self, _: &[P], _: bool, _: f32, _: Ink, _: bool) {}
    fn circle(&mut self, c: P, r: f32, _: f32, ink: Ink, fill: bool) {
        if ink == Ink::Wire && fill {
            self.dots.push((c, r));
        }
    }
    fn text(&mut self, _: P, _: &str, _: f32, _: Anchor, _: VAnchor, _: Ink) {}
}

fn drawn(prj: &Project) -> Wires {
    let an = Analysis::with_builtin(prj);
    let mut d = Wires::default();
    draw_page(&mut d, prj, &an.index, &an, 0, &Opts::default());
    d
}

#[test]
fn pages_draw_jumps_and_dots_in_the_style_of_the_project() {
    let mut prj = Project::default();
    prj.pages[0].wires = cross().wires;
    // A T-connection beside the crossing.
    prj.pages[0].wires.push(wire(3, p(50.0, 40.0), p(50.0, 60.0)));
    let plain = drawn(&prj);
    assert_eq!(plain.lines.len(), 3);
    assert_eq!(plain.dots, vec![(p(50.0, 40.0), DOT_R)]);

    prj.wire_settings.jump = JumpStyle::Arc;
    prj.wire_settings.tee = TeeStyle::None;
    let arcs = drawn(&prj);
    assert_eq!(arcs.lines.len(), 2 + 2 + ARC_STEPS);
    assert!(arcs.lines.iter().any(|(a, _)| a.x > 40.5 && (a.y - 40.0).abs() < 0.9));
    assert!(arcs.dots.is_empty());
    // The setting is kept in the project file.
    let back = Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(back.wire_settings, prj.wire_settings);
}
