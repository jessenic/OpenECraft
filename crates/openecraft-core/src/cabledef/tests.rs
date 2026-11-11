use super::*;
use crate::geom::p;

fn project() -> Project {
    let mut prj = Project::default();
    for (i, x) in [20.0, 28.0, 36.0].into_iter().enumerate() {
        prj.pages[0].wires.push(Wire { id: 10 + i as u64, a: p(x, 20.0), b: p(x, 80.0), ..Default::default() });
    }
    prj.fix_next_id();
    prj
}

#[test]
fn crossed_wires_in_order() {
    let prj = project();
    assert_eq!(crossed_wires(&prj.pages[0], p(40.0, 50.0), p(16.0, 50.0)), vec![12, 11, 10]);
    assert!(crossed_wires(&prj.pages[0], p(40.0, 90.0), p(16.0, 90.0)).is_empty());
}

#[test]
fn new_cable_gets_numbered_cores() {
    let mut prj = project();
    let id = add(&mut prj, 0, p(16.0, 50.0), p(32.0, 50.0), "");
    assert_eq!(prj.cables.len(), 1);
    assert_eq!(prj.cables[0].cores, "1 2");
    let tag = prj.cables[0].tag.clone();
    assert_eq!(prj.pages[0].cable_defs[0].id, id);
    let cores: Vec<(&str, &str)> = prj.pages[0].wires.iter().map(|w| (w.cable.as_str(), w.core.as_str())).collect();
    assert_eq!(cores, vec![(tag.as_str(), "1"), (tag.as_str(), "2"), ("", "")]);
    assert!(problems(&prj).is_empty());
}

#[test]
fn existing_cable_keeps_used_cores() {
    let mut prj = project();
    prj.cables.push(Cable { id: 90, tag: "-W5".into(), cores: "BN BK GY".into(), ..Default::default() });
    prj.pages[0].wires[1].cable = "-W5".into();
    prj.pages[0].wires[1].core = "BN".into();
    add(&mut prj, 0, p(16.0, 50.0), p(40.0, 50.0), "-W5");
    let cores: Vec<&str> = prj.pages[0].wires.iter().map(|w| w.core.as_str()).collect();
    assert_eq!(cores, vec!["BK", "BN", "GY"]);
}

#[test]
fn problems_unknown_cable_and_nothing_crossed() {
    let mut prj = project();
    prj.pages[0].cable_defs.push(CableDef { id: 1, a: p(0.0, 0.0), b: p(4.0, 0.0), cable: "-W9".into(), shield: false });
    prj.cables.push(Cable { id: 90, tag: "-W5".into(), ..Default::default() });
    prj.pages[0].cable_defs.push(CableDef { id: 2, a: p(0.0, 0.0), b: p(4.0, 0.0), cable: "-W5".into(), shield: false });
    let issues = problems(&prj);
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].severity, Severity::Error);
}

#[test]
fn numbered_core_list() {
    assert_eq!(numbered_cores(3, true), "1 2 3 GNYE");
    assert_eq!(numbered_cores(2, false), "1 2");
}

#[test]
fn cores_sit_where_the_cable_definition_line_crosses() {
    let mut prj = project();
    add(&mut prj, 0, p(16.0, 50.0), p(40.0, 35.0), "");
    let page = &prj.pages[0];
    for w in &page.wires {
        let t = core_position(page, w);
        let y = w.a.y + (w.b.y - w.a.y) * t;
        let on_line = 50.0 + (w.a.x - 16.0) * (35.0 - 50.0) / 24.0;
        assert!((y - on_line).abs() < 1e-3, "{} {y} {on_line}", w.id);
    }
    let other = Wire { id: 99, a: p(60.0, 20.0), b: p(60.0, 80.0), cable: page.wires[0].cable.clone(), ..Default::default() };
    assert_eq!(core_position(page, &other), 0.3, "not crossed: at 30 %");
}
