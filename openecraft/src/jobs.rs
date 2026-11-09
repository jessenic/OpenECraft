//! Work of the editor that would hold up the window (exports, imports of large files)
//! runs on another thread: [`Job::start`] starts it, [`Job::take`] gives the result
//! once it is there and the window repaints when it is. Without threads (the browser)
//! the work is done at once. No UI code; `App` keeps the jobs and applies the results.

use std::sync::{Arc, Mutex};

/// A result being worked out on another thread.
pub struct Job<T> {
    /// What is being done, for the status bar.
    pub label: String,
    result: Arc<Mutex<Option<Result<T, String>>>>,
}

impl<T: Send + 'static> Job<T> {
    /// Starts `work`; `ctx` repaints when it is done.
    pub fn start(label: String, ctx: &egui::Context, work: impl FnOnce() -> T + Send + 'static) -> Job<T> {
        let result = Arc::new(Mutex::new(None));
        let (out, ctx) = (result.clone(), ctx.clone());
        let run = move || {
            let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).map_err(|e| panic_text(e.as_ref()));
            if let Ok(mut o) = out.lock() {
                *o = Some(done);
            }
            ctx.request_repaint();
        };
        openecraft_ui::io::background(run);
        Job { label, result }
    }

    /// The result once the work is done; an error if it failed (panicked).
    pub fn take(&self) -> Option<Result<T, String>> {
        self.result.lock().ok()?.take()
    }
}

/// The message of a panic.
fn panic_text(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
