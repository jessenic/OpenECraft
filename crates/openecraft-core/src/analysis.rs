//! Everything derived from the project that the front ends, reports, checks and
//! exports share: device index, netlists, connections, panel wiring, 3D routing and
//! field plan, the report tables of the registered reports (generated on demand) and
//! the issues the host's checks found.

use crate::ext::Report;
use crate::index::Index;
pub use crate::issue::{Issue, Severity};
pub use crate::parallel::{join, par_map, par_map_n, threads};
use crate::model::{Project, ReportId};
use crate::netlist::{connections, Connection, Netlist, Scans};
use crate::panel::{self, OpenWire, RoutedWire};
use crate::table::Table;
use openecraft_i18n::tr;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Everything derived from the project that is expensive enough to cache. The report
/// tables are generated when they are first asked for ([`Analysis::report`]).
#[derive(Default)]
pub struct Analysis {
    /// Tells this analysis from every other one built before (caches of the front ends
    /// that follow the analysis); 0 for an empty one.
    pub id: u64,
    /// Device / cross-reference index.
    pub index: Index,
    pub netlist: Netlist,
    /// Netlist with the terminal sides apart: one net per physical connection
    /// (wire numbers, cables).
    pub open: Netlist,
    pub issues: Vec<Issue>,
    /// Virtual wiring of the panel layout.
    pub wiring: Vec<RoutedWire>,
    /// Connections the panel layout could not route.
    pub open_wires: Vec<OpenWire>,
    /// Changes since the last revision (revision marks).
    pub changes: Vec<crate::revision::PageChanges>,
    /// Point-to-point connections.
    pub connections: Vec<Connection>,
    /// The wires at each connection point, for the termination of their ends (twin
    /// ferrules).
    pub ends: crate::production::Ends,
    /// Fill levels of the cable ducts the wiring runs through.
    pub duct_fill: Vec<panel::DuctFill>,
    /// 3D routing of wires, cables and tubes.
    pub routing: crate::routing3d::Routing,
    /// `routing` is what is left of the routing of an earlier state until the routing
    /// of this one is ready ([`Analysis::build_kept`]).
    pub routes_pending: bool,
    /// Cables planned through the route network of the plant.
    pub field: crate::field::Plan,
    /// Reports the tables come from: the built-in ones until the host sets the
    /// reports of its registry ([`Analysis::set_reports`]).
    reports: Vec<Report>,
    /// Generated report tables.
    tables: Mutex<HashMap<ReportId, Arc<Table>>>,
    /// Findings of the layout checks per enclosure.
    layout: Mutex<HashMap<usize, Arc<Vec<panel::Finding>>>>,
}

/// Id of the next analysis built.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The steps of building an analysis, in their order, and the checks the host runs
/// on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Netlists,
    Connections,
    Wiring,
    Routing,
    Ducts,
    Checks,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::Netlists, Step::Connections, Step::Wiring, Step::Routing, Step::Ducts, Step::Checks];

    pub fn title(self) -> &'static str {
        match self {
            Step::Netlists => tr("Building the netlists"),
            Step::Connections => tr("Collecting the connections"),
            Step::Wiring => tr("Wiring the panels"),
            Step::Routing => tr("Routing cables and pipes in 3D"),
            Step::Ducts => tr("Filling the cable ducts"),
            Step::Checks => tr("Checking the project"),
        }
    }
}

/// Where a build of the analysis is: its step and, in a step of many items (the 3D
/// routing), how many of `total` are done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub step: Step,
    pub done: usize,
    pub total: usize,
}

/// What an analysis keeps for the next one of the same project: the scans of the pages
/// that did not change and the 3D routing while nothing it depends on changed are
/// reused (see [`Analysis::build_with`]).
#[derive(Clone, Default)]
pub struct Cache {
    netlist: Scans,
    open: Scans,
    routing: crate::routing3d::Cache,
}

impl Cache {
    /// Pages scanned again by the last build (both netlists).
    pub fn scanned(&self) -> usize {
        self.netlist.scanned + self.open.scanned
    }

    /// Takes the 3D routing kept by `other` (a build of the same project elsewhere,
    /// e.g. in the background).
    pub fn keep_routing(&mut self, other: Cache) {
        self.routing = other.routing;
    }
}

impl Analysis {
    pub fn build(prj: &Project) -> Analysis {
        Analysis::build_with(prj, &mut Cache::default())
    }

    /// Like [`Analysis::build`], taking from `cache` what did not change since the last
    /// build with it. On machines with several cores the two netlists are built at
    /// the same time.
    pub fn build_with(prj: &Project, cache: &mut Cache) -> Analysis {
        Analysis::build_progress(prj, cache, &|_| {})
    }

    /// Like [`Analysis::build_with`], telling `progress` each step it begins and, in
    /// the 3D routing, each route.
    pub fn build_progress(prj: &Project, cache: &mut Cache, progress: &(dyn Fn(Progress) + Sync)) -> Analysis {
        Analysis::build_inner(prj, cache, progress, None)
    }

    /// Like [`Analysis::build_with`], but the 3D routing is not searched: it is taken
    /// from `cache` where nothing it depends on changed, otherwise what still fits of
    /// `before` (the routing of an earlier state) stands in for it, and
    /// [`Analysis::routes_pending`] is set. Fast enough after every edit; the routing is
    /// searched by a build elsewhere and handed over with [`Cache::keep_routing`].
    pub fn build_kept(prj: &Project, cache: &mut Cache, before: &crate::routing3d::Routing) -> Analysis {
        Analysis::build_inner(prj, cache, &|_| {}, Some(before))
    }

    fn build_inner(prj: &Project, cache: &mut Cache, progress: &(dyn Fn(Progress) + Sync), before: Option<&crate::routing3d::Routing>) -> Analysis {
        let step = |step| progress(Progress { step, done: 0, total: 0 });
        step(Step::Netlists);
        let Cache { netlist: bridged, open: apart, routing: kept } = cache;
        let (netlist, open) = join(|| Netlist::build_with(prj, true, bridged), || Netlist::build_with(prj, false, apart));
        let idx = Index::build(prj);
        step(Step::Connections);
        let conns = connections(prj, &netlist, &open);
        step(Step::Wiring);
        let (mut wiring, open_wires) = panel::virtual_wiring(prj, &conns);
        step(Step::Routing);
        // The field plan and the revision marks do not need the routing.
        let ((routing, routes_pending), (field, changes)) = join(
            || match before {
                None => (crate::routing3d::analyse_cached(prj, &mut wiring, &conns, &open, kept, &|done, total| progress(Progress { step: Step::Routing, done, total })), false),
                Some(before) => match crate::routing3d::kept(prj, &mut wiring, &conns, &open, kept) {
                    Some(routing) => (routing, false),
                    None => (before.interim(prj.layout.enclosures.len()), true),
                },
            },
            || (crate::field::plan(prj, &conns), crate::revision::pending(prj)),
        );
        step(Step::Ducts);
        let fill = panel::duct_fill(prj, &wiring, &crate::routing3d::duct_contents(prj, &routing));
        Analysis {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            index: idx,
            netlist,
            open,
            issues: Vec::new(),
            wiring,
            open_wires,
            changes,
            ends: crate::production::Ends::new(prj, &conns),
            connections: conns,
            duct_fill: fill,
            routing,
            routes_pending,
            field,
            reports: Vec::new(),
            tables: Mutex::default(),
            layout: Mutex::default(),
        }
    }


    /// The reports the tables come from.
    pub fn reports(&self) -> &[Report] {
        &self.reports
    }

    /// Takes the reports of the host's registry (built-in and of plugins).
    pub fn set_reports(&mut self, reports: impl IntoIterator<Item = Report>) {
        self.reports = reports.into_iter().collect();
        self.clear_reports();
    }

    /// The table of a report: generated on the first call (from the project the
    /// analysis was built from) and kept. `None` for a report that is not registered.
    pub fn report(&self, prj: &Project, id: impl Into<ReportId>) -> Option<Arc<Table>> {
        let id = id.into();
        if let Some(t) = self.tables.lock().ok().and_then(|m| m.get(&id).cloned()) {
            return Some(t);
        }
        let r = self.reports.iter().find(|r| r.name == id.as_str())?;
        let table = Arc::new((r.run)(prj, self).unwrap_or_default());
        if let Ok(mut m) = self.tables.lock() {
            m.insert(id, table.clone());
        }
        Some(table)
    }

    /// Forgets the generated tables (e.g. after a change of the language).
    pub fn clear_reports(&self) {
        if let Ok(mut m) = self.tables.lock() {
            m.clear();
        }
        if let Ok(mut m) = self.layout.lock() {
            m.clear();
        }
    }

    /// The findings of the layout checks of enclosure `ei` ([`panel::validate`]),
    /// made on the first call (from the project the analysis was built from) and kept.
    pub fn layout_findings(&self, prj: &Project, ei: usize) -> Arc<Vec<panel::Finding>> {
        if let Some(f) = self.layout.lock().ok().and_then(|m| m.get(&ei).cloned()) {
            return f;
        }
        let Some(e) = prj.layout.enclosures.get(ei) else { return Arc::default() };
        let found = Arc::new(panel::validate(prj, e));
        if let Ok(mut m) = self.layout.lock() {
            m.insert(ei, found.clone());
        }
        found
    }

}

#[cfg(test)]
mod tests;
