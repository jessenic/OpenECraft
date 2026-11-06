use super::*;
use openecraft_parts::api::{ImportResult, PartList, ServerInfo};

fn part(number: &str) -> Part {
    Part { number: number.into(), ..Default::default() }
}

fn loaded(numbers: &[&str]) -> Session {
    let mut s = Session::default();
    let list = PartList { revision: 4, parts: numbers.iter().map(|n| part(n)).collect(), ..Default::default() };
    assert_eq!(s.apply(&Reply::Parts(Ok(list))), None);
    s
}

#[test]
fn connecting_loads_the_parts_once_per_revision() {
    let mut s = Session::default();
    let info = |revision| Reply::Info(Ok(ServerInfo { revision, ..Default::default() }));
    assert_eq!(s.apply(&info(4)), Some(FollowUp::Reload));
    let list = PartList { revision: 4, parts: vec![part("B10"), part("B9")], ..Default::default() };
    s.apply(&Reply::Parts(Ok(list)));
    assert!(s.loaded);
    assert_eq!(s.parts[0].number, "B9", "natural sort order");
    assert_eq!(s.apply(&info(4)), None);
    assert_eq!(s.apply(&info(5)), Some(FollowUp::Reload));
}

#[test]
fn editing_marks_the_draft_dirty_until_reverted() {
    let mut s = loaded(&["A", "B"]);
    assert!(!s.dirty());
    s.select("A");
    assert_eq!(s.selected(), Some("A"));
    assert!(!s.dirty());
    s.draft.as_mut().unwrap().part.description = "changed".into();
    assert!(s.dirty());
    s.revert();
    assert!(!s.dirty());
    assert_eq!(s.draft.as_ref().unwrap().part.description, "");
}

#[test]
fn new_parts_get_unique_numbers_and_are_dirty() {
    let mut s = loaded(&["NEW.PART"]);
    s.new_part();
    let d = s.draft.as_ref().unwrap();
    assert_eq!((d.original.clone(), d.part.number.as_str()), (None, "NEW.PART_1"));
    assert!(s.dirty());
    assert_eq!(s.save_problem(), None);
    s.copy_part();
    assert_eq!(s.draft.as_ref().unwrap().part.number, "NEW.PART_1_COPY");
    s.revert();
    assert!(s.draft.is_none());
}

#[test]
fn existing_or_empty_numbers_cannot_be_saved() {
    let mut s = loaded(&["A", "B"]);
    s.select("A");
    s.draft.as_mut().unwrap().part.number = "B".into();
    assert!(s.number_taken());
    assert!(s.save_problem().is_some());
    s.draft.as_mut().unwrap().part.number = " ".into();
    assert!(s.save_problem().is_some());
    s.draft.as_mut().unwrap().part.number = "A".into();
    assert_eq!(s.save_problem(), None);
}

#[test]
fn saving_a_renamed_part_deletes_the_old_number() {
    let mut s = loaded(&["A"]);
    s.select("A");
    s.draft.as_mut().unwrap().part.number = "A2".into();
    let follow = s.apply(&Reply::Saved(Ok(Box::new(part("A2")))));
    assert_eq!(follow, Some(FollowUp::Delete("A".into())));
    assert_eq!(s.draft.as_ref().unwrap().original.as_deref(), Some("A2"));
    // Deleting the old number keeps the renamed part in the editor.
    s.apply(&Reply::Deleted("A".into(), Ok(())));
    assert_eq!(s.parts.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["A2"]);
    assert_eq!(s.selected(), Some("A2"));
    assert!(!s.dirty());
}

#[test]
fn saving_a_new_part_adds_it() {
    let mut s = loaded(&["A"]);
    s.new_part();
    let number = s.draft.as_ref().unwrap().part.number.clone();
    assert_eq!(s.apply(&Reply::Saved(Ok(Box::new(part(&number))))), None);
    assert!(s.get(&number).is_some());
    assert!(!s.dirty());
}

#[test]
fn deleting_the_selected_part_closes_the_editor() {
    let mut s = loaded(&["A", "B"]);
    s.select("B");
    s.apply(&Reply::Deleted("B".into(), Ok(())));
    assert!(s.draft.is_none());
    assert_eq!(s.parts.len(), 1);
    // Failed requests change nothing.
    s.select("A");
    s.apply(&Reply::Deleted("A".into(), Err("missing or wrong access token".into())));
    assert_eq!(s.selected(), Some("A"));
}

#[test]
fn reloading_keeps_unsaved_changes() {
    let mut s = loaded(&["A"]);
    s.select("A");
    s.draft.as_mut().unwrap().part.description = "mine".into();
    let mut theirs = part("A");
    theirs.description = "theirs".into();
    s.apply(&Reply::Parts(Ok(PartList { revision: 5, parts: vec![theirs.clone()], ..Default::default() })));
    assert_eq!(s.draft.as_ref().unwrap().part.description, "mine");
    // Without changes the editor shows the new server state.
    s.revert();
    assert_eq!(s.draft.as_ref().unwrap().part.description, "theirs");
    assert_eq!(s.apply(&Reply::Imported(Ok(ImportResult { imported: 3, revision: 6 }))), Some(FollowUp::Reload));
}

fn catalog(id: &str, name: &str) -> Catalog {
    Catalog { id: id.into(), name: name.into(), ..Default::default() }
}

#[test]
fn new_catalogs_get_an_id_from_their_name() {
    let mut s = Session::default();
    s.apply(&Reply::Catalogs(Ok(vec![catalog("acme_2026", "ACME 2026")])));
    s.select_catalog(None, "ACME");
    assert_eq!(s.catalog.as_ref().unwrap().catalog.manufacturer, "ACME");
    assert_eq!(s.catalog_to_save(), Err("The catalog needs a name"));
    s.catalog.as_mut().unwrap().catalog.name = "ACME 2026".into();
    assert_eq!(s.catalog_to_save(), Err("A catalog with this id exists already"));
    s.catalog.as_mut().unwrap().catalog.name = "ACME Relays 2027".into();
    assert_eq!(s.catalog_to_save().unwrap().id, "acme_relays_2027");
    // The open catalog may keep its id.
    s.select_catalog(Some("acme_2026"), "");
    assert_eq!(s.catalog_to_save().unwrap().id, "acme_2026");
}

#[test]
fn catalog_replies_keep_the_list_and_draft_current() {
    let mut s = Session::default();
    s.apply(&Reply::Catalogs(Ok(vec![catalog("b", "Beta"), catalog("a", "Alpha")])));
    s.select_catalog(Some("a"), "");
    s.catalog.as_mut().unwrap().catalog.version = "2".into();
    // A reload keeps local changes …
    s.apply(&Reply::Catalogs(Ok(vec![catalog("a", "Alpha")])));
    assert_eq!(s.catalog.as_ref().unwrap().catalog.version, "2");
    // … saving shows the server state.
    s.apply(&Reply::CatalogSaved(Ok(Catalog { version: "2".into(), parts: 4, ..catalog("a", "Alpha") })));
    assert_eq!(s.catalog.as_ref().unwrap().catalog.parts, 4);
    s.apply(&Reply::CatalogSaved(Ok(catalog("c", "Gamma"))));
    assert_eq!(s.catalogs.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
    s.apply(&Reply::CatalogDeleted("c".into(), Ok(())));
    assert!(s.catalog.is_none());
    assert_eq!(s.catalogs.len(), 1);
}

#[test]
fn users_are_checked_before_saving() {
    let mut s = Session::default();
    let ann = User { name: "ann".into(), role: Role::Editor, manufacturer: String::new(), ..Default::default() };
    s.apply(&Reply::Users(Ok(vec![ann.clone()])));
    s.select_user(None);
    assert_eq!(s.user_problem(), Some("The user name must not be empty or contain blanks"));
    s.user.as_mut().unwrap().name = "ANN".into();
    assert_eq!(s.user_problem(), Some("A user with this name exists already"));
    s.user.as_mut().unwrap().name = "max".into();
    assert_eq!(s.user_problem(), Some("A new user needs a password"));
    s.user.as_mut().unwrap().update = UserUpdate { role: Role::Manufacturer, manufacturer: " ".into(), password: "pw".into(), groups: None };
    assert_eq!(s.user_problem(), Some("A manufacturer user needs a manufacturer"));
    s.user.as_mut().unwrap().update.manufacturer = "ACME".into();
    assert_eq!(s.user_problem(), None);
    // Existing users keep their password when none is given.
    s.select_user(Some("ann"));
    assert_eq!(s.user.as_ref().unwrap().update.role, Role::Editor);
    assert_eq!(s.user_problem(), None);

    s.apply(&Reply::UserSaved(Ok(User { name: "max".into(), role: Role::Manufacturer, manufacturer: "ACME".into(), ..Default::default() })));
    assert_eq!(s.users.len(), 2);
    assert_eq!(s.user.as_ref().unwrap().name, "max");
    assert!(s.user.as_ref().unwrap().update.password.is_empty());
    s.apply(&Reply::UserDeleted("max".into(), Ok(())));
    assert!(s.user.is_none());
    assert_eq!(s.users, [ann]);
}

#[test]
fn groups_of_local_and_directory_users() {
    let mut s = Session::default();
    let ann = User { name: "ann".into(), groups: vec!["qa".into(), "buyers".into()], ..Default::default() };
    let dir = User { name: "dora".into(), provider: "corp".into(), groups: vec!["corp:Staff".into()], ..Default::default() };
    s.apply(&Reply::Users(Ok(vec![ann, dir])));
    s.select_user(Some("ann"));
    let d = s.user.as_mut().unwrap();
    assert_eq!(d.groups, "qa, buyers");
    d.groups = "qa, , purchasing ,".into();
    assert_eq!(d.request().groups, Some(vec!["qa".to_string(), "purchasing".into()]));
    s.select_user(Some("dora"));
    let d = s.user.as_mut().unwrap();
    assert_eq!(d.provider, "corp");
    d.update.password = "typed".into();
    assert_eq!((d.request().groups, d.request().password), (None, String::new()), "the directory keeps groups and password");
}

#[test]
fn history_of_the_open_part() {
    let mut s = loaded(&["A"]);
    s.apply(&Reply::History("A".into(), Ok(vec![HistoryEntry { revision: 3, ..Default::default() }])));
    assert_eq!(s.history.as_ref().unwrap().1[0].revision, 3);
}

#[test]
fn files_of_an_edz_file_are_attached_after_its_parts_are_uploaded() {
    let mut s = loaded(&[]);
    let file = EdzFile { part: "SIE.LOGO".into(), name: "logo.png".into(), kind: openecraft_parts::portal::AttachmentKind::Image, data: vec![1] };
    s.pending_files = vec![file.clone()];
    assert_eq!(s.apply(&Reply::Imported(Ok(ImportResult::default()))), Some(FollowUp::Attach(vec![file.clone()])));
    assert!(s.pending_files.is_empty());
    assert_eq!(s.apply(&Reply::Imported(Ok(ImportResult::default()))), Some(FollowUp::Reload), "a later upload attaches nothing");
    s.pending_files = vec![file];
    assert_eq!(s.apply(&Reply::Imported(Err("denied".into()))), None);
    assert!(s.pending_files.is_empty(), "a failed upload drops the files");
    assert!(s.import_package("part.edz", b"no archive").is_err());
}

#[test]
fn parts_are_only_editable_with_the_right_to_change_them() {
    use openecraft_parts::access::{Access, Grant, Policy, Subject};
    use openecraft_parts::portal::PartsRight;
    let mut s = loaded(&["ACME.1", "OTHER.1"]);
    s.parts[0].manufacturer = "ACME".into();
    s.parts[1].manufacturer = "Other".into();
    s.select("ACME.1");
    // Not connected, or anonymous on a server that only lets read.
    assert!(!s.may_change(None));
    let reader = ServerInfo { access: Access::default(), ..Default::default() };
    assert!(!s.may_change(Some(&reader)));
    // A manufacturer user changes the parts of its manufacturer only.
    let grant = Grant::new(Subject::User("max".into()), "manufacturer", "ACME");
    let access = Policy::<PartsRight>::default().access(&Default::default(), &[grant]);
    let acme = ServerInfo { access, ..Default::default() };
    assert!(s.may_change(Some(&acme)));
    s.select("OTHER.1");
    assert!(!s.may_change(Some(&acme)));
    // A new part needs the right to change parts anywhere.
    s.new_part();
    assert!(s.may_change(Some(&acme)));
    assert!(!s.may_change(Some(&reader)));
}

#[test]
fn numbers_that_differ_only_in_case_are_taken() {
    let mut s = loaded(&["A.1", "B.1"]);
    s.select("A.1");
    s.draft.as_mut().unwrap().part.number = "b.1".into();
    assert!(s.number_taken());
    // Also the part's own number in other case, as on the server.
    s.draft.as_mut().unwrap().part.number = "a.1".into();
    assert!(s.number_taken());
    s.draft.as_mut().unwrap().part.number = "A.1".into();
    assert!(!s.number_taken());
}
