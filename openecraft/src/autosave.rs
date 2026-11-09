//! The autosaved project and parts database: kept in the app's [`Store`] (IndexedDB in
//! the browser, whose local storage is too small for large projects), read back
//! asynchronously at start-up. No UI code; `App` calls it.

use crate::session::Session;
use openecraft_parts::PartsDb;
use openecraft_ui::io::Store;

const PROJECT: &str = "project";
const PARTS: &str = "parts_db";

pub struct Autosave {
    store: Store,
    /// The stored project is still expected: it replaces the start-up project unless
    /// that was changed or replaced meanwhile.
    awaiting_project: bool,
    /// The stored parts database is still expected; the start-up one, which it replaces
    /// unless the parts were changed meanwhile.
    awaiting_parts: Option<PartsDb>,
    /// The write of the last `save` on another thread.
    writing: Option<std::thread::JoinHandle<()>>,
}

impl Autosave {
    /// Transfers the outstanding write to the closing worker; never joins on the UI.
    pub fn closing(&mut self) -> (Option<std::thread::JoinHandle<()>>, Store, bool) {
        (self.writing.take(), self.store.clone(), !self.awaiting_project && self.awaiting_parts.is_none())
    }

    /// Starts reading the stored project and parts database (`parts`: the database the
    /// app starts with).
    pub fn start(store: Store, ctx: &egui::Context, parts: &PartsDb) -> Autosave {
        store.load(ctx, PROJECT);
        store.load(ctx, PARTS);
        Autosave { store, awaiting_project: true, awaiting_parts: Some(parts.clone()), writing: None }
    }

    /// Takes values that finished loading. Returns true if the project was replaced.
    pub fn poll(&mut self, session: &mut Session, parts: &mut PartsDb) -> bool {
        let mut replaced = false;
        let waiting = self.awaiting_project || self.awaiting_parts.is_some();
        while let Some((key, value)) = self.store.poll() {
            match key.as_str() {
                PROJECT => {
                    let waited = std::mem::take(&mut self.awaiting_project);
                    let prj = value.and_then(|v| openecraft_core::archive::unpack(&String::from_utf8_lossy(&v)).ok());
                    if let Some(prj) = prj.filter(|_| waited && session.untouched()) {
                        // Unsaved changes at the last close stay unsaved.
                        let unsaved = session.dirty();
                        *session = Session::new(prj, session.file_name.clone());
                        if unsaved {
                            session.mark_unsaved();
                        }
                        replaced = true;
                    }
                }
                PARTS => {
                    let start = self.awaiting_parts.take();
                    let db = value.and_then(|v| serde_json::from_slice::<PartsDb>(&v).ok());
                    if let Some(db) = db.filter(|_| start.as_ref() == Some(parts)) {
                        *parts = db;
                    }
                }
                _ => {}
            }
        }
        if waiting && !self.awaiting_project && self.awaiting_parts.is_none() {
            openecraft_core::parts::link_unassigned_plc_parts(&mut session.prj, parts, None);
        }
        replaced
    }

    /// Writes the project and the parts database (not before the stored ones arrived,
    /// which would be overwritten by the start-up values). Returns whether the store
    /// has worked so far: until it does, the caller keeps both in eframe's storage too.
    /// Once it works, they are packed and written on another thread (after the write
    /// before has finished), so that the editor does not wait for it.
    pub fn save(&mut self, session: &Session, parts: &PartsDb) -> bool {
        let project = (!self.awaiting_project).then(|| session.prj.clone());
        let parts = self.awaiting_parts.is_none().then(|| parts.clone());
        let store = self.store.clone();
        let write = move || {
            if let Some(prj) = project {
                store.store(PROJECT, openecraft_core::archive::pack(&prj).into_bytes());
            }
            if let Some(parts) = parts {
                store.store(PARTS, serde_json::to_vec(&parts).unwrap_or_default());
            }
        };
        self.finish();
        if openecraft_core::analysis::threads() < 2 || !self.store.works() {
            write();
        } else {
            self.writing = Some(std::thread::spawn(write));
        }
        !self.awaiting_project && self.awaiting_parts.is_none() && self.store.works()
    }

    /// Waits for the write on another thread (before the app ends).
    pub fn finish(&mut self) {
        if let Some(w) = self.writing.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod tests;
