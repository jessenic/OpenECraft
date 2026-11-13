//! Web page of the data portal (dashboard, search with facets, part details and the
//! part form, files, favorites, administration of the server) and its translations. The page is plain HTML / JavaScript on top of the API; its
//! texts are written in English and translated through [`texts`].

use axum::extract::Query;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Json;
use openecraft_i18n::{tr_const, translate, Lang};
use openecraft_parts::access::Right;
use openecraft_parts::portal::{AttachmentKind, ChangeKind, Lifecycle, PartsRight, Role, Sort};
use openecraft_parts::{productgroups, AccessoryMount, FnKind, TopGroup, PRODUCT_GROUPS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const PAGE: &str = include_str!("web/portal.html");
const SCRIPT: &str = include_str!("web/portal.js");
const ADMIN_SCRIPT: &str = include_str!("web/admin.js");
const EDIT_SCRIPT: &str = include_str!("web/edit.js");
const DASHBOARD_SCRIPT: &str = include_str!("web/dashboard.js");
/// The OpenECraft logo (the application icon).
const LOGO: &str = include_str!("../../assets/icon.svg");

/// The page runs its own script only; parts data never becomes code.
pub const PAGE_CSP: &str =
    "default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// Texts of the page (`{}`: placeholders filled in by the page).
const TEXTS: &[&str] = &[
    tr_const!("Graphics"),
    tr_const!("Front view (2D)"),
    tr_const!("3D model"),
    tr_const!("Drag to turn the model"),
    tr_const!("OpenECraft data portal"),
    tr_const!("Search parts"),
    tr_const!("Search"),
    tr_const!("Sign in"),
    tr_const!("Sign out"),
    tr_const!("User name"),
    tr_const!("Password"),
    tr_const!("Signed in as {}"),
    tr_const!("Favorites"),
    tr_const!("All parts"),
    tr_const!("Catalogs"),
    tr_const!("Language"),
    tr_const!("Product top group"),
    tr_const!("Product group"),
    tr_const!("Product subgroup"),
    tr_const!("Manufacturer"),
    tr_const!("Catalog"),
    tr_const!("Lifecycle"),
    tr_const!("Technical data"),
    tr_const!("Clear filters"),
    tr_const!("{} parts"),
    tr_const!("No parts found"),
    tr_const!("Part number"),
    tr_const!("Description"),
    tr_const!("Price"),
    tr_const!("Sort by"),
    tr_const!("Descending"),
    tr_const!("Previous"),
    tr_const!("Next"),
    tr_const!("Page {} of {}"),
    tr_const!("Back to the results"),
    tr_const!("Type number"),
    tr_const!("Order number"),
    tr_const!("Supplier"),
    tr_const!("Note"),
    tr_const!("Identifier"),
    tr_const!("Voltage"),
    tr_const!("Current"),
    tr_const!("Power"),
    tr_const!("Wrong user name or password"),
    tr_const!("Too many failed sign-ins, try again later"),
    tr_const!("Part {} not found"),
    tr_const!("Breaking capacity"),
    tr_const!("Poles"),
    tr_const!("Cross-section"),
    tr_const!("Power loss"),
    tr_const!("Rated current"),
    tr_const!("Characteristic"),
    tr_const!("Coil voltage"),
    tr_const!("Certificates"),
    tr_const!("Datasheet"),
    tr_const!("Width"),
    tr_const!("Height"),
    tr_const!("Depth"),
    tr_const!("Weight"),
    tr_const!("Mounting"),
    tr_const!("Functions"),
    tr_const!("Accessories"),
    tr_const!("Accessory"),
    tr_const!("Fits"),
    tr_const!("Yes"),
    tr_const!("Successor"),
    tr_const!("Replaced by {}"),
    tr_const!("Files"),
    tr_const!("History"),
    tr_const!("Revision {}"),
    tr_const!("Last change"),
    tr_const!("Add to favorites"),
    tr_const!("Remove from favorites"),
    tr_const!("Sign in to keep favorites"),
    tr_const!("Version"),
    tr_const!("Parts"),
    tr_const!("Show parts"),
    tr_const!("Export (CSV)"),
    tr_const!("Loading…"),
    tr_const!("The server cannot be reached"),
    tr_const!("General"),
    tr_const!("Commercial data"),
    tr_const!("Mounting data"),
    tr_const!("Power loss"),
    tr_const!("Clearance top"),
    tr_const!("Clearance bottom"),
    tr_const!("Mounting holes"),
    tr_const!("Door cutout"),
    tr_const!("Cutout"),
    tr_const!("{} parts in {} catalogs, revision {}"),
    // Administration (admin.js)
    tr_const!("No"),
    tr_const!("Awaiting approval: {}"),
    tr_const!("Awaiting approval"),
    tr_const!("Approve"),
    tr_const!("Reject"),
    tr_const!("Approved user {}"),
    tr_const!("Reject the registration of {}? The account is deleted."),
    tr_const!("New accounts need the approval of an administrator"),
    tr_const!("Waiting accounts cannot sign in; approve or reject them in Users."),
    tr_const!("Your account was created. An administrator has to approve it before you can sign in."),
    tr_const!("Your account awaits the approval of an administrator"),
    tr_const!("Approval of new accounts"),
    tr_const!("Registration"),
    tr_const!("Users may register themselves (as readers)"),
    tr_const!("Set by the start options of the server (--registration or OPENECRAFT_REGISTRATION)."),
    tr_const!("Registered users become readers: they keep favorites and read the parts even when reading needs a sign-in. Give them more rights in Users."),
    tr_const!("The registration is open"),
    tr_const!("The registration is closed"),
    tr_const!("Register"),
    tr_const!("Create an account on this server"),
    tr_const!("This user name is taken"),
    tr_const!("Too many registrations from this address, try again later"),
    tr_const!("This user name cannot be used"),
    tr_const!("Export the parts, catalogs and settings, or download a backup of the whole database."),
    tr_const!("All parts with their data, for spreadsheets or to import them into another server. Their files are only in the database backup."),
    tr_const!("The catalogs with their manufacturer, version and description."),
    tr_const!("The users with their role, manufacturer, groups and last sign-in, without passwords."),
    tr_const!("Restore from a file"),
    tr_const!("Database backup"),
    tr_const!("One SQLite file with all parts, their history and files, the catalogs, the users and their favorites. It holds the password hashes of the users: keep it safe."),
    tr_const!("Download"),
    tr_const!("To restore it, start the server with --data <file>, or copy it into a new database with --migrate-from <file>."),
    tr_const!("The file holds no access settings"),
    tr_const!("Replace the roles and grants with those of the file?"),
    tr_const!("Export and backup"),
    tr_const!("Sign in as an administrator to manage the server"),
    tr_const!("Server"),
    tr_const!("Changes allowed"),
    tr_const!("Read only"),
    tr_const!("Users"),
    tr_const!("Access rights"),
    tr_const!("Sign-in needed to read"),
    tr_const!("Only local accounts"),
    tr_const!("All manufacturers"),
    tr_const!("Search users"),
    tr_const!("New user"),
    tr_const!("Role"),
    tr_const!("Groups"),
    tr_const!("Directory"),
    tr_const!("Last sign-in"),
    tr_const!("Open sessions: {}"),
    tr_const!("Never"),
    tr_const!("you"),
    tr_const!("Local"),
    tr_const!("Edit"),
    tr_const!("Sign out everywhere"),
    tr_const!("Delete"),
    tr_const!("No users"),
    tr_const!("Sign out {} on all devices?"),
    tr_const!("Delete user {}? Their sessions and favorites are removed."),
    tr_const!("Edit user {}"),
    tr_const!("Readers keep favorites, manufacturers maintain the parts and catalogs of their manufacturer, editors all parts, administrators also the users."),
    tr_const!("unchanged if empty"),
    tr_const!("At least {} characters"),
    tr_const!("separated by commas"),
    tr_const!("Grants of the server's access settings can refer to them"),
    tr_const!("The directory checks the password and sets the groups at every sign-in"),
    tr_const!("Save"),
    tr_const!("Cancel"),
    tr_const!("A user name must not be empty or contain blanks"),
    tr_const!("A manufacturer user needs a manufacturer"),
    tr_const!("The passwords do not match"),
    tr_const!("The password needs at least {} characters"),
    tr_const!("Saved user {}"),
    tr_const!("The last administrator cannot lose the right to manage users"),
    tr_const!("User {} exists already. Change it?"),
    tr_const!("A role is a set of rights. Grants give a user or a group a role on the parts of a manufacturer (* and ? as wildcards, * for all); a deny grant takes the rights of the role away, whatever other grants give."),
    tr_const!("Without sign-in"),
    tr_const!("Roles"),
    tr_const!("Built-in"),
    tr_const!("Name"),
    tr_const!("Remove"),
    tr_const!("New role"),
    tr_const!("Grants"),
    tr_const!("Type"),
    tr_const!("User or group"),
    tr_const!("Deny"),
    tr_const!("User"),
    tr_const!("Group"),
    tr_const!("directory:group"),
    tr_const!("No grants: users have the rights of their role"),
    tr_const!("Add grant"),
    tr_const!("Directories"),
    tr_const!("Discard changes"),
    tr_const!("Access settings saved"),
    tr_const!("The access settings would leave nobody who manages users"),
    tr_const!("New catalog"),
    tr_const!("Only empty catalogs can be deleted"),
    tr_const!("Bundled catalogs"),
    tr_const!("Catalogs shipped with the server. Installing adds their parts and updates those nobody changed on this server."),
    tr_const!("On this server"),
    tr_const!("Delete catalog {}?"),
    tr_const!("{}: {} parts added, {} updated, {} kept"),
    tr_const!("Edit catalog {}"),
    tr_const!("A catalog id must not be empty or contain blanks"),
    tr_const!("Catalog {} exists already. Change it?"),
    tr_const!("Saved catalog {}"),
    tr_const!("Import parts"),
    tr_const!("CSV (with the column names of the export), JSON, BMEcat or EPLAN XML. Parts with the same number are replaced; their files are kept."),
    tr_const!("Import"),
    tr_const!("Imported {} parts (revision {})"),
    tr_const!("Access token"),
    tr_const!("User account"),
    tr_const!("Sign in with a user account"),
    tr_const!("Sign in with the access token of the server"),
    tr_const!("Wrong access token"),
    tr_const!("Administration"),
    tr_const!("Overview"),
    tr_const!("Storage"),
    tr_const!("Status"),
    tr_const!("Revision"),
    tr_const!("Signed in"),
    tr_const!("Defined roles"),
    tr_const!("New password"),
    tr_const!("Repeat password"),
    tr_const!("Install"),
    tr_const!("Install again"),
    tr_const!("Update"),
    // Groups, directories, catalog permissions, part form, dashboard
    tr_const!("Per page"),
    tr_const!("New part"),
    tr_const!("Delete part {}?"),
    tr_const!("Dashboard"),
    tr_const!("Show the dashboard with graphs of the parts"),
    tr_const!("Set by the start options of the server (--dashboard or OPENECRAFT_DASHBOARD)."),
    tr_const!("Everybody who may read the parts sees it."),
    tr_const!("The dashboard is shown"),
    tr_const!("The dashboard is hidden"),
    tr_const!("Directory groups"),
    tr_const!("Groups of this server, next to those of the directory"),
    tr_const!("Grants on catalog:<id> apply to one catalog and its parts (Catalogs › Permissions)."),
    tr_const!("Groups gather users, local ones and those of a directory. Grants to a group give its members the rights of a role, on the whole server, a manufacturer or a catalog."),
    tr_const!("New group"),
    tr_const!("Members"),
    tr_const!("No groups"),
    tr_const!("Delete group {}? The grants to it are removed."),
    tr_const!("The groups would leave nobody who manages users"),
    tr_const!("Edit group {}"),
    tr_const!("Add"),
    tr_const!("Users of a directory too, by the name they sign in with"),
    tr_const!("A group name must not be empty or contain ':'"),
    tr_const!("Group {} exists already. Change it?"),
    tr_const!("Saved group {}"),
    tr_const!("Users sign in with the account of a directory (Active Directory, LDAP) when the server has no local account of their name. Grants refer to the groups of a directory as directory:group."),
    tr_const!("New directory"),
    tr_const!("Address"),
    tr_const!("Search base"),
    tr_const!("Delete directory {}? Its users can no longer sign in."),
    tr_const!("Edit directory {}"),
    tr_const!("Short name without blanks; qualifies the groups (id:group)"),
    tr_const!("ldap:// or ldaps://, with the port if it is not the usual one"),
    tr_const!("Encryption"),
    tr_const!("StartTLS (for ldap://)"),
    tr_const!("Accept any certificate (test installations only)"),
    tr_const!("Service account"),
    tr_const!("Looks the users up; empty: users bind with their own name (below)"),
    tr_const!("Better kept in an environment variable of the server"),
    tr_const!("Password variable"),
    tr_const!("Environment variable with the password of the service account"),
    tr_const!("User bind"),
    tr_const!("Bind name of a user without service account; {user} is replaced"),
    tr_const!("User filter"),
    tr_const!("Empty: the usual filter of the type"),
    tr_const!("Name attribute"),
    tr_const!("Also groups of groups (Active Directory)"),
    tr_const!("Group filter"),
    tr_const!("Empty: the memberOf attribute of the user"),
    tr_const!("Group search base"),
    tr_const!("Empty: the search base"),
    tr_const!("Allowed groups"),
    tr_const!("Only their members sign in; empty: all users"),
    tr_const!("Timeout (s)"),
    tr_const!("Test"),
    tr_const!("Without a user: connects and binds with the service account. With a user: signs them in and shows their groups."),
    tr_const!("A directory id must not be empty or contain blanks or ':'"),
    tr_const!("The address must start with ldap:// or ldaps://"),
    tr_const!("The test succeeded"),
    tr_const!("The test failed"),
    tr_const!("Directory {} exists already. Change it?"),
    tr_const!("Saved directory {}"),
    tr_const!("Permissions"),
    tr_const!("Permissions of catalog {}"),
    tr_const!("The roles given here apply to this catalog and its parts, whatever their manufacturer; deny takes the rights of the role away. Reading is a right of the whole server."),
    tr_const!("No grants: the rights by manufacturer apply"),
    tr_const!("A grant needs a user or a group"),
    tr_const!("Without catalog"),
    tr_const!("Edit part {}"),
    tr_const!("A part number must not be empty or start or end with blanks"),
    tr_const!("You may not change parts of manufacturer {}"),
    tr_const!("Part {} exists already"),
    tr_const!("Currency"),
    tr_const!("Price unit"),
    tr_const!("Quantity unit"),
    tr_const!("One line per value, e.g. Coil voltage: 24 V DC"),
    tr_const!("Back to the part"),
    tr_const!("Others"),
    tr_const!("Parts per manufacturer"),
    tr_const!("Parts per product group"),
    tr_const!("Parts per catalog"),
    tr_const!("Last changes per month"),
    tr_const!("Parts with …"),
    tr_const!("Users per role"),
    tr_const!("Changed last"),
    tr_const!("Dimensions"),
    tr_const!("Manufacturers"),
    tr_const!("Size of the files"),
    tr_const!("Logo"),
    tr_const!("Remove the logo"),
    tr_const!("The logo of the company: PNG, JPEG, GIF, WebP or SVG, at most 512 kB"),
    tr_const!("The roles, the grants, the groups and the access without sign-in, with the version of the server. Restoring replaces the roles and grants and adds the groups; the directories stay as set up on the server."),
];

#[derive(Deserialize)]
pub struct TextQuery {
    lang: Option<String>,
}

/// Translations for the page: `texts` by English text, `keys` by `kind:Key` of the
/// enumerations (`top_group:Fluid`, `lifecycle:Active`, `kind:Datasheet`, …).
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WebTexts {
    pub lang: String,
    /// All languages: code and native name.
    pub languages: Vec<(String, String)>,
    pub texts: BTreeMap<String, String>,
    pub keys: BTreeMap<String, String>,
    /// The rights of the server in display order, for the administration.
    pub rights: Vec<RightInfo>,
    /// The built-in roles with their rights, lowest first.
    pub roles: Vec<RoleInfo>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RightInfo {
    /// Name as in JSON (`edit_parts`).
    pub name: String,
    pub title: String,
    /// Only given on the whole server (`*`).
    pub server_wide: bool,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleInfo {
    /// Name in grants (`reader`).
    pub name: String,
    pub title: String,
    pub rights: Vec<String>,
}

pub async fn index() -> impl IntoResponse {
    ([(header::CONTENT_SECURITY_POLICY, PAGE_CSP)], Html(PAGE))
}

pub async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], SCRIPT)
}

pub async fn admin_script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], ADMIN_SCRIPT)
}

pub async fn edit_script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], EDIT_SCRIPT)
}

pub async fn dashboard_script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], DASHBOARD_SCRIPT)
}

pub async fn logo() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml"), (header::CACHE_CONTROL, "max-age=86400")], LOGO)
}

pub fn web_texts(lang: Lang) -> WebTexts {
    let own_groups = PRODUCT_GROUPS.iter().flat_map(|(g, subs)| std::iter::once(*g).chain(subs.iter().copied()));
    let group_names = productgroups::GROUPS.iter().chain(productgroups::SUBGROUPS).map(|(_, n)| *n).chain(own_groups);
    let texts = TEXTS.iter().copied().chain(group_names).map(|t| (t.to_string(), translate(t, lang).to_string())).collect();
    let mut keys = BTreeMap::new();
    let mut key = |kind: &str, k: String, title: &'static str| {
        keys.insert(format!("{kind}:{k}"), translate(title, lang).to_string());
    };
    for t in TopGroup::ALL {
        key("top_group", format!("{t:?}"), t.title());
    }
    for l in Lifecycle::ALL {
        key("lifecycle", format!("{l:?}"), l.title());
    }
    for k in AttachmentKind::ALL {
        key("kind", format!("{k:?}"), k.title());
    }
    for s in Sort::ALL {
        key("sort", format!("{s:?}"), s.title());
    }
    for r in Role::ALL {
        key("role", format!("{r:?}"), r.title());
    }
    for c in [ChangeKind::Created, ChangeKind::Changed, ChangeKind::Deleted] {
        key("change", format!("{c:?}"), c.title());
    }
    for f in FnKind::ALL {
        key("fn", format!("{f:?}"), f.title());
    }
    for m in AccessoryMount::ALL {
        key("mount", format!("{m:?}"), m.title());
    }
    let languages = Lang::ALL.iter().map(|l| (l.code().to_string(), l.native_name().to_string())).collect();
    let rights = PartsRight::ALL.iter().map(|r| RightInfo { name: r.name().into(), title: translate(r.title(), lang).into(), server_wide: r.server_wide() }).collect();
    let roles = Role::ALL
        .iter()
        .map(|r| RoleInfo { name: r.name().into(), title: translate(r.title(), lang).into(), rights: r.rights().iter().map(|x| x.name().to_string()).collect() })
        .collect();
    WebTexts { lang: lang.code().into(), languages, texts, keys, rights, roles }
}

/// `GET /api/v1/texts?lang=de`: the page's texts in a language (English if unknown).
pub async fn texts(Query(q): Query<TextQuery>) -> Json<WebTexts> {
    let lang = q.lang.as_deref().and_then(Lang::from_code).unwrap_or(Lang::En);
    Json(web_texts(lang))
}

#[cfg(test)]
mod tests;
