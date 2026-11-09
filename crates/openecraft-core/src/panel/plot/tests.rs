use super::*;
use crate::panel::buildings as bld;

const SQUARE: [[f32; 2]; 4] = [[0.0, 0.0], [20_000.0, 0.0], [20_000.0, 20_000.0], [0.0, 20_000.0]];

/// A building with a room of 4 × 3 m at the origin on its ground level.
fn with_room() -> (Project, usize) {
    let mut prj = Project::default();
    let bi = bld::add_building(&mut prj, "+B1").unwrap();
    prj.layout.buildings[bi].site = crate::panel::SitePlace { x: 1000.0, z: 1000.0, angle: 0.0 };
    let l0 = prj.layout.buildings[bi].levels[0].id;
    let ri = bld::add_room(&mut prj, "+ROOM1", Some(l0)).unwrap();
    (prj, ri)
}

#[test]
fn the_boundary_is_a_simple_polygon() {
    let mut prj = Project::default();
    set_outline(&mut prj, SQUARE.to_vec()).unwrap();
    assert_eq!((prj.layout.plot.outline.len(), area(&SQUARE), perimeter(&SQUARE)), (4, 400.0, 80.0));
    // Repeated corners, the closing corner and corners on an edge go.
    let mut pts = SQUARE.to_vec();
    pts.insert(1, [10_000.0, 0.0]);
    pts.insert(1, [10_000.0, 0.0]);
    pts.push([0.0, 0.0]);
    assert_eq!(cleaned(pts), SQUARE.to_vec());
    assert!(set_outline(&mut prj, vec![[0.0, 0.0], [1000.0, 0.0]]).is_err(), "two corners");
    assert!(set_outline(&mut prj, vec![[0.0, 0.0], [10_000.0, 10_000.0], [10_000.0, 0.0], [0.0, 10_000.0]]).is_err(), "a bow tie crosses itself");
    assert_eq!(prj.layout.plot.outline.len(), 4, "a refused boundary leaves the old one");
    set_outline(&mut prj, Vec::new()).unwrap();
    assert!(prj.layout.plot.is_default());
}

#[test]
fn polygons_inside_the_boundary() {
    // An L-shaped plot: the notch at the front right is outside.
    let l = [[0.0, 0.0], [20_000.0, 0.0], [20_000.0, 10_000.0], [10_000.0, 10_000.0], [10_000.0, 20_000.0], [0.0, 20_000.0]];
    let rect = |x: f32, z: f32, w: f32, d: f32| vec![[x, z], [x + w, z], [x + w, z + d], [x, z + d]];
    assert!(contains(&l, &rect(1000.0, 1000.0, 5000.0, 5000.0)));
    assert!(contains(&l, &rect(0.0, 0.0, 5000.0, 5000.0)), "touching the boundary from inside");
    assert!(!contains(&l, &rect(12_000.0, 12_000.0, 2000.0, 2000.0)), "in the notch");
    assert!(!contains(&l, &rect(8000.0, 8000.0, 4000.0, 4000.0)), "across the corner of the notch");
    assert!(!contains(&l, &rect(-500.0, 1000.0, 2000.0, 2000.0)));
    // Grown outwards by the walls, whichever way round.
    let g = grown(&rect(0.0, 0.0, 4000.0, 3000.0), 240.0);
    assert_eq!(g[0], [-240.0, -240.0]);
    let mut back = rect(0.0, 0.0, 4000.0, 3000.0);
    back.reverse();
    assert_eq!(grown(&back, 240.0)[0], [-240.0, 3240.0]);
}

#[test]
fn rooms_roofs_and_cabinets_outside_the_boundary_are_problems() {
    let (mut prj, ri) = with_room();
    assert!(problems(&prj).is_empty(), "no boundary, nothing outside");
    set_outline(&mut prj, SQUARE.to_vec()).unwrap();
    assert!(outside(&prj).is_empty(), "{:?}", problems(&prj));
    // The building moved over the edge: its room with its outer walls.
    prj.layout.buildings[0].site.x = 16_100.0;
    assert_eq!(outside(&prj), vec![Standing::Room(ri)]);
    let p = problems(&prj);
    assert_eq!((p.len(), p[0].severity), (1, Severity::Error));
    assert!(p[0].message().contains("+ROOM1"), "{}", p[0].message());
    prj.layout.buildings[0].site.x = 1000.0;
    // A roof reaching over the boundary with its overhang.
    prj.layout.buildings[0].roofs.push(crate::model::panel::Roof { name: "R1".into(), x: -900.0, width: 4000.0, depth: 3000.0, ..Default::default() });
    assert_eq!(outside(&prj), vec![Standing::Roof(0, 0)]);
    prj.layout.buildings[0].roofs.clear();
    // A cabinet outside the rooms.
    prj.layout.enclosures.push(crate::panel::Enclosure { name: "+CAB1".into(), width: 600.0, depth: 300.0, site: Some(crate::panel::SitePlace { x: 30_000.0, z: 0.0, angle: 0.0 }), ..Default::default() });
    assert_eq!(outside(&prj), vec![Standing::Cabinet(1)]);
    // Through the check of the buildings.
    assert!(bld::problems(&prj).iter().any(|i| i.message().contains("+CAB1")));
}

#[test]
fn points_of_the_plant_have_geographic_coordinates() {
    let mut prj = Project::default();
    let r = GeoReference { x: 1000.0, z: 2000.0, easting: 32_512_345.678, northing: 5_812_345.678, height: 54.3, crs: "EPSG:25832".into(), ..Default::default() };
    set_reference(&mut prj, Some(r.clone())).unwrap();
    // North towards the rear (-z), east towards +x.
    let g = to_geo(&r, [1000.0 + 10_000.0, 1500.0, 2000.0 - 5000.0]);
    assert!((g[0] - 32_512_355.678).abs() < 1e-6 && (g[1] - 5_812_350.678).abs() < 1e-6 && (g[2] - 55.8).abs() < 1e-9, "{g:?}");
    // Grid north turned towards -x: the plant's +x points south.
    let turned = GeoReference { north: 90.0, ..r.clone() };
    let g = to_geo(&turned, [11_000.0, 0.0, 2000.0]);
    assert!((g[0] - r.easting).abs() < 1e-6 && (g[1] - (r.northing - 10.0)).abs() < 1e-6, "{g:?}");
    for q in [[11_000.0, 0.0, 2000.0], [-3000.0, 2500.0, 7000.0]] {
        let back = from_geo(&turned, to_geo(&turned, q));
        assert!((0..3).all(|k| (back[k] - q[k]).abs() < 0.01), "{back:?} {q:?}");
    }
    // Measured: distance, east and north, the bearing from north.
    let m = measure(&prj.layout, [0.0, 0.0], [3000.0, -4000.0]);
    assert_eq!(m.distance, 5000.0);
    assert!((m.east - 3.0).abs() < 1e-9 && (m.north - 4.0).abs() < 1e-9 && (m.bearing - 36.869_897_645_844).abs() < 1e-6, "{m:?}");
    // Kept in 0 – 360°, coordinates must be numbers.
    set_reference(&mut prj, Some(GeoReference { north: -30.0, ..r.clone() })).unwrap();
    assert_eq!(prj.layout.plot.reference.as_ref().map(|r| r.north), Some(330.0));
    assert!(set_reference(&mut prj, Some(GeoReference { easting: f64::NAN, ..r.clone() })).is_err());
    assert!(set_reference(&mut prj, Some(GeoReference { latitude: Some(95.0), ..r })).is_err());
    set_reference(&mut prj, None).unwrap();
    assert!(prj.layout.plot.reference.is_none());
}

#[test]
fn the_plot_is_saved_with_the_project() {
    let mut prj = Project::default();
    set_outline(&mut prj, SQUARE.to_vec()).unwrap();
    set_reference(&mut prj, Some(GeoReference { name: "P1".into(), easting: 500_000.25, northing: 5_000_000.5, ..Default::default() })).unwrap();
    let json = serde_json::to_string(&prj.layout).unwrap();
    let back: Layout = serde_json::from_str(&json).unwrap();
    assert_eq!(back.plot, prj.layout.plot);
    assert!(!serde_json::to_string(&Layout::default()).unwrap().contains("plot"), "nothing written without a plot");
}
