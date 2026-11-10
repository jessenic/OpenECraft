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
fn a_page_on_the_grid_snaps_as_before() {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(40.0, 40.0), b: p(80.0, 40.0), ..Default::default() });
    let lines = SnapLines::of_page(&prj, &prj.pages[0], GRID);
    assert_eq!(lines, SnapLines::default());
    assert_eq!(lines.snap(p(41.9, 38.1), GRID), p(41.9, 38.1).snap(GRID));
}
