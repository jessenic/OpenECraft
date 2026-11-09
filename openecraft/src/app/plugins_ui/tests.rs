use super::*;
use openecraft_api::{optional, param};
use serde_json::json;

#[test]
fn optional_boolean_form_distinguishes_plugin_default_and_explicit_false() {
    let params=[optional("parameterize",Kind::Bool,"Insertion parameters")];
    assert!(form_args(&params,&[String::new()]).unwrap().is_empty());
    assert_eq!(form_args(&params,&["false".into()]).unwrap()["parameterize"],false);
    assert_eq!(form_args(&params,&["true".into()]).unwrap()["parameterize"],true);
}

#[test]
fn plugin_folder_loading_remembers_success_and_allows_retrying_errors() {
    let dir = std::env::temp_dir().join(format!("openecraft-ui-plugin-folder-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut plugins = openecraft_api::Plugins::default();
    let mut st = PluginsUi::default();
    assert!(!st.load_folder(&mut plugins));
    st.folder = dir.join("missing").to_string_lossy().into_owned();
    assert!(!st.load_folder(&mut plugins));
    assert!(st.folders().is_empty(), "failed folders are not stored");
    st.folder = format!("  {}  ", dir.display());
    assert!(st.load_folder(&mut plugins));
    assert_eq!(st.folders(), &[dir.to_string_lossy().into_owned()]);
    assert!(!st.load_folder(&mut plugins), "do not load the same plugins twice");
    let restored = PluginsUi::with_folders(st.folders().to_vec());
    assert_eq!(restored.folders(), st.folders());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn form_texts_become_typed_arguments() {
    let params = [param("tag", Kind::Str, "Tag"), optional("count", Kind::Int, "Count"), optional("ids", Kind::Ids, "Ids")];
    let texts = |v: [&str; 3]| v.map(String::from).to_vec();
    let args = form_args(&params, &texts(["-K1", "3", ""])).unwrap();
    assert_eq!(Value::Object(args), json!({ "tag": "-K1", "count": 3 }));
    assert_eq!(form_args(&params, &texts(["-K1", "", "1, 2"])).unwrap()["ids"], json!([1, 2]));
    assert!(form_args(&params, &texts(["-K1", "three", ""])).is_err());
}
