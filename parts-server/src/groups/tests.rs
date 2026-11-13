use super::*;

fn group(name: &str, members: &[&str]) -> Group {
    Group { name: name.into(), description: String::new(), members: members.iter().map(|m| m.to_string()).collect() }
}

fn account(name: &str, provider: &str, groups: &[&str]) -> Account {
    Account { user: User { name: name.into(), provider: provider.into(), groups: groups.iter().map(|g| g.to_string()).collect(), ..Default::default() }, ..Default::default() }
}

#[test]
fn memberships_are_set_and_groups_created() {
    let groups = vec![group("Wiring", &["anna", "bob"]), group("Purchase", &["Bob"])];
    let after = set_memberships(&groups, "BOB", &["wiring".into(), "Testing".into(), "corp:Engineering".into(), " ".into()]);
    assert_eq!(after, [group("Wiring", &["anna", "bob"]), group("Purchase", &[]), group("Testing", &["BOB"])]);
    assert_eq!(without_member(&after, "anna")[0], group("Wiring", &["bob"]));
}

#[test]
fn shown_users_have_their_defined_groups() {
    let user = account("anna", "corp", &["corp:Engineering"]).user;
    let shown = shown(&user, &[group("Wiring", &["Anna"]), group("Purchase", &["bob"])]);
    assert_eq!(shown.groups, ["corp:Engineering", "Wiring"]);
}

#[test]
fn groups_of_local_accounts_become_defined_groups() {
    let accounts = vec![account("anna", "", &["Wiring", "x:y"]), account("bob", "", &["wiring", "Purchase"]), account("carl", "corp", &["Wiring"]), account("dora", "", &[])];
    let (changed, groups) = adopt(&accounts, &[group("Purchase", &["eve"])]).unwrap();
    assert_eq!(changed.iter().map(|a| (a.user.name.as_str(), a.user.groups.clone())).collect::<Vec<_>>(), [("anna", vec!["x:y".to_string()]), ("bob", vec![])]);
    assert_eq!(groups, [group("Purchase", &["eve", "bob"]), group("Wiring", &["anna", "bob"])]);
    // Adopted once: the accounts name no local groups any more.
    let accounts: Vec<Account> = changed.into_iter().chain(accounts[2..].iter().cloned()).collect();
    assert!(adopt(&accounts, &groups).is_none());
}
