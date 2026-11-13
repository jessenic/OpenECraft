//! Dashboard of the parts server: figures and distributions of the parts (per
//! manufacturer, group, catalog, lifecycle, month of the last change, the share with
//! files and models), shown as graphs on the web page (`GET /api/v1/dashboard`).

use crate::portal::{FacetValue, Lifecycle};
use crate::{date_from_unix, Part};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest list of a distribution; the rest is summed up as [`OTHERS`].
pub const MAX_VALUES: usize = 10;
/// Value of the remaining entries of a shortened distribution.
pub const OTHERS: &str = "\u{0}others";
/// Months of the change history.
pub const MONTHS: usize = 12;
/// Parts in the list of the latest changes.
pub const RECENT: usize = 8;

/// How completely the parts are described: parts with each kind of data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Coverage {
    pub description: usize,
    pub price: usize,
    pub dimensions: usize,
    pub model_2d: usize,
    pub model_3d: usize,
    pub files: usize,
    pub datasheet: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dashboard {
    pub parts: usize,
    pub catalogs: usize,
    pub manufacturers: usize,
    pub revision: u64,
    /// Files of the parts (attachments).
    pub files: usize,
    /// Bytes of the files.
    pub file_size: u64,
    /// Parts per manufacturer, most first ([`MAX_VALUES`], then [`OTHERS`]).
    pub by_manufacturer: Vec<FacetValue>,
    /// Parts per top group (`Electrical`, `Fluid`, …).
    pub by_top_group: Vec<FacetValue>,
    pub by_group: Vec<FacetValue>,
    /// Parts per catalog id (empty: without catalog).
    pub by_catalog: Vec<FacetValue>,
    /// Parts per lifecycle (`Active`, `PhaseOut`, `Discontinued`).
    pub by_lifecycle: Vec<FacetValue>,
    /// Parts by the month of their last change (`2026-11`), the last [`MONTHS`]
    /// months, oldest first.
    pub changes: Vec<FacetValue>,
    pub coverage: Coverage,
    /// The parts changed last (without models), newest first.
    pub recent: Vec<Part>,
    /// Users per role (only for administrators).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<FacetValue>,
}

/// The values with their counts, most first (equal counts by name), shortened to
/// [`MAX_VALUES`] with the rest as [`OTHERS`].
pub fn ranked(counts: BTreeMap<String, usize>) -> Vec<FacetValue> {
    let mut list: Vec<FacetValue> = counts.into_iter().map(|(value, count)| FacetValue { value, count }).collect();
    list.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| crate::natural_cmp(&a.value, &b.value)));
    if list.len() > MAX_VALUES {
        let rest: usize = list[MAX_VALUES - 1..].iter().map(|v| v.count).sum();
        list.truncate(MAX_VALUES - 1);
        list.push(FacetValue { value: OTHERS.into(), count: rest });
    }
    list
}

/// The month (`2026-11`) of a Unix time.
pub fn month(secs: u64) -> String {
    date_from_unix(secs)[..7].to_string()
}

/// The [`MONTHS`] months up to the one of `now`, oldest first.
pub fn last_months(now: u64) -> Vec<String> {
    let current = month(now);
    let (mut y, mut m): (i32, i32) = (current[..4].parse().unwrap_or(1970), current[5..].parse().unwrap_or(1));
    let mut out = Vec::new();
    for _ in 0..MONTHS {
        out.push(format!("{y:04}-{m:02}"));
        m -= 1;
        if m == 0 {
            m = 12;
            y -= 1;
        }
    }
    out.reverse();
    out
}

/// The dashboard of `parts` at the time `now` (Unix time).
pub fn dashboard(parts: &[Part], now: u64) -> Dashboard {
    let mut d = Dashboard { parts: parts.len(), ..Default::default() };
    let count = |map: &mut BTreeMap<String, usize>, key: &str| *map.entry(key.trim().to_string()).or_default() += 1;
    let (mut makers, mut tops, mut groups, mut catalogs, mut life) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    for l in Lifecycle::ALL {
        life.insert(format!("{l:?}"), 0);
    }
    let months = last_months(now);
    let mut changes: BTreeMap<String, usize> = months.iter().map(|m| (m.clone(), 0)).collect();
    for p in parts {
        if !p.manufacturer.trim().is_empty() {
            count(&mut makers, &p.manufacturer);
        }
        count(&mut tops, &format!("{:?}", p.top_group));
        if !p.group.trim().is_empty() {
            count(&mut groups, &p.group);
        }
        count(&mut catalogs, &p.catalog);
        count(&mut life, &format!("{:?}", p.lifecycle));
        if p.modified > 0 {
            if let Some(n) = changes.get_mut(&month(p.modified)) {
                *n += 1;
            }
        }
        let c = &mut d.coverage;
        c.description += usize::from(!p.description.trim().is_empty());
        c.price += usize::from(p.price > 0.0);
        c.dimensions += usize::from(p.width > 0.0 && p.height > 0.0);
        c.model_2d += usize::from(p.model_2d.as_ref().is_some_and(|m| !m.prims.is_empty()));
        c.model_3d += usize::from(p.model_3d.as_ref().is_some_and(|m| !m.triangles.is_empty()));
        c.files += usize::from(!p.attachments.is_empty());
        c.datasheet += usize::from(!p.datasheet.trim().is_empty() || p.attachments.iter().any(|a| a.kind == crate::portal::AttachmentKind::Datasheet));
        d.files += p.attachments.len();
        d.file_size += p.attachments.iter().map(|a| a.size).sum::<u64>();
    }
    d.manufacturers = makers.len();
    d.by_manufacturer = ranked(makers);
    d.by_top_group = ranked(tops);
    d.by_group = ranked(groups);
    d.by_catalog = ranked(catalogs);
    d.by_lifecycle = Lifecycle::ALL.iter().map(|l| format!("{l:?}")).map(|k| FacetValue { count: life[&k], value: k }).collect();
    d.changes = months.into_iter().map(|m| FacetValue { count: changes[&m], value: m }).collect();
    let mut recent: Vec<&Part> = parts.iter().filter(|p| p.modified > 0).collect();
    recent.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.revision.cmp(&a.revision)));
    d.recent = recent.into_iter().take(RECENT).map(crate::portal::brief).collect();
    d
}

#[cfg(test)]
mod tests;
