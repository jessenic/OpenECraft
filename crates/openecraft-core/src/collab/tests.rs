use super::*;
use crate::model::PageKind;

#[test]
fn imported_source_records_participate_in_project_data_and_merge() {
    let mut source = Project::default();
    source.import_sources.insert("catalog".into(), openecraft_parts::Shared::new(serde_json::json!({"table": [1, 2]})));
    let mut target = Project::default();
    assert!(!same_data(&source, &target));
    with_data(&mut target, &source);
    assert!(same_data(&source, &target));
    let text = serde_json::to_string(&target).unwrap();
    let restored: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(restored.import_sources, source.import_sources);
}

fn project() -> Project {
    let mut prj = Project::default();
    let id = prj.new_id();
    prj.pages.push(Page::new(id, "2", "Motors", PageKind::Schematic));
    prj
}

fn ids(prj: &Project) -> (u64, u64) {
    (prj.pages[0].id, prj.pages[1].id)
}

fn lock(target: Target, session: &str) -> Lock {
    Lock { target, user: format!("user {session}"), session: session.into() }
}

#[test]
fn merge_takes_owned_pages_from_mine() {
    let theirs = project();
    let (a, b) = ids(&theirs);
    let mut mine = theirs.clone();
    mine.pages[0].description = "mine A".into();
    mine.pages[1].description = "mine B".into();
    mine.info.name = "mine".into();
    let mut theirs = theirs;
    theirs.pages[1].description = "theirs B".into();
    let owned = Owned { project: false, pages: [a].into() };
    let out = merge(&theirs, &mine, &owned);
    assert_eq!(out.pages[0].description, "mine A");
    assert_eq!(out.pages[1].description, "theirs B");
    assert_eq!(out.info.name, theirs.info.name);
    assert_eq!(out.pages.iter().map(|p| p.id).collect::<Vec<_>>(), vec![a, b]);
}

#[test]
fn merge_with_project_lock_takes_page_list_and_data_from_mine() {
    let theirs = project();
    let (a, b) = ids(&theirs);
    let mut mine = theirs.clone();
    mine.info.name = "renamed".into();
    mine.pages.swap(0, 1);
    mine.pages[0].description = "edited without page lock".into();
    let new = mine.new_id();
    mine.pages.push(Page::new(new, "3", "New", PageKind::Schematic));
    let owned = Owned { project: true, pages: BTreeSet::new() };
    let out = merge(&theirs, &mine, &owned);
    assert_eq!(out.info.name, "renamed");
    assert_eq!(out.pages.iter().map(|p| p.id).collect::<Vec<_>>(), vec![b, a, new]);
    // Page contents need the page lock; new pages come with the project lock.
    assert_eq!(out.pages[0].description, theirs.pages[1].description);
    assert_eq!(out.pages[2].description, "New");
}

#[test]
fn merge_without_project_lock_keeps_deleted_pages() {
    let theirs = project();
    let (a, b) = ids(&theirs);
    let mut mine = theirs.clone();
    mine.pages.remove(0);
    let out = merge(&theirs, &mine, &Owned { project: false, pages: [a, b].into() });
    assert_eq!(out.pages.len(), 2);
    let out = merge(&theirs, &mine, &Owned { project: true, pages: BTreeSet::new() });
    assert_eq!(out.pages.iter().map(|p| p.id).collect::<Vec<_>>(), vec![b]);
}

#[test]
fn merge_keeps_my_next_id() {
    let theirs = project();
    let mut mine = theirs.clone();
    mine.next_id = 5_000_000;
    assert_eq!(merge(&theirs, &mine, &Owned::default()).next_id, 5_000_000);
}

#[test]
fn changes_by_target() {
    let a = project();
    let (p1, p2) = ids(&a);
    let mut b = a.clone();
    assert!(changes(&a, &b).is_empty());
    b.next_id += 100;
    assert!(changes(&a, &b).is_empty());
    b.pages[1].description = "x".into();
    assert_eq!(changes(&a, &b), [Target::Page(p2)].into());
    b.devices.insert("-K1".into(), Default::default());
    assert_eq!(changes(&a, &b), [Target::Project, Target::Page(p2)].into());
    let mut c = a.clone();
    c.pages.swap(0, 1);
    assert_eq!(changes(&a, &c), [Target::Project].into());
    let mut d = a.clone();
    let id = d.new_id();
    d.pages.push(Page::new(id, "9", "", PageKind::Schematic));
    assert_eq!(changes(&a, &d), [Target::Project].into());
    let _ = p1;
}

fn shared(locks: Vec<Lock>) -> (Shared, Project) {
    let opened = Opened {
        session: Session { user: "me".into(), session: "s1".into() },
        first_id: 2_000_000,
        snapshot: Snapshot { revision: 3, project: project(), locks, ..Default::default() },
        rights: Vec::new(),
    };
    Shared::open("plant", opened)
}

#[test]
fn opening_uses_the_session_id_range() {
    let (sh, prj) = shared(Vec::new());
    assert_eq!(prj.next_id, 2_000_000);
    assert_eq!(sh.id_end, 2_000_000 + ID_BLOCK);
    assert_eq!(sh.revision, 3);
}

#[test]
fn unlocked_changes_skip_owned_and_requested() {
    let (mut sh, mut prj) = shared(Vec::new());
    let (a, b) = ids(&prj);
    sh.locks = vec![lock(Target::Page(a), "s1"), lock(Target::Page(b), "s2")];
    prj.pages[0].description = "x".into();
    prj.pages[1].description = "y".into();
    prj.info.name = "z".into();
    assert_eq!(sh.unlocked_changes(&prj), vec![Target::Project, Target::Page(b)]);
    assert_eq!(sh.holder(Target::Page(b)).unwrap().session, "s2");
    assert!(sh.holder(Target::Page(a)).is_none());
    sh.requested.insert(Target::Project);
    assert_eq!(sh.unlocked_changes(&prj), vec![Target::Page(b)]);
    assert!(sh.has_changes(&prj));
}

#[test]
fn revert_restores_the_server_version() {
    let (sh, mut prj) = shared(Vec::new());
    prj.pages[0].description = "x".into();
    prj.pages[1].description = "y".into();
    prj.info.name = "z".into();
    let b = prj.pages[1].id;
    let out = sh.revert(&prj, &[Target::Page(b), Target::Project]);
    assert_eq!(out.pages[0].description, "x");
    assert_eq!(out.pages[1].description, sh.base.pages[1].description);
    assert_eq!(out.info.name, sh.base.info.name);
    assert_eq!(out.next_id, prj.next_id);
}

#[test]
fn locks_granted_on_an_old_state_are_checked_on_update() {
    let (mut sh, mut prj) = shared(Vec::new());
    let (a, b) = ids(&prj);
    prj.pages[0].description = "mine A".into();
    prj.pages[1].description = "mine B".into();
    sh.requested.extend([Target::Page(a), Target::Page(b)]);
    let mine = vec![lock(Target::Page(a), "s1"), lock(Target::Page(b), "s1")];
    assert!(sh.status(Status { revision: 4, locks: mine.clone() }, &[Target::Page(a), Target::Page(b)]));
    assert!(sh.requested.is_empty());
    // Not used before the newer project is loaded.
    assert!(!sh.owned().contains(Target::Page(a)));

    // Another user changed page B before the lock was granted.
    let mut server = sh.base.clone();
    server.pages[1].description = "theirs B".into();
    server.info.name = "theirs".into();
    let (lost, apply) = sh.update(Snapshot { revision: 4, project: server, locks: mine, ..Default::default() });
    assert_eq!(lost, vec![Target::Page(b)]);
    let out = apply(&prj);
    assert_eq!(out.pages[0].description, "mine A");
    assert_eq!(out.pages[1].description, "theirs B");
    assert_eq!(out.info.name, "theirs");
    assert!(sh.fresh.is_empty());
    assert!(sh.owned().contains(Target::Page(b)));
    assert_eq!(sh.revision, 4);
}

#[test]
fn denied_locks_are_no_longer_requested() {
    let (mut sh, _) = shared(Vec::new());
    sh.requested.insert(Target::Project);
    sh.denied(&[Target::Project]);
    assert!(sh.requested.is_empty());
    assert!(!sh.status(Status { revision: 3, locks: Vec::new() }, &[]));
}

#[test]
fn names_and_paths() {
    assert!(valid_name("Plant 1"));
    assert!(valid_name("line-2_v1.0"));
    assert!(!valid_name(""));
    assert!(!valid_name(" a"));
    assert!(!valid_name(".hidden"));
    assert!(!valid_name("a/b"));
    assert!(!valid_name("a\\b"));
    assert!(!valid_name(&"x".repeat(65)));
    assert!(!valid_name("Plant."));
    for reserved in ["CON", "nul", "Aux.oec", "com1", "LPT9"] {
        assert!(!valid_name(reserved), "{reserved}");
    }
    assert!(valid_name("COM0"));
    assert!(valid_name("Console"));
    assert!(stored_name("CON") && stored_name("Plant."), "existing projects stay available");
    assert_eq!(name_key("Plant"), name_key("plant"));
    assert_eq!(project_path("Plant 1"), "/api/v1/projects/Plant%201");
}

#[test]
fn api_types_round_trip() {
    let req = LockRequest { session: Session { user: "u".into(), session: "s".into() }, targets: vec![Target::Project, Target::Page(7)] };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""page":7"#));
    assert_eq!(serde_json::from_str::<LockRequest>(&json).unwrap(), req);
}

#[test]
fn review_records_need_no_lock() {
    let (sh, mut prj) = shared(Vec::new());
    let page = prj.pages[0].id;
    crate::review::add_comment(&mut prj, page, crate::geom::p(1.0, 1.0), "me", "", "check fuse");
    assert!(sh.unlocked_changes(&prj).is_empty());
    assert!(sh.has_changes(&prj));
    // Saved without any lock, kept next to a comment of another user.
    let mut theirs = sh.base.clone();
    crate::review::add_comment(&mut theirs, page, crate::geom::p(2.0, 2.0), "ben", "", "other");
    theirs.review.comments[0].id = 77;
    let out = merge(&theirs, &prj, &Owned::default());
    assert_eq!(out.review.live_comments().count(), 2);
}

#[test]
fn roles_and_projects_of_users() {
    let rights = |u: &ProjectUser, project: &str| Policy::<ProjectRight>::default().access(&u.identity(), &u.grants()).rights(project);
    let user = |role, projects: &[&str]| ProjectUser { name: "u".into(), role, projects: projects.iter().map(|p| p.to_string()).collect(), ..Default::default() };
    let viewer = user(Some(ProjectRole::Viewer), &[]);
    assert_eq!(rights(&viewer, "Plant 1").into_iter().collect::<Vec<_>>(), [ProjectRight::View]);
    let editor = user(Some(ProjectRole::Editor), &["Plant*"]);
    assert!(rights(&editor, "Plant 2").contains(&ProjectRight::EditPages));
    assert!(rights(&editor, "Office").is_empty());
    let admin = user(Some(ProjectRole::Admin), &["none"]);
    assert!(rights(&admin, "Office").contains(&ProjectRight::Delete), "administrators see everything");
    assert!(rights(&admin, SERVER).contains(&ProjectRight::Administer));
    assert!(user(None, &[]).grants().is_empty(), "directory users start without rights");
    assert!(ProjectRole::Viewer < ProjectRole::Reviewer && ProjectRole::Editor < ProjectRole::Manager && ProjectRole::Manager < ProjectRole::Admin);
    for role in ProjectRole::ALL {
        assert_eq!(ProjectRight::role(role.name()), Some(role.rights()));
    }
    assert_eq!(ProjectRight::ROLES, ProjectRole::ALL.map(ProjectRole::name));
    assert_eq!(user_path("anna b"), "/api/v1/users/anna%20b");
    assert_eq!(access_path("Plant 1"), "/api/v1/projects/Plant%201/access");
}

#[test]
fn old_user_files_keep_their_role() {
    let u: ProjectUser = serde_json::from_str(r#"{"name":"anna","role":"editor","projects":["Plant*"]}"#).unwrap();
    assert_eq!(u.role, Some(ProjectRole::Editor));
    assert_eq!(serde_json::to_string(&ProjectUser { name: "x".into(), ..Default::default() }).unwrap(), r#"{"name":"x"}"#);
}

#[test]
fn roles_and_targets_are_read_in_either_case() {
    assert_eq!(serde_json::from_str::<ProjectRole>(r#""Editor""#).unwrap(), ProjectRole::Editor);
    assert_eq!(serde_json::from_str::<ProjectRole>(r#""editor""#).unwrap(), ProjectRole::Editor);
    assert_eq!(serde_json::from_str::<Target>(r#"{"Page": 3}"#).unwrap(), Target::Page(3));
    assert_eq!(serde_json::to_string(&Target::Project).unwrap(), r#""project""#);
}

#[test]
fn a_save_locks_free_changes_and_leaves_out_the_rest() {
    let lock = |t, s: &str| Lock { target: t, user: s.into(), session: s.into() };
    let locks = vec![lock(Target::Page(1), "me"), lock(Target::Page(2), "other")];
    let mine = BTreeSet::from([Target::Page(1), Target::Page(2), Target::Page(3), Target::Page(4), Target::Page(5), Target::Project]);
    // Page 4 was changed by another user since the base revision; page 5 is not allowed.
    let theirs = BTreeSet::from([Target::Page(4)]);
    let plan = save_plan(&mine, &theirs, &locks, "me", |t| t != Target::Page(5));
    assert_eq!(plan.lock, vec![Target::Project, Target::Page(3)]);
    assert_eq!(plan.ignored, vec![Target::Page(2), Target::Page(4), Target::Page(5)]);
}

#[test]
fn older_save_requests_still_parse() {
    let req: SaveRequest = serde_json::from_str(r#"{"session": {"user": "a", "session": "1"}, "release": []}"#).unwrap();
    assert_eq!((req.base_revision, req.strict), (0, false));
    let json = serde_json::to_string(&Snapshot::default()).unwrap();
    assert!(!json.contains("ignored"), "{json}");
}
