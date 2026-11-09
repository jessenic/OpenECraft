use super::*;

#[test]
fn a_direct_plugin_folder_is_loaded_instead_of_scanning_its_children() {
    let dir = std::env::temp_dir().join(format!("openecraft-direct-plugin-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("unrelated")).unwrap();
    std::fs::write(dir.join(external::MANIFEST), r#"{ "id": "direct", "name": "Direct", "api": 99, "run": ["unused"] }"#).unwrap();
    std::fs::write(dir.join("unrelated").join(external::MANIFEST), "{").unwrap();
    let mut plugins = Plugins::default();
    plugins.load_dir(&dir);
    assert_eq!(plugins.errors.len(), 1);
    assert!(plugins.errors[0].contains("API 99"), "read the requested manifest: {:?}", plugins.errors);
    std::fs::remove_dir_all(dir).unwrap();
}
use crate::Table;
use crate::{param, Kind, Text};
use openecraft_core::sample::demo_project;
use serde_json::json;

struct Test(&'static str, &'static str);

impl Plugin for Test {
    fn info(&self) -> Info {
        Info { id: self.0.into(), name: Text::Borrowed("Test"), version: "1".into(), description: Text::Borrowed("") }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.command(self.1, "Renames the project", vec![param("name", Kind::Str, "Name")], |prj, a| {
            prj.info.name = a.req("name").into();
            Ok(json!(prj.info.name))
        })
        .check(format!("{}_check", self.1), "Check", |prj, _| Ok(vec![Issue::plain(Severity::Info, prj.info.name.clone(), None)]))
        .check(format!("{}_broken", self.1), "Broken", |_, _| Err("no".into()))
        .report(self.1, "Report", |_, _| Ok(Table::new(["A"])));
        Ok(())
    }
}

fn plugins() -> Plugins {
    let mut p = Plugins::default();
    p.add(&Test("test", "rename"), Origin::BuiltIn).unwrap();
    p
}

fn args(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn commands_check_their_arguments_and_change_the_project() {
    let p = plugins();
    let mut prj = demo_project();
    let cmd = p.command("rename").unwrap();
    assert!(p.run(cmd, &mut prj, &args(json!({}))).is_err());
    assert_eq!(p.run(cmd, &mut prj, &args(json!({ "name": "X" }))).unwrap(), "X");
    assert_eq!(prj.info.name, "X");
}

#[test]
fn checks_add_issues_and_report_failures() {
    let p = plugins();
    let prj = demo_project();
    let issues = p.check(&prj, &Analysis::default());
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].message(), prj.info.name);
    assert_eq!(issues[1].severity, Severity::Error);
    assert!(issues[1].message().contains("rename_broken"), "{}", issues[1].message());
}

#[test]
fn names_must_be_snake_case_and_unique() {
    let mut p = plugins();
    assert!(p.add(&Test("test", "other"), Origin::BuiltIn).is_err(), "same id");
    assert!(p.add(&Test("second", "rename"), Origin::BuiltIn).is_err(), "same command");
    assert!(p.add(&Test("Third", "x"), Origin::BuiltIn).is_err());
    assert!(p.add(&Test("third", "Bad-Name"), Origin::BuiltIn).is_err());
    let mut reserved = Plugins::new(["rename"]);
    assert!(reserved.add(&Test("test", "rename"), Origin::BuiltIn).is_err(), "command of the front end");
    assert!(p.add(&Test("third", "rename2"), Origin::BuiltIn).is_ok());
    assert_eq!(p.list().len(), 2);
}

#[test]
fn disabled_plugins_offer_nothing() {
    let mut p = plugins();
    p.set_enabled("test", false);
    assert!(p.command("rename").is_none());
    assert!(p.report("rename").is_none());
    assert!(p.check(&demo_project(), &Analysis::default()).is_empty());
    assert_eq!(p.disabled(), ["test"]);
}

#[test]
fn built_in_plugins_are_loaded() {
    let p = Plugins::new(Vec::<String>::new());
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert!(p.list().iter().all(|l| l.origin == Origin::BuiltIn));
    assert!(p.command("project_statistics").is_some());
}

#[test]
fn plugin_folders_without_manifest_are_skipped() {
    let dir = std::env::temp_dir().join(format!("openecraft-api-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    std::fs::create_dir_all(dir.join("broken")).unwrap();
    std::fs::write(dir.join("broken").join(external::MANIFEST), "{").unwrap();
    let mut p = Plugins::default();
    p.load_dir(&dir);
    p.load_dir(&dir.join("missing"));
    assert!(p.list().is_empty());
    assert_eq!(p.errors.len(), 2, "{:?}", p.errors);
    std::fs::remove_dir_all(&dir).unwrap();
}
