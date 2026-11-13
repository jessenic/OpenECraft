use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum R {
    View,
    Edit,
    Admin,
}

impl Right for R {
    const ALL: &'static [R] = &[R::View, R::Edit, R::Admin];
    const ROLES: &'static [&'static str] = &["viewer", "editor", "admin"];

    fn role(name: &str) -> Option<&'static [R]> {
        match name.to_lowercase().as_str() {
            "viewer" => Some(&[R::View]),
            "editor" => Some(&[R::View, R::Edit]),
            "admin" => Some(R::ALL),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            R::View => "view",
            R::Edit => "edit",
            R::Admin => "admin",
        }
    }

    fn title(self) -> &'static str {
        self.name()
    }

    fn server_wide(self) -> bool {
        self == R::Admin
    }
}

fn anna() -> Identity {
    Identity { name: "Anna".into(), provider: "corp".into(), groups: vec!["corp:Engineering".into(), "corp:Plant A".into()] }
}

fn set(rights: &[R]) -> BTreeSet<R> {
    rights.iter().copied().collect()
}

#[test]
fn wildcards() {
    assert!(matches("*", "anything"));
    assert!(matches("Plant*", "plant 7"));
    assert!(matches("P?ant", "Plant"));
    assert!(!matches("Plant", "Plant 2"));
    assert!(matches("corp:*", "CORP:Engineering"));
    assert!(!matches("", "x"));
}

#[test]
fn subjects_match_users_and_groups() {
    let a = anna();
    assert!(a.is(&Subject::User("anna".into())));
    assert!(a.is(&Subject::User("*".into())));
    assert!(!a.is(&Subject::User("bob".into())));
    assert!(a.is(&Subject::Group("corp:engineering".into())));
    assert!(a.is(&Subject::Group("*:Plant*".into())));
    assert!(!a.is(&Subject::Group("Engineering".into())), "directory groups are qualified");
    assert!(!a.is(&Subject::Group(" ".into())));
}

#[test]
fn grants_give_rights_per_scope() {
    let policy = Policy::<R> {
        grants: vec![
            Grant::new(Subject::Group("corp:Engineering".into()), "viewer", "*"),
            Grant::new(Subject::User("anna".into()), "editor", "Plant*"),
            Grant::new(Subject::User("bob".into()), "admin", "*"),
        ],
        ..Default::default()
    };
    let access = policy.access(&anna(), &[]);
    assert_eq!(access.rights("Plant 1"), set(&[R::View, R::Edit]));
    assert_eq!(access.rights("Office"), set(&[R::View]));
    assert!(!access.allows(SERVER, R::Admin));
    assert!(access.anywhere(R::Edit));
    assert!(!access.anywhere(R::Admin));
}

#[test]
fn deny_grants_win() {
    let policy = Policy::<R> {
        grants: vec![
            Grant::new(Subject::User("anna".into()), "editor", "*"),
            Grant { deny: true, ..Grant::new(Subject::Group("corp:Plant A".into()), "editor", "Secret*") },
        ],
        ..Default::default()
    };
    let access = policy.access(&anna(), &[]);
    assert_eq!(access.rights("Plant 1"), set(&[R::View, R::Edit]));
    assert!(access.rights("Secret 1").is_empty());
}

#[test]
fn server_wide_rights_need_a_grant_on_everything() {
    let policy = Policy::<R>::default();
    let on_project = policy.access(&anna(), &[Grant::new(Subject::User("anna".into()), "admin", "Plant 1")]);
    assert!(on_project.allows("Plant 1", R::Edit));
    assert!(!on_project.allows("Plant 1", R::Admin));
    assert!(!on_project.allows(SERVER, R::Admin));
    let everywhere = policy.access(&anna(), &[Grant::new(Subject::User("anna".into()), "admin", "")]);
    assert!(everywhere.allows(SERVER, R::Admin));
    assert!(everywhere.allows("Plant 1", R::Admin), "server-wide rights are checked on the server");
    assert_eq!(Access::<R>::full().rights(SERVER), set(R::ALL));
}

#[test]
fn defined_roles() {
    let policy = Policy::<R> {
        roles: vec![Role { name: "Checker".into(), description: String::new(), rights: set(&[R::View]) }],
        grants: vec![Grant::new(Subject::Group("corp:*".into()), "checker", "*")],
        anonymous: "viewer".into(),
    };
    assert!(policy.validate().is_ok());
    assert_eq!(policy.role_names(), ["viewer", "editor", "admin", "Checker"]);
    assert_eq!(policy.access(&anna(), &[]).rights("x"), set(&[R::View]));
    assert_eq!(policy.anonymous_access().rights("x"), set(&[R::View]));
    assert!(Policy::<R>::default().anonymous_access().is_empty());
}

#[test]
fn invalid_policies_are_refused() {
    let role = |name: &str| Role::<R> { name: name.into(), ..Default::default() };
    let bad = [
        Policy::<R> { roles: vec![role("Editor")], ..Default::default() },
        Policy::<R> { roles: vec![role("a"), role("A")], ..Default::default() },
        Policy::<R> { roles: vec![role(" ")], ..Default::default() },
        Policy::<R> { grants: vec![Grant::new(Subject::User("anna".into()), "nobody", "*")], ..Default::default() },
        Policy::<R> { grants: vec![Grant::new(Subject::Group("".into()), "viewer", "*")], ..Default::default() },
        Policy::<R> { anonymous: "nobody".into(), ..Default::default() },
    ];
    for p in bad {
        assert!(p.validate().is_err(), "{p:?}");
    }
}

#[test]
fn grants_of_one_scope() {
    let mut policy = Policy::<R> {
        grants: vec![Grant::new(Subject::User("a".into()), "viewer", "P1"), Grant::new(Subject::User("b".into()), "viewer", "*")],
        ..Default::default()
    };
    assert_eq!(policy.grants_on("p1").len(), 1);
    policy.set_grants_on("P1", vec![Grant::new(Subject::User("c".into()), "editor", "ignored")]);
    assert_eq!(policy.grants.len(), 2);
    assert_eq!(policy.grants_on("P1"), [Grant::new(Subject::User("c".into()), "editor", "P1")]);
    assert_eq!(policy.grants_on("").len(), 1, "empty scope is everything");
}

#[test]
fn json_form() {
    let g = Grant::new(Subject::Group("corp:Engineering".into()), "editor", "Plant*");
    let text = serde_json::to_string(&g).unwrap();
    assert_eq!(text, r#"{"subject":{"group":"corp:Engineering"},"role":"editor","scope":"Plant*"}"#);
    let back: Grant = serde_json::from_str(r#"{"subject":{"user":"anna"},"role":"viewer","deny":true}"#).unwrap();
    assert!(back.deny);
    assert_eq!(back.scope(), SERVER);
    let access: Access<R> = serde_json::from_str(r#"{"entries":[{"scope":"*","rights":["view","edit"]}]}"#).unwrap();
    assert!(access.allows("x", R::Edit));
    assert_eq!(R::from_name("EDIT"), Some(R::Edit));
}

#[test]
fn long_names_do_not_overflow_the_match() {
    let name = format!("plant {}", "7".repeat(500_000));
    assert!(matches("*", &name));
    assert!(matches("Plant*", &name));
    assert!(!matches("*8", &name));
}

#[test]
fn defined_groups_add_their_members() {
    let groups = vec![
        Group { name: "Wiring".into(), description: String::new(), members: vec!["anna".into(), "Bob".into()] },
        Group { name: "Purchase".into(), members: vec!["bob".into()], ..Default::default() },
    ];
    assert_eq!(groups_of(&groups, "BOB"), ["Wiring", "Purchase"]);
    let anna = with_groups(anna(), &groups);
    assert_eq!(anna.groups, ["corp:Engineering", "corp:Plant A", "Wiring"]);
    assert!(anna.is(&Subject::Group("wiring".into())));
    let policy: Policy<R> = Policy { grants: vec![Grant::new(Subject::Group("Wiring".into()), "editor", "Plant*")], ..Default::default() };
    assert!(policy.access(&anna, &[]).allows("Plant 1", R::Edit));
    assert!(policy.access(&self::anna(), &[]).is_empty());
}

#[test]
fn group_names_and_members_are_normalized() {
    let g = Group { name: " Wiring ".into(), description: " Panel ".into(), members: vec!["anna".into(), " ".into(), "ANNA".into(), "bob".into()] }.normalized().unwrap();
    assert_eq!((g.name.as_str(), g.description.as_str()), ("Wiring", "Panel"));
    assert_eq!(g.members, ["anna", "bob"]);
    assert!(Group { name: "corp:Wiring".into(), ..Default::default() }.normalized().is_err());
    assert!(Group::default().normalized().is_err());
}

#[test]
fn rights_on_several_scopes() {
    let entry = |scope: &str, rights: &[R], deny: bool| Entry { scope: scope.into(), rights: set(rights), deny };
    let access = Access { entries: vec![entry("Plant*", &[R::Edit], false), entry("catalog:x", &[R::View], false), entry("catalog:secret", &[R::Edit], true)] };
    assert_eq!(access.rights_on(&["Plant 1", "catalog:x"]), set(&[R::View, R::Edit]));
    assert!(access.allows_on(&["Other", "catalog:x"], R::View));
    assert!(!access.allows_on(&["Other", "catalog:x"], R::Edit));
    // A deny on one of the scopes takes the right away.
    assert!(!access.allows_on(&["Plant 1", "catalog:secret"], R::Edit));
    // Server-wide rights only come from grants on the server.
    let admin = Access { entries: vec![entry("catalog:x", &[R::Admin], false)] };
    assert!(!admin.allows_on(&["catalog:x"], R::Admin));
}
