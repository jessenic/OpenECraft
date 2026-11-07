use super::*;
use crate::builtin::testing::*;

#[test]
fn user_fluid_port_roles_are_editable_through_the_shared_registry() {
    use openecraft_core::symlib::{UserPin, UserSymbol};
    use openecraft_core::symbols::SymbolDomain;
    let mut s = new_session();
    s.project_mut().unwrap().symbols.push(UserSymbol {
        key: "user:silencer".into(), domain: SymbolDomain::Fluid,
        pins: vec![UserPin::default()], ..Default::default()
    });
    let result = run(&mut s, "set_fluid_port_role", json!({"symbol":"user:silencer", "pin":0, "role":"exhaust"})).unwrap();
    assert_eq!(result["role"], "exhaust");
    assert_eq!(s.project().unwrap().symbols[0].fluid_ports[&0], fluid::PortKind::Exhaust);
    let definitions = run(&mut s, "list_symbols", json!({"query":"user:silencer"})).unwrap();
    assert_eq!(definitions[0]["fluid_ports"]["0"], "exhaust");
    assert!(run(&mut s, "set_fluid_port_role", json!({"symbol":"user:silencer", "pin":1, "role":"pressure"})).is_err());
    assert!(run(&mut s, "set_fluid_port_role", json!({"symbol":"user:silencer", "pin":0, "role":"unknown"})).is_err());
    run(&mut s, "set_fluid_port_role", json!({"symbol":"user:silencer", "pin":0, "role":"automatic"})).unwrap();
    assert!(s.project().unwrap().symbols[0].fluid_ports.is_empty());
}

#[test]
fn pct_loops_of_the_pre_planning() {
    let mut s = new_session();
    let v = run(&mut s, "set_pct_loop", json!({ "name": "101", "description": "Tank temperature", "devices": "-KF101 -TA101" })).unwrap();
    assert_eq!(v[0]["devices"], json!(["-KF101", "-TA101"]));
    assert_eq!(v[0]["status"], "planned");
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "pid_instrument", "x": 100, "y": 100, "tag": "-BT101", "label": "TIC 101" })).unwrap();
    let v = run(&mut s, "list_pct_loops", json!({})).unwrap();
    assert_eq!(v[0]["functions"], json!(["TIC"]));
    assert_eq!(v[0]["measured_variable"], "Temperature");
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 200, "y": 100, "tag": "-KF101" })).unwrap();
    assert_eq!(run(&mut s, "list_pct_loops", json!({})).unwrap()[0]["status"], "in_engineering");
    assert!(run(&mut s, "set_pct_loop", json!({ "name": "102", "rename": "101" })).unwrap_err().contains("already exists"));
    run(&mut s, "set_pct_loop", json!({ "name": "101", "rename": "110" })).unwrap();
    let v = run(&mut s, "delete_pct_loop", json!({ "name": "110" })).unwrap();
    // The instrument still names loop 101.
    assert_eq!(v.as_array().unwrap().iter().filter(|l| l["defined"] == true).count(), 0);
    assert!(run(&mut s, "delete_pct_loop", json!({ "name": "110" })).is_err());
}
