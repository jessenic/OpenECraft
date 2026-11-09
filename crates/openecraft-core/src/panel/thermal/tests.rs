use super::*;
use crate::model::DeviceData;
use crate::parts::PartRef;

fn device(id: u64, tag: &str, part: &str, x: f32, y: f32) -> PanelItem {
    PanelItem { id, kind: ItemKind::Device, tag: tag.into(), part: part.into(), x, y, w: 60.0, h: 100.0, d: 100.0, ..Default::default() }
}

/// A power supply of 40 W at the bottom and a PLC above it.
fn project() -> Project {
    let mut prj = Project::new_default();
    prj.parts.push(Part { number: "PS".into(), tech_data: "Power loss: 40 W".into(), ..Default::default() });
    prj.parts.push(Part { number: "PLC".into(), tech_data: "Operating temperature: 0 … +55 °C".into(), ..Default::default() });
    for (tag, part) in [("-T1", "PS"), ("-A1", "PLC")] {
        prj.devices.insert(tag.into(), DeviceData { parts: vec![PartRef { number: part.into(), count: 1, total:false }], ..Default::default() });
    }
    let items = vec![device(1, "-T1", "PS", 100.0, 500.0), device(2, "-A1", "PLC", 100.0, 300.0)];
    prj.layout.enclosures.push(Enclosure { items, ..Default::default() });
    prj
}

#[test]
fn limits_from_the_part_data() {
    let p = |t: &str| Part { tech_data: t.into(), ..Default::default() };
    assert_eq!(temperature_limit(&p("Operating temperature: -25 … +60 °C")), Some(60.0));
    assert_eq!(temperature_limit(&p("Max temperature: 45 °C")), Some(45.0));
    assert_eq!(temperature_limit(&p("Ambient temperature: -5...40 C")), Some(40.0));
    assert_eq!(temperature_limit(&p("Power loss: 3 W")), None);
}

#[test]
fn warm_air_rises_above_the_sources() {
    let prj = project();
    let e = &prj.layout.enclosures[0];
    let map = heat_map(&prj, e);
    assert_eq!(map.sources.len(), 1);
    assert_eq!((map.cols, map.rows), (55, 75));
    // Stratified: the top is warmer than the bottom.
    assert!(map.top > map.bottom);
    let above = map.at(p(130.0, 450.0)).unwrap();
    let below = map.at(p(130.0, 650.0)).unwrap();
    let beside = map.at(p(400.0, 450.0)).unwrap();
    assert!(above > below + 5.0, "{above} {below}");
    assert!(above > beside + 5.0, "{above} {beside}");
    assert!(map.max >= above && map.hottest.x > 100.0 && map.hottest.x < 160.0, "{:?}", map.hottest);
}

#[test]
fn devices_in_the_plume_get_too_warm() {
    let mut prj = project();
    let e = &prj.layout.enclosures[0];
    let map = heat_map(&prj, e);
    let t = device_temperatures(&prj, e, &map);
    let plc = t.iter().find(|t| t.tag == "-A1").unwrap();
    let ps = t.iter().find(|t| t.tag == "-T1").unwrap();
    assert_eq!((plc.limit, plc.loss, ps.loss), (Some(55.0), None, Some(40.0)));
    assert!(plc.temperature > ps.temperature, "the PLC sits in the warm air of the supply");
    // Hot ambient: the PLC above the supply exceeds its 55 °C.
    prj.layout.enclosures[0].climate.ambient_max = 45.0;
    prj.layout.enclosures[0].climate.inside_max = 50.0;
    let f = crate::panel::validate(&prj, &prj.layout.enclosures[0]);
    assert!(f.iter().any(|f| f.kind == crate::panel::CheckKind::Thermal && f.items == vec![2]), "{f:#?}");
    // Side by side it stays cool enough.
    prj.layout.enclosures[0].items[1].x = 400.0;
    let f = crate::panel::validate(&prj, &prj.layout.enclosures[0]);
    assert!(!f.iter().any(|f| f.kind == crate::panel::CheckKind::Thermal && f.items == vec![2]), "{f:#?}");
}

#[test]
fn a_duct_above_holds_the_plume_back() {
    let mut prj = project();
    let free = heat_map(&prj, &prj.layout.enclosures[0]);
    let duct = PanelItem { id: 3, kind: ItemKind::Duct, x: 0.0, y: 440.0, w: 500.0, h: 40.0, d: 60.0, ..Default::default() };
    prj.layout.enclosures[0].items.push(duct);
    let e = &prj.layout.enclosures[0];
    let held = heat_map(&prj, e);
    assert_eq!(held.sources[0].ceiling, Some(480.0));
    // Warmer between the supply and the duct, cooler above the duct.
    let (below, above) = (p(130.0, 482.0), p(130.0, 400.0));
    assert!(held.air(below, 0) > free.air(below, 0) + 0.5, "{} {}", held.air(below, 0), free.air(below, 0));
    assert!(held.air(above, 0) < free.air(above, 0) - 1.0, "{} {}", held.air(above, 0), free.air(above, 0));
    // The PLC above the duct gets less of it.
    let plc = |m: &HeatMap| device_temperatures(&prj, e, m).into_iter().find(|t| t.tag == "-A1").unwrap().temperature;
    assert!(plc(&held) < plc(&free));
}

#[test]
fn forced_air_weakens_the_plumes() {
    let mut prj = project();
    let still = heat_map(&prj, &prj.layout.enclosures[0]).sources[0].rise;
    prj.layout.enclosures[0].climate.extra_loss = 500.0;
    let forced = heat_map(&prj, &prj.layout.enclosures[0]).sources[0].rise;
    assert!((forced - still * 0.5).abs() < 1e-4, "{forced} {still}");
}
