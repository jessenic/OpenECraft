use super::*;
use openecraft_core::collab::{Opened, Session};
use openecraft_core::model::{Page, PageKind};

fn opened() -> Remote {
    let mut r = Remote::default();
    let opened = Opened { session: Session { user: "anna".into(), session: "s1".into() }, first_id: 100, snapshot: Snapshot::default(), rights: Vec::new() };
    r.shared = Some(Shared::open("plant", opened).0);
    r
}

#[test]
fn answers_of_old_sessions_are_dropped() {
    let mut r = opened();
    let ctx = egui::Context::default();
    r.pending = 3;
    r.inbox.lock().unwrap().extend([
        Reply::Status("old".into(), Ok(Status::default())),
        Reply::Status("s1".into(), Ok(Status::default())),
        Reply::List(Ok(vec![ProjectSummary { name: "p".into(), ..Default::default() }])),
    ]);
    let replies = r.poll(&ctx);
    assert_eq!(replies.len(), 2);
    assert!(matches!(replies[0], Reply::Status(ref s, _) if s == "s1"));
    assert_eq!(r.projects.len(), 1);
}

#[test]
fn errors_are_shown() {
    let mut r = opened();
    let ctx = egui::Context::default();
    r.pending = 1;
    r.inbox.lock().unwrap().push(Reply::Locked("s1".into(), vec![Target::Project], Err("project data is locked by ben".into())));
    r.poll(&ctx);
    assert!(r.status.contains("locked by ben"), "{}", r.status);
}

#[test]
fn lock_requests_are_remembered() {
    let mut r = opened();
    r.url = "http://127.0.0.1:9".into();
    r.lock(vec![Target::Page(4)]);
    assert!(r.shared.as_ref().unwrap().requested.contains(&Target::Page(4)));
    assert!(r.busy());
}

#[test]
fn targets_are_described() {
    let mut prj = Project::default();
    prj.pages.push(Page::new(42, "7", "", PageKind::Schematic));
    assert!(Remote::describe(&prj, Target::Page(42)).contains('7'));
    assert!(Remote::describe(&prj, Target::Page(5)).contains('5'));
}

#[test]
fn rights_decide_what_may_be_changed() {
    let mut r = opened();
    assert!(!r.viewer(), "token or open server: everything");
    r.role = Some(ProjectRole::Viewer);
    assert!(r.viewer(), "older servers: the role decides");
    r.role = None;
    r.rights = vec![ProjectRight::View, ProjectRight::Review];
    assert!(r.viewer() && r.may(ProjectRight::Review) && !r.may(ProjectRight::ManageAccess));
    r.rights.push(ProjectRight::EditPages);
    assert!(!r.viewer());

    // Opening takes the rights of the project; grants come with their own reply.
    let ctx = egui::Context::default();
    let opened = Opened { rights: vec![ProjectRight::View, ProjectRight::ManageAccess], ..Default::default() };
    let access = ProjectAccess { grants: vec![Grant::default()], roles: vec!["viewer".into()] };
    r.pending = 2;
    r.inbox.lock().unwrap().extend([Reply::Opened("p".into(), Ok(Box::new(opened))), Reply::Access("s1".into(), Ok(access.clone()))]);
    r.poll(&ctx);
    assert_eq!(r.rights, [ProjectRight::View, ProjectRight::ManageAccess]);
    assert_eq!(r.access, Some(access));
}

#[test]
fn a_failed_status_marks_the_connection_lost() {
    let mut r = opened();
    r.url = "http://srv:8091".into();
    let ctx = egui::Context::default();
    r.pending = 1;
    r.inbox.lock().unwrap().push(Reply::Status("s1".into(), Err("connection refused".into())));
    r.poll(&ctx);
    assert!(r.lost);
    r.pending = 1;
    r.inbox.lock().unwrap().push(Reply::Status("s1".into(), Ok(Status::default())));
    r.poll(&ctx);
    assert!(!r.lost);
    assert_eq!(r.project(), Some(crate::startup::ServerProject { url: "http://srv:8091".into(), name: "plant".into() }));
}
