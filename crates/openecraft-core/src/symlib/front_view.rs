use super::{Prim, UserSymbol};
use crate::geom::{p, P, R};
use openecraft_parts::{Model2d, Part, Prim2d, Shared};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

struct Cached {
    source: Shared<serde_json::Value>,
    size: [f32; 2],
    model: Option<Shared<Model2d>>,
}

/// The schematic and cabinet can use the same source representation. Keep the
/// converted artwork outside the saved project, and invalidate it on reimport.
#[derive(Clone, Default)]
pub(super) struct FrontCache(Arc<Mutex<HashMap<(String, String), Cached>>>);

impl FrontCache {
    pub(super) fn get(&self, part: &Part) -> Option<Shared<Model2d>> {
        let [w, h, _] = part.size()?;
        for kind in [6, 4] {
            for v in part.eplan_variants.iter().filter(|v| v["function_type"].as_u64() == Some(kind)) {
                let Some(key) = v["symbol"]["key"].as_str() else { continue };
                let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
                let id = (part.number.clone(), key.to_owned());
                if let Some(hit) = cache.get(&id).filter(|c| c.source.ptr_eq(v) && c.size == [w, h]) {
                    if hit.model.is_some() {
                        return hit.model.clone();
                    }
                    continue;
                }
                let model = UserSymbol::deserialize(&v["symbol"]).ok().and_then(|sym| fit(&sym, w, h)).map(Shared::new);
                cache.insert(id, Cached { source: v.clone(), size: [w, h], model: model.clone() });
                if model.is_some() {
                    return model;
                }
            }
        }
        None
    }
}

/// Fit the visible drawing inside the physical dimensions, ignoring pins and
/// property fields. The macro insertion point and drawing scale vary by vendor.
fn fit(symbol: &UserSymbol, width: f32, height: f32) -> Option<Model2d> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return None;
    }
    let mut bounds = None::<R>;
    let mut include = |q: P| {
        let r = bounds.get_or_insert(R::from_points(q, q));
        r.min = p(r.min.x.min(q.x), r.min.y.min(q.y));
        r.max = p(r.max.x.max(q.x), r.max.y.max(q.y));
    };
    for g in &symbol.prims {
        match g {
            Prim::Image { corners, .. } => corners.iter().copied().for_each(&mut include),
            Prim::Line { a, b, .. } => {
                include(*a);
                include(*b);
            }
            Prim::Path { pts, .. } | Prim::Poly { pts, .. } => pts.iter().copied().for_each(&mut include),
            Prim::Circle { c, r, .. } => {
                include(*c - p(*r, *r));
                include(*c + p(*r, *r));
            }
            Prim::Text { .. } | Prim::Field { .. } | Prim::PropertyBlock(_) => {}
        }
    }
    let r = bounds?;
    if r.width() <= 0. || r.height() <= 0. {
        return None;
    }
    let scale = (width / r.width()).min(height / r.height());
    let margin = p(width - r.width() * scale, height - r.height() * scale) * 0.5;
    let point = |q: P| {
        let v = (q - r.min) * scale + margin;
        [v.x, v.y]
    };
    let prims = symbol
        .prims
        .iter()
        .filter_map(|g| {
            Some(match g {
                Prim::Line { a, b, .. } => Prim2d::Line { a: point(*a), b: point(*b) },
                Prim::Poly { pts, closed, fill } | Prim::Path { pts, closed, fill, .. } => {
                    Prim2d::Polyline { pts: pts.iter().copied().map(point).collect(), closed: *closed, fill: *fill }
                }
                Prim::Circle { c, r, fill, .. } => Prim2d::Circle { c: point(*c), r: r * scale, fill: *fill },
                Prim::Text { pos, text, size, .. } if r.contains(*pos) => Prim2d::Text { pos: point(*pos), text: text.clone(), size: size * scale },
                _ => return None,
            })
        })
        .collect();
    Some(Model2d { width, height, prims })
}

#[cfg(test)]
mod tests;
