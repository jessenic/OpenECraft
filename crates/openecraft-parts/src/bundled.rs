//! Manufacturer catalogs shipped with OpenECraft (`catalogs/<id>.json`, the logo in
//! `catalogs/logos/<id>.svg`): installed into new databases of the parts server and
//! with `--install-catalogs`; the example projects take their manufacturer parts from
//! them.

use crate::portal::Catalog;
use crate::{add_generated_models, Part};
use serde::Deserialize;

/// Catalog id and file content, one entry per file in `catalogs/`.
const FILES: &[(&str, &str)] = &[
    ("abb", include_str!("../catalogs/abb.json")),
    ("beckhoff", include_str!("../catalogs/beckhoff.json")),
    ("cosmo", include_str!("../catalogs/cosmo.json")),
    ("dehn", include_str!("../catalogs/dehn.json")),
    ("gira", include_str!("../catalogs/gira.json")),
    ("hager", include_str!("../catalogs/hager.json")),
    ("janitza", include_str!("../catalogs/janitza.json")),
    ("jinko", include_str!("../catalogs/jinko.json")),
    ("longi", include_str!("../catalogs/longi.json")),
    ("loxone", include_str!("../catalogs/loxone.json")),
    ("mdt", include_str!("../catalogs/mdt.json")),
    ("phoenix_contact", include_str!("../catalogs/phoenix_contact.json")),
    ("pylontech", include_str!("../catalogs/pylontech.json")),
    ("siemens", include_str!("../catalogs/siemens.json")),
    ("victron", include_str!("../catalogs/victron.json")),
    ("weidmueller", include_str!("../catalogs/weidmueller.json")),
];

/// The logos of the bundled catalogs: the companies' logos from Wikimedia Commons, or
/// neutral badges with the name where it has no freely licensed one (sources and
/// licences: `catalogs/logos/README.md`). Administrators replace them on their server.
const LOGOS: &[(&str, &str)] = &[
    ("abb", include_str!("../catalogs/logos/abb.svg")),
    ("beckhoff", include_str!("../catalogs/logos/beckhoff.svg")),
    ("cosmo", include_str!("../catalogs/logos/cosmo.svg")),
    ("dehn", include_str!("../catalogs/logos/dehn.svg")),
    ("gira", include_str!("../catalogs/logos/gira.svg")),
    ("hager", include_str!("../catalogs/logos/hager.svg")),
    ("janitza", include_str!("../catalogs/logos/janitza.svg")),
    ("jinko", include_str!("../catalogs/logos/jinko.svg")),
    ("longi", include_str!("../catalogs/logos/longi.svg")),
    ("loxone", include_str!("../catalogs/logos/loxone.svg")),
    ("mdt", include_str!("../catalogs/logos/mdt.svg")),
    ("phoenix_contact", include_str!("../catalogs/logos/phoenix_contact.svg")),
    ("pylontech", include_str!("../catalogs/logos/pylontech.svg")),
    ("siemens", include_str!("../catalogs/logos/siemens.svg")),
    ("victron", include_str!("../catalogs/logos/victron.svg")),
    ("weidmueller", include_str!("../catalogs/logos/weidmueller.svg")),
];

/// A catalog with its parts.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Bundle {
    pub catalog: Catalog,
    pub parts: Vec<Part>,
    /// The logo (SVG) the server gives the catalog while it has none.
    #[serde(skip)]
    pub logo: Option<&'static str>,
}

/// The logo of a bundled catalog (SVG).
pub fn logo(id: &str) -> Option<&'static str> {
    LOGOS.iter().find(|(i, _)| i.eq_ignore_ascii_case(id)).map(|(_, svg)| *svg)
}

pub fn ids() -> Vec<&'static str> {
    FILES.iter().map(|(id, _)| *id).collect()
}

/// A bundled catalog; its parts refer to it and get generated models where they have none.
pub fn load(id: &str) -> Result<Bundle, String> {
    let (_, text) = FILES.iter().find(|(i, _)| i.eq_ignore_ascii_case(id)).ok_or_else(|| format!("unknown catalog {id} (bundled: {})", ids().join(", ")))?;
    let mut bundle: Bundle = serde_json::from_str(text).map_err(|e| format!("catalog {id}: {e}"))?;
    for p in &mut bundle.parts {
        p.catalog = bundle.catalog.id.clone();
        p.revision = 0;
        p.modified = 0;
        add_generated_models(p);
    }
    bundle.catalog.logo = None;
    bundle.logo = logo(id);
    Ok(bundle)
}

pub fn all() -> Result<Vec<Bundle>, String> {
    ids().into_iter().map(load).collect()
}

/// The generic catalog and the parts of all bundled catalogs: the data a new parts
/// server starts with.
pub fn default_parts() -> Vec<Part> {
    let mut parts = crate::default_catalog();
    parts.extend(all().unwrap_or_default().into_iter().flat_map(|b| b.parts));
    parts
}

#[cfg(test)]
mod tests;
