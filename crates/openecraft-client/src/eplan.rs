//! Wrapper of the EPLAN Data Portal API (`https://dataportal.eplan.com/api`, JSON:API)
//! that answers in the types of the parts server API, so that the part handling reads
//! the EPLAN Data Portal like a parts server: server info, faceted search, parts by
//! number and the files of a part (commercial data as CSV, DXF drawings, pictures).
//! The portal needs the personal access token of an EPLAN account (sent as
//! `Authorization: Bearer PAT:…`). Its API offers no changes, so the
//! connection is read-only.
//!
//! | Parts server | EPLAN Data Portal |
//! | --- | --- |
//! | `GET /info` | `GET /token/fileprovider`, then `GET /parts?page[limit]=1` |
//! | `POST /search` | `GET /parts?search=&filter[manufacturer]=&filter[catalog_eplan]=&sort=&page[offset]=&page[limit]=` |
//! | `POST /search` with numbers | `POST /parts` with the portal ids ([`Part::portal_id`]) separated by commas |
//! | `GET /parts/{number}` | `GET /parts?search="{number}"` (the portal finds parts by id only) |
//! | `GET /files/{id}` | `POST /download/commercial_data/part`, `POST /download/dxf_data/part` (id in the body), picture previews |
//!
//! Manufacturers are filtered by their short name, product groups by the id of the
//! EPLAN catalog entry: (generic product group + 1) · 10⁸ + (product group + 1) · 10⁴ +
//! (subgroup + 1) with the numbers of the EPLAN part exchange, 0 for a level that is not
//! chosen, e.g. `200050000` for electrical engineering / plugs.

use crate::endpoint::with_content_type;
use openecraft_i18n::{lang, tr, tr_fmt};
use openecraft_parts::api::{self, encode_component, PartQuery, ServerInfo};
use openecraft_parts::portal::{Attachment, AttachmentKind, Facet, Lifecycle, SearchResult, Sort};
use openecraft_parts::productgroups::{self, Classification};
use openecraft_parts::{unix_from_date, Part};
use serde_json::{Map, Value};

pub const EPLAN_URL: &str = "https://dataportal.eplan.com/api";
/// [`ServerInfo::name`] of the portal.
pub const EPLAN_NAME: &str = "EPLAN Data Portal";
/// Parts per request if the query sets no limit.
const LIMIT: usize = 50;
/// Parts a search for one number may return (numbers that contain it come as well).
const FIND_LIMIT: usize = 20;
/// Related resources the answers include: names of manufacturer and groups, pictures.
const INCLUDE: &str = "manufacturer,product_top_group,product_group,product_sub_group,picture_file.preview";
/// The facets the portal can filter by.
const FACETS: [&str; 4] = ["top_group", "group", "subgroup", "manufacturer"];

/// Address of the portal's API and the user's token; builds the requests.
#[derive(Clone, PartialEq)]
pub struct EplanEndpoint {
    pub url: String,
    pub token: String,
}

impl std::fmt::Debug for EplanEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EplanEndpoint").field("url", &self.url).field("token", &if self.token.is_empty() { "(empty)" } else { "(redacted)" }).finish()
    }
}

impl Default for EplanEndpoint {
    fn default() -> Self {
        EplanEndpoint { url: EPLAN_URL.into(), token: String::new() }
    }
}

fn params(list: &[(&str, String)]) -> String {
    let list: Vec<String> = list.iter().map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v))).collect();
    format!("?{}", list.join("&"))
}

impl EplanEndpoint {
    fn url(&self, path: &str) -> String {
        api::url(&self.url, path)
    }

    /// The download gateway accepts PAT credentials as `Bearer PAT:<token>`.
    /// Accept pasted PAT or Bearer prefixes without duplicating them.
    fn authorized(&self, mut req: ehttp::Request) -> ehttp::Request {
        let token = self.token.trim();
        if !token.is_empty() {
            let token = token.get(..7).filter(|p| p.eq_ignore_ascii_case("Bearer ")).map_or(token, |_| token[7..].trim());
            let value = format!("Bearer PAT:{}", token.strip_prefix("PAT:").unwrap_or(token));
            req.headers.insert("Authorization", value);
        }
        req
    }

    fn get(&self, path: &str) -> ehttp::Request {
        self.authorized(ehttp::Request::get(self.url(path)))
    }

    /// Public metadata and catalog size. This alone does not check authentication.
    pub fn info(&self) -> ehttp::Request {
        self.get(&format!("/parts{}", params(&[("page[limit]", "1".into())])))
    }

    /// Check the protected file-provider service without downloading a part or
    /// depending on the first catalog entry having commercial data. The returned
    /// provider token is deliberately discarded, never exposed as server metadata.
    pub fn connection_check(&self) -> crate::Call<ServerInfo> {
        let endpoint = self.clone();
        crate::Call::then(self.get("/token/fileprovider"), move |result| {
            let doc = document(result)?;
            if doc["data"]["attributes"]["token"].as_str().is_none_or(|t| t.trim().is_empty()) {
                return Err(tr("Unexpected Data Portal response").to_string());
            }
            Ok(crate::Call::new(endpoint.info(), decode_info))
        })
    }

    /// Search with text, manufacturer, product groups, order and page of the query.
    pub fn search(&self, query: &PartQuery) -> ehttp::Request {
        self.get(&format!("/parts{}", query_string(query)))
    }

    /// POST /parts: the parts with these portal ids.
    pub fn parts(&self, ids: &[String]) -> ehttp::Request {
        let req = ehttp::Request::post(self.url(&format!("/parts{}", params(&[("include", INCLUDE.into())]))), ids.join(",").into_bytes());
        self.authorized(with_content_type(req, "text/plain"))
    }

    /// Search for exactly this part number ([`decode_part`] picks the part).
    pub fn part(&self, number: &str) -> ehttp::Request {
        let query = [("search", format!("\"{}\"", number.trim().replace('"', ""))), ("include", INCLUDE.into()), ("page[limit]", FIND_LIMIT.to_string())];
        self.get(&format!("/parts{}", params(&query)))
    }

    /// The parts with these numbers: by the portal ids known for them (`ids`), without
    /// ids a search for the first number.
    pub fn lookup(&self, numbers: &[String], ids: &[String]) -> ehttp::Request {
        match (ids.is_empty(), numbers.first()) {
            (true, Some(number)) => self.part(number),
            _ => self.parts(ids),
        }
    }

    /// Address of a file of a part ([`Attachment::id`]: a path of the API or an address).
    pub fn file_url(&self, id: &str) -> String {
        if id.contains("://") {
            id.to_string()
        } else {
            self.url(&format!("/{}", id.trim_start_matches('/')))
        }
    }

    /// Use the same parsed origin and case-sensitive API path for authorization
    /// and protocol selection. Host case and an explicit default port are equivalent.
    fn api_file(&self, url: &str) -> Option<(url::Url, String)> {
        if url.contains('\\') || self.url.contains('\\') { return None; }
        let base = url::Url::parse(self.url.trim()).ok()?;
        let file = url::Url::parse(url).ok()?;
        if !matches!(base.scheme(), "http" | "https") || base.origin() != file.origin()
            || !base.username().is_empty() || base.password().is_some()
            || !file.username().is_empty() || file.password().is_some()
            || base.query().is_some() || base.fragment().is_some() { return None; }
        let path = file.path().strip_prefix(base.path().trim_end_matches('/'))?;
        let path = path.strip_prefix('/')?.to_string();
        Some((file, path))
    }

    fn owns_file(&self, url: &str) -> bool {
        self.api_file(url).is_some()
    }

    /// The file; the token is only sent to the portal itself.
    pub fn file(&self, id: &str) -> ehttp::Request {
        let url = self.file_url(id);
        let Some((mut parsed, path)) = self.api_file(&url) else { return ehttp::Request::get(url); };
        // Old stored attachment ids include the part id in the path; the current
        // gateway expects it in a text/plain POST body. Preserve query parameters.
        let download = ["dxf_data", "commercial_data"].into_iter().find_map(|kind| {
            let part = path.strip_prefix(&format!("download/{kind}/part/"))?;
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.to_string())
        });
        let req = match download {
            Some(part) => {
                let path = parsed.path().rsplit_once('/').map(|(p, _)| p).unwrap_or_default().to_string();
                parsed.set_path(&path);
                with_content_type(ehttp::Request::post(parsed.to_string(), part.into_bytes()), "text/plain")
            }
            None => ehttp::Request::get(url),
        };
        self.authorized(req)
    }

    /// True if the file can only be loaded with the token (not in a web browser).
    pub fn needs_token(&self, id: &str) -> bool {
        self.owns_file(&self.file_url(id))
    }
}

/// Query parameters of `GET /parts`. Catalogs, lifecycle and technical data are not
/// known to the portal; a text search keeps the portal's order by relevance unless
/// another order is chosen.
pub fn query_string(query: &PartQuery) -> String {
    let mut list: Vec<(&str, String)> = Vec::new();
    let text = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    if let Some(q) = text {
        list.push(("search", q.to_string()));
    }
    if let Some(m) = query.manufacturer.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        list.push(("filter[manufacturer]", m.to_string()));
    }
    if let Some(id) = catalog_id(query) {
        list.push(("filter[catalog_eplan]", id));
    }
    let sort = match query.sort {
        Sort::Manufacturer => Some("manufacturer.short_name"),
        Sort::Number if text.is_some() && !query.descending => None,
        _ => Some("part_number"),
    };
    if let Some(sort) = sort {
        list.push(("sort", format!("{}{sort}", if query.descending { "-" } else { "" })));
    }
    list.push(("include", INCLUDE.into()));
    list.push(("page[offset]", query.offset.unwrap_or(0).to_string()));
    list.push(("page[limit]", query.limit.unwrap_or(LIMIT).to_string()));
    params(&list)
}

/// Id of the EPLAN catalog entry of the query's product groups (the generic product
/// group defaults to electrical engineering).
pub fn catalog_id(query: &PartQuery) -> Option<String> {
    let group = query.group.as_deref().and_then(productgroups::group_id).filter(|g| *g != 0);
    let sub = query.subgroup.as_deref().and_then(productgroups::subgroup_id).filter(|s| *s != 0);
    if query.top_group.is_none() && group.is_none() {
        return None;
    }
    let top = u32::from(query.top_group.unwrap_or_default().eplan_id()) + 1;
    let level = |n: Option<u16>| n.map_or(0, |n| u32::from(n) + 1);
    Some((top * 100_000_000 + level(group) * 10_000 + if group.is_some() { level(sub) } else { 0 }).to_string())
}

/// Message of a failed request: access denied, the JSON:API `errors`, otherwise the
/// HTTP status.
pub fn error_text(status: u16, status_text: &str, body: &[u8]) -> String {
    if status == 401 || status == 403 {
        return tr("access denied – check the personal access token").into();
    }
    let doc: Value = serde_json::from_slice(body).unwrap_or_default();
    let messages: Vec<String> = doc["errors"]
        .as_array()
        .map(|errors| errors.iter().map(|e| first(e, &["detail", "title"])).filter(|m| !m.is_empty()).collect())
        .unwrap_or_default();
    if messages.is_empty() {
        api::error_text(status, status_text, body)
    } else {
        messages.join("; ")
    }
}

fn answer(result: ehttp::Result<ehttp::Response>) -> Result<ehttp::Response, String> {
    match result {
        Err(e) => Err(tr_fmt!("connection failed: {}", crate::reason(&e))),
        Ok(resp) if resp.ok => Ok(resp),
        Ok(resp) => Err(error_text(resp.status, &resp.status_text, &resp.bytes)),
    }
}

fn document(result: ehttp::Result<ehttp::Response>) -> Result<Value, String> {
    serde_json::from_slice(&answer(result)?.bytes).map_err(|e| tr_fmt!("unexpected answer: {}", e))
}

/// Answer of [`EplanEndpoint::info`]: the portal as a read-only parts server.
pub fn decode_info(result: ehttp::Result<ehttp::Response>) -> Result<ServerInfo, String> {
    let doc = document(result)?;
    let data = doc["data"].as_array().ok_or_else(|| tr("Unexpected Data Portal response").to_string())?;
    if data.iter().any(|part| part["id"].as_str().is_none_or(|id| id.trim().is_empty())) {
        return Err(tr("Unexpected Data Portal response").to_string());
    }
    let shown = data.len();
    Ok(ServerInfo { name: EPLAN_NAME.into(), version: "API".into(), parts: total(&doc, shown), storage: "eplan".into(), ..Default::default() })
}

/// Answer of [`EplanEndpoint::search`] or [`EplanEndpoint::lookup`] for `query`. The
/// facets count the parts of the answer (the portal does not count them).
pub fn decode_search(query: &PartQuery, result: ehttp::Result<ehttp::Response>) -> Result<SearchResult, String> {
    let doc = document(result)?;
    let mut parts = parts(&doc);
    let (total, offset) = if query.numbers.is_empty() {
        let offset = query.offset.unwrap_or(0);
        (total(&doc, offset + parts.len()), offset)
    } else {
        parts.retain(|p| query.numbers.contains(&p.number));
        (parts.len(), 0)
    };
    Ok(SearchResult { revision: 0, total, offset, facets: facets(query, &parts), attributes: Vec::new(), parts })
}

/// Answer of [`EplanEndpoint::part`]: the part with exactly this number, `None` if the
/// portal does not know it.
pub fn decode_part(number: &str, result: ehttp::Result<ehttp::Response>) -> Result<Option<Part>, String> {
    if result.as_ref().is_ok_and(|r| r.status == 404) {
        return Ok(None);
    }
    Ok(parts(&document(result)?).into_iter().find(|p| p.number.eq_ignore_ascii_case(number.trim())))
}

/// Answer of [`EplanEndpoint::file`].
pub fn decode_file(result: ehttp::Result<ehttp::Response>) -> Result<Vec<u8>, String> {
    answer(result).map(|r| r.bytes)
}

fn number(v: &Value) -> Option<usize> {
    v.as_u64().and_then(|n| usize::try_from(n).ok()).or_else(|| v.as_str()?.trim().parse().ok())
}

/// Number of matching parts from `meta`; without it `fallback`, one more if there is
/// a link to the next page.
fn total(doc: &Value, fallback: usize) -> usize {
    let meta = &doc["meta"];
    [&meta["page"]["total"], &meta["total"], &meta["count"]]
        .into_iter()
        .find_map(number)
        .unwrap_or_else(|| fallback + usize::from(!doc["links"]["next"].is_null()))
}

fn facets(query: &PartQuery, parts: &[Part]) -> Vec<Facet> {
    let filters = PartQuery {
        top_group: query.top_group,
        group: query.group.clone(),
        subgroup: query.subgroup.clone(),
        manufacturer: query.manufacturer.clone(),
        ..Default::default()
    };
    filters.search(parts, 0).facets.into_iter().filter(|f| FACETS.contains(&f.name.as_str())).collect()
}

/// Text of a value: strings, numbers, the first text of a list, a multilingual text.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(list) => list.iter().map(text).find(|t| !t.is_empty()).unwrap_or_default(),
        Value::Object(map) => localized(map),
        _ => String::new(),
    }
}

/// A multilingual text (`{"de_DE": "…", "en_US": "…"}`) in the user's language,
/// otherwise in English, otherwise the first one.
fn localized(map: &Map<String, Value>) -> String {
    let find = |code: &str| {
        map.iter()
            .filter(|(k, _)| k.get(..2).is_some_and(|p| p.eq_ignore_ascii_case(code)) && (k.len() == 2 || matches!(k.as_bytes()[2], b'_' | b'-')))
            .map(|(_, v)| text(v))
            .find(|t| !t.is_empty())
    };
    find(lang().code()).or_else(|| find("en")).or_else(|| map.values().filter(|v| !v.is_object()).map(text).find(|t| !t.is_empty())).unwrap_or_default()
}

/// The first non-empty text of these keys of an object; the text of anything else.
fn first(v: &Value, keys: &[&str]) -> String {
    match v {
        Value::Object(map) => keys.iter().filter_map(|k| map.get(*k)).map(text).find(|t| !t.is_empty()).unwrap_or_default(),
        _ => text(v),
    }
}

/// A resource identifier (`{"type", "id"}`) looked up in `included`.
fn lookup<'a>(res: &'a Value, included: &'a [Value]) -> &'a Value {
    if res.get("attributes").is_none() && res.get("type").is_some() {
        included.iter().find(|i| i["type"] == res["type"] && i["id"] == res["id"]).unwrap_or(res)
    } else {
        res
    }
}

/// A resource with its attributes and `id` in one object.
fn flat(res: &Value, included: &[Value]) -> Value {
    let res = lookup(res, included);
    match res.get("attributes") {
        Some(Value::Object(attributes)) => {
            let mut out = attributes.clone();
            if let Some(id) = res.get("id") {
                out.entry("id").or_insert_with(|| id.clone());
            }
            Value::Object(out)
        }
        _ => res.clone(),
    }
}

/// An attribute of a resource, or one of its relationships.
fn field(res: &Value, included: &[Value], name: &str) -> Value {
    let attribute = &res["attributes"][name];
    let v = if attribute.is_null() { &res["relationships"][name]["data"] } else { attribute };
    match v {
        Value::Array(list) => Value::Array(list.iter().map(|x| flat(x, included)).collect()),
        Value::Object(_) => flat(v, included),
        _ => v.clone(),
    }
}

/// The resource a to-one relationship points to.
fn related<'a>(res: &'a Value, included: &'a [Value], name: &str) -> &'a Value {
    lookup(&res["relationships"][name]["data"], included)
}

/// The parts of a JSON:API document (`data` is one resource or a list).
pub fn parts(doc: &Value) -> Vec<Part> {
    let included = doc["included"].as_array().map(Vec::as_slice).unwrap_or_default();
    let data: Vec<&Value> = match &doc["data"] {
        Value::Array(list) => list.iter().collect(),
        v @ Value::Object(_) => vec![v],
        _ => Vec::new(),
    };
    data.into_iter().filter(|r| r["type"].as_str().is_none_or(|t| t == "parts")).map(|r| part(r, included)).filter(|p| !p.number.is_empty()).collect()
}

/// EPLAN number and name of a product group: `level` 0 generic product group, 1 product
/// group, 2 subgroup. The id of a catalog entry holds all three levels, each + 1.
fn group(v: &Value, level: usize) -> (u16, String) {
    let name = first(v, &["name"]);
    let by_id = first(v, &["id"]).parse::<u64>().ok().map(|id| [id / 100_000_000, id / 10_000 % 10_000, id % 10_000][level]).filter(|n| *n > 1);
    let by_name = || match level {
        0 => productgroups::TOP_GROUPS.iter().find(|(_, n)| n.eq_ignore_ascii_case(&name)).map(|(i, _)| *i),
        1 => productgroups::group_id(&name),
        _ => productgroups::subgroup_id(&name),
    };
    (by_id.and_then(|n| u16::try_from(n - 1).ok()).or_else(by_name).unwrap_or(0), name)
}

/// Lower case names of the data types of a part (`commercial_data`, `dxf_data`, …).
fn data_types(v: &Value) -> Vec<String> {
    let list = match v {
        Value::Array(list) => list.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    };
    list.iter().map(|t| first(t, &["id", "name"]).to_lowercase()).filter(|t| !t.is_empty()).collect()
}

/// Lifecycle of the portal (`{"value": 0, "name": "Active"}`).
fn lifecycle(v: &Value) -> Lifecycle {
    let name = first(v, &["name"]).to_lowercase();
    if name.contains("phase") {
        Lifecycle::PhaseOut
    } else if name.contains("discontinued") || name.contains("obsolete") || name.contains("inactive") {
        Lifecycle::Discontinued
    } else {
        Lifecycle::Active
    }
}

/// A part number as part of a file name.
fn file_stem(number: &str) -> String {
    number.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect()
}

/// A part resource of the portal as a part of the parts server.
pub fn part(res: &Value, included: &[Value]) -> Part {
    let attr = |name: &str| text(&field(res, included, name));
    let id = text(&res["id"]);
    let number = Some(attr("part_number")).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
    let designation = attr("designation");
    let description = attr("description");
    let mut part = Part {
        type_number: attr("type_number"),
        order_number: attr("order_number"),
        manufacturer: first(&field(res, included, "manufacturer"), &["short_name", "name"]),
        note: if designation.is_empty() || description == designation { String::new() } else { description.clone() },
        description: if designation.is_empty() { description } else { designation },
        lifecycle: lifecycle(&field(res, included, "lifecycle")),
        portal_id: id.clone(),
        ..Default::default()
    };
    let [top, grp, sub] = [("product_top_group", 0), ("product_group", 1), ("product_sub_group", 2)].map(|(name, level)| group(&field(res, included, name), level));
    productgroups::apply(&mut part, Classification { top_group: top.0, group: grp.0, subgroup: sub.0 });
    // Groups the tables do not know keep their name.
    for (target, (_, name)) in [(&mut part.group, grp), (&mut part.subgroup, sub)] {
        if target.is_empty() && !name.bytes().all(|b| b.is_ascii_digit()) {
            *target = name;
        }
    }
    if let Value::Array(list) = field(res, included, "accessories") {
        part.accessories = list.iter().map(|a| first(a, &["part_number"])).filter(|n| !n.is_empty()).collect();
    }
    part.modified = unix_from_date(&attr("update_date")).unwrap_or(0);
    part.revision = part.modified;
    if !id.is_empty() {
        let (pid, stem) = (encode_component(&id), file_stem(&number));
        let types = data_types(&field(res, included, "data_types"));
        let has = |t: &str| types.iter().any(|x| x == t);
        if types.is_empty() || has("commercial_data") {
            part.attachments.push(Attachment { id: format!("download/commercial_data/part/{pid}"), name: format!("{stem}_commercial_data.csv"), kind: AttachmentKind::Other, size: 0 });
        }
        // The portal also makes DXF drawings of the 2D macros.
        if has("dxf_data") || has("2d_graphical_data") {
            part.attachments.push(Attachment { id: format!("download/dxf_data/part/{pid}"), name: format!("{stem}_dxf.zip"), kind: AttachmentKind::Drawing, size: 0 });
        }
    }
    let preview = field(related(res, included, "picture_file"), included, "preview");
    let url = first(&preview, &["512", "96"]);
    if url.contains("://") {
        let path = first(&preview, &["id"]);
        let name = path.rsplit(['/', '\\']).next().filter(|n| n.contains('.')).map_or_else(|| format!("{}.png", file_stem(&number)), str::to_string);
        part.attachments.push(Attachment { id: url, name, kind: AttachmentKind::Image, size: 0 });
    }
    part.number = number;
    part
}

#[cfg(test)]
mod tests;
