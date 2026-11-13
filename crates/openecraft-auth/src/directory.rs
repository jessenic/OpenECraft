//! Sign-in with a directory: Active Directory or another LDAP server (OpenLDAP,
//! 389 Directory Server, FreeIPA, …). The server looks the user up (with a service
//! account or as the user), checks the password with a bind as the user and reads
//! the groups, which grants refer to as `directory:group`.

use ldap3::{dn_escape, ldap_escape, Ldap, LdapConnAsync, LdapConnSettings, LdapError, Scope, SearchEntry};
use openecraft_parts::access::{matches, Identity, Provider};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// LDAP result code of a failed bind (wrong password, disabled or locked account).
const INVALID_CREDENTIALS: u32 = 49;
/// Matching rule of Active Directory for nested group membership.
const IN_CHAIN: &str = "1.2.840.113556.1.4.1941";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryKind {
    #[default]
    ActiveDirectory,
    Ldap,
}

impl DirectoryKind {
    pub fn name(self) -> &'static str {
        match self {
            DirectoryKind::ActiveDirectory => "active_directory",
            DirectoryKind::Ldap => "ldap",
        }
    }
}

/// A directory of the server's access settings (`directories` in `access.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Directory {
    /// Short name; qualifies the group names (`corp:Engineering`) and is shown on the
    /// accounts of its users.
    pub id: String,
    pub title: String,
    pub kind: DirectoryKind,
    /// `ldap://dc1.corp.example` or `ldaps://dc1.corp.example:636`.
    pub url: String,
    /// Upgrade an `ldap://` connection with StartTLS.
    pub starttls: bool,
    /// Accept any server certificate (test installations only).
    pub insecure_tls: bool,
    /// Service account for looking users up; empty: bind as the user ([`Self::user_bind`]).
    pub bind_dn: String,
    pub bind_password: String,
    /// Environment variable with the password of the service account (instead of
    /// `bind_password` in the file).
    pub bind_password_env: String,
    /// Bind name of a user without service account, `{user}` is replaced:
    /// `{user}@corp.example` (Active Directory) or `uid={user},ou=people,dc=example,dc=org`.
    pub user_bind: String,
    /// Where users are searched.
    pub base_dn: String,
    /// Filter of a user entry, `{user}` is replaced. Default: Active Directory
    /// `(&(objectClass=user)(sAMAccountName={user}))`, LDAP `(&(objectClass=person)(uid={user}))`.
    pub user_filter: String,
    /// Attribute with the account name. Default: `sAMAccountName` / `uid`.
    pub name_attribute: String,
    /// Also groups of groups (Active Directory).
    pub nested_groups: bool,
    /// Filter of the groups of a user (`{dn}` and `{user}` are replaced), e.g.
    /// `(&(objectClass=groupOfNames)(member={dn}))`; empty: the `memberOf` attribute.
    pub group_filter: String,
    /// Where groups are searched; empty = `base_dn`.
    pub group_base_dn: String,
    /// Only members of these groups (names, wildcards) may sign in; empty = all.
    pub allowed_groups: Vec<String>,
    pub timeout_secs: u64,
}

impl Default for Directory {
    fn default() -> Self {
        Directory {
            id: String::new(),
            title: String::new(),
            kind: DirectoryKind::default(),
            url: String::new(),
            starttls: false,
            insecure_tls: false,
            bind_dn: String::new(),
            bind_password: String::new(),
            bind_password_env: String::new(),
            user_bind: String::new(),
            base_dn: String::new(),
            user_filter: String::new(),
            name_attribute: String::new(),
            nested_groups: false,
            group_filter: String::new(),
            group_base_dn: String::new(),
            allowed_groups: Vec::new(),
            timeout_secs: 10,
        }
    }
}

/// Why a sign-in failed.
#[derive(Clone, Debug, PartialEq)]
pub enum SignInError {
    /// The directory does not know the user.
    Unknown,
    /// Wrong password, disabled account or not in an allowed group.
    Refused,
    /// The directory cannot be reached or is configured wrongly.
    Unavailable(String),
}

impl From<LdapError> for SignInError {
    fn from(e: LdapError) -> Self {
        SignInError::Unavailable(e.to_string())
    }
}

impl Directory {
    pub fn provider(&self) -> Provider {
        let title = if self.title.trim().is_empty() { self.id.clone() } else { self.title.clone() };
        Provider { id: self.id.clone(), title, kind: self.kind.name().into() }
    }

    pub fn check(&self) -> Result<(), String> {
        let id = self.id.trim();
        if id.is_empty() || id.chars().any(|c| c == ':' || c.is_whitespace() || c.is_control()) {
            return Err(format!("directory '{id}': the id must not be empty or contain blanks or ':'"));
        }
        if !(self.url.starts_with("ldap://") || self.url.starts_with("ldaps://")) {
            return Err(format!("directory '{id}': the url must start with ldap:// or ldaps://"));
        }
        if self.bind_dn.trim().is_empty() && self.user_bind.trim().is_empty() {
            return Err(format!("directory '{id}': set bind_dn (service account) or user_bind"));
        }
        if self.base_dn.trim().is_empty() {
            return Err(format!("directory '{id}': base_dn is missing"));
        }
        Ok(())
    }

    fn bind_password(&self) -> String {
        match self.bind_password_env.trim() {
            "" => self.bind_password.clone(),
            var => std::env::var(var).unwrap_or_default(),
        }
    }

    /// The filter of the user's entry.
    pub fn user_filter(&self, user: &str) -> String {
        let filter = match (self.user_filter.trim(), self.kind) {
            ("", DirectoryKind::ActiveDirectory) => "(&(objectClass=user)(sAMAccountName={user}))",
            ("", DirectoryKind::Ldap) => "(&(objectClass=person)(uid={user}))",
            (f, _) => f,
        };
        filter.replace("{user}", &ldap_escape(user))
    }

    fn name_attribute(&self) -> &str {
        match (self.name_attribute.trim(), self.kind) {
            ("", DirectoryKind::ActiveDirectory) => "sAMAccountName",
            ("", DirectoryKind::Ldap) => "uid",
            (a, _) => a,
        }
    }

    /// Bind name of the user without service account.
    pub fn user_bind(&self, user: &str) -> String {
        let value = if self.user_bind.contains('=') { dn_escape(user) } else { user.into() };
        self.user_bind.replace("{user}", &value)
    }

    /// The filter of the user's groups (searched instead of reading `memberOf`).
    pub fn group_filter(&self, dn: &str, user: &str) -> Option<String> {
        if !self.group_filter.trim().is_empty() {
            return Some(self.group_filter.replace("{dn}", &ldap_escape(dn)).replace("{user}", &ldap_escape(user)));
        }
        (self.nested_groups && self.kind == DirectoryKind::ActiveDirectory).then(|| format!("(&(objectClass=group)(member:{IN_CHAIN}:={}))", ldap_escape(dn)))
    }

    /// Group names as grants see them: `directory:group`.
    pub fn qualify(&self, groups: impl IntoIterator<Item = String>) -> Vec<String> {
        let mut out: Vec<String> = groups.into_iter().map(|g| format!("{}:{g}", self.id.trim())).collect();
        out.sort_by_key(|g| g.to_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
    }

    /// The groups let the user sign in.
    pub fn allows(&self, groups: &[String]) -> bool {
        self.allowed_groups.is_empty()
            || self.allowed_groups.iter().any(|p| groups.iter().any(|g| matches(p, g) || matches(&format!("{}:{}", self.id.trim(), p.trim()), g)))
    }

    /// Signs the user in: their identity with the qualified groups.
    pub async fn authenticate(&self, user: &str, password: &str) -> Result<Identity, SignInError> {
        let user = user.trim();
        // An empty password is an anonymous bind, which LDAP servers accept.
        if user.is_empty() || password.is_empty() {
            return Err(SignInError::Refused);
        }
        let (mut ldap, timeout) = self.connect().await?;
        let result = self.look_up(&mut ldap, timeout, user, password).await;
        let _ = ldap.unbind().await;
        let (name, groups) = result?;
        let groups = self.qualify(groups);
        if !self.allows(&groups) {
            return Err(SignInError::Refused);
        }
        Ok(Identity { name, provider: self.id.trim().to_string(), groups })
    }

    async fn connect(&self) -> Result<(Ldap, Duration), SignInError> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let settings = LdapConnSettings::new().set_conn_timeout(timeout).set_starttls(self.starttls).set_no_tls_verify(self.insecure_tls);
        let (conn, ldap) = LdapConnAsync::with_settings(settings, &self.url).await?;
        ldap3::drive!(conn);
        Ok((ldap, timeout))
    }

    /// Checks the settings and the connection: connects and, with a service account,
    /// binds with it and reads the entry of `base_dn`. Returns what was checked.
    pub async fn test_connection(&self) -> Result<String, SignInError> {
        self.check().map_err(SignInError::Unavailable)?;
        let (mut ldap, timeout) = self.connect().await?;
        let result = async {
            let dn = self.bind_dn.trim();
            if dn.is_empty() {
                return Ok(format!("connected to {}", self.url));
            }
            let bound = ldap.with_timeout(timeout).simple_bind(dn, &self.bind_password()).await?;
            if bound.rc != 0 {
                return Err(SignInError::Unavailable(format!("bind as {dn} failed: {bound}")));
            }
            let base = self.base_dn.trim();
            ldap.with_timeout(timeout).search(base, Scope::Base, "(objectClass=*)", vec!["1.1"]).await?.success()?;
            Ok(format!("connected to {} as {dn}, {base} found", self.url))
        }
        .await;
        let _ = ldap.unbind().await;
        result
    }

    /// True if the password of the service account is set (in the file or in its
    /// environment variable).
    pub fn has_bind_password(&self) -> bool {
        !self.bind_password().is_empty()
    }

    async fn look_up(&self, ldap: &mut Ldap, timeout: Duration, user: &str, password: &str) -> Result<(String, Vec<String>), SignInError> {
        let service = !self.bind_dn.trim().is_empty();
        let (dn, pw) = if service { (self.bind_dn.trim().to_string(), self.bind_password()) } else { (self.user_bind(user), password.to_string()) };
        let bound = ldap.with_timeout(timeout).simple_bind(&dn, &pw).await?;
        match bound.rc {
            0 => {}
            INVALID_CREDENTIALS if !service => return Err(SignInError::Refused),
            _ => return Err(SignInError::Unavailable(format!("bind as {dn} failed: {bound}"))),
        }
        let name_attr = self.name_attribute().to_string();
        let attrs = vec![name_attr.as_str(), "memberOf"];
        let (entries, _) = ldap.with_timeout(timeout).search(self.base_dn.trim(), Scope::Subtree, &self.user_filter(user), attrs).await?.success()?;
        let mut entries: Vec<SearchEntry> = entries.into_iter().filter(|e| !e.is_ref()).map(SearchEntry::construct).collect();
        let entry = match entries.len() {
            0 => return Err(SignInError::Unknown),
            1 => entries.remove(0),
            n => return Err(SignInError::Unavailable(format!("{n} entries match the user '{user}'"))),
        };
        let attr = |name: &str| entry.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone()).unwrap_or_default();
        let name = attr(&name_attr).into_iter().next().unwrap_or_else(|| user.to_string());
        let groups = match self.group_filter(&entry.dn, user) {
            Some(filter) => {
                let base = if self.group_base_dn.trim().is_empty() { self.base_dn.trim() } else { self.group_base_dn.trim() };
                let (found, _) = ldap.with_timeout(timeout).search(base, Scope::Subtree, &filter, vec!["cn"]).await?.success()?;
                found.into_iter().filter(|e| !e.is_ref()).map(SearchEntry::construct).filter_map(|e| e.attrs.get("cn").and_then(|v| v.first()).cloned().or_else(|| common_name(&e.dn))).collect()
            }
            None => attr("memberOf").iter().filter_map(|dn| common_name(dn)).collect(),
        };
        if service {
            let checked = ldap.with_timeout(timeout).simple_bind(&entry.dn, password).await?;
            match checked.rc {
                0 => {}
                INVALID_CREDENTIALS => return Err(SignInError::Refused),
                _ => return Err(SignInError::Unavailable(format!("bind as {} failed: {checked}", entry.dn))),
            }
        }
        Ok((name, groups))
    }
}

/// The value of the first RDN of a DN (`CN=Smith\, John,OU=…` → `Smith, John`).
pub fn common_name(dn: &str) -> Option<String> {
    let mut chars = dn.chars();
    let mut bytes = Vec::new();
    let push = |c: char, bytes: &mut Vec<u8>| bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = chars.next()?;
                if next.is_ascii_hexdigit() {
                    let low = chars.next()?;
                    bytes.push(u8::from_str_radix(&format!("{next}{low}"), 16).ok()?);
                } else {
                    push(next, &mut bytes);
                }
            }
            ',' | '+' => break,
            c => push(c, &mut bytes),
        }
    }
    let rdn = String::from_utf8_lossy(&bytes);
    let (_, value) = rdn.split_once('=')?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Signs in with the directories in order (or only the one with the id `provider`):
/// the first that knows the user decides.
pub async fn sign_in(directories: &[Directory], provider: &str, user: &str, password: &str) -> Result<Identity, SignInError> {
    let mut unavailable = None;
    for d in directories.iter().filter(|d| provider.trim().is_empty() || d.id.trim().eq_ignore_ascii_case(provider.trim())) {
        match d.authenticate(user, password).await {
            Ok(identity) => return Ok(identity),
            Err(SignInError::Unknown) => {}
            Err(SignInError::Unavailable(e)) => {
                log::warn!("directory {}: {e}", d.id);
                unavailable = Some(SignInError::Unavailable(format!("directory {} is not available", d.id)));
            }
            Err(e) => return Err(e),
        }
    }
    Err(unavailable.unwrap_or(SignInError::Unknown))
}

#[cfg(test)]
mod tests;
