use super::*;
use crate::geom::GRID;
use crate::model::{SymbolInst, Wire};
use crate::symbols::DEVICE_POINT;

/// A wire off the grid at x = 115.5 and a device connection point at (101.5, 40),
/// whose connection is 4 mm above it.
fn project() -> Project {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(115.5, 40.0), b: p(115.5, 80.0), ..Default::default() });
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst { id, sym: DEVICE_POINT.into(), pos: p(101.5, 40.0), pins: vec!["1".into()], ..Default::default() });
    prj
}

#[test]
fn off_grid_connection_points_and_wire_ends_make_lines() {
    let prj = project();
    let lines = SnapLines::of_page(&prj, &prj.pages[0], GRID);
    assert_eq!(lines.xs, [101.5, 115.5]);
    assert!(lines.ys.is_empty(), "40, 80 and 36 are on the grid: {:?}", lines.ys);
}

#[test]
fn a_line_wins_where_it_is_nearer_than_the_grid() {
    let prj = project();
    let lines = SnapLines::of_page(&prj, &prj.pages[0], GRID);
    // Drawing from the connection point and onto the wire.
    assert_eq!(lines.snap(p(101.2, 35.4), GRID), p(101.5, 36.0));
    assert_eq!(lines.snap(p(115.0, 61.0), GRID), p(115.5, 60.0));
    // Nearer to the grid, the grid stays.
    assert_eq!(lines.snap(p(116.1, 61.0), GRID), p(116.0, 60.0));
    assert_eq!(lines.snap(p(140.9, 61.0), GRID), p(140.0, 60.0));
}

#[test]
fn lines_follow_the_grid_size() {
    let mut prj = project();
    prj.pages[0].wires[0].a = p(116.0, 40.0);
    prj.pages[0].wires[0].b = p(116.0, 80.0);
    assert_eq!(SnapLines::of_page(&prj, &prj.pages[0], GRID).xs, [101.5]);
    assert_eq!(SnapLines::of_page(&prj, &prj.pages[0], 8.0).xs, [101.5, 116.0], "116 is off an 8 mm grid");
    assert!(SnapLines::of_page(&prj, &prj.pages[0], 0.5).xs.is_empty());
}

#[test]
fn a_symbol_snaps_from_its_connection_points() {
    let prj = project();
    let lines = SnapLines::of_page(&prj, &prj.pages[0], GRID);
    // Connection points 3 mm left and right of the insertion point: the right one
    // lands on the line at 101.5, which shifts the symbol less than the grid would.
    let pins = [p(-3.0, 0.0), p(3.0, 0.0)];
    assert_eq!(lines.snap_from(p(98.7, 61.0), &pins, GRID), p(98.5, 60.0));
    assert_eq!(lines.snap_from(p(112.4, 61.0), &pins, GRID), p(112.5, 60.0), "the wire at 115.5");
    // Far from any line, the grid.
    assert_eq!(lines.snap_from(p(130.9, 61.0), &pins, GRID), p(132.0, 60.0));
    assert_eq!(lines.snap_from(p(98.7, 61.0), &[], GRID), p(100.0, 60.0));
}

#[test]
fn a_moved_selection_snaps_to_what_stays() {
    let mut prj = project();
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(60.0, 20.0), b: p(80.0, 20.0), ..Default::default() });
    let sel = BTreeSet::from([prj.pages[0].symbols[0].id]);
    let page = &prj.pages[0];
    let points = points_of(&prj, page, &sel);
    assert_eq!(points, [p(101.5, 36.0)]);
    let lines = SnapLines::without(&prj, page, GRID, &points);
    assert_eq!(lines.xs, [115.5], "the point that moves is no line");
    // Moved 13.8 mm right, the point lands on the wire at 115.5.
    assert_eq!(lines.snap_from(p(13.8, 0.2), &points, GRID), p(14.0, 0.0));
    assert_eq!(lines.snap_from(p(14.2, 0.2), &points, GRID), p(14.0, 0.0));
    // Moved a wire: its ends snap like connection points.
    let sel = BTreeSet::from([id]);
    let points = points_of(&prj, page, &sel);
    let lines = SnapLines::without(&prj, page, GRID, &points);
    assert_eq!(lines.snap_from(p(35.3, 0.0), &points, GRID), p(35.5, 0.0));
}

#[test]
fn a_page_on_the_grid_snaps_as_before() {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(40.0, 40.0), b: p(80.0, 40.0), ..Default::default() });
    let lines = SnapLines::of_page(&prj, &prj.pages[0], GRID);
    assert_eq!(lines, SnapLines::default());
    assert_eq!(lines.snap(p(41.9, 38.1), GRID), p(41.9, 38.1).snap(GRID));
}
