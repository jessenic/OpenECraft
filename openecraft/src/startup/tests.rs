use super::*;

fn plant() -> ServerProject {
    ServerProject { url: "http://srv:8091".into(), name: "plant".into() }
}

#[test]
fn reopen_last_reconnects_a_server_project() {
    assert_eq!(plan(&Startup::ReopenLast, None, Some(&plant())), Action::Open(Target::Server(plant())));
    // A local file is restored as it was, with its unsaved changes.
    assert_eq!(plan(&Startup::ReopenLast, Some("a.oec"), None), Action::Keep);
    assert_eq!(plan(&Startup::ReopenLast, None, None), Action::Keep);
}

#[test]
fn modes_without_the_last_project() {
    assert_eq!(plan(&Startup::ProjectManager, Some("a.oec"), None), Action::ShowStart);
    assert_eq!(plan(&Startup::ProjectServer, None, Some(&plant())), Action::ShowServer);
}

#[test]
fn a_specific_project_opens_unless_it_is_restored() {
    let file = Startup::Project(Target::File("a.oec".into()));
    assert_eq!(plan(&file, Some("a.oec"), None), Action::Keep);
    assert_eq!(plan(&file, Some("b.oec"), None), Action::Open(Target::File("a.oec".into())));
    let server = Startup::Project(Target::Server(plant()));
    // The restored copy is never taken for the open server project.
    assert_eq!(plan(&server, None, Some(&plant())), Action::Open(Target::Server(plant())));
}

#[test]
fn the_mode_keeps_its_target() {
    let mut s = Startup::Project(Target::File("a.oec".into()));
    s.set_mode(Mode::Project, Target::File(String::new()));
    assert_eq!(s, Startup::Project(Target::File("a.oec".into())));
    s.set_mode(Mode::ProjectServer, Target::File(String::new()));
    assert_eq!(s, Startup::ProjectServer);
    for (m, _, _) in MODES {
        s.set_mode(m, Target::File(String::new()));
        assert_eq!(s.mode(), m);
    }
}

#[test]
fn the_origin_tells_where_the_project_is_saved() {
    openecraft_i18n::with_lang(openecraft_i18n::Lang::En, || {
        let p = plant();
        assert_eq!(Origin::File("C:\\work\\a.oec").label(), "Local file: a.oec");
        assert_eq!(Origin::File("C:\\work\\a.oec").save_tip(), "Save locally to a.oec");
        let open = Origin::Server { url: &p.url, name: &p.name, connected: true };
        assert_eq!(open.label(), "Project server: http://srv:8091 / Project: plant");
        assert_eq!(open.save_tip(), "Save to project server: plant");
        let lost = Origin::Server { url: &p.url, name: &p.name, connected: false };
        assert!(lost.label().ends_with("connection lost"));
        // A local copy never reads like the server project.
        let copy = Origin::Recovery(&p);
        assert_eq!(copy.label(), "Local recovery copy of server project: plant");
        assert!(copy.save_tip().starts_with("Save locally"));
        assert!(copy.is_server() && !Origin::New.is_server());
    });
}

#[test]
fn the_setting_is_stored() {
    let s = Startup::Project(Target::Server(plant()));
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<Startup>(&json).unwrap(), s);
}
