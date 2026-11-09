//! Prepare the last autosave without blocking the closing window.

use crate::{autosave::Autosave, jobs::Job, session::Session};
use openecraft_core::macros::MacroLibrary;
use openecraft_i18n::tr_const;
use openecraft_parts::PartsDb;
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

pub const STEPS: [&str; 4] = [
    tr_const!("Waiting for the previous autosave"),
    tr_const!("Saving the recovery project"),
    tr_const!("Saving the parts database"),
    tr_const!("Preparing the macro library"),
];

pub struct Shutdown {
    job: Job<Prepared>,
    step: Arc<AtomicUsize>,
    pub prepared: Option<Prepared>,
    pub error: Option<String>,
}

impl Shutdown {
    pub fn start(autosave: &mut Autosave, session: &Session, parts: &PartsDb, macros: &MacroLibrary, ctx: &egui::Context) -> Self {
        let step = Arc::new(AtomicUsize::new(0));
        let progress = step.clone();
        let (previous, store, ready) = autosave.closing();
        let (project, parts, macros) = (session.prj.clone(), parts.clone(), macros.clone());
        let job = Job::start(String::new(), ctx, move || {
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            progress.store(1, Ordering::Relaxed);
            let mut prepared = Prepared::default();
            let packed = openecraft_core::archive::pack(&project);
            if ready {
                store.store("project", packed.as_bytes().to_vec());
            }
            let project_ok = ready && store.works();
            prepared.set_string("project_packed", if project_ok { String::new() } else { packed });
            progress.store(2, Ordering::Relaxed);
            if ready {
                store.store("parts_db", serde_json::to_vec(&parts).expect("serializable parts database"));
            }
            if ready && store.works() {
                prepared.set_string("parts_db", String::new());
            } else {
                eframe::set_value(&mut prepared, "parts_db", &parts);
            }
            progress.store(3, Ordering::Relaxed);
            eframe::set_value(&mut prepared, "macros", &macros);
            prepared
        });
        Self { job, step, prepared: None, error: None }
    }

    pub fn step(&self) -> usize {
        self.step.load(Ordering::Relaxed).min(STEPS.len() - 1)
    }

    pub fn poll(&mut self) -> bool {
        if let Some(result) = self.job.take() {
            match result {
                Ok(prepared) => self.prepared = Some(prepared),
                Err(error) => self.error = Some(error),
            }
        }
        self.prepared.is_some()
    }
}

/// Values already encoded for eframe's final save, including its recovery fallback.
#[derive(Default)]
pub struct Prepared(HashMap<String, String>);

impl Prepared {
    pub fn apply(&self, storage: &mut dyn eframe::Storage) {
        for (key, value) in &self.0 {
            storage.set_string(key, value.clone());
        }
    }
}

use eframe::Storage;

impl Storage for Prepared {
    fn get_string(&self, key: &str) -> Option<String> { self.0.get(key).cloned() }
    fn set_string(&mut self, key: &str, value: String) { self.0.insert(key.into(), value); }
    fn remove_string(&mut self, key: &str) { self.0.remove(key); }
    fn flush(&mut self) {}
}

#[cfg(test)]
mod tests;
