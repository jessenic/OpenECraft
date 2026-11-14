//! State the tools work on: the open project (with its file, undo history and saved
//! state), the parts server and Autodesk Vault. The command line uses one session per
//! call, the MCP server one for the whole connection.

use crate::parts_server::{Http, Server};
use crate::vault::Vault;
use openecraft_core::archive::{self, Conversion};
use openecraft_api::Plugins;
use openecraft_core::history::History;
use openecraft_core::model::Project;
use openecraft_core::storage::folder;
use openecraft_i18n::{tr, tr_fmt};
use std::path::{Path, PathBuf};

pub struct Session {
    project: Option<Project>,
    pub path: Option<PathBuf>,
    /// Archive the project was read from: saving writes a project file, not the archive.
    pub archive: Option<PathBuf>,
    /// The files of a split project as they were when it was opened or last saved.
    tracked: Option<folder::Tracked>,
    /// Project as last opened / saved; `None` if it was never saved.
    saved: Option<Project>,
    history: Option<History>,
    pub server: Server,
    pub vault: Vault,
    /// Built-in plugins; the command line adds the external ones (see `main`).
    pub plugins: Plugins,
    /// Tools the MCP server offers: names or prefixes ending in `*`; empty = all
    /// (`OPENECRAFT_MCP_TOOLS`).
    pub mcp_tools: Vec<String>,
    /// Folder the files of the MCP tools must be in; `None` = anywhere (the command
    /// line). See `mcp::root_from_env`.
    pub mcp_root: Option<PathBuf>,
}

impl Session {
    pub fn new(server: Server) -> Session {
        let vault = Vault::new(Box::new(Http));
        Session {
            project: None,
            path: None,
            archive: None,
            tracked: None,
            saved: None,
            history: None,
            server,
            vault,
            plugins: Plugins::new(crate::tools::all().map(|t| t.name)),
            mcp_tools: Vec::new(),
            mcp_root: None,
        }
    }

    pub fn project(&self) -> Result<&Project, String> {
        self.project.as_ref().ok_or_else(no_project)
    }

    pub fn project_mut(&mut self) -> Result<&mut Project, String> {
        self.project.as_mut().ok_or_else(no_project)
    }

    /// Replaces the project. `path` is the file it was read from (counts as saved).
    pub fn replace(&mut self, prj: Project, path: Option<PathBuf>) {
        // Not read from that folder: nothing is known of its files, so a save
        // replaces none that differs from the project.
        let tracked = path.as_deref().filter(|p| folder::is_index(p)).map(folder::Tracked::unknown);
        self.replace_tracked(prj, path, tracked);
    }

    /// Like [`Session::replace`], with the files of a split project as they were read.
    fn replace_tracked(&mut self, mut prj: Project, path: Option<PathBuf>, tracked: Option<folder::Tracked>) {
        prj.normalize();
        if path.as_deref().is_some_and(folder::is_index) {
            prj.scatter_ids();
        }
        self.tracked = tracked;
        self.history = Some(History::new(&prj));
        self.saved = path.is_some().then(|| prj.clone());
        self.path = path;
        self.archive = None;
        self.project = Some(prj);
    }

    /// Opens a project file or archive (converted to the current file format).
    pub fn open(&mut self, path: &str) -> Result<Conversion, String> {
        if folder::is_index(Path::new(path)) {
            let (prj, conv, tracked) = folder::open_tracked(Path::new(path))?;
            self.replace_tracked(prj, Some(PathBuf::from(path)), Some(tracked));
            return Ok(conv);
        }
        let (prj, conv, is_archive) = read_project(Path::new(path))?;
        // Saving writes a project file, not the archive.
        let file = (!is_archive).then(|| PathBuf::from(path));
        self.replace(prj, file);
        if is_archive {
            // Counts as saved: only changes made after opening are refused.
            self.saved = self.project.clone();
            self.archive = Some(PathBuf::from(path));
        }
        Ok(conv)
    }

    /// Saves to `path`, or to the file of the project. Returns the file.
    pub fn save(&mut self, path: Option<&str>) -> Result<PathBuf, String> {
        let prj = self.project.as_ref().ok_or_else(no_project)?;
        let no_file = || match &self.archive {
            Some(a) => tr_fmt!("{} is an archive: pass a path to save the project as a project file", a.display()),
            None => tr("The project has no file yet: pass a path").to_string(),
        };
        let path = path.map(PathBuf::from).or_else(|| self.path.clone()).ok_or_else(no_file)?;
        if !folder::is_index(&path) {
            std::fs::write(&path, prj.to_json()).map_err(|e| tr_fmt!("Cannot write {}: {}", path.display(), openecraft_i18n::io_error(&e)))?;
            self.saved = Some(prj.clone());
            self.path = Some(path.clone());
            return Ok(path);
        }
        let saved = folder::save_tracked(&path, prj, &mut self.tracked)?;
        if saved.foreign > 0 {
            // Another program changed files of the folder meanwhile (git): they were
            // kept and are taken over into the project and its undo steps, which
            // must not take back the changes of others.
            let names = self.tracked.as_ref().map(|t| t.foreign().clone()).unwrap_or_default();
            let (disk, _, tracked) = folder::open_tracked(&path)?;
            if let (Some(history), Some(prj)) = (&mut self.history, &mut self.project) {
                history.rebase(prj, |p| openecraft_core::storage::take_over(p, &disk, &names));
            }
            self.tracked = Some(tracked);
        }
        self.project_mut()?.scatter_ids();
        self.saved = self.project.clone();
        self.path = Some(path.clone());
        Ok(path)
    }

    /// The project has changes that are not in its file.
    pub fn unsaved(&self) -> bool {
        self.project.is_some() && self.saved != self.project
    }

    /// The command line has no session: changes are written back to the project file
    /// at once. An archive is not rewritten, the changes are refused instead.
    pub fn write_back(&mut self) -> Result<(), String> {
        if !self.unsaved() {
            return Ok(());
        }
        match (&self.path, &self.archive) {
            (Some(_), _) => self.save(None).map(|_| ()),
            (None, Some(a)) => Err(tr_fmt!("{} is an archive and stays unchanged: save the project as a project file (save_project --path <file>.oec) or convert the archive (convert_project)", a.display())),
            (None, None) => Ok(()),
        }
    }

    /// Runs a command of a plugin on the open project.
    pub fn run_command(&mut self, command: &openecraft_api::Command, args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, String> {
        let prj = self.project.as_mut().ok_or_else(no_project)?;
        self.plugins.run(command, prj, args)
    }

    /// Puts back the project as it was before a failed command (no undo step).
    pub fn restore(&mut self, prj: Project) {
        if self.project.is_some() {
            self.project = Some(prj);
        }
    }

    /// Makes the changes of a tool one undo step.
    pub fn commit(&mut self) {
        if let (Some(h), Some(prj)) = (&mut self.history, &self.project) {
            h.track(prj, true);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.as_ref().is_some_and(|h| h.can_undo())
    }

    pub fn can_redo(&self) -> bool {
        self.history.as_ref().is_some_and(|h| h.can_redo())
    }

    /// Undoes the last change; false if there is none.
    pub fn undo(&mut self) -> Result<bool, String> {
        let (Some(h), Some(prj)) = (&mut self.history, &mut self.project) else { return Err(no_project()) };
        let can = h.can_undo();
        h.undo(prj);
        Ok(can)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        let (Some(h), Some(prj)) = (&mut self.history, &mut self.project) else { return Err(no_project()) };
        let can = h.can_redo();
        h.redo(prj);
        Ok(can)
    }
}

/// Reads a project file, the index of a split project or an archive (`true`).
pub fn read_project(path: &Path) -> Result<(Project, Conversion, bool), String> {
    if folder::is_index(path) {
        let (prj, conv) = folder::open(path)?;
        return Ok((prj, conv, false));
    }
    let bytes = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path.display(), openecraft_i18n::io_error(&e)))?;
    let (prj, conv) = archive::read_project(&bytes)?;
    Ok((prj, conv, archive::is_archive(&bytes)))
}

fn no_project() -> String {
    tr("No project is open: use open_project or new_project first").to_string()
}

#[cfg(test)]
mod tests;
