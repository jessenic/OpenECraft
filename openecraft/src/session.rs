//! The open project and what the editor derives from it: undo history, analysis,
//! device index, saved state and file name. No UI code, so the rules (dirty, first
//! page, when the analysis is rebuilt) are unit tested; `App` adds the windows.

use openecraft_core::storage::folder::Tracked;
use openecraft_api::{Check, Plugins, Report};
use openecraft_core::analysis::{self, Analysis, Cache, Issue, Progress, Step};
use openecraft_core::history::History;
use openecraft_core::index::Index;
use openecraft_core::model::{PageKind, Project};
use std::sync::{Arc, Mutex};

pub struct Session {
    pub prj: Project,
    /// Index of the page shown.
    pub page: usize,
    pub history: History,
    pub analysis: Analysis,
    /// Page scans of the last analysis: only changed pages are scanned again.
    cache: Cache,
    analysis_rev: u64,
    analysis_lang: openecraft_i18n::Lang,
    /// Device index; while an edit is in progress it follows every frame.
    pub index: Index,
    index_rev: u64,
    /// History state of the last save; none: the project differs from every file.
    saved: Option<u64>,
    /// File the project was opened from or saved to.
    pub file_name: Option<String>,
    /// The files of a split project as they were when it was opened or last saved:
    /// a save writes what differs from them and leaves what another program changed.
    pub tracked: Option<Tracked>,
    /// The project is new to the session (at its start or replaced) and has no analysis
    /// yet: the next one is built in the background (`opening`).
    replaced: bool,
    /// The analysis of the opened project being built on another thread.
    opening: Option<Opening>,
    /// Without threads: the build of `opening`, run by the next `refresh`.
    waiting: Option<Box<dyn FnOnce() + Send>>,
    /// The 3D routing of an edited project being searched on another thread: the
    /// analysis after an edit takes it from the cache once it is there.
    routing: Option<Routed>,
    /// The search in the background failed: the next analysis searches here, where a
    /// failure shows.
    route_here: bool,
    /// Devices that could not follow the structure box they were moved into (see
    /// `openecraft_core::structure_boxes::follow`), to be shown once.
    pub structure_kept: Vec<String>,
}

/// An analysis built in the background for the history revision `rev` in the language
/// `lang`.
struct Opening {
    rev: u64,
    lang: openecraft_i18n::Lang,
    /// Where the build is and, once it is done, its result.
    shared: Arc<Mutex<Built>>,
}

/// The cache of a 3D routing searched in the background, once it is done (an error if
/// the search failed).
type Routed = Arc<Mutex<Option<Result<Cache, ()>>>>;

/// Where a build in the background starts.
const STARTED: Progress = Progress { step: Step::Netlists, done: 0, total: 0 };

struct Built {
    progress: Progress,
    result: Option<Result<(Analysis, Cache), ()>>,
}

/// For a split project: its files as they are now. Its new objects get ids apart
/// from those of other copies of the folder.
fn track(prj: &mut Project, file_name: Option<&str>) -> Option<Tracked> {
    let index = std::path::Path::new(file_name.filter(|f| crate::io::is_split(f))?);
    prj.scatter_ids();
    // The project was not read from the folder just now (it is restored, or came
    // from elsewhere): nothing is known of the files, so a save replaces none that
    // differs from the project. The app puts the stamps of the last run in place.
    Some(Tracked::unknown(index))
}

impl Session {
    /// Opens `prj` (normalised) on its first schematic page; it counts as saved. Its
    /// analysis is built in the background like that of a project opened later (the
    /// project at the start of the app, restored from the last session).
    pub fn new(mut prj: Project, file_name: Option<String>) -> Session {
        prj.normalize();
        let tracked = track(&mut prj, file_name.as_deref());
        let history = History::new(&prj);
        let saved = Some(history.state());
        let page = first_schematic(&prj);
        Session {
            prj,
            page,
            history,
            analysis: Analysis::default(),
            cache: Cache::default(),
            analysis_rev: 0,
            analysis_lang: openecraft_i18n::lang(),
            index: Index::default(),
            index_rev: 0,
            saved,
            file_name,
            tracked,
            replaced: true,
            opening: None,
            waiting: None,
            routing: None,
            route_here: false,
            structure_kept: Vec::new(),
        }
    }

    /// Replaces the project; the replacement counts as saved. Its history starts
    /// anew: undo must not bring back the project before (whose changes may have been
    /// discarded) under the file name of this one.
    /// `tracked`: the files of a split project as it was read (none: they are looked
    /// at now).
    pub fn replace(&mut self, prj: Project, file_name: Option<String>, tracked: Option<Tracked>) {
        self.prj = prj;
        self.prj.normalize();
        let read = tracked.filter(|t| file_name.as_deref().is_some_and(|f| crate::io::is_split(f) && t.is_for(std::path::Path::new(f))));
        self.tracked = match read {
            Some(read) => {
                self.prj.scatter_ids();
                Some(read)
            }
            None => track(&mut self.prj, file_name.as_deref()),
        };
        self.file_name = file_name;
        self.page = first_schematic(&self.prj);
        // A new revision, so that the analysis and the index are built again.
        let rev = self.history.rev + 1;
        self.history = History::new(&self.prj);
        self.history.rev = rev;
        self.mark_saved();
        self.replaced = true;
        self.opening = None;
        self.waiting = None;
        self.routing = None;
    }

    /// Puts the files of the split project in place as an earlier run of the app
    /// read them (stored with the project that is restored at the start).
    pub fn restore_tracked(&mut self, tracked: Option<Tracked>) {
        let index = self.file_name.as_deref().filter(|f| crate::io::is_split(f)).map(std::path::Path::new);
        if let Some(tracked) = tracked.filter(|t| index.is_some_and(|index| t.is_for(index))) {
            self.tracked = Some(tracked);
        }
    }

    /// Takes over the files another program changed in the folder (`names`) from
    /// the project read again, into the project and its undo and redo steps: undo
    /// must not take back the changes of others. The result counts as saved.
    pub fn take_over(&mut self, from: &Project, names: &std::collections::BTreeSet<String>, tracked: Tracked) {
        self.history.rebase(&mut self.prj, |p| openecraft_core::storage::take_over(p, from, names));
        self.tracked = Some(tracked);
        self.page = self.page.min(self.prj.pages.len().saturating_sub(1));
        self.mark_saved();
    }

    /// Changes since the last save (undo back to the saved state leaves none).
    pub fn dirty(&self) -> bool {
        self.saved != Some(self.history.state())
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.history.state());
    }

    /// The project has changes no file holds (e.g. restored after the app was closed
    /// with unsaved changes).
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// Nothing happened since the session started: no edit, undo or replacement.
    pub fn untouched(&self) -> bool {
        self.history.rev == 1
    }

    /// Keeps the page index inside the project.
    pub fn clamp_page(&mut self) {
        self.page = self.page.min(self.prj.pages.len().saturating_sub(1));
    }

    /// Makes the finished edits one undo step (`can_commit` is false while the user
    /// is still dragging or typing).
    /// Returns whether a change was committed (the shell shows it at once). Devices
    /// moved into or out of structure boxes take their structure in the same undo step.
    pub fn commit(&mut self, can_commit: bool) -> bool {
        if can_commit {
            let kept = openecraft_core::structure_boxes::follow(&mut self.prj, self.history.committed());
            self.structure_kept.extend(kept);
        }
        self.history.track(&self.prj, can_commit)
    }

    /// Rebuilds the analysis after a committed change or a change of the language
    /// (reports and checks of `plugins`), and the device index after a committed
    /// change or, while `editing`, every frame. The analysis of a project just opened
    /// is built in the background ([`Session::opening`]); after an edit, where threads
    /// are, only the 3D routing is searched in the background
    /// ([`Analysis::routes_pending`] until it is ready, then the analysis is built
    /// again with it).
    pub fn refresh(&mut self, plugins: &Plugins, editing: bool) {
        self.take_opened();
        self.take_routed();
        let lang = openecraft_i18n::lang();
        if self.opening.is_none() && (self.analysis_rev != self.history.rev || self.analysis_lang != lang) {
            if self.replaced {
                self.open_in_background(plugins.checks().cloned().collect(), plugins.reports().cloned().collect());
            } else {
                let before = (analysis::threads() >= 2 && !std::mem::take(&mut self.route_here)).then(|| std::mem::take(&mut self.analysis.routing));
                self.analysis = analyse(&self.prj, &mut self.cache, before.as_ref(), plugins.reports().cloned(), |p, a| plugins.check(p, a), &|_| {});
                self.analysis_rev = self.history.rev;
                self.analysis_lang = lang;
                if self.analysis.routes_pending && self.routing.is_none() {
                    self.route_in_background();
                }
            }
        }
        if editing || self.index_rev != self.history.rev {
            self.index = Index::build(&self.prj);
            self.index_rev = self.history.rev;
        }
    }

    /// Where the analysis of the project just opened is while it is built in the
    /// background: the editor shows it instead of the project.
    pub fn opening(&self) -> Option<Progress> {
        let o = self.opening.as_ref()?;
        Some(o.shared.lock().ok().map(|b| b.progress).unwrap_or(STARTED))
    }

    /// Starts building the analysis of the project on another thread, or (without
    /// threads) has the next `refresh` build it, after the progress was shown once.
    fn open_in_background(&mut self, checks: Vec<Check>, reports: Vec<Report>) {
        let shared = Arc::new(Mutex::new(Built { progress: STARTED, result: None }));
        let (rev, lang) = (self.history.rev, openecraft_i18n::lang());
        let prj = self.prj.clone();
        let mut cache = std::mem::take(&mut self.cache);
        let job = {
            let shared = shared.clone();
            move || {
                let progress = |p| {
                    if let Ok(mut b) = shared.lock() {
                        b.progress = p;
                    }
                };
                let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    openecraft_i18n::with_lang(lang, || {
                        let an = analyse(&prj, &mut cache, None, reports, |p, a| openecraft_api::run_checks(&checks, p, a), &progress);
                        (an, cache)
                    })
                }));
                if let Ok(mut b) = shared.lock() {
                    b.result = Some(built.map_err(|_| ()));
                }
            }
        };
        if analysis::threads() < 2 {
            self.waiting = Some(Box::new(job));
        } else {
            std::thread::spawn(job);
        }
        self.opening = Some(Opening { rev, lang, shared });
    }

    /// Takes the analysis built in the background once it is ready (or builds it,
    /// without threads). One built for an earlier state or language is dropped and the
    /// next `refresh` builds it again.
    fn take_opened(&mut self) {
        if let Some(job) = self.waiting.take() {
            job();
        }
        let Some(o) = &self.opening else { return };
        let Some(result) = o.shared.lock().ok().and_then(|mut b| b.result.take()) else { return };
        let (rev, lang) = (o.rev, o.lang);
        self.opening = None;
        match result {
            Ok((analysis, cache)) => {
                self.cache = cache;
                if rev == self.history.rev && lang == openecraft_i18n::lang() {
                    self.analysis = analysis;
                    self.analysis_rev = rev;
                    self.analysis_lang = lang;
                    self.replaced = false;
                }
            }
            // Built again here, where a failure shows.
            Err(_) => self.replaced = false,
        }
    }

    /// Searches the 3D routing of the project as it is now on another thread (see
    /// [`Session::refresh`]).
    fn route_in_background(&mut self) {
        let shared = Arc::new(Mutex::new(None));
        let (prj, mut cache, lang) = (self.prj.clone(), self.cache.clone(), openecraft_i18n::lang());
        let out = shared.clone();
        std::thread::spawn(move || {
            let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                openecraft_i18n::with_lang(lang, || Analysis::build_with(&prj, &mut cache));
                cache
            }));
            if let Ok(mut o) = out.lock() {
                *o = Some(built.map_err(|_| ()));
            }
        });
        self.routing = Some(shared);
    }

    /// Takes the 3D routing searched in the background once it is ready; the next
    /// `refresh` builds the analysis again with it (or, if the project changed in the
    /// meantime, searches again).
    fn take_routed(&mut self) {
        let Some(shared) = &self.routing else { return };
        let Some(result) = shared.lock().ok().and_then(|mut r| r.take()) else { return };
        self.routing = None;
        match result {
            Ok(cache) => self.cache.keep_routing(cache),
            Err(()) => self.route_here = true,
        }
        self.invalidate();
    }

    /// The 3D routing is being searched in the background.
    pub fn routing(&self) -> bool {
        self.routing.is_some()
    }

    /// What an analysis of the project as it is now needs on another thread (an
    /// export): the project, the cache and the reports and checks of `plugins`.
    pub fn snapshot(&self, plugins: &Plugins) -> Snapshot {
        Snapshot {
            prj: self.prj.clone(),
            cache: self.cache.clone(),
            checks: plugins.checks().cloned().collect(),
            reports: plugins.reports().cloned().collect(),
            lang: openecraft_i18n::lang(),
        }
    }

    /// The next `refresh` rebuilds the analysis (plugins changed).
    pub fn invalidate(&mut self) {
        self.analysis_rev = self.history.rev.wrapping_add(1);
    }

    pub fn undo(&mut self) {
        self.history.undo(&mut self.prj);
    }

    pub fn redo(&mut self) {
        self.history.redo(&mut self.prj);
    }

    /// Whether the items of page `page` may be changed: not released, not generated
    /// (reports, mounting layouts), not read only for this user (locked by another user
    /// of a shared project, a viewer).
    pub fn may_edit_page(&self, page: usize, read_only: bool) -> bool {
        self.prj.pages.get(page).is_some_and(|p| p.is_editable()) && !read_only
    }

    /// Whether page `page` may be deleted: not the last one, not released, not locked
    /// by another user of a shared project.
    pub fn may_delete_page(&self, page: usize, locked_by_other: bool) -> bool {
        self.prj.pages.len() > 1 && self.prj.pages.get(page).is_some_and(|p| !p.locked) && !locked_by_other
    }

    /// What deleting page `page` loses: its items (a document page counts its file);
    /// an empty page is deleted without asking.
    pub fn page_items(&self, page: usize) -> usize {
        let Some(p) = self.prj.pages.get(page) else { return 0 };
        p.symbols.len() + p.wires.len() + p.texts.len() + p.shapes.len() + p.dimensions.len() + p.images.len() + p.conn_defs.len() + p.cable_defs.len() + p.junctions.len() + p.macro_boxes.len() + p.structure_boxes.len() + usize::from(p.document.is_some())
    }
}

/// The project and what its analysis needs, taken to another thread
/// ([`Session::snapshot`]).
pub struct Snapshot {
    pub prj: Project,
    cache: Cache,
    checks: Vec<Check>,
    pub reports: Vec<Report>,
    lang: openecraft_i18n::Lang,
}

impl Snapshot {
    /// The analysis of the project with the reports and the issues of the checks, in
    /// the language of the editor when the snapshot was taken.
    pub fn analyse(&mut self) -> Analysis {
        let Snapshot { prj, cache, checks, reports, lang } = self;
        openecraft_i18n::with_lang(*lang, || analyse(prj, cache, None, reports.clone(), |p, a| openecraft_api::run_checks(checks.iter(), p, a), &|_| {}))
    }
}

/// The analysis of `prj` with the reports and the issues `check` finds, telling
/// `progress` each step; with `before`, the 3D routing is not searched
/// ([`Analysis::build_kept`]).
pub fn analyse(prj: &Project, cache: &mut Cache, before: Option<&openecraft_core::routing3d::Routing>, reports: impl IntoIterator<Item = Report>, check: impl Fn(&Project, &Analysis) -> Vec<Issue>, progress: &(dyn Fn(Progress) + Sync)) -> Analysis {
    let mut an = match before {
        Some(before) => Analysis::build_kept(prj, cache, before),
        None => Analysis::build_progress(prj, cache, progress),
    };
    an.set_reports(reports);
    progress(Progress { step: Step::Checks, done: 0, total: 0 });
    an.issues = check(prj, &an);
    an.issues.sort_by_key(|i| i.severity);
    an
}

/// The question before an action drops unsaved changes (a new or opened project,
/// closing the window): the action waits for the answer, after Save until the save
/// has finished.
#[derive(Debug)]
pub struct Guard<A> {
    pending: Option<A>,
    saving: bool,
    /// The last save of the question ended with changes left (cancelled or failed).
    pub not_saved: bool,
}

impl<A> Default for Guard<A> {
    fn default() -> Self {
        Guard { pending: None, saving: false, not_saved: false }
    }
}

impl<A> Guard<A> {
    /// The action runs now (returned) unless it would drop unsaved changes: then it
    /// waits for the answer.
    pub fn request(&mut self, dirty: bool, action: A) -> Option<A> {
        if !dirty {
            return Some(action);
        }
        self.pending = Some(action);
        self.saving = false;
        self.not_saved = false;
        None
    }

    /// The action waiting for the answer.
    pub fn pending(&self) -> Option<&A> {
        self.pending.as_ref()
    }

    /// Discard: the waiting action runs.
    pub fn discard(&mut self) -> Option<A> {
        self.saving = false;
        self.pending.take()
    }

    pub fn cancel(&mut self) {
        self.pending = None;
        self.saving = false;
    }

    /// Save was chosen: the action waits for the save.
    pub fn save(&mut self) {
        self.saving = self.pending.is_some();
        self.not_saved = false;
    }

    /// A save is running for the question.
    pub fn saving(&self) -> bool {
        self.saving
    }

    /// Call after a save attempt and every frame while saving (`busy`: a save on a
    /// server is still running). Returns the action once the project is clean; a save
    /// that ended with changes left keeps the question open.
    pub fn after_save(&mut self, dirty: bool, busy: bool) -> Option<A> {
        if !self.saving {
            return None;
        }
        if !dirty {
            self.saving = false;
            return self.pending.take();
        }
        if !busy {
            self.saving = false;
            self.not_saved = true;
        }
        None
    }
}

fn first_schematic(prj: &Project) -> usize {
    prj.pages.iter().position(|p| p.kind == PageKind::Schematic).unwrap_or(0)
}

#[cfg(test)]
mod tests;
