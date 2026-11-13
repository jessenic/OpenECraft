//! Groups of users defined on the server (`groups` of the access settings): who is a
//! member, the changes of a user's memberships and the adoption of the groups that
//! local accounts named before groups were defined on the server.

use crate::auth::Account;
use openecraft_parts::access::{groups_of, Group};
use openecraft_parts::portal::User;

/// Groups of a directory are qualified with its id (`corp:Engineering`).
pub fn is_directory_group(name: &str) -> bool {
    name.contains(':')
}

/// The user as clients see it: its groups with those defined on the server it is a
/// member of.
pub fn shown(user: &User, groups: &[Group]) -> User {
    let mut user = user.clone();
    for g in groups_of(groups, &user.name) {
        if !user.groups.iter().any(|x| x.eq_ignore_ascii_case(&g)) {
            user.groups.push(g);
        }
    }
    user
}

/// Makes `user` a member of exactly the defined groups named in `wanted` (names of
/// directory groups are skipped); groups that do not exist yet are created.
pub fn set_memberships(groups: &[Group], user: &str, wanted: &[String]) -> Vec<Group> {
    let wanted: Vec<&str> = wanted.iter().map(|g| g.trim()).filter(|g| !g.is_empty() && !is_directory_group(g)).collect();
    let mut out: Vec<Group> = groups.to_vec();
    for g in &mut out {
        let want = wanted.iter().any(|w| w.eq_ignore_ascii_case(g.name.trim()));
        if want && !g.has(user) {
            g.members.push(user.trim().to_string());
        } else if !want {
            g.members.retain(|m| !m.trim().eq_ignore_ascii_case(user.trim()));
        }
    }
    for w in wanted {
        if !out.iter().any(|g| g.name.trim().eq_ignore_ascii_case(w)) {
            out.push(Group { name: w.to_string(), description: String::new(), members: vec![user.trim().to_string()] });
        }
    }
    out
}

/// The groups without `user` as a member (a deleted user).
pub fn without_member(groups: &[Group], user: &str) -> Vec<Group> {
    set_memberships(groups, user, &[])
}

/// Turns the groups named by local accounts into defined groups with these accounts
/// as members (the accounts keep only names of directory groups). Returns the changed
/// accounts and the groups; `None` if nothing changes.
pub fn adopt(accounts: &[Account], groups: &[Group]) -> Option<(Vec<Account>, Vec<Group>)> {
    let mut groups = groups.to_vec();
    let mut changed = Vec::new();
    for a in accounts.iter().filter(|a| a.user.provider.is_empty() && a.user.groups.iter().any(|g| !is_directory_group(g))) {
        let mut wanted = groups_of(&groups, &a.user.name);
        wanted.extend(a.user.groups.iter().filter(|g| !is_directory_group(g)).cloned());
        groups = set_memberships(&groups, &a.user.name, &wanted);
        let mut account = a.clone();
        account.user.groups.retain(|g| is_directory_group(g));
        changed.push(account);
    }
    (!changed.is_empty()).then_some((changed, groups))
}

#[cfg(test)]
mod tests;
