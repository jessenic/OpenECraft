use super::*;

#[test]
fn a_removed_function_part_is_reported_at_its_symbol() {
    let mut prj = Project::default();
    let mut s = crate::model::SymbolInst { id: 900, sym: "coil".into(), tag: "-K1".into(), ..Default::default() };
    s.values.insert("function.part_number".into(), "OLD.PART".into());
    prj.pages[0].symbols.push(s);
    let issues = check_function_parts(&prj);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, Severity::Error);
    assert!(issues[0].message().contains("OLD.PART"));
    prj.devices.entry("-K1".into()).or_default().parts.push(crate::model::PartRef { number: "OLD.PART".into(), count: 1, total:false });
    assert!(check_function_parts(&prj).is_empty());
}
use crate::sample::demo_project;

#[test]
fn built_in_checks_have_unique_snake_case_names() {
    let checks = builtin();
    let mut names = std::collections::HashSet::new();
    for c in &checks {
        assert!(c.name.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'), "{}", c.name);
        assert!(names.insert(c.name.clone()), "duplicate check {}", c.name);
        assert!(!c.title.is_empty());
    }
    assert!(checks.len() > 20);
}

#[test]
fn build_leaves_the_issues_to_the_checks() {
    let prj = demo_project();
    assert!(Analysis::build(&prj).issues.is_empty());
    let an = Analysis::checked(&prj);
    assert_eq!(an.issues.len(), run_builtin(&prj, &an).len());
    assert!(an.issues.windows(2).all(|w| w[0].severity <= w[1].severity), "sorted by severity");
}

#[test]
fn check_schemes_are_exchanged_without_the_accepted_messages() {
    let mut a = crate::sample::demo_project();
    a.check_settings.disabled.push("cables".into());
    a.check_settings.levels.insert("devices.abc123".into(), crate::model::checks::Level::Info);
    a.check_settings.accepted.push(crate::model::checks::Accepted { id: "x".into(), message: "m".into(), ..Default::default() });
    let json = scheme_json(&a);
    assert!(!json.contains("accepted"));
    let mut b = crate::sample::demo_project();
    b.check_settings.accepted.push(crate::model::checks::Accepted { id: "y".into(), ..Default::default() });
    apply_scheme(&mut b, &json).unwrap();
    assert_eq!(b.check_settings.disabled, vec!["cables".to_string()]);
    assert_eq!(b.check_settings.levels.len(), 1);
    assert_eq!(b.check_settings.accepted.len(), 1, "own acceptances stay");
    assert!(apply_scheme(&mut b, "[1, 2]").is_err());
}

/// Regression: a project file or any other JSON object was taken as an empty scheme
/// and switched every check on again.
#[test]
fn other_json_objects_are_no_check_scheme() {
    let mut prj = crate::sample::demo_project();
    prj.check_settings.disabled.push("cables".into());
    let project_file = serde_json::to_string(&crate::sample::demo_project()).unwrap();
    for json in [project_file.as_str(), r#"{"name": "x"}"#, "5"] {
        let err = apply_scheme(&mut prj, json).unwrap_err();
        assert!(err.contains("Not a check scheme"), "{err}");
    }
    assert_eq!(prj.check_settings.disabled, vec!["cables".to_string()], "unchanged");
    apply_scheme(&mut prj, "{}").unwrap();
    assert!(prj.check_settings.disabled.is_empty(), "an empty scheme switches every check on");
}

#[test]
fn a_wire_ending_in_empty_space_is_reported() {
    let mut prj = demo_project();
    let open = |prj: &Project| {
        let an = Analysis::checked(prj);
        run_builtin(prj, &an).iter().filter(|i| i.message().starts_with("Wire ends open")).count()
    };
    assert_eq!(open(&prj), 0);
    let pi = prj.pages.iter().position(|p| p.kind == PageKind::Schematic && !p.wires.is_empty()).unwrap();
    let id = prj.new_id();
    prj.pages[pi].wires.push(Wire { id, a: crate::geom::p(300.0, 200.0), b: crate::geom::p(320.0, 200.0), ..Default::default() });
    assert_eq!(open(&prj), 2, "both ends of a lone wire");
    let an = Analysis::checked(&prj);
    let issue = run_builtin(&prj, &an).into_iter().find(|i| i.message().starts_with("Wire ends open")).unwrap();
    assert_eq!(issue.severity, Severity::Warning);
    assert_eq!(issue.target, Some((pi, id)));
}

#[test]
fn poles_drawn_one_by_one_are_one_device() {
    let mut prj = demo_project();
    let twice = |prj: &Project| {
        let idx = crate::index::Index::build(prj);
        let nl = crate::netlist::Netlist::build(prj, true);
        check(prj, &idx, &nl).iter().filter(|i| i.message().starts_with("=") || i.message().contains("main function placed")).map(|i| i.message()).collect::<Vec<_>>()
    };
    assert!(twice(&prj).is_empty());
    // A second breaker -F1: its part has one breaker function.
    let pi = prj.pages.iter().position(|p| p.symbols.iter().any(|s| s.tag == "-F1")).unwrap();
    let mut second = prj.pages[pi].symbols.iter().find(|s| s.tag == "-F1").unwrap().clone();
    second.id = prj.new_id();
    second.pos = crate::geom::p(360.0, 200.0);
    prj.pages[pi].symbols.push(second);
    assert_eq!(twice(&prj), ["-F1: main function placed 2 times"]);
    // Two breakers in the parts of -F1: two poles of one device.
    prj.devices.get_mut("-F1").unwrap().parts[0].count = 2;
    assert!(twice(&prj).is_empty());
    // Not when the second is of another kind.
    prj.pages[pi].symbols.last_mut().unwrap().sym = "fuse".into();
    assert_eq!(twice(&prj), ["-F1: main function placed 2 times"]);
}

#[test]
fn a_duct_too_small_for_its_wires_is_an_error() {
    let mut prj = Project::default();
    let mut e = crate::panel::Enclosure { name: "+A".into(), ..Default::default() };
    e.items.push(crate::panel::PanelItem { id: 7, kind: crate::panel::ItemKind::Duct, x: 0.0, y: 0.0, w: 400.0, h: 40.0, d: 60.0, ..Default::default() });
    prj.layout.enclosures.push(e);
    let fill = |diameters: Vec<f32>| crate::panel::DuctFill { enclosure: 0, duct: 7, capacity: 2400.0, percent: 10.0, from: 0.0, to: 100.0, wires: diameters.len(), total_wires: diameters.len(), diameters };
    // Two 28.7 mm wires on top of each other need 57.6 mm, the duct is 55.5 deep inside.
    let issues = check_duct_fill(&prj, &[fill(vec![28.7, 28.7])]);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, Severity::Error);
    assert!(issues[0].message().starts_with("+A: cable duct at ") && issues[0].message().contains("its 2 wires cannot be routed"), "{}", issues[0].message());
    assert!(check_duct_fill(&prj, &[fill(vec![28.7])]).is_empty());
}
