//! EPLAN PXF window macros. Coordinates in the file are millimetres, Y up.
//! Embedded O5 drawings are definitions; O144/S144x5 objects instantiate them.
use super::{UserPin, UserSymbol};
use crate::geom::{p, P};
use crate::symbols::{Anchor, Prim};
use openecraft_parts::xml::{self, Element};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod contact;
mod placeholder;
pub use placeholder::Placeholder;
mod transfer;
pub use transfer::Transfer;

#[derive(Debug, Serialize, Deserialize)]
pub struct Variant {
    pub function_type: u32,
    pub variant_id: u32,
    pub description: String,
    pub symbol: UserSymbol,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    pub warnings: BTreeSet<String>,
    /// Per-placement diagnostics. The insertion point is not a wire endpoint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<Reference>,
    /// Schematic connection metadata, separate from native wire segments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connection_markers: Vec<ConnectionMarker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transfers: Vec<Transfer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placeholders: Vec<Placeholder>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionMarker {
    pub object: String,
    pub kind: String,
    pub position: P,
    pub properties: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub designations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reference {
    pub object: String,
    pub record: String,
    pub master: String,
    pub placement: P,
    pub plug: String,
    pub designations: Vec<String>,
    pub descriptions: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bus_interface: String,
    pub reason: String,
}

fn plug(el: &Element) -> &str {
    el.child("P11").and_then(|p| p.attribute("P20406")).filter(|p| !p.trim().is_empty()).or_else(|| el.attribute("A194")).unwrap_or("")
}

fn fields(el: &Element, name: &str) -> Vec<String> {
    el.attribute(name).map(|v| v.split(';').map(|s| s.trim().to_owned()).collect()).unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub tag: String,
    pub position: P,
    pub symbol: UserSymbol,
}

impl Variant {
    pub fn definitions(&self) -> impl Iterator<Item = &UserSymbol> {
        std::iter::once(&self.symbol).chain(self.devices.iter().map(|d| &d.symbol))
    }
}

fn device_tag(el: &Element) -> Option<&str> {
    el.child("P11")?.attribute("P20006").filter(|tag| !tag.is_empty())
}

fn number(s: &str) -> Option<f32> {
    s.parse::<f32>().ok().filter(|n| n.is_finite() && n.abs() <= 1_000_000.)
}

fn xy(s: &str) -> Option<P> {
    let mut v = s.split('/');
    Some(p(number(v.next()?)?, number(v.next()?)?))
}

fn display_text(raw: &str) -> String {
    if raw.contains('@')
        && raw
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .all(|s| s.trim().split_once('@').is_some_and(|(code, text)| code.len() == 5 && code.as_bytes()[2] == b'_' && text.trim().is_empty()))
    {
        return String::new();
    }
    // Some PXF exports store the English drawing labels in the language-neutral slot.
    if raw.contains("??_??@") && !raw.contains("en_US@") && !raw.contains("en_GB@") {
        let neutral = raw.replace("??_??@", "en_US@");
        return openecraft_parts::language_text(&neutral);
    }
    openecraft_parts::language_text(raw)
}

#[derive(Clone, Copy)]
struct Frame {
    origin: P,
    x: P,
    y: P,
}

impl Frame {
    fn at(self, q: P) -> P {
        self.origin + self.x * q.x + self.y * q.y
    }

    fn local(self, el: &Element) -> Option<Self> {
        let Some(t) = el.child("S40x1201") else { return Some(self) };
        let origin = self.at(match t.attribute("A762") {
            Some(value) => xy(value)?,
            None => P::default(),
        });
        let Some(m) = t.child("S39x761") else { return Some(Self { origin, ..self }) };
        let n = |k| number(m.attribute(k)?);
        Some(Self { origin, x: self.x * n("A751")? + self.y * n("A752")?, y: self.x * n("A753")? + self.y * n("A754")? })
    }
}

struct Reader<'a> {
    definitions: BTreeMap<&'a str, &'a Element>,
    libraries: BTreeMap<&'a str, &'a str>,
    layers: BTreeMap<&'a str, &'a Element>,
    mappings: &'a [UserSymbol],
    split_devices: bool,
    result: Variant,
    budget: usize,
    logic: transfer::Logic,
}

impl Reader<'_> {
    fn unresolved(&mut self, el: &Element, f: Frame, reason: &str) {
        let library = el.attribute("A1261").unwrap_or("");
        let library = self.libraries.get(library).copied().unwrap_or(library);
        let master = format!("{library}/{}/{}", el.attribute("A1262").unwrap_or(""), el.attribute("A1263").unwrap_or(""));
        self.result.warnings.insert(format!("library {master}"));
        self.result.unresolved.push(Reference {
            object: el.attribute("A1").unwrap_or("").into(),
            record: el.name.clone(),
            master,
            placement: f.origin,
            plug: plug(el).into(),
            designations: fields(el, "A185"),
            descriptions: fields(el, "A186"),
            bus_interface: el.child("P11").and_then(|p| p.attribute("P20447")).unwrap_or("").into(),
            reason: reason.into(),
        });
    }
    fn styled(&mut self, el: &Element, primitive: Prim) -> Prim {
        let layer = el.attribute("A411").and_then(|key| self.layers.get(key).copied());
        let style =
            |object, default| el.attribute(object).filter(|s| *s != "L" && *s != "-16002").or_else(|| layer.and_then(|l| l.attribute(default)));
        let color = match style("A412", "A1428") {
            Some("1") => Some([255, 0, 0]),
            Some("2") => Some([255, 255, 0]),
            Some("3") => Some([0, 255, 0]),
            Some("4") => Some([0, 255, 255]),
            Some("5") => Some([0, 0, 255]),
            Some("6") => Some([255, 0, 255]),
            Some("7") => Some([255, 255, 255]),
            Some("252") => Some([64, 64, 64]),
            Some("253") => Some([128, 128, 128]),
            Some(code) if code != "0" && code != "-1" => {
                self.result.warnings.insert(format!("palette {code}"));
                None
            }
            _ => None,
        };
        let width = style("A414", "A1429").and_then(number).filter(|w| *w > 0.);
        let dashed = style("A413", "A1427").and_then(|s| s.parse::<i32>().ok()).is_some_and(|n| n > 0);
        if color.is_none() && width.is_none() && !dashed {
            return primitive;
        }
        let width = width.unwrap_or(0.25);
        match primitive {
            Prim::Line { a, b, .. } => Prim::Path { pts: vec![a, b], closed: false, fill: false, width, color, dashed },
            Prim::Poly { pts, closed, fill } => Prim::Path { pts, closed, fill, width, color, dashed },
            Prim::Text { pos, text, size, anchor, angle, .. } => Prim::Text { pos, text, size, anchor, angle, color },
            other => other,
        }
    }

    fn graphic(&mut self, el: &Element, f: Frame) -> Option<Prim> {
        let q = |k| xy(el.attribute(k)?);
        // PXF omits zero-valued positions, including the first or last point of a line.
        let local_point = |key| match el.attribute(key) {
            Some(s) => xy(s),
            None => Some(P::default()),
        };
        match el.name.as_str() {
            "S31x5" | "O31" if el.attribute("A531").is_some() || el.attribute("A532").is_some() => {
                Some(Prim::Line { a: f.at(local_point("A531")?), b: f.at(local_point("A532")?), dashed: false })
            }
            "S89x5" | "O89" => {
                let a = q("A1651")?;
                let b = q("A1652")?;
                Some(Prim::Poly { pts: [a, p(b.x, a.y), b, p(a.x, b.y)].map(|q| f.at(q)).to_vec(), closed: true, fill: false })
            }
            "S34x5" | "O34" => {
                let pts = el
                    .attribute("A621")?
                    .trim_end_matches(';')
                    .split(';')
                    .map(|s| Some(f.at(if s.is_empty() { P::default() } else { xy(s)? })))
                    .collect::<Option<Vec<_>>>()?;
                Some(Prim::Poly { pts, closed: true, fill: false })
            }
            "S90x5" | "O90" => {
                let a = local_point("A1661")?;
                let b = local_point("A1662")?;
                let center = (a + b) * 0.5;
                let radius = (b - a) * 0.5;
                if radius.x == 0. || radius.y == 0. {
                    return None;
                }
                let pts = (0..64)
                    .map(|i| {
                        let (s, c) = (std::f32::consts::TAU * i as f32 / 64.).sin_cos();
                        f.at(center + p(c * radius.x, s * radius.y))
                    })
                    .collect();
                Some(Prim::Poly { pts, closed: true, fill: false })
            }
            "S30x5" | "O30" | "O83" => {
                let style = el.child("S54x505");
                let size = style.and_then(|s| s.attribute("A961")).and_then(number).filter(|n| *n > 0.).unwrap_or(2.5);
                let angle = style.and_then(|s| s.attribute("A962")).and_then(number).unwrap_or_default();
                let axis = f.x * angle.cos() + f.y * angle.sin();
                Some(Prim::Text {
                    pos: f.at(local_point("A501")?),
                    text: display_text(el.attribute("A511")?),
                    size,
                    anchor: Anchor::Left,
                    color: None,
                    angle: Some((-axis.y).atan2(axis.x).to_degrees()),
                })
            }
            "S32x5" | "O32" => {
                let center = q("A555")?;
                let radius = q("A556")?;
                let start = number(el.attribute("A553")?)?;
                let end = number(el.attribute("A554")?)?;
                let mut sweep = (end - start).rem_euclid(std::f32::consts::TAU);
                if sweep < 0.00001 {
                    sweep = std::f32::consts::TAU;
                }
                let steps = (sweep * 16.).ceil() as usize;
                let pts = (0..=steps)
                    .map(|i| {
                        let (s, c) = (start + sweep * i as f32 / steps as f32).sin_cos();
                        f.at(center + p(radius.x * c - radius.y * s, radius.x * s + radius.y * c))
                    })
                    .collect();
                Some(Prim::Poly { pts, closed: false, fill: false })
            }
            _ => None,
        }
    }

    fn function(&mut self, el: &Element, f: Frame) {
        let lib = el.attribute("A1261").unwrap_or("");
        let lib = self.libraries.get(lib).copied().unwrap_or(lib);
        let id = el.attribute("A1262").unwrap_or("");
        let variant = el.attribute("A1263").unwrap_or("");
        let pin = el.attribute("A185").unwrap_or("");
        let mapping_key = format!("user:eplan-lib:{lib}:{id}:{variant}");
        if let Some(mapping) = self.mappings.iter().find(|s| s.key == mapping_key) {
            self.mapped_symbol(mapping, el, f);
            return;
        }
        if lib == "SPECIAL" && id == "5" && el.child("S90x5").is_some() {
            // EPLAN locates the shield connection at the insertion point. Keep its
            // cable identity separate from the enclosing device's terminals.
            self.marker(el, f, "shield connection point");
            return;
        }
        if lib == "SPECIAL" && id == "4" && el.child("S31x5").is_some() {
            self.marker(el, f, "cable definition line");
            return;
        }
        if lib == "SPECIAL" && (id != "40" || self.result.function_type == 4) && contact::decode(el, f, id, variant, &mut self.result.symbol) {
            return;
        }
        // These variable boxes and the 2D part placement embed their own frame.
        if lib == "SPECIAL" && matches!(id, "0" | "1" | "12") && pin.is_empty() && el.child("S89x5").is_some() {
            return;
        }
        let reason = match (lib, id) {
            ("SPECIAL", "33" | "36" | "38" | "21" | "27") => "hidden contact; master pin anchor unavailable",
            ("SPECIAL", "22" | "23") => "two-sided contact; pin assignment unavailable",
            ("SPECIAL", "236" | "237" | "238") => "network port; master pin layout unavailable",
            _ => "function master or pin layout unavailable",
        };
        self.unresolved(el, f, reason);
    }

    fn marker(&mut self, el: &Element, f: Frame, kind: &str) {
        self.result.connection_markers.push(ConnectionMarker {
            object: el.attribute("A1").unwrap_or("").into(),
            kind: kind.into(),
            position: f.origin,
            properties: el.child("P11").map(|p| p.attributes.iter().cloned().collect()).unwrap_or_default(),
            designations: fields(el, "A185"),
        });
        self.result.warnings.insert(format!("{kind}: internal connection topology unavailable"));
    }

    fn mapped_symbol(&mut self, symbol: &UserSymbol, el: &Element, f: Frame) {
        // Native definitions use Y down. EMA drawing frames use Y up.
        let pt = |q: P| f.at(p(q.x, -q.y));
        let target = &mut self.result.symbol;
        for g in &symbol.prims {
            let transformed = match g {
                Prim::Line { a, b, dashed } => Prim::Line { a: pt(*a), b: pt(*b), dashed: *dashed },
                Prim::Poly { pts, closed, fill } => Prim::Poly { pts: pts.iter().copied().map(pt).collect(), closed: *closed, fill: *fill },
                Prim::Circle { c, r, fill, color } => Prim::Path {
                    pts: (0..64)
                        .map(|i| {
                            let (s, cos) = (std::f32::consts::TAU * i as f32 / 64.).sin_cos();
                            pt(*c + p(cos, s) * *r)
                        })
                        .collect(),
                    closed: true,
                    fill: *fill,
                    width: 0.25,
                    dashed: false,
                    color: *color,
                },
                Prim::Path { pts, closed, fill, width, color, dashed } => Prim::Path {
                    pts: pts.iter().copied().map(pt).collect(),
                    closed: *closed,
                    fill: *fill,
                    width: *width,
                    color: *color,
                    dashed: *dashed,
                },
                Prim::Text { pos, text, size, anchor, angle, color } => {
                    let (s, c) = angle.unwrap_or_default().to_radians().sin_cos();
                    let axis = f.x * c + f.y * s;
                    Prim::Text {
                        pos: pt(*pos),
                        text: text.clone(),
                        size: *size,
                        anchor: *anchor,
                        angle: Some((-axis.y).atan2(axis.x).to_degrees()),
                        color: *color,
                    }
                }
                Prim::Field { kind, pos, size, anchor } => Prim::Field { kind: *kind, pos: pt(*pos), size: *size, anchor: *anchor },
            };
            target.prims.push(transformed);
        }
        let labels: Vec<_> = el.attribute("A185").map(|s| s.split(';').collect()).unwrap_or_else(|| vec![""; symbol.pins.len()]);
        if symbol.pins.is_empty()
            && (labels.iter().any(|s| !s.trim().is_empty()) || el.child("S61x183").is_some_and(|r| r.child("S16x1062").is_some()))
        {
            self.unresolved(el, f, "mapping has artwork but no pin anchors");
            return;
        }
        if !symbol.pins.is_empty() && labels.len() != symbol.pins.len() {
            self.result.warnings.insert(format!("pin count {}", symbol.key));
            self.unresolved(el, f, "supplied mapping has a different pin count");
            return;
        }
        let base = target.pins.len();
        let plug = plug(el);
        for (pin, label) in symbol.pins.iter().zip(labels) {
            let name = if label.is_empty() { pin.label.as_str() } else { label };
            let label = if plug.is_empty() { name.to_string() } else { format!("{plug}.{name}") };
            target.pins.push(UserPin { pos: pt(pin.pos), label });
        }
        for &(a, b) in &symbol.bridges {
            if a < symbol.pins.len() && b < symbol.pins.len() {
                target.bridges.push((base + a, base + b));
            }
        }
    }

    fn walk(&mut self, el: &Element, frame: Frame, depth: usize) -> Result<(), String> {
        if self.split_devices && el.name == "O17" {
            if let Some(tag) = device_tag(el) {
                let index = self.result.devices.iter().position(|m| m.tag == tag);
                let mut member = match index {
                    Some(i) => self.result.devices.remove(i),
                    None => {
                        let symbol = UserSymbol {
                            key: format!("{}:device:{tag}", self.result.symbol.key),
                            name: tag.into(),
                            library: "EPLAN".into(),
                            ..Default::default()
                        };
                        Device { tag: tag.into(), position: frame.local(el).unwrap_or(frame).origin, symbol }
                    }
                };
                std::mem::swap(&mut self.result.symbol, &mut member.symbol);
                let result = self.object(el, frame, depth);
                std::mem::swap(&mut self.result.symbol, &mut member.symbol);
                self.result.devices.push(member);
                return result;
            }
        }
        self.object(el, frame, depth)
    }

    fn object(&mut self, el: &Element, frame: Frame, depth: usize) -> Result<(), String> {
        if depth > 128 || self.budget == 0 {
            return Err("EMA drawing expansion limit exceeded".into());
        }
        self.budget -= 1;
        let Some(f) = frame.local(el) else {
            self.result.warnings.insert(format!("invalid transform {}", el.name));
            return Ok(());
        };
        let initial_pins = self.result.symbol.pins.len();
        match el.name.as_str() {
            "O5" | "ESymLib" | "O76" | "O6" | "O14" => return Ok(()),
            "O144" | "S144x5" => {
                let id = el.attribute("R9").unwrap_or("");
                if let Some(&def) = self.definitions.get(id) {
                    for g in &def.children {
                        self.walk(g, f, depth + 1)?;
                    }
                } else {
                    self.result.warnings.insert(format!("drawing {id}"));
                }
                return Ok(());
            }
            "O17" => {
                self.function(el, f);
                let static_contact = el.attribute("A1261").and_then(|id| self.libraries.get(id)).copied() == Some("SPECIAL")
                    && matches!(el.attribute("A1262"), Some("13" | "17" | "20" | "22" | "34" | "35" | "37" | "40"));
                self.logic.collect(el, &self.result.symbol, initial_pins, static_contact);
            }
            "O42" | "O59" | "O93" | "O33" | "O70" | "O106" => {
                let lib = el.attribute("A1261").and_then(|id| self.libraries.get(id).copied());
                match (el.name.as_str(), lib, el.attribute("A1262")) {
                    ("O42", Some("SPECIAL"), Some("255")) => self.marker(el, f, "autoconnect break point"),
                    ("O59", Some("SPECIAL"), Some("311")) => self.marker(el, f, "connection definition point"),
                    ("O59", Some("SPECIAL"), Some("308")) => {
                        self.marker(el, f, "connection definition point");
                        if el.attribute("R9").is_none() && el.child("S31x5").is_none() {
                            self.unresolved(el, f, "visible connection-definition glyph unavailable");
                        }
                    }
                    ("O93", Some("SPECIAL"), Some("18")) => {
                        self.marker(el, f, "potential connection point");
                        self.unresolved(el, f, "potential master anchor unavailable");
                    }
                    ("O106", Some("SPECIAL"), Some("323")) => {
                        self.result.placeholders.push(placeholder::read(el));
                        self.result.warnings.insert("placeholder alternatives retained; placement uses the saved state".into());
                    }
                    _ => self.unresolved(el, f, "connection or control placement; master semantics unavailable"),
                }
                // Some placements also instantiate a device-specific embedded drawing.
                if let Some(id) = el.attribute("R9") {
                    if let Some(&def) = self.definitions.get(id) {
                        for g in &def.children {
                            self.walk(g, f, depth + 1)?;
                        }
                    } else {
                        self.result.warnings.insert(format!("drawing {id}"));
                    }
                }
            }
            "O37" => {
                // The macro box and its property fields are design-time objects.
                for child in el.children.iter().filter(|c| c.name.starts_with('O')) {
                    self.walk(child, frame, depth + 1)?;
                }
                return Ok(());
            }
            "O26" | "O4" | "MacroVariant" => (),
            tag if tag.starts_with('P') || tag == "S40x1201" || tag == "S61x183" => return Ok(()),
            _ => {
                if let Some(g) = self.graphic(el, f) {
                    if el.attribute("A406") == Some("1") {
                        return Ok(());
                    }
                    let g = self.styled(el, g);
                    self.result.symbol.prims.push(g);
                } else {
                    self.result.warnings.insert(el.name.clone());
                    if el.attribute("A1261").is_some() && el.attribute("A1262").is_some() {
                        self.unresolved(el, f, "unsupported placement record");
                    }
                }
                return Ok(());
            }
        }
        for c in &el.children {
            if c.name == "S53x5" {
                self.property(el, c, f, el.name == "O17" && self.result.symbol.pins.len() > initial_pins);
            } else {
                self.walk(c, f, depth + 1)?;
            }
        }
        Ok(())
    }

    fn property(&mut self, owner: &Element, field: &Element, f: Frame, native_pins: bool) {
        if field.attribute("A406") == Some("1") {
            return;
        }
        let id = field.attribute("A771").unwrap_or("");
        let sub = field.attribute("A772").unwrap_or("0");
        let key = if sub == "0" { format!("P{id}") } else { format!("P{id}_{sub}") };
        let pos = field.attribute("A501").and_then(xy);
        let size = field.child("S54x505").and_then(|s| s.attribute("A961")).and_then(number).filter(|v| *v > 0.).unwrap_or(2.);
        // Native pin labels follow the instance and stay editable after placement.
        if id == "20038" && native_pins && self.result.symbol.show_pin_labels {
            return;
        }
        let stored = owner.child("P11").and_then(|p| p.attribute(&key));
        let source = if stored.is_none() && sub == "0" {
            match id {
                "20038" => owner.attribute("A185"),
                "20039" => owner.attribute("A186"),
                "20406" => Some(plug(owner)),
                _ => None,
            }
        } else {
            None
        };
        if let (Some(pos), Some(value)) = (pos, stored.or(source)) {
            let text = display_text(value);
            let text = if stored.is_none() && matches!(id, "20038" | "20039") { text.split(';').collect::<Vec<_>>().join("\n") } else { text };
            if !text.trim().is_empty() {
                let g = self.styled(field, Prim::Text { pos: f.at(pos), text, size, anchor: Anchor::Left, angle: None, color: None });
                self.result.symbol.prims.push(g);
            }
        } else if id == "20010" && owner.attribute("A1262") == Some("1") && pos.is_some() {
            self.result.symbol.prims.push(Prim::Field {
                kind: crate::symbols::FieldKind::Tag,
                pos: f.at(pos.unwrap_or_default()),
                size,
                anchor: Anchor::Left,
            });
            self.result.symbol.show_tag = false;
        } else {
            // The graphic arrangement can reference a neighbouring field or an instance property.
            self.result.warnings.insert(format!("property {id}/{sub}"));
        }
    }
}

pub fn decode(text: &str) -> Result<Vec<Variant>, String> {
    decode_with_library(text, &[])
}

pub fn decode_with_library(text: &str, mappings: &[UserSymbol]) -> Result<Vec<Variant>, String> {
    decode_source(text, None, mappings)
}

/// Catalog references identify same-named macros in different vendor folders.
pub fn decode_source(text: &str, source: Option<&str>, mappings: &[UserSymbol]) -> Result<Vec<Variant>, String> {
    let root = xml::parse(text.trim_start_matches('\u{feff}'))?;
    if root.name != "EplanPxfRoot" || root.attribute("Type") != Some("WindowMacro") {
        return Err("Expected an EPLAN window macro".into());
    }
    let name = root.attribute("Name").filter(|s| !s.is_empty()).ok_or("Missing EMA name")?;
    let identity = source.filter(|s| !s.is_empty()).map(|s| s.replace('\\', "/").to_lowercase()).unwrap_or_else(|| name.to_owned());
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for v in root.all("MacroVariant") {
        let n = |k| v.attribute(k).and_then(|s| s.parse::<u32>().ok()).ok_or("Invalid EMA variant identifier");
        let (function_type, variant_id) = (n("MacroFuncType")?, n("VariantId")?);
        if !seen.insert((function_type, variant_id)) {
            return Err("Duplicate EMA variant".into());
        }
        let origin = xy(v.attribute("ReferencePoint").ok_or("Missing EMA reference point")?).ok_or("Invalid EMA reference point")?;
        let mut defs = Vec::new();
        v.descendants("O5", &mut defs);
        let func = if function_type == 1 { crate::symbols::Func::Main { mirror: false } } else { crate::symbols::Func::Overview };
        let symbol = UserSymbol {
            key: format!("user:ema:{identity}:{function_type}:{variant_id}"),
            name: format!("{name} ({function_type}/{variant_id})"),
            func,
            library: "EPLAN".into(),
            ..Default::default()
        };
        let mut functions = Vec::new();
        v.descendants("O17", &mut functions);
        let tags: BTreeSet<_> = functions.into_iter().filter_map(device_tag).collect();
        let mut r = Reader {
            definitions: defs.into_iter().filter_map(|e| Some((e.attribute("A1")?, e))).collect(),
            libraries: v.all("ESymLib").filter_map(|e| Some((e.attribute("ID")?, e.attribute("Name")?))).collect(),
            layers: v.all("O76").filter_map(|e| Some((e.attribute("A1426")?, e))).collect(),
            mappings,
            split_devices: tags.len() > 1,
            result: Variant {
                function_type,
                variant_id,
                description: openecraft_parts::language_text(v.attribute("Description").unwrap_or("")),
                symbol,
                devices: Vec::new(),
                warnings: BTreeSet::new(),
                unresolved: Vec::new(),
                connection_markers: Vec::new(),
                transfers: Vec::new(),
                placeholders: Vec::new(),
            },
            budget: 250_000,
            logic: Default::default(),
        };
        if matches!(function_type, 1 | 2 | 4 | 6) {
            r.walk(v, Frame { origin: p(-origin.x, origin.y), x: p(1., 0.), y: p(0., -1.) }, 0)?;
        } else {
            r.result.warnings.insert(format!("representation {function_type}"));
        }
        r.logic.finish(&mut r.result);
        for device in &mut r.result.devices {
            for prim in &mut device.symbol.prims {
                super::translate(prim, -device.position);
            }
            for pin in &mut device.symbol.pins {
                pin.pos = pin.pos - device.position;
            }
            device.symbol.func = func;
            device.symbol.ema_revision = Some(revision(&device.symbol));
        }
        if !r.result.devices.is_empty() && r.result.symbol.pins.is_empty() {
            r.result.symbol.func = crate::symbols::Func::Overview;
            r.result.symbol.show_tag = false;
        }
        r.result.symbol.ema_revision = Some(revision(&r.result.symbol));
        out.push(r.result);
    }
    if out.is_empty() {
        return Err("No EMA variants".into());
    }
    Ok(out)
}

fn revision(symbol: &UserSymbol) -> String {
    format!("{:016x}", super::fingerprint(std::slice::from_ref(symbol)))
}

/// Refresh unedited graphics only when every existing electrical endpoint is preserved.
pub fn install(symbols: &mut Vec<UserSymbol>, variants: &[Variant]) -> usize {
    let mut added = 0;
    for symbol in variants.iter().flat_map(Variant::definitions) {
        if symbol.prims.is_empty() && symbol.pins.is_empty() {
            continue;
        }
        if let Some(old) = symbols.iter_mut().find(|s| s.key == symbol.key) {
            if old.ema_revision != symbol.ema_revision
                && old.ema_revision.as_ref().is_some_and(|r| r == &revision(old))
                && old.pins == symbol.pins
                && old.bridges == symbol.bridges
                && old.func == symbol.func
            {
                *old = symbol.clone();
            }
            continue;
        }
        symbols.push(symbol.clone());
        added += 1;
    }
    added
}

/// Explicit reimport can add newly understood pins to an unedited definition.
/// Existing instances keep their pin overrides and all wire ends stay at the same coordinates.
pub fn import_project(project: &mut crate::model::Project, variants: &[Variant]) -> usize {
    for new in variants.iter().flat_map(Variant::definitions) {
        let Some(old) = project.symbols.iter_mut().find(|s| s.key == new.key) else { continue };
        if new.ema_revision == old.ema_revision
            || old.ema_revision.as_ref().is_none_or(|r| r != &revision(old))
            || old.func != new.func
            || (new.prims.is_empty() && new.pins.is_empty())
        {
            continue;
        }
        let used = project.pages.iter().any(|page| page.symbols.iter().any(|s| s.sym == new.key));
        if used {
            let map: Option<Vec<_>> = old
                .pins
                .iter()
                .map(|p| {
                    let mut hits = new.pins.iter().enumerate().filter(|(_, q)| *q == p);
                    let index = hits.next()?.0;
                    hits.next().is_none().then_some(index)
                })
                .collect();
            let Some(map) = map else { continue };
            if map.iter().collect::<BTreeSet<_>>().len() != map.len()
                || old.bridges.iter().any(|&(a, b)| match (map.get(a), map.get(b)) {
                    (Some(a), Some(b)) => !new.bridges.contains(&(*a, *b)) && !new.bridges.contains(&(*b, *a)),
                    _ => true,
                })
            {
                continue;
            }
            for inst in project.pages.iter_mut().flat_map(|p| &mut p.symbols).filter(|i| i.sym == new.key) {
                let mut labels: Vec<_> = new.pins.iter().map(|p| p.label.clone()).collect();
                for (&index, label) in map.iter().zip(&inst.pins) {
                    labels[index] = label.clone();
                }
                inst.pins = labels;
            }
        }
        *old = new.clone();
    }
    install(&mut project.symbols, variants)
}

/// Resolve a part representation before placement and carry its definition into the project.
pub fn part_symbol(project: &mut crate::model::Project, part: &openecraft_parts::Part, key: Option<&str>) -> Option<(&'static str, Vec<String>)> {
    let mut candidates = part
        .eplan_variants
        .iter()
        .filter(|v| match key {
            Some(k) => v["symbol"]["key"].as_str() == Some(k),
            None => v["function_type"].as_u64() == Some(1),
        })
        .filter(|v| v["devices"].as_array().is_none_or(Vec::is_empty));
    let selected = candidates
        .clone()
        .find(|v| v["symbol"]["pins"].as_array().is_some_and(|pins| !pins.is_empty()))
        .or_else(|| candidates.find(|v| v["symbol"]["prims"].as_array().is_some_and(|g| !g.is_empty())))?;
    let v = Variant::deserialize(selected.as_ref()).ok()?;
    let k = v.symbol.key.clone();
    install(&mut project.symbols, &[v]);
    let def = project.symbol(&k)?;
    Some((def.key, def.pins.iter().map(|p| p.label.to_owned()).collect()))
}

#[cfg(test)]
mod tests;
