//! Access settings of a server in a JSON file (`access.json`): the directories users
//! sign in with and the policy (defined roles, grants, role of anonymous requests).
//! Written atomically when an administrator changes the policy.
//!
//! ```json
//! {
//!   "directories": [{ "id": "corp", "url": "ldaps://dc1.corp.example", "base_dn": "DC=corp,DC=example",
//!                     "bind_dn": "CN=svc-openecraft,OU=Service,DC=corp,DC=example",
//!                     "bind_password_env": "OPENECRAFT_LDAP_PASSWORD", "nested_groups": true }],
//!   "groups": [{ "name": "Wiring", "description": "Panel shop", "members": ["anna", "bob"] }],
//!   "roles": [{ "name": "checker", "rights": ["view", "review"] }],
//!   "grants": [{ "subject": { "group": "corp:Engineering" }, "role": "editor", "scope": "Plant*" },
//!              { "subject": { "group": "corp:Contractors" }, "role": "editor", "scope": "Secret*", "deny": true }]
//! }
//! ```

use crate::directory::Directory;
use openecraft_parts::access::{Group, Policy, Right, Settings};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct AccessConfig<R: Right> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub directories: Vec<Directory>,
    /// Groups of users defined by administrators (parts server); grants to a group
    /// apply to its members.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(flatten)]
    pub policy: Policy<R>,
    /// Anybody may create a local account on the sign-in page (parts server; its
    /// start options can override it).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub registration: bool,
    /// Registered accounts wait for the approval of an administrator (the default).
    #[serde(skip_serializing_if = "is_true")]
    pub registration_approval: bool,
    /// The dashboard of the web page is shown (parts server, the default; its start
    /// options can override it).
    #[serde(skip_serializing_if = "is_true")]
    pub dashboard: bool,
}

fn is_true(b: &bool) -> bool {
    *b
}

impl<R: Right> Default for AccessConfig<R> {
    fn default() -> Self {
        AccessConfig { directories: Vec::new(), groups: Vec::new(), policy: Policy::default(), registration: false, registration_approval: true, dashboard: true }
    }
}

impl<R: Right> AccessConfig<R> {
    pub fn check(&self) -> Result<(), String> {
        self.policy.validate()?;
        check_directories(&self.directories)?;
        check_groups(&self.groups)
    }
}

/// Directories are complete and their ids unique.
pub fn check_directories(directories: &[Directory]) -> Result<(), String> {
    for (i, d) in directories.iter().enumerate() {
        d.check()?;
        if directories[..i].iter().any(|o| o.id.trim().eq_ignore_ascii_case(d.id.trim())) {
            return Err(format!("directory '{}' is defined twice", d.id.trim()));
        }
    }
    Ok(())
}

/// Group names are valid and unique.
pub fn check_groups(groups: &[Group]) -> Result<(), String> {
    for (i, g) in groups.iter().enumerate() {
        openecraft_parts::access::check_group_name(&g.name)?;
        if groups[..i].iter().any(|o| o.name.trim().eq_ignore_ascii_case(g.name.trim())) {
            return Err(format!("group '{}' is defined twice", g.name.trim()));
        }
    }
    Ok(())
}

/// The access settings of a server and where they are stored.
#[derive(Clone, Debug)]
pub struct AccessFile<R: Right> {
    path: Option<PathBuf>,
    pub config: AccessConfig<R>,
}

impl<R: Right> AccessFile<R> {
    /// Settings that are not stored (tests).
    pub fn in_memory(config: AccessConfig<R>) -> AccessFile<R> {
        AccessFile { path: None, config }
    }

    /// Reads the file; a missing file gives `default` (written on the first change).
    pub fn open(path: &Path, default: AccessConfig<R>) -> io::Result<AccessFile<R>> {
        let invalid = |e: String| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display()));
        let config = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str::<AccessConfig<R>>(&text).map_err(|e| invalid(e.to_string()))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => default,
            Err(e) => return Err(e),
        };
        config.check().map_err(invalid)?;
        for d in config.directories.iter().filter(|d| !d.bind_password.is_empty()) {
            log::warn!("directory {}: the password of the service account is in {}, better use bind_password_env", d.id, path.display());
        }
        Ok(AccessFile { path: Some(path.to_path_buf()), config })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        write_private(path, serde_json::to_string_pretty(&self.config).unwrap_or_default().as_bytes())
    }

    /// Replaces the policy (checked) and stores it.
    pub fn set_policy(&mut self, policy: Policy<R>) -> Result<(), String> {
        policy.validate()?;
        self.config.policy = policy;
        self.save().map_err(|e| format!("cannot store the access settings: {e}"))
    }

    /// Opens or closes the registration of new users, with or without the approval of
    /// an administrator, and stores it.
    pub fn set_registration(&mut self, open: bool, approval: bool) -> Result<(), String> {
        self.config.registration = open;
        self.config.registration_approval = approval;
        self.save().map_err(|e| format!("cannot store the access settings: {e}"))
    }

    /// Replaces the directories (checked) and stores them.
    pub fn set_directories(&mut self, directories: Vec<Directory>) -> Result<(), String> {
        check_directories(&directories)?;
        self.config.directories = directories;
        self.save().map_err(|e| format!("cannot store the access settings: {e}"))
    }

    /// Replaces the groups (checked) and stores them.
    pub fn set_groups(&mut self, groups: Vec<Group>) -> Result<(), String> {
        check_groups(&groups)?;
        self.config.groups = groups;
        self.save().map_err(|e| format!("cannot store the access settings: {e}"))
    }

    /// Shows or hides the dashboard and stores it.
    pub fn set_dashboard(&mut self, enabled: bool) -> Result<(), String> {
        self.config.dashboard = enabled;
        self.save().map_err(|e| format!("cannot store the access settings: {e}"))
    }

    /// What administrators see: the policy, the directories (without their settings)
    /// and the groups.
    pub fn settings(&self) -> Settings<R> {
        Settings {
            policy: self.config.policy.clone(),
            providers: self.config.directories.iter().map(Directory::provider).collect(),
            groups: self.config.groups.clone(),
        }
    }
}

/// Writes a file with secrets (password hashes, sessions, access settings): first a
/// temporary file, then renamed, so a crash never leaves a half written file. On Unix
/// only the owner may read it.
pub fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests;
