//! Schematic builder for the unit tests of the electrical calculations: places
//! symbols, connects their pins by designation and assigns parts.

use crate::geom::p;
use crate::model::*;
use crate::parts::{FnKind, FunctionTemplate, PartRef};

pub struct Kit {
    pub prj: Project,
}

impl Default for Kit {
    fn default() -> Self {
        Kit::new()
    }
}

impl Kit {
    /// An empty project with the IEC 60204-1 potentials.
    pub fn new() -> Kit {
        Kit { prj: Project::new_default() }
    }

    /// Places a symbol, returns its id.
    pub fn sym(&mut self, key: &str, x: f32, y: f32, tag: &str, label: &str) -> u64 {
        let def = self.prj.symbol(key).unwrap();
        let id = self.prj.new_id();
        self.prj.pages[0].symbols.push(SymbolInst {
            id,
            sym: key.into(),
            pos: p(x, y),
            tag: tag.into(),
            label: if label.is_empty() { def.default_label.into() } else { label.into() },
            pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
            ..Default::default()
        });
        id
    }

    fn pin_at(&self, id: u64, pin: &str) -> crate::geom::P {
        let s = self.prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap();
        let i = s.pins.iter().position(|x| x == pin).unwrap_or_else(|| panic!("no pin {pin} on {}", s.tag));
        self.prj.symbol(&s.sym).unwrap().pin_world(i, s.pos, s.rot, s.mirror)
    }

    /// Wire between two pins (symbol id, designation), returns its id.
    pub fn wire(&mut self, a: (u64, &str), b: (u64, &str)) -> u64 {
        let (pa, pb) = (self.pin_at(a.0, a.1), self.pin_at(b.0, b.1));
        let id = self.prj.new_id();
        self.prj.pages[0].wires.push(Wire { id, a: pa, b: pb, ..Default::default() });
        id
    }

    /// Wire as a core of a cable.
    pub fn core(&mut self, a: (u64, &str), b: (u64, &str), cable: &str, core: &str) -> u64 {
        let id = self.wire(a, b);
        let w = self.prj.pages[0].wires.last_mut().unwrap();
        w.cable = cable.into();
        w.core = core.into();
        id
    }

    pub fn cable(&mut self, tag: &str, cores: &str, cross_section: &str, length: f32) {
        let id = self.prj.new_id();
        self.prj.cables.push(Cable { id, tag: tag.into(), cores: cores.into(), cross_section: cross_section.into(), length, ..Default::default() });
    }

    /// Assigns a new part with one function to a device.
    pub fn part(&mut self, tag: &str, kind: FnKind, pins: &str, f: impl FnOnce(&mut Part)) {
        let number = format!("TEST.{}-{}", tag.trim_start_matches('-'), self.prj.parts.len());
        let mut part = Part { number: number.clone(), functions: vec![FunctionTemplate::new(kind, pins)], ..Default::default() };
        f(&mut part);
        self.prj.devices.entry(tag.into()).or_default().parts.push(PartRef { number, count: 1, total:false });
        self.prj.parts.push(part);
    }
}

/// Makes the wires of the first connection of harness -WH1 a core of a new cable
/// `-W77` (the demo harness has no cable).
pub fn with_cable(prj: &mut Project) {
    let an = crate::analysis::Analysis::checked(prj);
    let c = an.connections.iter().find(|c| c.harness == "-WH1").unwrap();
    let net = an.open.nets.iter().find(|n| n.pins.contains(&c.a) && n.pins.contains(&c.b)).unwrap().clone();
    for (pi, id) in &net.wires {
        if let Some(w) = prj.pages[*pi].wires.iter_mut().find(|w| w.id == *id) {
            w.cable = "-W77".into();
            w.core = "1".into();
        }
    }
    let id = prj.new_id();
    prj.cables.push(Cable { id, tag: "-W77".into(), cable_type: "LIYY 2x0.75".into(), cores: "1 2".into(), cross_section: "0.75 mm\u{b2}".into(), length: 2.0, ..Default::default() });
}
