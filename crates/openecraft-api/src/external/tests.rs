use super::*;

#[test]
fn every_project_feature_is_documented() {
    let docs=include_str!("../../../../docs/plugins.md");
    for feature in PROJECT_FEATURES {
        assert!(docs.contains(&format!("`{feature}`")),"{feature} is missing in docs/plugins.md");
    }
}

#[test]
fn requests_advertise_directional_contact_support_of_the_actual_host() {
    let transport:Transport=std::sync::Arc::new(|request| {
        assert_eq!(request["project_features"],json!(PROJECT_FEATURES));
        Ok(json!({"result":true}))
    });
    assert_eq!(ask(&transport,json!({"method":"describe"})).unwrap()["result"],true);
}
use crate::{Args, Origin, Plugins};
use serde_json::Map;
use openecraft_core::analysis::Analysis;
use openecraft_core::sample::demo_project;
use std::sync::Mutex;

/// Plugin answering like an external program; records the requests.
fn fake(log: Arc<Mutex<Vec<Value>>>) -> External {
    let info = Info { id: "fake".into(), name: Text::Owned("Fake".into()), version: "1".into(), description: Text::Owned(String::new()) };
    External::new(
        info,
        Arc::new(move |req: &Value| {
            log.lock().unwrap().push(req.clone());
            Ok(match (req["method"].as_str().unwrap(), req["name"].as_str()) {
                ("describe", _) => json!({
                    "commands": [{ "name": "set_title", "help": "Sets the title", "params": [{ "name": "title", "kind": "str" }] }, { "name": "fail" }],
                    "checks": [{ "name": "titled", "title": "Title set" }],
                    "reports": [{ "name": "titles", "title": "Titles" }],
                    "exporters": [{ "name": "title_txt", "title": "Title", "extension": "txt" }],
                }),
                ("command", Some("fail")) => json!({ "error": "failed on purpose" }),
                ("command", _) => {
                    let mut prj = req["project"].clone();
                    prj["info"]["name"] = req["params"]["title"].clone();
                    json!({ "result": "ok", "project": prj })
                }
                ("check", _) => json!({ "issues": [
                    { "severity": "error", "message": "no title", "page": 0, "item": 7 },
                    { "message": "hint" },
                    { "message": "page only", "page": 1 },
                    { "message": "no such page", "page": 999 },
                ] }),
                ("report", _) => json!({ "headers": ["Title"], "rows": [[req["project"]["info"]["name"].clone()]] }),
                ("export", _) => json!({ "content": "title" }),
                _ => json!({}),
            })
        }),
    )
}

#[test]
fn external_plugins_answer_json_requests() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut p = Plugins::default();
    p.add(&fake(log.clone()), Origin::External("fake".into())).unwrap();
    assert_eq!(log.lock().unwrap()[0]["method"], "describe");
    assert_eq!(log.lock().unwrap()[0]["api"], API_VERSION);

    let mut prj = demo_project();
    let args = json!({ "title": "Plant 7" }).as_object().unwrap().clone();
    assert_eq!(p.run(p.command("set_title").unwrap(), &mut prj, &args).unwrap(), "ok");
    assert_eq!(prj.info.name, "Plant 7");
    assert_eq!(p.run(p.command("fail").unwrap(), &mut prj, &Default::default()).unwrap_err(), "failed on purpose");

    let issues = p.check(&prj, &Analysis::default());
    assert_eq!(issues[0].severity, Severity::Error);
    assert_eq!(issues[0].target, Some((0, 7)));
    assert_eq!((issues[1].severity, issues[1].target), (Severity::Warning, None));
    // A page without item points at the page; a page the project lacks at nothing.
    assert_eq!(issues[2].target, Some((1, 0)));
    assert_eq!(issues[3].target, None);

    let table = (p.report("titles").unwrap().run)(&prj, &Analysis::default()).unwrap();
    assert_eq!(table.rows, [["Plant 7"]]);
    assert_eq!((p.exporter("title_txt").unwrap().run)(&prj, &Analysis::default(), &Args(&Map::new())).unwrap().text().unwrap(), "title");
}

#[test]
fn manifests_are_read_from_the_plugin_folder() {
    let dir = std::env::temp_dir().join(format!("openecraft-api-manifest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(MANIFEST), r#"{ "id": "x", "name": "X", "api": 99, "run": ["x"] }"#).unwrap();
    assert!(External::open(&dir).err().unwrap().contains("API 99"));
    std::fs::write(dir.join(MANIFEST), r#"{ "id": "x", "name": "X", "run": [] }"#).unwrap();
    assert!(External::open(&dir).is_err());
    std::fs::write(dir.join(MANIFEST), r#"{ "id": "x", "name": "X", "run": ["openecraft-no-such-program"] }"#).unwrap();
    let plugin = External::open(&dir).unwrap();
    assert_eq!(plugin.info().id, "x");
    assert!(Plugins::default().add(&plugin, Origin::External(dir.clone())).unwrap_err().contains("openecraft-no-such-program"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn plugin_programs_talk_utf8_and_python3_falls_back_to_python() {
    let c = command(Path::new("."), Path::new("python3"), &[]);
    let envs: Vec<_> = c.get_envs().map(|(k, v)| (k.to_string_lossy().to_string(), v.map(|v| v.to_string_lossy().to_string()))).collect();
    assert!(envs.contains(&("PYTHONUTF8".to_string(), Some("1".to_string()))));
    assert!(envs.contains(&("PYTHONIOENCODING".to_string(), Some("utf-8".to_string()))));
    assert_eq!(alternative(Path::new("python3")), Some(PathBuf::from("python")));
    assert_eq!(alternative(Path::new("node")), None);
}

/// Not a test: the plugin program of [`plugin_programs_that_hang_are_stopped`].
#[test]
#[ignore]
fn hanging_plugin_program() {
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn plugin_programs_that_hang_are_stopped() {
    // This test binary, running the test above, is a program that does not answer.
    let exe = std::env::current_exe().unwrap();
    let run = vec![exe.display().to_string(), "--ignored".into(), "--exact".into(), "external::tests::hanging_plugin_program".into()];
    let transport = process(std::env::temp_dir(), run, Duration::from_millis(500));
    let start = Instant::now();
    let err = transport(&json!({ "method": "describe" })).unwrap_err();
    assert!(err.contains("did not answer"), "{err}");
    assert!(start.elapsed() < Duration::from_secs(20));
}
