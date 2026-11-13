//! Role based access control of the OpenECraft servers (parts server, project
//! server), shared by the servers (checks) and their clients (what to offer).
//!
//! Every server defines its rights ([`Right`]) and built-in roles. A [`Role`] is a
//! named set of rights; administrators add their own. A [`Grant`] gives a role to a
//! user or a group (local or of a directory such as Active Directory) on a scope: a
//! project or a manufacturer, with `*` and `?` wildcards; `*` also covers the server
//! itself ([`SERVER`]). Deny grants take rights away again, whatever the other grants
//! give. [`Policy::access`] resolves what an [`Identity`] may do; the result
//! ([`Access`]) is checked by the server and sent to the client.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt::Debug;

/// Scope of the rights on the whole server (users, roles, settings). Only grants on
/// `*` cover it.
pub const SERVER: &str = "*";

/// The rights of a server.
pub trait Right: Copy + Ord + Debug + Serialize + DeserializeOwned + 'static {
    /// All rights, in display order.
    const ALL: &'static [Self];
    /// Names of the built-in roles, lowest first.
    const ROLES: &'static [&'static str];
    /// Rights of a built-in role (case insensitive).
    fn role(name: &str) -> Option<&'static [Self]>;
    /// Name as in JSON (snake_case).
    fn name(self) -> &'static str;
    /// English title, translated where it is shown.
    fn title(self) -> &'static str;
    /// The right only applies to the whole server ([`SERVER`]).
    fn server_wide(self) -> bool;

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// A named set of rights defined by an administrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct Role<R: Right> {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub rights: BTreeSet<R>,
}

impl<R: Right> Default for Role<R> {
    fn default() -> Self {
        Role { name: String::new(), description: String::new(), rights: BTreeSet::new() }
    }
}

/// Who a grant is for: a user or a group (names with wildcards; groups of a
/// directory are qualified with its id, e.g. `corp:Engineering`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    User(String),
    Group(String),
}

impl Default for Subject {
    fn default() -> Self {
        Subject::User(String::new())
    }
}

impl Subject {
    pub fn name(&self) -> &str {
        match self {
            Subject::User(n) | Subject::Group(n) => n,
        }
    }
}

/// Gives (or with `deny` takes away) the rights of a role on a scope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grant {
    pub subject: Subject,
    /// Name of a built-in or defined role.
    pub role: String,
    /// Project or manufacturer name, `*` and `?` wildcards; empty = `*`.
    pub scope: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deny: bool,
}

impl Grant {
    pub fn new(subject: Subject, role: &str, scope: &str) -> Grant {
        Grant { subject, role: role.into(), scope: scope.into(), deny: false }
    }

    pub fn scope(&self) -> &str {
        normal_scope(&self.scope)
    }
}

fn normal_scope(scope: &str) -> &str {
    match scope.trim() {
        "" => SERVER,
        s => s,
    }
}

/// Who signed in: the user, the directory (empty for local accounts) and the groups.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Identity {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub provider: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

impl Identity {
    pub fn is(&self, subject: &Subject) -> bool {
        match subject {
            Subject::User(p) => !p.trim().is_empty() && matches(p.trim(), &self.name),
            Subject::Group(p) => !p.trim().is_empty() && self.groups.iter().any(|g| matches(p.trim(), g)),
        }
    }
}

/// A group of users defined on the server: grants to `Subject::Group(name)` apply to
/// its members (local users and users of a directory), next to the groups of a
/// directory (`directory:group`), whose names hold a `:`, which local names cannot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// User names (case insensitive).
    pub members: Vec<String>,
}

impl Group {
    pub fn has(&self, user: &str) -> bool {
        self.members.iter().any(|m| m.trim().eq_ignore_ascii_case(user.trim()))
    }

    /// Checks the name and removes empty and repeated members.
    pub fn normalized(mut self) -> Result<Group, String> {
        self.name = self.name.trim().to_string();
        check_group_name(&self.name)?;
        self.description = self.description.trim().to_string();
        let mut members: Vec<String> = Vec::new();
        for m in self.members.iter().map(|m| m.trim()).filter(|m| !m.is_empty()) {
            if !members.iter().any(|x| x.eq_ignore_ascii_case(m)) {
                members.push(m.to_string());
            }
        }
        self.members = members;
        Ok(self)
    }
}

/// A group name is not empty, has no control characters and no `:` (the separator of
/// directory groups).
pub fn check_group_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().any(|c| c.is_control() || c == ':') {
        return Err(format!("'{}' cannot be used as a group name", name.escape_debug()));
    }
    Ok(())
}

/// The names of the defined groups `user` is a member of.
pub fn groups_of(groups: &[Group], user: &str) -> Vec<String> {
    groups.iter().filter(|g| g.has(user)).map(|g| g.name.clone()).collect()
}

/// The identity with the defined groups it is a member of.
pub fn with_groups(mut who: Identity, groups: &[Group]) -> Identity {
    for g in groups_of(groups, &who.name) {
        if !who.groups.iter().any(|x| x.eq_ignore_ascii_case(&g)) {
            who.groups.push(g);
        }
    }
    who
}

/// A directory users can sign in with, as shown to clients.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Provider {
    pub id: String,
    pub title: String,
    /// `local`, `active_directory` or `ldap`.
    pub kind: String,
}

/// Access settings of a server (`GET /api/v1/access`; `PUT` takes the policy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct Settings<R: Right> {
    pub policy: Policy<R>,
    /// The directories users can sign in with (configured on the server).
    pub providers: Vec<Provider>,
    /// The groups defined on the server.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
}

impl<R: Right> Default for Settings<R> {
    fn default() -> Self {
        Settings { policy: Policy::default(), providers: Vec::new(), groups: Vec::new() }
    }
}

/// Defined roles and grants of a server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct Policy<R: Right> {
    /// Role of requests without sign-in on every scope; empty = none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub anonymous: String,
    pub roles: Vec<Role<R>>,
    pub grants: Vec<Grant>,
}

impl<R: Right> Default for Policy<R> {
    fn default() -> Self {
        Policy { anonymous: String::new(), roles: Vec::new(), grants: Vec::new() }
    }
}

impl<R: Right> Policy<R> {
    /// Rights of a built-in or defined role.
    pub fn rights_of(&self, role: &str) -> Option<BTreeSet<R>> {
        let role = role.trim();
        if let Some(rights) = R::role(role) {
            return Some(rights.iter().copied().collect());
        }
        self.roles.iter().find(|r| r.name.trim().eq_ignore_ascii_case(role)).map(|r| r.rights.clone())
    }

    /// Names of all roles: the built-in ones, then the defined ones.
    pub fn role_names(&self) -> Vec<String> {
        R::ROLES.iter().map(|r| r.to_string()).chain(self.roles.iter().map(|r| r.name.clone())).collect()
    }

    /// What `who` may do: the grants of the policy that apply to them and `own`
    /// (the grants of their account, e.g. its role).
    pub fn access(&self, who: &Identity, own: &[Grant]) -> Access<R> {
        let mut entries = Vec::new();
        for g in own.iter().chain(self.grants.iter().filter(|g| who.is(&g.subject))) {
            if let Some(rights) = self.rights_of(&g.role).filter(|r| !r.is_empty()) {
                entries.push(Entry { scope: g.scope().to_string(), rights, deny: g.deny });
            }
        }
        Access { entries }
    }

    /// What requests without sign-in may do.
    pub fn anonymous_access(&self) -> Access<R> {
        let rights = self.rights_of(&self.anonymous).unwrap_or_default();
        let entries = if rights.is_empty() { Vec::new() } else { vec![Entry { scope: SERVER.into(), rights, deny: false }] };
        Access { entries }
    }

    /// The grants whose scope is exactly `scope` (e.g. those of one project).
    pub fn grants_on(&self, scope: &str) -> Vec<Grant> {
        self.grants.iter().filter(|g| g.scope().eq_ignore_ascii_case(normal_scope(scope))).cloned().collect()
    }

    /// Replaces the grants whose scope is exactly `scope`.
    pub fn set_grants_on(&mut self, scope: &str, grants: Vec<Grant>) {
        let scope = normal_scope(scope).to_string();
        self.grants.retain(|g| !g.scope().eq_ignore_ascii_case(&scope));
        self.grants.extend(grants.into_iter().map(|g| Grant { scope: scope.clone(), ..g }));
    }

    /// Checks names and references: role names are unique and not those of built-in
    /// roles, grants name a user or group and an existing role.
    pub fn validate(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        for r in &self.roles {
            let name = r.name.trim();
            if name.is_empty() || name.chars().any(|c| c.is_control()) {
                return Err("a role needs a name".into());
            }
            if R::role(name).is_some() {
                return Err(format!("'{name}' is the name of a built-in role"));
            }
            if !names.insert(name.to_lowercase()) {
                return Err(format!("role '{name}' is defined twice"));
            }
        }
        if !self.anonymous.trim().is_empty() && self.rights_of(&self.anonymous).is_none() {
            return Err(format!("unknown role '{}'", self.anonymous.trim()));
        }
        for g in &self.grants {
            if g.subject.name().trim().is_empty() {
                return Err("a grant needs a user or a group".into());
            }
            if self.rights_of(&g.role).is_none() {
                return Err(format!("unknown role '{}'", g.role.trim()));
            }
        }
        Ok(())
    }
}

/// Rights given (or denied) on a scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct Entry<R: Right> {
    pub scope: String,
    pub rights: BTreeSet<R>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deny: bool,
}

impl<R: Right> Default for Entry<R> {
    fn default() -> Self {
        Entry { scope: SERVER.into(), rights: BTreeSet::new(), deny: false }
    }
}

impl<R: Right> Entry<R> {
    fn covers(&self, scope: &str) -> bool {
        let pattern = normal_scope(&self.scope);
        if scope == SERVER {
            pattern == SERVER
        } else {
            matches(pattern, scope)
        }
    }
}

/// What a caller may do: rights per scope, as resolved by [`Policy::access`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "R: Right")]
pub struct Access<R: Right> {
    pub entries: Vec<Entry<R>>,
}

impl<R: Right> Default for Access<R> {
    fn default() -> Self {
        Access { entries: Vec::new() }
    }
}

impl<R: Right> Access<R> {
    /// Every right everywhere (the server token, servers without accounts).
    pub fn full() -> Access<R> {
        Access { entries: vec![Entry { scope: SERVER.into(), rights: R::ALL.iter().copied().collect(), deny: false }] }
    }

    /// The rights on a scope (a project, a manufacturer or [`SERVER`]): what the
    /// grants covering it give, without what deny grants covering it take away.
    pub fn rights(&self, scope: &str) -> BTreeSet<R> {
        self.rights_on(&[scope])
    }

    /// The rights on several scopes together (a part: its manufacturer and its
    /// catalog): what grants covering any of them give, without what deny grants
    /// covering any of them take away.
    pub fn rights_on(&self, scopes: &[&str]) -> BTreeSet<R> {
        let scopes: Vec<&str> = scopes.iter().map(|s| s.trim()).collect();
        let server = scopes.contains(&SERVER);
        let covering = || self.entries.iter().filter(|e| scopes.iter().any(|s| e.covers(s)));
        let denied: BTreeSet<R> = covering().filter(|e| e.deny).flat_map(|e| e.rights.iter().copied()).collect();
        covering()
            .filter(|e| !e.deny)
            .flat_map(|e| e.rights.iter().copied())
            .filter(|r| !denied.contains(r) && (server || !r.server_wide()))
            .collect()
    }

    pub fn allows(&self, scope: &str, right: R) -> bool {
        self.allows_on(&[scope], right)
    }

    /// The right on any of the scopes, unless a deny grant covers one of them.
    pub fn allows_on(&self, scopes: &[&str], right: R) -> bool {
        if right.server_wide() {
            return self.rights(SERVER).contains(&right);
        }
        self.rights_on(scopes).contains(&right)
    }

    /// The right is given on some scope (to offer a command at all).
    pub fn anywhere(&self, right: R) -> bool {
        self.entries.iter().any(|e| !e.deny && e.rights.contains(&right))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.deny || e.rights.is_empty())
    }

    /// Both rights together (e.g. of the account and of anonymous requests).
    pub fn with(mut self, other: Access<R>) -> Access<R> {
        self.entries.extend(other.entries);
        self
    }
}

/// Case-insensitive match with `*` (any text) and `?` (one character).
pub fn matches(pattern: &str, text: &str) -> bool {
    crate::wildcard(&pattern.trim().to_lowercase(), &text.trim().to_lowercase())
}

#[cfg(test)]
mod tests;
