//! Data portal: catalogs, part lifecycle, attachments, users and the faceted part
//! search of the parts server. Shared by the server and its clients, so a search or
//! an update check gives the same result everywhere.

use crate::access::{Access, Grant, Identity, Policy, Right, Subject, SERVER};
use crate::api::PartQuery;
use crate::{natural_cmp, Part, TopGroup};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Delivery state of a part; also read in lower case (`active`, `phase_out`,
/// `discontinued`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Lifecycle {
    #[default]
    #[serde(alias = "active")]
    Active,
    /// Still available, replaced soon (successor known).
    #[serde(alias = "phase_out", alias = "phaseout")]
    PhaseOut,
    #[serde(alias = "discontinued")]
    Discontinued,
}

impl Lifecycle {
    pub const ALL: [Lifecycle; 3] = [Lifecycle::Active, Lifecycle::PhaseOut, Lifecycle::Discontinued];

    pub fn title(self) -> &'static str {
        match self {
            Lifecycle::Active => "Active",
            Lifecycle::PhaseOut => "Phase-out",
            Lifecycle::Discontinued => "Discontinued",
        }
    }

    pub fn is_active(&self) -> bool {
        *self == Lifecycle::Active
    }
}

/// Also read in lower case (`?kind=datasheet`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttachmentKind {
    #[serde(alias = "datasheet")]
    Datasheet,
    #[serde(alias = "image")]
    Image,
    #[serde(alias = "manual")]
    Manual,
    #[serde(alias = "certificate")]
    Certificate,
    #[serde(alias = "drawing")]
    Drawing,
    #[serde(alias = "model3d")]
    Model3d,
    /// Macro library (JSON) for the schematic.
    #[serde(alias = "macro")]
    Macro,
    #[default]
    #[serde(alias = "other")]
    Other,
}

impl AttachmentKind {
    pub const ALL: [AttachmentKind; 8] = [
        AttachmentKind::Datasheet,
        AttachmentKind::Image,
        AttachmentKind::Manual,
        AttachmentKind::Certificate,
        AttachmentKind::Drawing,
        AttachmentKind::Model3d,
        AttachmentKind::Macro,
        AttachmentKind::Other,
    ];

    pub fn title(self) -> &'static str {
        match self {
            AttachmentKind::Datasheet => "Datasheet",
            AttachmentKind::Image => "Image",
            AttachmentKind::Manual => "Manual",
            AttachmentKind::Certificate => "Certificate",
            AttachmentKind::Drawing => "Drawing",
            AttachmentKind::Model3d => "3D model",
            AttachmentKind::Macro => "Macro",
            AttachmentKind::Other => "Other file",
        }
    }

    /// Kind suggested by the file name.
    pub fn guess(name: &str) -> AttachmentKind {
        let lower = name.to_lowercase();
        let ext = lower.rsplit('.').next().unwrap_or("");
        match ext {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => AttachmentKind::Image,
            "svg" | "dxf" | "dwg" => AttachmentKind::Drawing,
            "step" | "stp" | "iges" | "igs" | "x_t" | "x_b" | "xmt_txt" | "xmt_bin" | "stl" | "obj" | "jt" | "ifc" => AttachmentKind::Model3d,
            "json" if lower.contains("macro") => AttachmentKind::Macro,
            "pdf" if lower.contains("manual") || lower.contains("handbuch") => AttachmentKind::Manual,
            "pdf" if lower.contains("cert") || lower.contains("zert") => AttachmentKind::Certificate,
            "pdf" => AttachmentKind::Datasheet,
            _ => AttachmentKind::Other,
        }
    }
}

/// Media type of a file by its extension (for downloads).
pub fn content_type(name: &str) -> &'static str {
    match name.to_lowercase().rsplit('.').next().unwrap_or("") {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "csv" => "text/csv",
        "txt" => "text/plain",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "step" | "stp" => "model/step",
        "x_t" => "model/vnd.parasolid.transmit.text",
        "x_b" => "model/vnd.parasolid.transmit.binary",
        "stl" => "model/stl",
        "obj" => "model/obj",
        _ => "application/octet-stream",
    }
}

/// A file of a part, stored on the parts server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachment {
    /// File id on the server (download: [`crate::api::file_path`]).
    pub id: String,
    /// Original file name.
    pub name: String,
    pub kind: AttachmentKind,
    pub size: u64,
}

/// A manufacturer catalog: a named, versioned set of parts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Catalog {
    /// Key used by the parts ([`Part::catalog`]).
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub version: String,
    pub description: String,
    /// Unix time of the last change of the catalog data (set by the server).
    pub modified: u64,
    /// Number of parts (filled in by the server).
    pub parts: usize,
}

/// What may be done on the parts server. The scope of a right is the manufacturer
/// of the parts and catalogs; reading and administering apply to the whole server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartsRight {
    /// Browse and search parts, catalogs and files; keep favorites.
    Read,
    /// Create and change parts and their files, import parts.
    EditParts,
    DeleteParts,
    /// Create, change and delete catalogs.
    EditCatalogs,
    /// Manage users, roles and grants.
    Administer,
}

impl Right for PartsRight {
    const ALL: &'static [PartsRight] = &[PartsRight::Read, PartsRight::EditParts, PartsRight::DeleteParts, PartsRight::EditCatalogs, PartsRight::Administer];
    const ROLES: &'static [&'static str] = &["reader", "manufacturer", "editor", "admin"];

    fn role(name: &str) -> Option<&'static [PartsRight]> {
        Role::ALL.into_iter().find(|r| r.name().eq_ignore_ascii_case(name.trim())).map(Role::rights)
    }

    fn name(self) -> &'static str {
        match self {
            PartsRight::Read => "read",
            PartsRight::EditParts => "edit_parts",
            PartsRight::DeleteParts => "delete_parts",
            PartsRight::EditCatalogs => "edit_catalogs",
            PartsRight::Administer => "administer",
        }
    }

    fn title(self) -> &'static str {
        match self {
            PartsRight::Read => "Read parts",
            PartsRight::EditParts => "Edit parts",
            PartsRight::DeleteParts => "Delete parts",
            PartsRight::EditCatalogs => "Edit catalogs",
            PartsRight::Administer => "Administer the server",
        }
    }

    fn server_wide(self) -> bool {
        matches!(self, PartsRight::Read | PartsRight::Administer)
    }
}

/// Built-in role of a user account on the parts server. Ordered by permissions.
/// Further rights come from the grants of the server's policy ([`crate::access`]).
/// Also read in lower case, as the project server writes its roles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Browse and keep favorites.
    #[default]
    #[serde(alias = "reader")]
    Reader,
    /// Change the parts and catalogs of the user's own manufacturer.
    #[serde(alias = "manufacturer")]
    Manufacturer,
    /// Change all parts and catalogs.
    #[serde(alias = "editor")]
    Editor,
    /// Also manage the users.
    #[serde(alias = "admin")]
    Admin,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Reader, Role::Manufacturer, Role::Editor, Role::Admin];

    pub fn title(self) -> &'static str {
        match self {
            Role::Reader => "Reader",
            Role::Manufacturer => "Manufacturer",
            Role::Editor => "Editor",
            Role::Admin => "Administrator",
        }
    }

    /// Name in grants.
    pub fn name(self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Manufacturer => "manufacturer",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }

    pub fn rights(self) -> &'static [PartsRight] {
        use PartsRight::*;
        match self {
            Role::Reader => &[Read],
            Role::Manufacturer | Role::Editor => &[Read, EditParts, DeleteParts, EditCatalogs],
            Role::Admin => PartsRight::ALL,
        }
    }
}

/// A user of the parts server (without credentials).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    pub name: String,
    pub role: Role,
    /// Manufacturer whose parts a [`Role::Manufacturer`] user maintains.
    pub manufacturer: String,
    /// Directory the user signs in with; empty for a local account.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// Groups for the grants (directory groups as `directory:group`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

impl User {
    pub fn identity(&self) -> Identity {
        Identity { name: self.name.clone(), provider: self.provider.clone(), groups: self.groups.clone() }
    }

    /// What the built-in role gives: its rights everywhere; a manufacturer user gets
    /// them on its own manufacturer only.
    pub fn grants(&self) -> Vec<Grant> {
        let me = || Subject::User(self.name.clone());
        let reader = || Grant::new(me(), Role::Reader.name(), SERVER);
        match self.role {
            Role::Manufacturer if self.manufacturer.trim().is_empty() => vec![reader()],
            Role::Manufacturer => vec![reader(), Grant::new(me(), Role::Manufacturer.name(), self.manufacturer.trim())],
            role => vec![Grant::new(me(), role.name(), SERVER)],
        }
    }

    /// The rights of the built-in role (without the grants of a policy).
    pub fn access(&self) -> Access<PartsRight> {
        Policy::default().access(&self.identity(), &self.grants())
    }

    /// True if the built-in role allows changing parts of this manufacturer.
    pub fn may_edit(&self, manufacturer: &str) -> bool {
        self.access().allows(manufacturer.trim(), PartsRight::EditParts)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    #[default]
    Created,
    Changed,
    Deleted,
}

impl ChangeKind {
    pub fn title(self) -> &'static str {
        match self {
            ChangeKind::Created => "Created",
            ChangeKind::Changed => "Changed",
            ChangeKind::Deleted => "Deleted",
        }
    }
}

/// One change of a part on the server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub revision: u64,
    /// Unix time.
    pub time: u64,
    pub user: String,
    pub change: ChangeKind,
}

/// Value of a facet with the number of matching parts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FacetValue {
    pub value: String,
    pub count: usize,
}

/// A property the search can be narrowed by, with the values found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Facet {
    /// [`FACETS`] key or the name of a technical attribute.
    pub name: String,
    pub values: Vec<FacetValue>,
}

/// Keys of the fixed facets.
pub const FACETS: [&str; 6] = ["top_group", "group", "subgroup", "manufacturer", "catalog", "lifecycle"];

/// Answer of the part search.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchResult {
    pub revision: u64,
    pub total: usize,
    pub offset: usize,
    pub parts: Vec<Part>,
    /// The fixed facets ([`FACETS`]).
    pub facets: Vec<Facet>,
    /// Technical attributes of the matching parts (most frequent first).
    pub attributes: Vec<Facet>,
}

impl SearchResult {
    pub fn facet(&self, name: &str) -> Option<&Facet> {
        self.facets.iter().chain(&self.attributes).find(|f| f.name == name)
    }
}

const MAX_ATTRIBUTE_FACETS: usize = 12;
const MAX_FACET_VALUES: usize = 50;

/// Technical attributes of a part: voltage, current, power and the `Name: value`
/// lines of the technical data.
pub fn attributes(part: &Part) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = [("Voltage", &part.voltage), ("Current", &part.current), ("Power", &part.power)]
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.to_string(), v.trim().to_string()))
        .collect();
    for line in part.tech_data.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() && !v.is_empty() && !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(k)) {
                out.push((k.to_string(), v.to_string()));
            }
        }
    }
    for key in crate::MOUNTING_KEYS {
        if let Some(v) = part.mounting_data.get(key).filter(|_| !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(key))) {
            out.push((key.to_string(), with_unit(key, v)));
        }
    }
    out
}

/// A number of the mounting data with its unit, as the other technical data shows it.
fn with_unit(key: &str, value: String) -> String {
    let unit = match key {
        "Power loss" => "W",
        "Clearance top" | "Clearance bottom" | "Clearance left" | "Clearance right" | "Clearance front" | "EMC clearance" | "Service space" | "Outside" => "mm",
        _ => return value,
    };
    if value.parse::<f64>().is_ok() {
        format!("{value} {unit}")
    } else {
        value
    }
}

/// Value of a fixed facet of a part.
pub fn facet_value(part: &Part, facet: &str) -> String {
    match facet {
        "top_group" => format!("{:?}", part.top_group),
        "group" => part.group.clone(),
        "subgroup" => part.subgroup.clone(),
        "manufacturer" => part.manufacturer.clone(),
        "catalog" => part.catalog.clone(),
        "lifecycle" => format!("{:?}", part.lifecycle),
        _ => String::new(),
    }
}

fn eq(value: &str, filter: &Option<String>) -> bool {
    filter.as_deref().is_none_or(|f| value.trim().eq_ignore_ascii_case(f.trim()))
}

/// Filter of the query that a facet belongs to (skipped when counting that facet).
#[derive(Clone, Copy, PartialEq)]
enum Skip<'a> {
    None,
    Facet(&'a str),
    Attribute(&'a str),
}

impl PartQuery {
    /// True if the part matches the query. Free text: every word must occur.
    pub fn matches(&self, part: &Part) -> bool {
        self.matches_except(part, Skip::None)
    }

    fn matches_except(&self, p: &Part, skip: Skip) -> bool {
        let text = self.q.as_deref().unwrap_or("");
        if !text.split_whitespace().all(|w| p.matches_text(w)) {
            return false;
        }
        let facet = |name: &str, value: &str, filter: &Option<String>| skip == Skip::Facet(name) || eq(value, filter);
        if !(facet("group", &p.group, &self.group)
            && facet("subgroup", &p.subgroup, &self.subgroup)
            && facet("manufacturer", &p.manufacturer, &self.manufacturer)
            && facet("catalog", &p.catalog, &self.catalog))
        {
            return false;
        }
        if skip != Skip::Facet("top_group") && self.top_group.is_some_and(|t| t != p.top_group) {
            return false;
        }
        if skip != Skip::Facet("lifecycle") && self.lifecycle.is_some_and(|l| l != p.lifecycle) {
            return false;
        }
        if !self.numbers.is_empty() && !self.numbers.contains(&p.number) {
            return false;
        }
        if self.changed_since.is_some_and(|r| p.revision <= r) {
            return false;
        }
        if self.attributes.iter().any(|(k, _)| skip != Skip::Attribute(k)) {
            let attrs = attributes(p);
            for (k, v) in &self.attributes {
                if skip == Skip::Attribute(k) {
                    continue;
                }
                if !attrs.iter().any(|(n, x)| n.eq_ignore_ascii_case(k) && x.eq_ignore_ascii_case(v.trim())) {
                    return false;
                }
            }
        }
        true
    }

    fn sort(&self, hits: &mut [&Part]) {
        match self.sort {
            Sort::Number => hits.sort_by(|a, b| natural_cmp(&a.number, &b.number)),
            Sort::Manufacturer => hits.sort_by(|a, b| a.manufacturer.to_lowercase().cmp(&b.manufacturer.to_lowercase()).then(natural_cmp(&a.number, &b.number))),
            Sort::Description => hits.sort_by(|a, b| a.description.to_lowercase().cmp(&b.description.to_lowercase()).then(natural_cmp(&a.number, &b.number))),
            Sort::Price => hits.sort_by(|a, b| a.price.total_cmp(&b.price).then(natural_cmp(&a.number, &b.number))),
            Sort::Modified => hits.sort_by(|a, b| a.modified.cmp(&b.modified).then(natural_cmp(&a.number, &b.number))),
        }
        if self.descending {
            hits.reverse();
        }
    }

    fn page(&self, hits: Vec<&Part>) -> Vec<Part> {
        hits.into_iter()
            .skip(self.offset.unwrap_or(0))
            .take(self.limit.unwrap_or(usize::MAX))
            .map(|p| if self.brief { brief(p) } else { p.clone() })
            .collect()
    }

    /// Filters, sorts and pages `parts`. Returns the number of matches before paging
    /// and the requested page.
    pub fn apply(&self, parts: &[Part]) -> (usize, Vec<Part>) {
        let mut hits: Vec<&Part> = parts.iter().filter(|p| self.matches(p)).collect();
        self.sort(&mut hits);
        (hits.len(), self.page(hits))
    }

    /// Search with facets. A facet counts the parts that match all other filters, so
    /// its values stay selectable while one of them is chosen.
    pub fn search(&self, parts: &[Part], revision: u64) -> SearchResult {
        let mut hits: Vec<&Part> = parts.iter().filter(|p| self.matches(p)).collect();
        self.sort(&mut hits);
        let total = hits.len();
        let facets = FACETS
            .iter()
            .map(|name| {
                let values = parts.iter().filter(|p| self.matches_except(p, Skip::Facet(name))).map(|p| facet_value(p, name));
                Facet { name: name.to_string(), values: count(values) }
            })
            .collect();
        // Attributes of the hits, most frequent first.
        let mut frequency: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for p in &hits {
            for (k, _) in attributes(p) {
                frequency.entry(k.to_lowercase()).or_insert((k, 0)).1 += 1;
            }
        }
        for (k, _) in &self.attributes {
            frequency.entry(k.to_lowercase()).or_insert((k.clone(), usize::MAX));
        }
        let mut names: Vec<(String, usize)> = frequency.into_values().collect();
        names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let attributes = names
            .into_iter()
            .take(MAX_ATTRIBUTE_FACETS)
            .map(|(name, _)| {
                let values = count(parts.iter().filter(|p| self.matches_except(p, Skip::Attribute(&name))).filter_map(|p| {
                    attributes(p).into_iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)).map(|(_, v)| v)
                }));
                Facet { name, values }
            })
            .collect();
        SearchResult { revision, total, offset: self.offset.unwrap_or(0), parts: self.page(hits), facets, attributes }
    }
}

/// Distinct non-empty values with their counts, in natural order.
fn count(values: impl Iterator<Item = String>) -> Vec<FacetValue> {
    let mut map: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for v in values.filter(|v| !v.trim().is_empty()) {
        map.entry(v.trim().to_lowercase()).or_insert((v.trim().to_string(), 0)).1 += 1;
    }
    let mut out: Vec<FacetValue> = map.into_values().map(|(value, count)| FacetValue { value, count }).collect();
    out.sort_by(|a, b| natural_cmp(&a.value, &b.value));
    out.truncate(MAX_FACET_VALUES);
    out
}

/// Order of the search results; also read in lower case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    #[default]
    #[serde(alias = "number")]
    Number,
    #[serde(alias = "manufacturer")]
    Manufacturer,
    #[serde(alias = "description")]
    Description,
    #[serde(alias = "price")]
    Price,
    #[serde(alias = "modified")]
    Modified,
}

impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Number, Sort::Manufacturer, Sort::Description, Sort::Price, Sort::Modified];

    pub fn title(self) -> &'static str {
        match self {
            Sort::Number => "Part number",
            Sort::Manufacturer => "Manufacturer",
            Sort::Description => "Description",
            Sort::Price => "Price",
            Sort::Modified => "Last change",
        }
    }
}

/// The part without its models (search result lists).
pub fn brief(part: &Part) -> Part {
    Part { model_2d: None, model_3d: None, connection_points: Vec::new(), eplan_variants: Vec::new(), source_data: None, ..part.clone() }
}

/// The part as stored, without the server's bookkeeping (revision, time of change).
pub fn content(part: &Part) -> Part {
    Part { revision: 0, modified: 0, ..part.clone() }
}

/// Why a local part should be updated from the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateKind {
    /// The server has a newer version.
    Changed,
    /// The newer version is being phased out.
    PhaseOut,
    /// The newer version is discontinued.
    Discontinued,
    /// The part came from the server, which no longer has it.
    Removed,
}

impl UpdateKind {
    pub fn title(self) -> &'static str {
        match self {
            UpdateKind::Changed => "Newer version",
            UpdateKind::PhaseOut => "Phase-out",
            UpdateKind::Discontinued => "Discontinued",
            UpdateKind::Removed => "Removed from the server",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartUpdate {
    pub number: String,
    pub kind: UpdateKind,
    /// Server revision of the new version (0 for removed parts).
    pub revision: u64,
    pub successor: String,
}

/// Compares local parts with the server's versions of them (`remote`: the server
/// parts with these numbers). Local parts the server never had are skipped, as are
/// local changes made after the server version.
pub fn check_updates(local: &[Part], remote: &[Part]) -> Vec<PartUpdate> {
    let mut out = Vec::new();
    for l in local {
        match remote.iter().find(|r| r.number == l.number) {
            None if l.revision > 0 => {
                out.push(PartUpdate { number: l.number.clone(), kind: UpdateKind::Removed, revision: 0, successor: String::new() })
            }
            None => {}
            Some(r) => {
                let newer = l.revision == 0 || r.revision > l.revision;
                if newer && content(l) != content(r) {
                    let kind = match r.lifecycle {
                        Lifecycle::Active => UpdateKind::Changed,
                        Lifecycle::PhaseOut => UpdateKind::PhaseOut,
                        Lifecycle::Discontinued => UpdateKind::Discontinued,
                    };
                    out.push(PartUpdate { number: l.number.clone(), kind, revision: r.revision, successor: r.successor.clone() });
                }
            }
        }
    }
    out.sort_by(|a, b| natural_cmp(&a.number, &b.number));
    out
}

/// Replaces the local parts that have an update with the server version. Returns the
/// number of replaced parts; removed parts are kept.
pub fn apply_updates(local: &mut [Part], remote: &[Part], updates: &[PartUpdate]) -> usize {
    let mut n = 0;
    for u in updates.iter().filter(|u| u.kind != UpdateKind::Removed) {
        if let (Some(l), Some(r)) = (local.iter_mut().find(|p| p.number == u.number), remote.iter().find(|p| p.number == u.number)) {
            *l = r.clone();
            n += 1;
        }
    }
    n
}

/// Top group of a facet value (`format!("{:?}")` of [`TopGroup`]).
pub fn top_group_from_key(key: &str) -> Option<TopGroup> {
    TopGroup::ALL.into_iter().find(|t| format!("{t:?}") == key)
}

pub fn lifecycle_from_key(key: &str) -> Option<Lifecycle> {
    Lifecycle::ALL.into_iter().find(|l| format!("{l:?}") == key)
}

/// A catalog id from a name: lower case letters, digits and `_`.
pub fn catalog_id(name: &str) -> String {
    let mut id = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.ends_with('_') && !id.is_empty() {
            id.push('_');
        }
    }
    id.trim_end_matches('_').to_string()
}

#[cfg(test)]
mod tests;
