use super::*;
use crate::model::panel::RoofShape;
use crate::panel::buildings::add_building;

/// A building with a gable roof and a PV array of `columns` × `rows` generic 430 W
/// modules, and an inverter =PV-T1 (2 trackers, 1000 V, MPP range 160 - 950 V, 16 A /
/// 20 A per tracker).
fn plant(columns: u32, rows: u32) -> Project {
    let mut prj = Project::default();
    let bi = add_building(&mut prj, "+B1").unwrap();
    let a = PvArray { name: "PV1".into(), x: 5000.0, z: 6000.0, columns, rows, ..Default::default() };
    prj.layout.buildings[bi].roofs.push(Roof { name: "R1".into(), shape: RoofShape::Gable, width: 30_000.0, depth: 30_000.0, pitch: 30.0, solar: vec![a], ..Default::default() });
    let mut inv = crate::model::DeviceData::default();
    inv.parts.push(openecraft_parts::PartRef { number: "GEN.INV-PV-10KW".into(), count: 1, total: false });
    prj.devices.insert("=PV-T1".into(), inv);
    prj
}

fn strings(prj: &mut Project, list: &[(u32, u32, &str, u32)]) {
    let a = &mut prj.layout.buildings[0].roofs[0].solar[0];
    a.strings = list.iter().enumerate().map(|(k, &(series, parallel, inverter, mppt))| PvString { id: k as u64 + 1, name: format!("S{}", k + 1), series, parallel, inverter: inverter.into(), mppt }).collect();
}

fn messages(prj: &Project) -> Vec<String> {
    problems(prj).iter().map(|i| i.message()).collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn string_values_follow_the_temperatures() {
    let prj = plant(4, 2);
    let m = module_data(&prj, "GEN.PV-430W").unwrap();
    assert_eq!((m.voc, m.isc, m.vmp, m.imp, m.pmax), (39.2, 13.9, 32.6, 13.2, 430.0));
    assert!(near(m.tc_voc, -0.0025) && near(m.tc_pmax, -0.003) && m.max_system == Some(1500.0));
    let d = PvDesign::default();
    // Eight in series: the open circuit voltage at -10 °C, the MPP voltage at 70 °C.
    let v = values(&m, &d, &PvString { series: 8, ..Default::default() });
    assert!(near(v.voc_max, 8.0 * 39.2 * 1.0875) && near(v.vmp_min, 8.0 * 32.6 * 0.865) && near(v.vmp_mean, 260.8), "{v:?}");
    assert!(near(v.imp, 13.2) && near(v.isc_max, 13.9 * (1.0 + 0.00046 * 45.0)) && v.power == 3440.0);
    // Two strings of four in parallel: half the voltage, twice the current.
    let w = values(&m, &d, &PvString { series: 4, parallel: 2, ..Default::default() });
    assert!(near(w.voc_max * 2.0, v.voc_max) && near(w.imp, 26.4) && w.power == v.power && w.modules == 8);
    // Without electrical data: none.
    assert!(module_data(&prj, "GEN.PV-RAIL-40").is_none());
    let input = pv_input(&prj, "=PV-T1").unwrap();
    assert_eq!((input.max_voltage, input.mppt_range, input.trackers, input.max_current, input.max_isc), (Some(1000.0), Some((160.0, 950.0)), 2, Some(16.0), Some(20.0)));
    assert_eq!(inverters(&prj), vec!["=PV-T1".to_string()]);
}

#[test]
fn new_strings_take_the_free_modules() {
    let mut prj = plant(4, 2);
    let id = add_string(&mut prj, 0, 0, 0).unwrap();
    let a = &prj.layout.buildings[0].roofs[0].solar[0];
    let s = a.strings.iter().find(|s| s.id == id).unwrap();
    assert_eq!((s.name.as_str(), s.series, s.parallel, s.inverter.as_str(), s.mppt), ("S1", 8, 1, "=PV-T1", 1));
    assert!(problems(&prj).is_empty(), "{:?}", messages(&prj));
    add_string(&mut prj, 0, 0, 0).unwrap();
    let s2 = &prj.layout.buildings[0].roofs[0].solar[0].strings[1];
    assert_eq!((s2.name.as_str(), s2.series, s2.mppt), ("S2", 1, 2), "on the next tracker");
}

#[test]
fn strings_beyond_the_inverter_are_reported() {
    let mut prj = plant(10, 4);
    // 26 in series: 1108 V at -10 °C on a 1000 V input; 14 modules left over.
    strings(&mut prj, &[(26, 1, "=PV-T1", 1)]);
    let m = messages(&prj);
    assert!(m.contains(&"+B1: string S1 of PV array PV1 reaches 1108 V at -10 °C, more than the 1000 V =PV-T1 takes".to_string()), "{m:?}");
    assert!(m.contains(&"+B1: 14 modules of PV array PV1 are not connected to a string".to_string()), "{m:?}");
    assert_eq!(problems(&prj).iter().filter(|i| i.severity == Severity::Error).count(), 1);
    // Too many modules, a third tracker, two strings in parallel above the currents, a
    // short string below the MPP range, strings of different lengths on one tracker.
    strings(&mut prj, &[(8, 2, "=PV-T1", 1), (8, 1, "=PV-T1", 3), (5, 1, "=PV-T1", 2), (8, 1, "=PV-T1", 2), (8, 1, "=X1", 1)]);
    let m = messages(&prj);
    for expected in [
        "+B1: the strings of PV array PV1 connect 45 modules, the array has 40",
        "+B1: string S2 of PV array PV1: =PV-T1 has only 2 MPP trackers",
        "=PV-T1, MPP tracker 1: the strings reach a short-circuit current of 28.4 A, more than the 20.0 A it takes",
        "=PV-T1, MPP tracker 1: the strings deliver 26.4 A, more than the 16.0 A it uses (the power is limited)",
        "+B1: string S3 of PV array PV1 drops to 141 V at 70 °C, below the MPP range of =PV-T1 (160 - 950 V)",
        "=PV-T1, MPP tracker 2: its strings differ in voltage (163 V and 261 V)",
        "+B1: string S5 of PV array PV1: =X1 is no device with PV input data",
    ] {
        assert!(m.iter().any(|t| t == expected), "{expected}: {m:?}");
    }
    // Above the system voltage of the modules.
    strings(&mut prj, &[(40, 1, "", 1)]);
    let m = messages(&prj);
    assert!(m.iter().any(|t| t == "+B1: string S1 of PV array PV1 reaches 1705 V at -10 °C, more than the 1500 V system voltage of its modules"), "{m:?}");
    assert!(m.iter().any(|t| t == "+B1: string S1 of PV array PV1 is connected to no inverter"), "{m:?}");
}

#[test]
fn the_report_lists_every_string_with_its_values() {
    let mut prj = plant(4, 2);
    strings(&mut prj, &[(4, 1, "=PV-T1", 1), (4, 1, "=PV-T1", 2)]);
    let an = crate::reports::Analysis::build(&prj);
    let t = crate::reports::table(&prj, &an, crate::model::ReportKind::PvStrings);
    assert_eq!(t.rows.len(), 4, "two strings, the total and the temperatures");
    assert_eq!(t.rows[0][..7], ["+B1", "PV1", "S1", "4 × 1", "GEN.PV-430W", "=PV-T1", "1"]);
    assert_eq!(t.rows[0][7..], ["171", "130", "113", "13.2", "14.2", "1.72"]);
    assert_eq!((t.rows[2][0].as_str(), t.rows[2][12].as_str()), ("Total", "3.44"));
    assert!(t.rows[3][0].contains("-10 °C"));
}
