//! Default SPECIAL contact forms measured on pp. 1–2 of EPLAN's 2007 IEC symbol
//! overview. The EK1100 reference fixes the insertion point at the ring centre.
use super::*;

/// Variants E–H mirror A–D about the local Y axis.
fn axis(orientation: &str) -> Option<P> {
    Some(match orientation {
        "0" | "4" => p(0., 1.),
        "1" | "7" => p(-1., 0.),
        "2" | "6" => p(0., -1.),
        "3" | "5" => p(1., 0.),
        _ => return None,
    })
}

pub(super) fn decode(el: &Element, f: Frame, id: &str, orientation: &str, target: &mut UserSymbol) -> bool {
    let Some(axis) = axis(orientation) else { return false };
    let bus_plug = matches!(id, "34" | "37");
    let dual = id == "22";
    if !matches!(id, "13" | "17" | "20" | "34" | "35" | "37" | "22" | "40") {
        return false;
    }
    let raw = el.attribute("A185").unwrap_or("");
    let mut labels: Vec<_> = raw.split(';').map(str::trim).collect();
    let logic = el.child("S61x183");
    let overrides: Vec<_> = logic.into_iter().flat_map(|s| s.all("S16x1062")).collect();
    let has_pins = if dual {
        // The two ends share one designation. Do not invent an ordering if the
        // export supplies conflicting names.
        let common = labels.iter().copied().find(|s| !s.is_empty()).unwrap_or("");
        let valid = labels.len() == 2 && labels.iter().all(|s| s.is_empty() || *s == common) && (!common.is_empty() || overrides.len() == 2);
        if valid {
            labels.fill(common);
        }
        valid
    } else {
        labels.len() == 1
            && overrides.len() <= 1
            && logic.is_some_and(|s| {
                s.attribute("A1061") == Some("1")
                    || s.attribute("A1063") == Some("1")
                    || overrides.first().is_some_and(|p| p.attribute("A161") == Some("1"))
            })
    };
    let color = Some([255, 0, 255]);
    let outer = if id == "40" { 8. } else { 4. };
    let at = |along: f32, side: f32| f.at(axis * along + p(axis.y, -axis.x) * side);
    let stroke = |pts: Vec<P>, closed, fill| Prim::Path { pts, closed, fill, width: 0.25, color, dashed: false };
    target.prims.push(Prim::Circle { c: f.origin, r: 1., fill: false, color });
    if bus_plug {
        // Ring, short stem, plug and semicircular socket (table forms 34 and 37).
        target.prims.push(stroke(vec![at(1., 0.), at(1.5, 0.)], false, false));
        target.prims.push(stroke(vec![at(1.5, -0.5), at(1.5, 0.5), at(3., 0.5), at(3., -0.5)], true, true));
        let arc = (0..=16)
            .map(|i| {
                let (s, c) = (std::f32::consts::PI * i as f32 / 16.).sin_cos();
                at(3. + s, c)
            })
            .collect();
        target.prims.push(stroke(arc, false, false));
    } else {
        target.prims.push(stroke(vec![at(1., 0.), at(outer, 0.)], false, false));
    }
    if dual {
        target.prims.push(stroke(vec![at(-1., 0.), at(-2., 0.)], false, false));
    }
    if !has_pins {
        return false;
    }
    let connector = plug(el);
    for (i, label) in labels.iter().enumerate() {
        let label = if connector.is_empty() {
            (*label).to_owned()
        } else if label.is_empty() && id == "40" {
            connector.to_owned()
        } else if label.is_empty() {
            String::new()
        } else {
            format!("{connector}.{label}")
        };
        let along = if i == 0 { outer } else { -2. };
        target.pins.push(UserPin { pos: at(along, 0.), label, internal: false });
    }
    if !el.all("S53x5").any(|field| field.attribute("A771") == Some("20039") && field.attribute("A501").is_some()) {
        if let Some(text) = el.attribute("A186").filter(|s| !s.contains(';') && !s.is_empty()) {
            target.prims.push(Prim::Text {
                pos: at(-1.5, 0.),
                text: text.into(),
                size: 1.5,
                anchor: Anchor::Center,
                color: Some([0, 0, 255]),
                angle: None,
            });
        }
    }
    true
}
