//! The loaded plugins of a front end and the access to their extensions.

use crate::params::{self, Args};
use crate::{Check, Command, Exporter, Registry, Report};
use crate::{external, is_snake_case, Info, Plugin};
use openecraft_core::model::Project;
use openecraft_core::analysis::Analysis;
use openecraft_core::issue::{Issue, Severity};
use openecraft_i18n::tr_fmt;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Environment variable with folders of external plugins (separated like `PATH`).
pub const PLUGIN_PATH_VAR: &str = "OPENECRAFT_PLUGINS";

#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    BuiltIn,
    /// Folder of an external plugin.
    External(PathBuf),
}

pub struct Loaded {
    pub info: Info,
    pub origin: Origin,
    pub enabled: bool,
    pub registry: Registry,
}

#[derive(Default)]
pub struct Plugins {
    loaded: Vec<Loaded>,
    /// Plugins that could not be loaded (message per plugin).
    pub errors: Vec<String>,
    /// Command names of the front end itself, which plugins must not take.
    reserved: Vec<String>,
}

impl Plugins {
    /// The built-in plugins.
    pub fn new(reserved: impl IntoIterator<Item = impl Into<String>>) -> Plugins {
        let mut p = Plugins { reserved: reserved.into_iter().map(Into::into).collect(), ..Default::default() };
        for plugin in crate::builtin::plugins() {
            if let Err(e) = p.add(plugin.as_ref(), Origin::BuiltIn) {
                p.errors.push(e);
            }
        }
        p
    }

    /// Registers a plugin. Fails (and adds nothing) for a duplicate id or name.
    pub fn add(&mut self, plugin: &dyn Plugin, origin: Origin) -> Result<(), String> {
        let info = plugin.info();
        if !is_snake_case(&info.id) {
            return Err(tr_fmt!("Plugin id '{}' is not snake_case", info.id));
        }
        if self.loaded.iter().any(|l| l.info.id == info.id) {
            return Err(tr_fmt!("Plugin {} is loaded twice", info.id));
        }
        let mut registry = Registry::default();
        plugin.register(&mut registry).map_err(|e| tr_fmt!("Plugin {}: {}", info.id, e))?;
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for (kind, name) in registry.names() {
            let taken = seen.contains(&(kind, name))
                || self.loaded.iter().any(|l| l.registry.names().any(|n| n == (kind, name)))
                || (kind == "command" && self.reserved.iter().any(|r| r == name));
            if !is_snake_case(name) || taken {
                return Err(tr_fmt!("Plugin {}: the name '{}' is invalid or already taken", info.id, name));
            }
            seen.push((kind, name));
        }
        for c in &registry.commands {
            if let Some(p) = c.params.iter().find(|p| !is_snake_case(&p.name)) {
                return Err(tr_fmt!("Plugin {}: the name '{}' is invalid or already taken", info.id, p.name));
            }
        }
        self.loaded.push(Loaded { info, origin, enabled: true, registry });
        Ok(())
    }

    /// Loads a plugin folder or the plugins in its subfolders (each with a `plugin.json`).
    pub fn load_dir(&mut self, dir: &Path) {
        if dir.join(external::MANIFEST).is_file() {
            self.load_plugin_dir(dir);
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            self.errors.push(tr_fmt!("Cannot read the plugin folder {}", dir.display()));
            return;
        };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join(external::MANIFEST).is_file()).collect();
        dirs.sort();
        for d in dirs {
            self.load_plugin_dir(&d);
        }
    }

    fn load_plugin_dir(&mut self, dir: &Path) {
        let result = external::External::open(dir).and_then(|plugin| self.add(&plugin, Origin::External(dir.to_path_buf())));
        if let Err(e) = result {
            self.errors.push(e);
        }
    }

    /// Loads the plugin folders of [`PLUGIN_PATH_VAR`].
    pub fn load_env(&mut self) {
        if let Some(var) = std::env::var_os(PLUGIN_PATH_VAR) {
            for dir in std::env::split_paths(&var).filter(|d| !d.as_os_str().is_empty()) {
                self.load_dir(&dir);
            }
        }
    }

    pub fn list(&self) -> &[Loaded] {
        &self.loaded
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(l) = self.loaded.iter_mut().find(|l| l.info.id == id) {
            l.enabled = enabled;
        }
    }

    /// Ids of the disabled plugins (stored by the editor).
    pub fn disabled(&self) -> Vec<String> {
        self.loaded.iter().filter(|l| !l.enabled).map(|l| l.info.id.clone()).collect()
    }

    fn enabled(&self) -> impl Iterator<Item = &Registry> {
        self.loaded.iter().filter(|l| l.enabled).map(|l| &l.registry)
    }

    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.enabled().flat_map(|r| &r.commands)
    }

    pub fn command(&self, name: &str) -> Option<&Command> {
        self.commands().find(|c| c.name == name)
    }

    pub fn checks(&self) -> impl Iterator<Item = &Check> {
        self.enabled().flat_map(|r| &r.checks)
    }

    pub fn reports(&self) -> impl Iterator<Item = &Report> {
        self.enabled().flat_map(|r| &r.reports)
    }

    pub fn report(&self, name: &str) -> Option<&Report> {
        self.reports().find(|r| r.name == name)
    }

    pub fn exporters(&self) -> impl Iterator<Item = &Exporter> {
        self.enabled().flat_map(|r| &r.exporters)
    }

    pub fn exporter(&self, name: &str) -> Option<&Exporter> {
        self.exporters().find(|e| e.name == name)
    }

    /// Checks the arguments (renamed parameters are also taken by their old names) and
    /// runs the command. A command that fails leaves the project as it was.
    pub fn run(&self, command: &Command, prj: &mut Project, args: &Map<String, Value>) -> Result<Value, String> {
        let args = crate::builtin::current_names(&command.name, args);
        params::check(&command.params, &args)?;
        let before = prj.clone();
        let result = (command.run)(prj, &Args(&args));
        if result.is_err() {
            *prj = before;
        }
        result
    }

    /// Issues of all checks; a failing check is reported as error.
    /// Runs the enabled checks; the project's check settings change levels, hide
    /// switched off messages and mark accepted ones.
    /// The checks run at the same time where the platform has threads.
    pub fn check(&self, prj: &Project, an: &Analysis) -> Vec<Issue> {
        run_checks(self.checks(), prj, an)
    }
}

/// Runs `checks` that the project enables (e.g. the checks of [`Plugins::checks`] taken
/// along to another thread) with the project's settings applied to what they find.
pub fn run_checks<'a>(checks: impl IntoIterator<Item = &'a Check>, prj: &Project, an: &Analysis) -> Vec<Issue> {
    let checks: Vec<&Check> = checks.into_iter().filter(|c| openecraft_core::checks::is_enabled(prj, &c.name)).collect();
    let found = openecraft_core::analysis::par_map(&checks, |c| {
        let found = match (c.run)(prj, an) {
            Ok(found) => found,
            Err(e) => vec![openecraft_core::issue!(Severity::Error, None, "Check {} failed: {}", c.name, e)],
        };
        found
            .into_iter()
            .map(|mut i| {
                i.check = c.name.clone();
                i
            })
            .collect::<Vec<_>>()
    });
    openecraft_core::checks::apply_settings(prj, found.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests;
