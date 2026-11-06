//! Local copy of the server's parts, catalogs and users and what is being edited.
//! No UI code, so the editing rules (new parts, renames, unsaved changes, catalogs,
//! users) are unit testable.

use openecraft_parts::api::{ServerInfo, UserUpdate};
use openecraft_parts::portal::{catalog_id, Catalog, HistoryEntry, Role, User};
use openecraft_parts::{natural_cmp, unique_number, Part, PartsDb};
use openecraft_edz::EdzFile;
use openecraft_ui::client::Reply;

/// The part in the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// Part number on the server; `None` for a part that was never saved.
    pub original: Option<String>,
    pub part: Part,
}

/// Request the application has to send after a server reply.
#[derive(Clone, Debug, PartialEq)]
pub enum FollowUp {
    /// Load all parts again.
    Reload,
    /// Remove the old number of a renamed part.
    Delete(String),
    /// Load all parts again and store these files with their parts (pictures and
    /// macros of an imported EDZ file).
    Attach(Vec<EdzFile>),
}

/// A catalog being edited; `original` is its id on the server (`None`: new).
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogDraft {
    pub original: Option<String>,
    pub catalog: Catalog,
}

/// A user being edited. An empty password keeps the current one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserDraft {
    pub new: bool,
    pub name: String,
    pub update: UserUpdate,
    /// Directory of the user; empty for a local account.
    pub provider: String,
    /// Local groups, separated by commas (a directory sets the groups of its users).
    pub groups: String,
}

impl UserDraft {
    /// The request to save the user.
    pub fn request(&self) -> UserUpdate {
        let groups = self.groups.split(',').map(str::trim).filter(|g| !g.is_empty()).map(String::from).collect();
        let local = self.provider.is_empty();
        UserUpdate { groups: local.then_some(groups), password: if local { self.update.password.clone() } else { String::new() }, ..self.update.clone() }
    }
}

#[derive(Default)]
pub struct Session {
    /// Parts on the server (as of the last download).
    pub parts: Vec<Part>,
    pub revision: u64,
    pub loaded: bool,
    pub draft: Option<Draft>,
    pub catalogs: Vec<Catalog>,
    pub catalog: Option<CatalogDraft>,
    pub users: Vec<User>,
    pub user: Option<UserDraft>,
    /// Changes of the open part (loaded on request).
    pub history: Option<(String, Vec<HistoryEntry>)>,
    /// Files of an EDZ file being uploaded, stored with their parts once the parts are.
    pub pending_files: Vec<EdzFile>,
}

impl Session {
    pub fn get(&self, number: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.number == number)
    }

    /// Server version of the draft.
    fn saved(&self) -> Option<&Part> {
        self.draft.as_ref()?.original.as_deref().and_then(|n| self.get(n))
    }

    /// True if the draft has changes that are not on the server.
    pub fn dirty(&self) -> bool {
        match &self.draft {
            None => false,
            Some(d) => self.saved() != Some(&d.part),
        }
    }

    /// The draft's part number belongs to another part on the server.
    pub fn number_taken(&self) -> bool {
        // Like the server: numbers that differ only in case are the same.
        let key = |n: &str| n.to_lowercase();
        self.draft.as_ref().is_some_and(|d| {
            d.original.as_deref() != Some(d.part.number.as_str()) && self.parts.iter().any(|p| key(&p.number) == key(&d.part.number))
        })
    }

    /// Why the draft cannot be saved, if it cannot.
    pub fn save_problem(&self) -> Option<&'static str> {
        let d = self.draft.as_ref()?;
        if d.part.number.trim().is_empty() {
            Some("The part number must not be empty")
        } else if self.number_taken() {
            Some("Part number already exists")
        } else {
            None
        }
    }

    pub fn selected(&self) -> Option<&str> {
        self.draft.as_ref().map(|d| d.original.as_deref().unwrap_or(&d.part.number))
    }

    /// True if the caller (`info`: the server's answer for the token) may change the
    /// part in the editor: a saved part of a manufacturer they may edit, or a new part
    /// when they may edit any. Otherwise the editor only shows it.
    pub fn may_change(&self, info: Option<&ServerInfo>) -> bool {
        let (Some(info), Some(d)) = (info, self.draft.as_ref()) else { return false };
        match &d.original {
            Some(n) => info.may_edit(self.get(n).map_or(d.part.manufacturer.as_str(), |p| p.manufacturer.as_str())),
            None => info.can_edit(),
        }
    }

    /// Opens a server part in the editor.
    pub fn select(&mut self, number: &str) {
        self.draft = self.get(number).map(|p| Draft { original: Some(p.number.clone()), part: p.clone() });
    }

    /// Discards the changes of the draft.
    pub fn revert(&mut self) {
        match self.draft.as_ref().and_then(|d| d.original.clone()) {
            Some(n) => self.select(&n),
            None => self.draft = None,
        }
    }

    /// Starts a new part.
    pub fn new_part(&mut self) {
        let number = unique_number(&self.parts, "NEW.PART");
        self.draft = Some(Draft { original: None, part: Part { number, manufacturer: "NEW".into(), ..Default::default() } });
    }

    /// Starts a new part as a copy of the draft.
    pub fn copy_part(&mut self) {
        if let Some(d) = &self.draft {
            let mut part = d.part.clone();
            part.number = unique_number(&self.parts, &format!("{}_COPY", part.number));
            self.draft = Some(Draft { original: None, part });
        }
    }

    /// All parts as a parts database (export).
    pub fn database(&self) -> PartsDb {
        PartsDb { parts: self.parts.clone() }
    }

    /// Opens a server catalog, or starts a new one (`None`) for `manufacturer`.
    pub fn select_catalog(&mut self, id: Option<&str>, manufacturer: &str) {
        self.catalog = match id {
            Some(id) => self.catalogs.iter().find(|c| c.id == id).map(|c| CatalogDraft { original: Some(c.id.clone()), catalog: c.clone() }),
            None => Some(CatalogDraft { original: None, catalog: Catalog { manufacturer: manufacturer.into(), ..Default::default() } }),
        };
    }

    /// The catalog to save, with an id derived from the name for new catalogs; or
    /// why it cannot be saved.
    pub fn catalog_to_save(&self) -> Result<Catalog, &'static str> {
        let d = self.catalog.as_ref().ok_or("No catalog open")?;
        let mut c = d.catalog.clone();
        if c.id.trim().is_empty() {
            c.id = catalog_id(&c.name);
        }
        if c.id.is_empty() {
            return Err("The catalog needs a name");
        }
        if d.original.as_deref() != Some(c.id.as_str()) && self.catalogs.iter().any(|x| x.id.eq_ignore_ascii_case(&c.id)) {
            return Err("A catalog with this id exists already");
        }
        Ok(c)
    }

    pub fn select_user(&mut self, name: Option<&str>) {
        self.user = match name {
            Some(n) => self.users.iter().find(|u| u.name == n).map(|u| UserDraft {
                new: false,
                name: u.name.clone(),
                update: UserUpdate { role: u.role, manufacturer: u.manufacturer.clone(), password: String::new(), groups: None },
                provider: u.provider.clone(),
                groups: u.groups.join(", "),
            }),
            None => Some(UserDraft { new: true, ..Default::default() }),
        };
    }

    /// Why the user cannot be saved, if it cannot.
    pub fn user_problem(&self) -> Option<&'static str> {
        let d = self.user.as_ref()?;
        if d.name.trim().is_empty() || d.name.chars().any(char::is_whitespace) {
            Some("The user name must not be empty or contain blanks")
        } else if d.new && self.users.iter().any(|u| u.name.eq_ignore_ascii_case(d.name.trim())) {
            Some("A user with this name exists already")
        } else if d.new && d.update.password.is_empty() {
            Some("A new user needs a password")
        } else if d.update.role == Role::Manufacturer && d.update.manufacturer.trim().is_empty() {
            Some("A manufacturer user needs a manufacturer")
        } else {
            None
        }
    }

    /// Updates the local copy with a server reply.
    /// The parts of an EDZ file to upload; its files wait for the upload.
    pub fn import_package(&mut self, path: &str, bytes: &[u8]) -> Result<Vec<Part>, String> {
        let edz = openecraft_edz::folder::read_file(std::path::Path::new(path),bytes)?;
        self.pending_files = edz.files;
        Ok(edz.parts)
    }

    pub fn apply(&mut self, reply: &Reply) -> Option<FollowUp> {
        match reply {
            Reply::Info(Ok(info)) if !self.loaded || info.revision != self.revision => Some(FollowUp::Reload),
            Reply::Parts(Ok(list)) => {
                let keep = self.dirty();
                self.parts = list.parts.clone();
                self.parts.sort_by(|a, b| natural_cmp(&a.number, &b.number));
                self.revision = list.revision;
                self.loaded = true;
                if !keep {
                    // Show the current server state of the selected part.
                    match self.draft.as_ref().and_then(|d| d.original.clone()) {
                        Some(n) => self.select(&n),
                        None => self.draft = None,
                    }
                }
                None
            }
            Reply::Saved(Ok(part)) => {
                match self.parts.iter_mut().find(|p| p.number == part.number) {
                    Some(p) => *p = (**part).clone(),
                    None => {
                        self.parts.push((**part).clone());
                        self.parts.sort_by(|a, b| natural_cmp(&a.number, &b.number));
                    }
                }
                let mut follow = None;
                if let Some(d) = self.draft.as_mut().filter(|d| d.part.number == part.number) {
                    if let Some(old) = d.original.replace(part.number.clone()).filter(|old| *old != part.number) {
                        follow = Some(FollowUp::Delete(old));
                    }
                    d.part = (**part).clone();
                }
                follow
            }
            Reply::Deleted(number, Ok(())) => {
                self.parts.retain(|p| &p.number != number);
                if self.draft.as_ref().is_some_and(|d| d.original.as_ref() == Some(number)) {
                    self.draft = None;
                }
                None
            }
            Reply::Imported(Ok(_)) if !self.pending_files.is_empty() => Some(FollowUp::Attach(std::mem::take(&mut self.pending_files))),
            Reply::Imported(Ok(_)) => Some(FollowUp::Reload),
            Reply::Imported(Err(_)) => {
                self.pending_files.clear();
                None
            }
            Reply::History(number, Ok(h)) => {
                self.history = Some((number.clone(), h.clone()));
                None
            }
            Reply::Catalogs(Ok(list)) => {
                self.catalogs = list.clone();
                if let Some(original) = self.catalog.as_ref().and_then(|d| d.original.clone()) {
                    let changed = self.catalog.as_ref().is_some_and(|d| self.catalogs.iter().find(|c| c.id == original) != Some(&d.catalog));
                    if !changed {
                        self.select_catalog(Some(&original), "");
                    }
                }
                None
            }
            Reply::CatalogSaved(Ok(c)) => {
                self.catalogs.retain(|x| !x.id.eq_ignore_ascii_case(&c.id));
                self.catalogs.push(c.clone());
                self.catalogs.sort_by_key(|c| c.name.to_lowercase());
                self.catalog = Some(CatalogDraft { original: Some(c.id.clone()), catalog: c.clone() });
                None
            }
            Reply::CatalogDeleted(id, Ok(())) => {
                self.catalogs.retain(|c| &c.id != id);
                if self.catalog.as_ref().is_some_and(|d| d.original.as_ref() == Some(id)) {
                    self.catalog = None;
                }
                None
            }
            Reply::Users(Ok(list)) => {
                self.users = list.clone();
                None
            }
            Reply::UserSaved(Ok(u)) => {
                self.users.retain(|x| x.name != u.name);
                self.users.push(u.clone());
                self.users.sort_by_key(|u| u.name.to_lowercase());
                self.select_user(Some(&u.name));
                None
            }
            Reply::UserDeleted(name, Ok(())) => {
                self.users.retain(|u| &u.name != name);
                if self.user.as_ref().is_some_and(|d| &d.name == name) {
                    self.user = None;
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
