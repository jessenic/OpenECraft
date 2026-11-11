//! Client of the project server (`openecraft-project-server`): opening and saving
//! shared projects, page locks. Requests run in the background; [`Remote::poll`]
//! collects the answers. The rules of shared editing are in `openecraft_core::collab`.

use openecraft_client::{decode, ehttp, ProjectEndpoint};
use openecraft_core::collab::{Grant, LoginResponse, Opened, ProjectAccess, ProjectRight, ProjectRole, ProjectSummary, Shared, Snapshot, Status, Target};
use openecraft_core::model::Project;
use openecraft_i18n::{tr, tr_fmt};
use std::sync::{Arc, Mutex};

/// Answer of a request. Answers for a session that is no longer open are dropped.
#[derive(Debug)]
pub enum Reply {
    List(Result<Vec<ProjectSummary>, String>),
    /// Name on the server and the opened project.
    Opened(String, Result<Box<Opened>, String>),
    Saved(String, Result<Box<Snapshot>, String>),
    Snapshot(String, Result<Box<Snapshot>, String>),
    Status(String, Result<Status, String>),
    /// Requested targets and the answer.
    Locked(String, Vec<Target>, Result<Status, String>),
    Login(Result<LoginResponse, String>),
    /// Grants on the open project (loaded or saved).
    Access(String, Result<ProjectAccess, String>),
    Closed,
}

impl Reply {
    fn session(&self) -> Option<&str> {
        match self {
            Reply::Saved(s, _) | Reply::Snapshot(s, _) | Reply::Status(s, _) | Reply::Locked(s, _, _) | Reply::Access(s, _) => Some(s),
            Reply::List(_) | Reply::Opened(..) | Reply::Login(_) | Reply::Closed => None,
        }
    }
}

pub struct Remote {
    pub url: String,
    /// Access token of the server (kept in memory only).
    pub token: String,
    pub user: String,
    /// Password of a user account (kept until signing in).
    pub password: String,
    /// Role of the signed-in user; None without user account (token or open server)
    /// or without built-in role (rights from grants only).
    pub role: Option<ProjectRole>,
    /// What the user may do with the open project; empty from older servers.
    pub rights: Vec<ProjectRight>,
    /// Grants on the open project, as edited by a project manager.
    pub access: Option<ProjectAccess>,
    /// Projects on the server (last list).
    pub projects: Vec<ProjectSummary>,
    /// The open shared project.
    pub shared: Option<Shared>,
    /// Name for uploading the current project.
    pub new_name: String,
    pub status: String,
    pub window: bool,
    /// The open shared project's server stopped answering (the last status failed).
    pub lost: bool,
    pending: usize,
    /// A status request is under way.
    polling: bool,
    /// A newer project is being loaded.
    fetching: bool,
    last_status: f64,
    /// History revision checked for changes without lock.
    pub checked_rev: u64,
    /// History revision of the last save request.
    pub saving_rev: u64,
    ctx: Option<egui::Context>,
    inbox: Arc<Mutex<Vec<Reply>>>,
}

impl Default for Remote {
    fn default() -> Self {
        let ProjectEndpoint { url, token } = ProjectEndpoint::default();
        let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        Remote {
            url,
            token,
            user,
            password: String::new(),
            role: None,
            rights: Vec::new(),
            access: None,
            projects: Vec::new(),
            shared: None,
            new_name: String::new(),
            status: String::new(),
            window: false,
            lost: false,
            pending: 0,
            polling: false,
            fetching: false,
            last_status: 0.0,
            checked_rev: 0,
            saving_rev: 0,
            ctx: None,
            inbox: Arc::default(),
        }
    }
}

impl Remote {
    fn endpoint(&self) -> ProjectEndpoint {
        ProjectEndpoint { url: self.url.clone(), token: self.token.clone() }
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    fn send(&mut self, request: ehttp::Request, status: String, wrap: impl 'static + Send + FnOnce(ehttp::Result<ehttp::Response>) -> Reply) {
        self.status = status;
        self.pending += 1;
        let inbox = self.inbox.clone();
        let ctx = self.ctx.clone();
        ehttp::fetch(request, move |result| {
            if let Ok(mut inbox) = inbox.lock() {
                inbox.push(wrap(result));
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Answers wake up the window from the start on.
    pub fn attach(&mut self, ctx: &egui::Context) {
        self.ctx = Some(ctx.clone());
    }

    /// The open shared project with the address of its server.
    pub fn project(&self) -> Option<crate::startup::ServerProject> {
        self.shared.as_ref().map(|sh| crate::startup::ServerProject { url: self.url.clone(), name: sh.name.clone() })
    }

    fn session_id(&self) -> String {
        self.shared.as_ref().map(|s| s.session.session.clone()).unwrap_or_default()
    }

    /// Signs in with user name and password; the session token replaces the token.
    pub fn sign_in(&mut self) {
        let req = self.endpoint().login(self.user.trim(), &std::mem::take(&mut self.password));
        self.send(req, tr("Signing in…").into(), |r| Reply::Login(decode(r)));
    }

    /// The user may do this with the open project. Older servers send no rights: then
    /// the role decides (none: token or open server, everything).
    pub fn may(&self, right: ProjectRight) -> bool {
        if self.rights.is_empty() {
            return self.role.is_none_or(|r| r.rights().contains(&right));
        }
        self.rights.contains(&right)
    }

    /// A viewer (or reviewer) may not change shared projects.
    pub fn viewer(&self) -> bool {
        self.shared.is_some() && !self.may(ProjectRight::EditPages) && !self.may(ProjectRight::EditProject)
    }

    /// Loads the grants on the open project.
    pub fn load_access(&mut self) {
        let Some(sh) = &self.shared else { return };
        let req = self.endpoint().project_access(&sh.name);
        let s = self.session_id();
        self.send(req, tr("Loading the project access…").into(), move |r| Reply::Access(s, decode(r)));
    }

    /// Replaces the grants on the open project.
    pub fn save_access(&mut self, grants: &[Grant]) {
        let Some(sh) = &self.shared else { return };
        let req = self.endpoint().save_project_access(&sh.name, grants);
        let s = self.session_id();
        self.send(req, tr("Saving the project access…").into(), move |r| Reply::Access(s, decode(r)));
    }

    /// Lists the projects of the server.
    pub fn list(&mut self) {
        let req = self.endpoint().list();
        self.send(req, tr("Connecting…").into(), |r| Reply::List(decode(r)));
    }

    pub fn open(&mut self, name: &str) {
        let req = self.endpoint().open(name, self.user.trim());
        let n = name.to_string();
        self.send(req, tr_fmt!("Opening {}…", name), move |r| Reply::Opened(n, decode(r).map(Box::new)));
    }

    /// Stores `prj` as a new project on the server and opens it.
    pub fn create(&mut self, name: &str, prj: &Project) {
        let req = self.endpoint().create(name, self.user.trim(), prj);
        let n = name.to_string();
        self.send(req, tr_fmt!("Uploading {}…", name), move |r| Reply::Opened(n, decode(r).map(Box::new)));
    }

    /// Saves what this session has locked, then releases `release`.
    pub fn save(&mut self, prj: &Project, release: Vec<Target>) {
        let Some(sh) = &self.shared else { return };
        let req = self.endpoint().save(&sh.name, &sh.session, prj, release, sh.revision);
        let s = self.session_id();
        self.send(req, tr("Saving to the project server…").into(), move |r| Reply::Saved(s, decode(r).map(Box::new)));
    }

    pub fn lock(&mut self, targets: Vec<Target>) {
        let endpoint = self.endpoint();
        let Some(sh) = &mut self.shared else { return };
        sh.requested.extend(targets.iter().copied());
        let req = endpoint.lock(&sh.name, &sh.session, targets.clone());
        let s = self.session_id();
        self.send(req, tr("Locking…").into(), move |r| Reply::Locked(s, targets, decode(r)));
    }

    /// Releases locks without saving.
    pub fn release(&mut self, targets: Vec<Target>) {
        let Some(sh) = &self.shared else { return };
        let req = self.endpoint().release(&sh.name, &sh.session, targets);
        let s = self.session_id();
        self.send(req, tr("Releasing…").into(), move |r| Reply::Status(s, decode(r)));
    }

    /// Ends the session (releases its locks); the project stays open locally.
    pub fn close(&mut self) {
        let Some(sh) = self.shared.take() else { return };
        self.lost = false;
        let req = self.endpoint().close(&sh.name, &sh.session);
        self.send(req, tr_fmt!("Closed {} on the project server", sh.name), |_| Reply::Closed);
    }

    fn fetch(&mut self) {
        let Some(sh) = &self.shared else { return };
        if self.fetching {
            return;
        }
        self.fetching = true;
        let req = self.endpoint().snapshot(&sh.name);
        let s = self.session_id();
        self.send(req, tr("Loading changes…").into(), move |r| Reply::Snapshot(s, decode(r).map(Box::new)));
    }

    /// Takes the finished requests (of the open session) and keeps the session alive.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<Reply> {
        self.ctx = Some(ctx.clone());
        let replies: Vec<Reply> = self.inbox.lock().map(|mut i| std::mem::take(&mut *i)).unwrap_or_default();
        let current = self.session_id();
        let mut out = Vec::new();
        for reply in replies {
            self.pending = self.pending.saturating_sub(1);
            match &reply {
                Reply::Status(..) => self.polling = false,
                Reply::Snapshot(..) => self.fetching = false,
                _ => {}
            }
            if reply.session().is_some_and(|s| s != current) {
                continue;
            }
            self.status = match &reply {
                Reply::List(Ok(list)) if list.len() == 1 => tr("1 project on the server").to_string(),
                Reply::List(Ok(list)) => tr_fmt!("{} projects on the server", list.len()),
                Reply::Opened(name, Ok(_)) => tr_fmt!("Opened {} from the project server", name),
                Reply::Saved(_, Ok(s)) => tr_fmt!("Saved to the project server (revision {})", s.revision),
                Reply::Login(Ok(r)) => match r.user.role {
                    Some(role) => tr_fmt!("Signed in as {} ({})", r.user.name, role.title()),
                    None => tr_fmt!("Signed in as {}", r.user.name),
                },
                Reply::Login(Err(e)) => tr_fmt!("Project server: {}", e),
                Reply::Access(_, Ok(a)) => tr_fmt!("{} grants on the project", a.grants.len()),
                // The "Locking…" of the request is over.
                Reply::Locked(_, _, Ok(_)) => String::new(),
                Reply::Access(_, Err(e)) => tr_fmt!("Project server: {}", e),
                Reply::List(Err(e))
                | Reply::Opened(_, Err(e))
                | Reply::Saved(_, Err(e))
                | Reply::Snapshot(_, Err(e))
                | Reply::Status(_, Err(e))
                | Reply::Locked(_, _, Err(e)) => tr_fmt!("Project server: {}", e),
                _ => std::mem::take(&mut self.status),
            };
            match &reply {
                Reply::Status(_, Err(_)) => self.lost = true,
                Reply::Status(_, Ok(_)) | Reply::Saved(_, Ok(_)) | Reply::Snapshot(_, Ok(_)) | Reply::Locked(_, _, Ok(_)) | Reply::Opened(_, Ok(_)) => self.lost = false,
                _ => {}
            }
            if let Reply::List(Ok(list)) = &reply {
                self.projects = list.clone();
            }
            if let Reply::Login(Ok(r)) = &reply {
                self.token = r.token.clone();
                self.user = r.user.name.clone();
                self.role = r.user.role;
                self.list();
            }
            if let Reply::Opened(_, Ok(o)) = &reply {
                self.rights = o.rights.clone();
                self.access = None;
            }
            if let Reply::Access(_, Ok(a)) = &reply {
                self.access = Some(a.clone());
            }
            out.push(reply);
        }
        if self.shared.is_some() {
            let now = ctx.input(|i| i.time);
            if !self.polling && now - self.last_status >= openecraft_core::collab::HEARTBEAT_SECS {
                self.heartbeat(now);
            }
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(openecraft_core::collab::HEARTBEAT_SECS));
        }
        out
    }

    fn heartbeat(&mut self, now: f64) {
        let Some(sh) = &self.shared else { return };
        self.polling = true;
        self.last_status = now;
        let req = self.endpoint().status(&sh.name, &sh.session);
        let s = self.session_id();
        let status = std::mem::take(&mut self.status);
        self.send(req, status, move |r| Reply::Status(s, decode(r)));
    }

    /// Answer to a status or lock request: loads the newer project if there is one.
    pub fn take_status(&mut self, status: Status, granted: &[Target]) {
        let Some(sh) = &mut self.shared else { return };
        if sh.status(status, granted) {
            self.fetch();
        }
    }

    /// Text for a target, e.g. "page 3".
    pub fn describe(prj: &Project, target: Target) -> String {
        match target {
            Target::Project => tr("the project data (page list, devices, settings)").into(),
            Target::Page(id) => match prj.pages.iter().find(|p| p.id == id) {
                Some(p) => tr_fmt!("page {}", openecraft_core::structure::page_full_name(p)),
                None => tr_fmt!("page {}", id),
            },
        }
    }
}

#[cfg(test)]
mod tests;
