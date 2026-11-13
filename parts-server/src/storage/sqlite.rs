//! SQLite storage: one database file with indexed parts (models stored apart from
//! the part data, so searches do not load them), history, accounts and files.

use super::{join_models, search_text, split_models, Backend, Change, Snapshot, StoredFile, HISTORY_LIMIT};
use crate::auth::Account;
use openecraft_parts::portal::{Catalog, HistoryEntry};
use openecraft_parts::Part;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS parts (
    number TEXT PRIMARY KEY,
    search TEXT NOT NULL,
    catalog TEXT NOT NULL,
    data TEXT NOT NULL,
    models TEXT
);
CREATE INDEX IF NOT EXISTS parts_catalog ON parts(catalog);
CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    number TEXT NOT NULL,
    entry TEXT NOT NULL,
    part TEXT
);
CREATE INDEX IF NOT EXISTS history_number ON history(number, id);
CREATE TABLE IF NOT EXISTS catalogs (id TEXT PRIMARY KEY COLLATE NOCASE, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS accounts (name TEXT PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS favorites (user TEXT PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS libraries (name TEXT PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files (id TEXT PRIMARY KEY, name TEXT NOT NULL, data BLOB NOT NULL);
";

pub struct SqliteBackend {
    conn: Connection,
    revision: u64,
}

fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("database: {e}"))
}

fn json<T: serde::Serialize>(v: &T) -> io::Result<String> {
    serde_json::to_string(v).map_err(err)
}

fn parse<T: DeserializeOwned>(s: &str) -> io::Result<T> {
    serde_json::from_str(s).map_err(err)
}

/// `LIKE` pattern that matches the text literally.
fn like(word: &str) -> String {
    let escaped = word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

impl SqliteBackend {
    pub fn open(path: &Path) -> io::Result<SqliteBackend> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        SqliteBackend::init(Connection::open(path).map_err(err)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> io::Result<SqliteBackend> {
        SqliteBackend::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> io::Result<SqliteBackend> {
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(())).map_err(err)?;
        conn.execute_batch(SCHEMA).map_err(err)?;
        let revision: Option<String> = conn.query_row("SELECT value FROM meta WHERE key = 'revision'", [], |r| r.get(0)).optional().map_err(err)?;
        let revision = revision.and_then(|r| r.parse().ok()).unwrap_or(0);
        Ok(SqliteBackend { conn, revision })
    }

    fn rows<T: DeserializeOwned>(&self, sql: &str, args: &[&dyn rusqlite::ToSql]) -> io::Result<Vec<T>> {
        let mut stmt = self.conn.prepare(sql).map_err(err)?;
        let texts: Vec<String> = stmt.query_map(args, |r| r.get(0)).map_err(err)?.collect::<Result<_, _>>().map_err(err)?;
        texts.iter().map(|t| parse(t)).collect()
    }
}

impl Backend for SqliteBackend {
    fn kind(&self) -> &'static str {
        "sqlite"
    }

    fn revision(&self) -> u64 {
        self.revision
    }

    fn count(&self) -> io::Result<usize> {
        self.conn.query_row("SELECT COUNT(*) FROM parts", [], |r| r.get::<_, i64>(0)).map(|n| n as usize).map_err(err)
    }

    fn candidates(&self, words: &[String], numbers: &[String], brief: bool) -> io::Result<Vec<Part>> {
        let mut sql = format!("SELECT data, {} FROM parts WHERE 1 = 1", if brief { "NULL" } else { "models" });
        let mut args: Vec<String> = Vec::new();
        for w in words {
            sql.push_str(" AND search LIKE ? ESCAPE '\\'");
            args.push(like(w));
        }
        if !numbers.is_empty() {
            sql.push_str(&format!(" AND number IN ({})", vec!["?"; numbers.len()].join(", ")));
            args.extend(numbers.iter().cloned());
        }
        let mut stmt = self.conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map(params_from_iter(args.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        rows.into_iter().map(|(data, models)| Ok(join_models(parse(&data)?, models.as_deref()))).collect()
    }

    fn part(&self, number: &str) -> io::Result<Option<Part>> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row("SELECT data, models FROM parts WHERE number = ?", [number], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(err)?;
        row.map(|(data, models)| Ok(join_models(parse(&data)?, models.as_deref()))).transpose()
    }

    fn catalog_counts(&self) -> io::Result<HashMap<String, usize>> {
        let mut stmt = self.conn.prepare("SELECT lower(catalog), COUNT(*) FROM parts WHERE catalog <> '' GROUP BY lower(catalog)").map_err(err)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))).map_err(err)?;
        rows.collect::<Result<HashMap<_, _>, _>>().map_err(err)
    }

    fn catalogs(&self) -> io::Result<Vec<Catalog>> {
        self.rows("SELECT data FROM catalogs", &[])
    }

    fn accounts(&self) -> io::Result<Vec<Account>> {
        self.rows("SELECT data FROM accounts ORDER BY name", &[])
    }

    fn history(&self, number: &str) -> io::Result<Vec<Snapshot>> {
        let mut stmt = self.conn.prepare("SELECT entry, part FROM history WHERE number = ? ORDER BY id DESC").map_err(err)?;
        let rows = stmt
            .query_map([number], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        rows.into_iter()
            .map(|(entry, part)| {
                let entry: HistoryEntry = parse(&entry)?;
                let part = part.map(|p| parse(&p)).transpose()?;
                Ok(Snapshot { number: number.into(), entry, part })
            })
            .collect()
    }

    fn favorites(&self, user: &str) -> io::Result<Vec<String>> {
        Ok(self.rows::<Vec<String>>("SELECT data FROM favorites WHERE user = ?", &[&user])?.pop().unwrap_or_default())
    }

    fn library(&self, name: &str) -> io::Result<Option<String>> {
        self.conn.query_row("SELECT data FROM libraries WHERE name = ?", [name], |r| r.get(0)).optional().map_err(err)
    }

    fn file(&self, id: &str) -> io::Result<Option<StoredFile>> {
        self.conn
            .query_row("SELECT name, data FROM files WHERE id = ?", [id], |r| Ok(StoredFile { id: id.into(), name: r.get(0)?, data: r.get(1)? }))
            .optional()
            .map_err(err)
    }

    fn file_ids(&self) -> io::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM files").map_err(err)?;
        let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?.collect::<Result<Vec<String>, _>>();
        ids.map_err(err)
    }

    /// The attachments are part of the stored part data, so the (content hash) id is
    /// looked up as text in the parts and the history.
    fn unused_files(&self, ids: &[String]) -> io::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT EXISTS (SELECT 1 FROM parts WHERE instr(data, ?1) > 0) OR EXISTS (SELECT 1 FROM history WHERE instr(part, ?1) > 0)")
            .map_err(err)?;
        let mut out = Vec::new();
        for id in ids {
            let used: bool = stmt.query_row([id], |r| r.get(0)).map_err(err)?;
            if !used {
                out.push(id.clone());
            }
        }
        Ok(out)
    }

    fn commit(&mut self, change: Change) -> io::Result<()> {
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("INSERT OR REPLACE INTO meta (key, value) VALUES ('revision', ?)", [change.revision.to_string()]).map_err(err)?;
        for p in &change.parts {
            let (data, models) = split_models(p);
            let models = models.map(|m| json(&m)).transpose()?;
            tx.execute(
                "INSERT OR REPLACE INTO parts (number, search, catalog, data, models) VALUES (?, ?, ?, ?, ?)",
                params![p.number, search_text(p), p.catalog, json(&data)?, models],
            )
            .map_err(err)?;
        }
        for n in &change.deleted {
            tx.execute("DELETE FROM parts WHERE number = ?", [n]).map_err(err)?;
        }
        for s in &change.history {
            let part = s.part.as_ref().map(json).transpose()?;
            tx.execute("INSERT INTO history (number, entry, part) VALUES (?, ?, ?)", params![s.number, json(&s.entry)?, part]).map_err(err)?;
            tx.execute(
                "DELETE FROM history WHERE number = ?1 AND id NOT IN (SELECT id FROM history WHERE number = ?1 ORDER BY id DESC LIMIT ?2)",
                params![s.number, HISTORY_LIMIT as i64],
            )
            .map_err(err)?;
        }
        for c in &change.catalogs {
            tx.execute("INSERT OR REPLACE INTO catalogs (id, data) VALUES (?, ?)", params![c.id, json(c)?]).map_err(err)?;
        }
        for id in &change.deleted_catalogs {
            tx.execute("DELETE FROM catalogs WHERE id = ?", [id]).map_err(err)?;
        }
        for a in &change.accounts {
            tx.execute("INSERT OR REPLACE INTO accounts (name, data) VALUES (?, ?)", params![a.user.name, json(a)?]).map_err(err)?;
        }
        for name in &change.deleted_accounts {
            tx.execute("DELETE FROM accounts WHERE name = ?", [name]).map_err(err)?;
            tx.execute("DELETE FROM favorites WHERE user = ?", [name]).map_err(err)?;
        }
        for (user, numbers) in &change.favorites {
            tx.execute("INSERT OR REPLACE INTO favorites (user, data) VALUES (?, ?)", params![user, json(numbers)?]).map_err(err)?;
        }
        for (name, data) in &change.libraries {
            tx.execute("INSERT OR REPLACE INTO libraries (name, data) VALUES (?, ?)", params![name, data]).map_err(err)?;
        }
        for f in &change.files {
            tx.execute("INSERT OR IGNORE INTO files (id, name, data) VALUES (?, ?, ?)", params![f.id, f.name, f.data]).map_err(err)?;
        }
        for id in &change.deleted_files {
            tx.execute("DELETE FROM files WHERE id = ?", [id]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        self.revision = change.revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
