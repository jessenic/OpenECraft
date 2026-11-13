use super::*;

#[test]
fn texts_are_translated() {
    let de = web_texts(Lang::De);
    assert_eq!(de.lang, "de");
    assert_eq!(de.texts["Search"], translate("Search", Lang::De));
    assert_ne!(de.texts["Product group"], "Product group");
    assert_eq!(de.texts["Relays, contactors"], "Relais, Schütze");
    assert_eq!(de.keys["top_group:Fluid"], translate("Fluid power", Lang::De));
    assert_eq!(de.keys["lifecycle:Discontinued"], translate("Discontinued", Lang::De));
    assert!(de.keys.contains_key("kind:Datasheet") && de.keys.contains_key("sort:Price") && de.keys.contains_key("change:Deleted"));
    // Functions are shown by their names, not by the keys of the file format.
    assert_eq!(de.keys["fn:MainContacts3"], translate("Main contacts 3-pole", Lang::De));
    assert_eq!(de.keys["mount:Left"], translate("On the left side", Lang::De));
    assert_eq!(de.languages.len(), Lang::ALL.len());
    // Rights and built-in roles for the administration.
    assert_eq!(de.rights.len(), PartsRight::ALL.len());
    let administer = de.rights.iter().find(|r| r.name == "administer").unwrap();
    assert!(administer.server_wide && administer.title == translate("Administer the server", Lang::De));
    let roles: Vec<&str> = de.roles.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(roles, PartsRight::ROLES);
    assert_eq!(de.roles[0].rights, ["read"]);
    assert_eq!(de.roles[3].title, translate("Administrator", Lang::De));
    let en = web_texts(Lang::En);
    assert!(en.texts.iter().all(|(k, v)| k == v));
}

/// Group names and the titles of the portal's enumerations are translated with
/// dynamic texts, which the source scan of the catalog test cannot see.
#[test]
fn names_and_titles_are_in_the_catalog() {
    use openecraft_parts::portal::UpdateKind;
    let known: std::collections::HashSet<&str> = openecraft_i18n::catalog_entries().iter().map(|e| e[0]).collect();
    let mut names: Vec<&str> = productgroups::TOP_GROUPS.iter().chain(productgroups::GROUPS).chain(productgroups::SUBGROUPS).map(|(_, n)| *n).collect();
    names.extend(PRODUCT_GROUPS.iter().flat_map(|(g, subs)| std::iter::once(*g).chain(subs.iter().copied())));
    names.extend(TopGroup::ALL.map(|t| t.title()));
    names.extend(Lifecycle::ALL.map(|l| l.title()));
    names.extend(AttachmentKind::ALL.map(|k| k.title()));
    names.extend(Sort::ALL.map(|s| s.title()));
    names.extend(Role::ALL.map(|r| r.title()));
    names.extend([ChangeKind::Created, ChangeKind::Changed, ChangeKind::Deleted].map(|c| c.title()));
    names.extend(openecraft_parts::FnKind::ALL.map(|f| f.title()));
    names.extend(openecraft_parts::AccessoryMount::ALL.map(|m| m.title()));
    names.extend([UpdateKind::Changed, UpdateKind::PhaseOut, UpdateKind::Discontinued, UpdateKind::Removed].map(|u| u.title()));
    let missing: Vec<&str> = names.into_iter().filter(|n| !known.contains(n)).collect();
    assert!(missing.is_empty(), "add to crates/openecraft-i18n/src/catalog/parts/required.tsv: {missing:?}");
}

/// Every text the page shows through `t("…")` / `tf("…")` / `data-t` is in [`TEXTS`].
#[test]
fn the_page_uses_known_texts_only() {
    let known: Vec<&str> = TEXTS.to_vec();
    let mut missing = Vec::new();
    for marker in ["t(\"", "tf(\"", "tp(\"", "data-t=\"", "data-tp=\""] {
        let page = format!("{PAGE}{SCRIPT}{ADMIN_SCRIPT}{EDIT_SCRIPT}{DASHBOARD_SCRIPT}");
        let mut rest = page.as_str();
        while let Some(i) = rest.find(marker) {
            let before = rest[..i].chars().last();
            rest = &rest[i + marker.len()..];
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') && !marker.starts_with("data") {
                continue;
            }
            let end = rest.find('"').unwrap();
            let text = &rest[..end];
            if !known.contains(&text) {
                missing.push(text.to_string());
            }
        }
    }
    // Titles the scripts translate from tables (facets, tabs, field labels).
    let titles = ["Product top group", "Product group", "Product subgroup", "Manufacturer", "Catalog", "Lifecycle", "Overview", "Users", "Access rights", "Catalogs", "Import parts"];
    let labels = ["Storage", "Status", "Revision", "Signed in", "Defined roles", "Grants", "Directories", "Without sign-in", "Directory", "User name", "Role", "Groups"];
    let more = ["New password", "Password", "Repeat password", "Identifier", "Name", "Version", "Description", "Install", "Install again", "Update"];
    for title in titles.into_iter().chain(labels).chain(more) {
        assert!(known.contains(&title));
    }
    assert!(missing.is_empty(), "texts of portal.html missing in web.rs TEXTS: {missing:?}");
}
