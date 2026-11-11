//! What the editor opens when it starts (Settings › On startup) and where the open
//! project comes from: a local file, a project open on the project server or the local
//! copy of a server project. The status bar and Save show it ([`Origin`]).

use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde::{Deserialize, Serialize};

/// A project on a project server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerProject {
    pub url: String,
    pub name: String,
}

/// A project to open at the start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    File(String),
    Server(ServerProject),
}

/// Settings › On startup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Startup {
    /// The start page: local files and the projects of the project server.
    ProjectManager,
    /// The project of the last session, from where it came.
    #[default]
    ReopenLast,
    /// The project list of the project server, no project of the last session.
    ProjectServer,
    /// Always the same project.
    Project(Target),
}

/// The modes offered, with title and description.
pub const MODES: [(Mode, &str, &str); 4] = [
    (Mode::ProjectManager, tr_const!("Show project manager"), tr_const!("Choose a project on every start: local files and projects of the project server")),
    (Mode::ReopenLast, tr_const!("Reopen last project"), tr_const!("A local file opens locally, a server project is opened on the project server again")),
    (Mode::ProjectServer, tr_const!("Open project server"), tr_const!("Connects to the project server and shows its projects, without the project of the last session")),
    (Mode::Project, tr_const!("Open a specific project"), tr_const!("A local file or a project on the project server")),
];

/// The kind of [`Startup`], for choosing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    ProjectManager,
    ReopenLast,
    ProjectServer,
    Project,
}

impl Startup {
    pub fn mode(&self) -> Mode {
        match self {
            Startup::ProjectManager => Mode::ProjectManager,
            Startup::ReopenLast => Mode::ReopenLast,
            Startup::ProjectServer => Mode::ProjectServer,
            Startup::Project(_) => Mode::Project,
        }
    }

    /// Switches to `mode`; a specific project starts as `target`.
    pub fn set_mode(&mut self, mode: Mode, target: Target) {
        if mode == self.mode() {
            return;
        }
        *self = match mode {
            Mode::ProjectManager => Startup::ProjectManager,
            Mode::ReopenLast => Startup::ReopenLast,
            Mode::ProjectServer => Startup::ProjectServer,
            Mode::Project => Startup::Project(target),
        };
    }
}

/// What the start does with the project restored from the last session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Keep it (a local file, a new project).
    Keep,
    /// Show the start page.
    ShowStart,
    /// Show the project list of the project server.
    ShowServer,
    Open(Target),
}

/// What to do at the start. `file` is the local file of the restored project, `server`
/// the server project it is a copy of.
pub fn plan(startup: &Startup, file: Option<&str>, server: Option<&ServerProject>) -> Action {
    match startup {
        Startup::ProjectManager => Action::ShowStart,
        Startup::ProjectServer => Action::ShowServer,
        Startup::ReopenLast => match server {
            Some(s) => Action::Open(Target::Server(s.clone())),
            // The restored state is the file, with the changes not saved yet.
            None => Action::Keep,
        },
        Startup::Project(Target::File(path)) if server.is_none() && file == Some(path.as_str()) => Action::Keep,
        Startup::Project(target) => Action::Open(target.clone()),
    }
}

/// Where the open project comes from and where Save puts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin<'a> {
    /// Not saved yet (new project, demo, example).
    New,
    File(&'a str),
    /// Open on the project server; `connected` is false while the server does not answer.
    Server { url: &'a str, name: &'a str, connected: bool },
    /// A local copy of a server project: restored from the last session or closed on the server.
    Recovery(&'a ServerProject),
}

impl Origin<'_> {
    /// Text of the status bar: where the project is (not only a colour).
    pub fn label(&self) -> String {
        match self {
            Origin::New => tr("New project, not saved yet").to_string(),
            Origin::File(path) => tr_fmt!("Local file: {}", crate::io::short_name(path)),
            Origin::Server { url, name, connected: true } => tr_fmt!("Project server: {} / Project: {}", url, name),
            Origin::Server { url, name, connected: false } => tr_fmt!("Project server: {} / Project: {} – connection lost", url, name),
            Origin::Recovery(p) => tr_fmt!("Local recovery copy of server project: {}", p.name),
        }
    }

    /// Tooltip of Save: where the project goes.
    pub fn save_tip(&self) -> String {
        match self {
            Origin::New => tr("Save locally").to_string(),
            Origin::File(path) => tr_fmt!("Save locally to {}", crate::io::short_name(path)),
            Origin::Server { name, connected: true, .. } => tr_fmt!("Save to project server: {}", name),
            Origin::Server { name, connected: false, .. } => tr_fmt!("Save to project server: {} – the connection is lost, saving tries again", name),
            Origin::Recovery(p) => tr_fmt!("Save locally; {} on the project server stays unchanged", p.name),
        }
    }

    /// A project of the project server, open or as a local copy.
    pub fn is_server(&self) -> bool {
        matches!(self, Origin::Server { .. } | Origin::Recovery(_))
    }
}

/// A server project being opened at the start, until it is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconnect {
    pub project: ServerProject,
    pub state: ReconnectState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconnectState {
    Connecting,
    /// The server did not open it: the dialog offers retry, another project, the local copy.
    Failed(String),
    /// Waits for the sign-in in the project server pane.
    SignIn,
}

#[cfg(test)]
mod tests;
