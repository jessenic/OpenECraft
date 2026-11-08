use super::*;
use crate::parts::PartRef;

fn sym(prj: &mut Project, key: &str, x: f32, y: f32, tag: &str, label: &str) -> u64 {
    let def = prj.symbol(key).unwrap();
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst {
        id,
        sym: key.into(),
        pos: p(x, y),
        tag: tag.into(),
        label: label.into(),
        pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
        ..Default::default()
    });
    id
}

fn wire(prj: &mut Project, x1: f32, y1: f32, x2: f32, y2: f32) -> u64 {
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(x1, y1), b: p(x2, y2), ..Default::default() });
    id
}

fn assign(prj: &mut Project, tag: &str, number: &str) {
    prj.devices.entry(tag.into()).or_default().parts.push(PartRef { number: number.into(), count: 1, total:false });
}

/// Pin pairs of `plug` at x = 100, 120 … with a wire above (socket side) and one
/// below (pin side) each; returns the wires below.
fn pairs(prj: &mut Project, plug: &str, labels: &[&str]) -> Vec<u64> {
    let mut below = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let x = 100.0 + 20.0 * i as f32;
        wire(prj, x, 80.0, x, 96.0);
        sym(prj, "plug_pair", x, 100.0, plug, label);
        below.push(wire(prj, x, 104.0, x, 120.0));
    }
    below
}

fn nets(prj: &Project) -> (Netlist, Netlist) {
    (Netlist::build(prj, true), Netlist::build(prj, false))
}

#[test]
fn the_face_shows_the_pins_of_the_part_with_the_spares() {
    let mut prj = Project::default();
    pairs(&mut prj, "-X1", &["1", "4"]);
    assign(&mut prj, "-X1", "GEN.CON-M8-3M");
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X1", &nl, &open);
    assert_eq!(po.layout.as_ref().map(|l| l.text()), Some("circle 4".to_string()));
    assert_eq!(po.coding, "A");
    let v: Vec<(&str, Option<usize>, bool)> = po.pins.iter().map(|p| (p.label.as_str(), p.slot, p.used())).collect();
    assert_eq!(v, [("1", Some(0), true), ("3", Some(2), false), ("4", Some(3), true)]);
    // The socket side is the mirror image of the pin side.
    let pin = pin_points(&po, Side::Pin, p(0.0, 0.0), 1.0);
    let socket = pin_points(&po, Side::Socket, p(0.0, 0.0), 1.0);
    assert_eq!(pin.len(), 3);
    assert!((pin[0].1.x + socket[0].1.x).abs() < 1e-5 && (pin[0].1.y - socket[0].1.y).abs() < 1e-5);
    assert!(problems(&prj).is_empty());
}

#[test]
fn plugs_without_a_pin_layout_get_a_row() {
    let mut prj = Project::default();
    pairs(&mut prj, "-X2", &["1", "2", "3"]);
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X2", &nl, &open);
    assert_eq!(po.layout, None);
    assert_eq!(po.face.slots.len(), 3);
    assert!(po.face.slots.iter().all(|s| s[1] == 0.0));
}

#[test]
fn each_side_shows_its_wire_colour_and_targets() {
    let mut prj = Project::default();
    let below = pairs(&mut prj, "-X1", &["1"]);
    prj.pages[0].wires.iter_mut().find(|w| w.id == below[0]).unwrap().color = "BN".into();
    sym(&mut prj, "coil", 100.0, 124.0, "-K1", "");
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X1", &nl, &open);
    assert_eq!(po.pins[0].pin.color, "BN");
    assert_eq!(po.pins[0].pin.targets, "-K1:A1");
    assert_eq!(po.pins[0].socket.targets, "");
}

#[test]
fn a_flat_cable_connected_to_an_idc_connector_puts_core_n_on_pin_n() {
    let mut prj = Project::default();
    let below = pairs(&mut prj, "-X20", &["2", "1", "3"]);
    assign(&mut prj, "-X20", "GEN.CON-IDC-10F");
    let part = crate::parts::catalog().iter().find(|p| p.number == "GEN.CBL-FLAT-10").unwrap();
    prj.cables.push(Cable { id: 900, tag: "-W20".into(), cores: part.tech_value("Cores").unwrap().into(), part: part.number.clone(), ..Default::default() });
    assert!(is_idc(&prj, "-X20") && is_ribbon(&prj, &prj.cables[0]) && is_flat(&prj, &prj.cables[0]));
    assert_eq!(connect_flat_cable(&mut prj, "-W20", "-X20", Side::Pin), Ok(3));
    let core = |id: u64| prj.pages[0].wires.iter().find(|w| w.id == id).map(|w| (w.cable.clone(), w.core.clone())).unwrap();
    assert_eq!(core(below[0]), ("-W20".to_string(), "2".to_string()));
    assert_eq!(core(below[1]).1, "1");
    assert!(problems(&prj).is_empty(), "{:?}", problems(&prj).iter().map(|i| i.message()).collect::<Vec<_>>());
    // Ribbon cables are grey with core 1 marked red.
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X20", &nl, &open);
    let pin = |l: &str| po.pins.iter().find(|p| p.label == l).unwrap();
    assert_eq!((pin("1").pin.color.as_str(), pin("2").pin.color.as_str()), ("RD", "GY"));
    assert_eq!(pin("3").pin.core, "-W20:3");
    assert_eq!(po.pins.len(), 10);
    // A core on the wrong pin, and a cable of another width.
    prj.pages[0].wires.iter_mut().find(|w| w.id == below[2]).unwrap().core = "5".into();
    prj.cables[0].cores = "1 2 3 4 5 6 7 8".into();
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert!(msgs.contains(&"-X20: core 5 of flat cable -W20 lands on pin 3".to_string()), "{msgs:?}");
    assert!(msgs.contains(&"Flat cable -W20 has 8 cores, the IDC connector -X20 has 10 pins".to_string()), "{msgs:?}");
    assert!(connect_flat_cable(&mut prj, "-W99", "-X20", Side::Pin).is_err());
}

#[test]
fn pins_the_part_does_not_have_are_reported() {
    let mut prj = Project::default();
    pairs(&mut prj, "-X1", &["1", "7"]);
    assign(&mut prj, "-X1", "GEN.CON-M8-3M");
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert_eq!(msgs, ["-X1: pin 7 is not a pin of the part GEN.CON-M8-3M"]);
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X1", &nl, &open);
    assert_eq!(po.pins.last().map(|p| (p.label.as_str(), p.slot)), Some(("7", None)));
}

#[test]
fn the_svg_drawing_shows_every_plug_with_its_faces_and_table() {
    let mut prj = Project::default();
    pairs(&mut prj, "-X1", &["1", "2"]);
    assign(&mut prj, "-X1", "GEN.CON-GX16-4F");
    pairs(&mut prj, "-X3", &["1"]);
    assign(&mut prj, "-X3", "GEN.CON-DSUB-9M");
    let (nl, open) = nets(&prj);
    let svg = svg(&prj, &nl, &open, &[]);
    assert!(svg.starts_with("<svg") && svg.contains("-X1") && svg.contains("-X3"));
    assert!(svg.contains("GEN.CON-GX16-4F") && svg.contains("circle 4"));
    // Two faces of 4 + 9 pins each, the GX outline and its inner ring.
    assert!(svg.matches("<circle").count() > 2 * (4 + 9) * 2);
    assert!(svg.contains("spare"));
}


#[test]
fn hermaphroditic_parts_name_both_faces() {
    let mut prj = Project::default();
    pairs(&mut prj, "-X5", &["1", "2"]);
    assign(&mut prj, "-X5", "GEN.CON-BAT-50");
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X5", &nl, &open);
    assert_eq!(caption(&po, Side::Pin), format!("{}  GEN.CON-BAT-50", Side::Pin.title()));
    assert_eq!(caption(&po, Side::Socket), format!("{}  GEN.CON-BAT-50", Side::Socket.title()));
    // Free positions of an appliance inlet: earth contact above line and neutral.
    let mut prj = Project::default();
    assign(&mut prj, "-X6", "GEN.CON-IEC-C14");
    pairs(&mut prj, "-X6", &["L", "N", "PE"]);
    let (nl, open) = nets(&prj);
    let po = pinout(&prj, "-X6", &nl, &open);
    assert_eq!(po.pins.iter().map(|p| p.slot).collect::<Vec<_>>(), [Some(0), Some(1), Some(2)]);
    assert!(po.face.slots[2][1] < po.face.slots[0][1]);
    assert!(problems(&prj).is_empty());
}
