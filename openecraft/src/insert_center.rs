//! The Insert center (EPLAN's *Insert center*): one place to find symbols, macros,
//! devices (parts with function templates) and unplaced functions by a search with
//! `*` / `?` placeholders, with favorites and the recently used entries first. No UI
//! code: `app/insert_center_ui.rs` draws it and places what is chosen.

use openecraft_core::{
    model::{natural_cmp, Project},
    parts::PartsDb,
    symbols::SymbolRef,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Storage key of the favorites and recently used entries.
pub const STORAGE_KEY: &str = "insert_center";

/// At most this many recently used entries are kept.
pub const RECENT: usize = 20;

pub struct SymbolGroup {
    pub title: String,
    pub entries: Vec<Entry>,
    pub initially_open: bool,
}

struct CatalogKey {
    symbols: openecraft_core::symlib::Symbols,
    parts: Vec<openecraft_core::parts::Part>,
    db: PartsDb,
    query: String,
    lang: openecraft_i18n::Lang,
}

/// Keep sorting, translations and wildcard matching out of scrolling frames.
#[derive(Default)]
pub struct CatalogCache {
    key: Option<CatalogKey>,
    groups: Arc<Vec<SymbolGroup>>,
}

impl CatalogCache {
    pub fn get(&mut self, prj: &Project, db: &PartsDb, center: &InsertCenter) -> Arc<Vec<SymbolGroup>> {
        let symbols = prj.symbol_table();
        let lang = openecraft_i18n::lang();
        if self.key.as_ref().is_none_or(|key| !key.symbols.same_snapshot(&symbols) || key.parts != prj.parts || key.db != *db || key.query != center.query || key.lang != lang) {
            self.groups = Arc::new(symbol_groups(prj, db, center));
            self.key = Some(CatalogKey { symbols, parts: prj.parts.clone(), db: db.clone(), query: center.query.clone(), lang });
        }
        self.groups.clone()
    }
}

fn symbol_groups(prj: &Project, db: &PartsDb, center: &InsertCenter) -> Vec<SymbolGroup> {
    use openecraft_core::symbols::{self, Category};
    use openecraft_i18n::tr;
    let user = listed_user_symbols(prj, db);
    let mut libraries: Vec<&str> = user.iter().map(|d| d.library).collect();
    libraries.dedup();
    let groups = libraries.iter().map(|lib| (
        format!("{}: {lib}", Category::User.title()),
        user.iter().map(|d| &**d).filter(|d| d.library == *lib).collect::<Vec<_>>(),
    )).chain(Category::ALL.into_iter().map(|cat| (
        cat.title().to_string(), symbols::library().iter().filter(|d| d.category == cat).collect(),
    )));
    let searching = !center.query.trim().is_empty();
    groups.filter_map(|(title, defs)| {
        let defs: Vec<_> = defs.into_iter().filter(|d| !searching || center.matches(&[d.name, tr(d.name), d.letter, &title])).collect();
        let first = defs.first()?;
        let initially_open = !matches!(first.category, Category::Nfpa | Category::Fluid | Category::Process | Category::SingleLine | Category::BlackBox | Category::Installation | Category::Gb | Category::Gost | Category::Jis) && defs.len() <= 24;
        Some(SymbolGroup { title, initially_open, entries: defs.iter().map(|d| Entry::Symbol(d.key.to_string())).collect() })
    }).collect()
}

/// Unplaced functions change with project edits, catalog edits or language, not scrolling.
#[derive(Default)]
pub struct UnplacedCache {
    key: Option<(u64, openecraft_i18n::Lang, PartsDb)>,
    entries: Vec<openecraft_core::unplaced::Unplaced>,
}

impl UnplacedCache {
    pub fn get(&mut self, prj: &Project, db: &PartsDb, revision: u64) -> &[openecraft_core::unplaced::Unplaced] {
        let lang = openecraft_i18n::lang();
        if self.key.as_ref().is_none_or(|(rev, language, catalog)| *rev != revision || *language != lang || catalog != db) {
            self.entries = openecraft_core::unplaced::device_functions(prj, Some(db));
            self.entries.extend(openecraft_core::unplaced::plc_channels(prj));
            self.entries.extend(openecraft_core::unplaced::potentials(prj));
            self.key = Some((revision, lang, db.clone()));
        }
        &self.entries
    }
}

/// What an entry inserts; its key (`symbol:coil`, `macro:Motor`, `part:3RT2016`) is
/// stored in the favorites and the recent list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Entry {
    Symbol(String),
    Macro(String),
    Part(String),
}

impl Entry {
    pub fn key(&self) -> String {
        match self {
            Entry::Symbol(k) => format!("symbol:{k}"),
            Entry::Macro(n) => format!("macro:{n}"),
            Entry::Part(n) => format!("part:{n}"),
        }
    }

    pub fn from_key(key: &str) -> Option<Entry> {
        let (kind, name) = key.split_once(':')?;
        if name.is_empty() {
            return None;
        }
        Some(match kind {
            "symbol" => Entry::Symbol(name.into()),
            "macro" => Entry::Macro(name.into()),
            "part" => Entry::Part(name.into()),
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InsertCenter {
    pub favorites: Vec<String>,
    /// Newest first.
    pub recent: Vec<String>,
    /// Tiles instead of a list.
    pub tiles: bool,
    #[serde(skip)]
    pub query: String,
    #[serde(skip)]
    pub variant_part: Option<String>,
}

impl InsertCenter {
    /// An entry was inserted: it moves to the front of the recent list.
    pub fn used(&mut self, e: &Entry) {
        let k = e.key();
        self.recent.retain(|r| *r != k);
        self.recent.insert(0, k);
        self.recent.truncate(RECENT);
    }

    pub fn is_favorite(&self, e: &Entry) -> bool {
        self.favorites.contains(&e.key())
    }

    pub fn toggle_favorite(&mut self, e: &Entry) {
        let k = e.key();
        if self.favorites.contains(&k) {
            self.favorites.retain(|f| *f != k);
        } else {
            self.favorites.push(k);
        }
    }

    /// Whether the texts of an entry match the search (all words, `*` / `?` placeholders).
    pub fn matches(&self, texts: &[&str]) -> bool {
        let all = texts.join(" ");
        self.query.split_whitespace().all(|w| crate::commands::wildcard_match(w, &all))
    }

    /// The favorites, then the recent entries, as entries (unknown keys skipped).
    pub fn favorites(&self) -> Vec<Entry> {
        self.favorites.iter().filter_map(|k| Entry::from_key(k)).collect()
    }

    pub fn recent(&self) -> Vec<Entry> {
        self.recent.iter().filter_map(|k| Entry::from_key(k)).collect()
    }

    /// Favorites as JSON (exported and imported in the Insert center).
    pub fn favorites_json(&self) -> String {
        serde_json::to_string_pretty(&self.favorites).unwrap_or_default()
    }

    /// Adds the favorites of an exported file; returns how many were new.
    pub fn import_favorites(&mut self, text: &str) -> Result<usize, String> {
        let list: Vec<String> = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut n = 0;
        for k in list.into_iter().filter(|k| Entry::from_key(k).is_some()) {
            if !self.favorites.contains(&k) {
                self.favorites.push(k);
                n += 1;
            }
        }
        Ok(n)
    }
}

/// Registered symbols of the project symbol library, sorted by library and name.
pub fn listed_user_symbols(prj: &Project, db: &PartsDb) -> Vec<SymbolRef> {
    // Place catalog cards from their single device entry. Loose EMA symbols remain available.
    let represented: std::collections::HashSet<&str> = prj
        .parts
        .iter()
        .chain(&db.parts)
        .filter(|p| p.eplan_variants.iter().any(|v| !v["symbol"]["plc_channel"].is_null()))
        .flat_map(|p| &p.eplan_variants)
        .filter_map(|v| v["symbol"]["key"].as_str())
        .collect();
    let mut families = std::collections::BTreeMap::new();
    for sym in prj.symbols.iter().filter(|s| !represented.contains(s.key.as_str())) {
        if let Some(c) = &sym.plc_channel {
            let first = families.entry(c.family.as_str()).or_insert((c.index, &sym.key));
            if c.index < first.0 {
                *first = (c.index, &sym.key);
            }
        }
    }
    let mut v: Vec<SymbolRef> = prj
        .symbols
        .iter()
        .filter(|s| {
            if represented.contains(s.key.as_str()) {
                return false;
            }
            let family = s.key.strip_prefix("user:ema:").and_then(|k| k.rsplit_once(':')).and_then(|(k, _)| k.rsplit_once(':')).map(|(f, _)| f);
            family.and_then(|f| families.get(f)).is_none_or(|(_, first)| *first == &s.key)
        })
        .filter_map(|u| prj.symbol(&u.key))
        .collect();
    v.sort_by(|a, b| a.library.cmp(b.library).then(natural_cmp(a.name, b.name)));
    v
}

#[cfg(test)]
mod tests;
