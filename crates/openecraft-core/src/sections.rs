//! Page sections: a tree of numbered sections that holds the pages like directories
//! hold files. A page's name starts with the full number of its section, so the page
//! numbers include the sections and subsections (`2.3.1` is page 1 of section 2.3);
//! moving a page or renumbering a section renames the pages. The pages stay ordered
//! by the tree: the top-level pages first, then every section with its own pages
//! before its subsections.

use crate::model::{Project, Section};
use openecraft_i18n::tr_const;
use std::collections::{HashMap, HashSet};

/// Between the numbers of the levels.
pub const SEPARATOR: char = '.';

pub fn get(prj: &Project, id: u64) -> Option<&Section> {
    if id == 0 {
        return None;
    }
    prj.sections.iter().find(|s| s.id == id)
}

/// Parent of a section; 0 for the top level and unknown parents.
fn parent_of(prj: &Project, s: &Section) -> u64 {
    if get(prj, s.parent).is_some() { s.parent } else { 0 }
}

/// Section of a page; 0 for top-level pages and unknown sections.
pub fn of_page(prj: &Project, page: usize) -> u64 {
    let id = prj.pages.get(page).map_or(0, |p| p.section);
    if get(prj, id).is_some() { id } else { 0 }
}

/// Full number, e.g. `2.3`; empty for the top level.
pub fn full_number(prj: &Project, id: u64) -> String {
    let mut chain: Vec<&Section> = Vec::new();
    let mut cur = get(prj, id);
    while let Some(s) = cur {
        if chain.iter().any(|x| x.id == s.id) {
            break;
        }
        chain.push(s);
        cur = get(prj, s.parent);
    }
    let numbers: Vec<&str> = chain.iter().rev().map(|s| s.number.as_str()).collect();
    numbers.join(&SEPARATOR.to_string())
}

/// What the names of the section's pages start with, e.g. `2.3.`; empty for the top
/// level.
pub fn prefix(prj: &Project, id: u64) -> String {
    let n = full_number(prj, id);
    if n.is_empty() { n } else { format!("{n}{SEPARATOR}") }
}

/// `2.3  Main supply`.
pub fn title(prj: &Project, id: u64) -> String {
    match get(prj, id) {
        Some(s) if !s.description.is_empty() => format!("{}  {}", full_number(prj, id), s.description),
        _ => full_number(prj, id),
    }
}

/// The subsections of `parent` (0: the top-level sections) in their order.
pub fn children(prj: &Project, parent: u64) -> Vec<u64> {
    prj.sections.iter().filter(|s| parent_of(prj, s) == parent && s.id != 0).map(|s| s.id).collect()
}

/// All sections in tree order with their depth (0 = top level).
pub fn tree(prj: &Project) -> Vec<(u64, usize)> {
    fn walk(prj: &Project, parent: u64, depth: usize, out: &mut Vec<(u64, usize)>) {
        for c in children(prj, parent) {
            out.push((c, depth));
            walk(prj, c, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(prj, 0, 0, &mut out);
    out
}

/// The section and all sections below it.
pub fn subtree(prj: &Project, id: u64) -> Vec<u64> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        for c in children(prj, out[i]) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        i += 1;
    }
    out
}

/// Section of a page inserted at `at`: that of the page before it.
pub fn section_at(prj: &Project, at: usize) -> u64 {
    match at.min(prj.pages.len()) {
        0 => 0,
        at => of_page(prj, at - 1),
    }
}

/// Number of a page within its section: its name without the section's prefix.
pub fn local_name(prj: &Project, page: usize) -> &str {
    let pg = &prj.pages[page];
    let prefix = prefix(prj, of_page(prj, page));
    pg.name.strip_prefix(prefix.as_str()).unwrap_or(&pg.name)
}

/// Name for a new page of a section (0: top level): one more than the highest page
/// number in it, not used by another page.
pub fn next_page_name(prj: &Project, section: u64) -> String {
    next_name(prj, section, None)
}

/// [`next_page_name`] as if page `skip` were not there.
fn next_name(prj: &Project, section: u64, skip: Option<usize>) -> String {
    let prefix = prefix(prj, section);
    let others = |i: &usize| Some(*i) != skip;
    let max = (0..prj.pages.len()).filter(others).filter(|&i| of_page(prj, i) == section).filter_map(|i| local_name(prj, i).chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>().ok()).max().unwrap_or(0);
    let used: HashSet<&str> = (0..prj.pages.len()).filter(others).map(|i| prj.pages[i].name.as_str()).collect();
    (max.checked_add(1).unwrap_or(1)..).map(|n| format!("{prefix}{n}")).find(|n| !used.contains(n.as_str())).unwrap_or_default()
}

/// Sorts the pages by the tree; pages of the same section keep their order.
pub fn sort_pages(prj: &mut Project) {
    if prj.sections.is_empty() {
        return;
    }
    let rank: HashMap<u64, usize> = tree(prj).iter().enumerate().map(|(i, (id, _))| (*id, i + 1)).collect();
    prj.pages.sort_by_key(|p| rank.get(&p.section).copied().unwrap_or(0));
}

/// Renames the pages of the sections `ids` whose name starts with the full number
/// `old` so that it starts with `new`; released pages keep their name.
fn rename_pages(prj: &mut Project, ids: &[u64], old: &str, new: &str) {
    if old == new {
        return;
    }
    let (old, new) = (format!("{old}{SEPARATOR}"), format!("{new}{SEPARATOR}"));
    for pg in prj.pages.iter_mut().filter(|p| ids.contains(&p.section) && !p.locked) {
        if let Some(rest) = pg.name.strip_prefix(old.as_str()) {
            pg.name = format!("{new}{rest}");
        }
    }
}

/// One more than the highest numeric number of the subsections of `parent` (but
/// `except`).
fn next_number(prj: &Project, parent: u64, except: u64) -> String {
    let max = children(prj, parent).into_iter().filter(|&c| c != except).filter_map(|c| get(prj, c)?.number.parse::<u32>().ok()).max().unwrap_or(0);
    (max + 1).to_string()
}

fn number_taken(prj: &Project, parent: u64, number: &str, except: u64) -> bool {
    children(prj, parent).into_iter().any(|c| c != except && get(prj, c).is_some_and(|s| s.number == number))
}

/// Adds a section after the subsections of `parent` (0: top level) with the next
/// free number. Returns its id.
pub fn add(prj: &mut Project, parent: u64, description: &str) -> u64 {
    let parent = if get(prj, parent).is_some() { parent } else { 0 };
    let number = next_number(prj, parent, 0);
    let id = prj.new_id();
    prj.sections.push(Section { id, parent, number, description: description.into() });
    id
}

/// Changes the number of a section; the names of its pages and of the pages of its
/// subsections follow.
pub fn set_number(prj: &mut Project, id: u64, number: &str) -> Result<(), &'static str> {
    let number = number.trim();
    let s = get(prj, id).ok_or(tr_const!("No such section"))?;
    if number.is_empty() || number.contains(SEPARATOR) {
        return Err(tr_const!("A section number must not be empty or contain a dot"));
    }
    // Letters and digits; numbers count from 1 like the automatic ones.
    if !number.chars().all(char::is_alphanumeric) || number.starts_with('0') {
        return Err(tr_const!("A section number consists of letters and digits and does not start with 0"));
    }
    if number_taken(prj, parent_of(prj, s), number, id) {
        return Err(tr_const!("Another section here has this number"));
    }
    let old = full_number(prj, id);
    if let Some(s) = prj.sections.iter_mut().find(|s| s.id == id) {
        s.number = number.to_string();
    }
    let new = full_number(prj, id);
    rename_pages(prj, &subtree(prj, id), &old, &new);
    Ok(())
}

/// Moves a section after the subsections of `parent` (0: top level); it gets the
/// next free number there if its own is taken. The pages follow.
pub fn set_parent(prj: &mut Project, id: u64, parent: u64) -> Result<(), &'static str> {
    let s = get(prj, id).ok_or(tr_const!("No such section"))?.clone();
    if parent != 0 && get(prj, parent).is_none() {
        return Err(tr_const!("No such section"));
    }
    let ids = subtree(prj, id);
    if ids.contains(&parent) {
        return Err(tr_const!("A section cannot be moved into itself"));
    }
    if parent_of(prj, &s) == parent {
        return Ok(());
    }
    let old = full_number(prj, id);
    let number = if number_taken(prj, parent, &s.number, id) { next_number(prj, parent, id) } else { s.number.clone() };
    prj.sections.retain(|x| x.id != id);
    prj.sections.push(Section { parent, number, ..s });
    let new = full_number(prj, id);
    rename_pages(prj, &ids, &old, &new);
    sort_pages(prj);
    Ok(())
}

/// Swaps a section with the sibling before (`up`) or after it; the two swap their
/// numbers too, so the numbers keep counting in tree order. The pages follow.
/// Returns whether it moved.
pub fn move_section(prj: &mut Project, id: u64, up: bool) -> bool {
    let Some(s) = get(prj, id) else { return false };
    let siblings = children(prj, parent_of(prj, s));
    let Some(k) = siblings.iter().position(|&c| c == id) else { return false };
    let other = match up {
        true if k > 0 => siblings[k - 1],
        false if k + 1 < siblings.len() => siblings[k + 1],
        _ => return false,
    };
    let (a, b) = (prj.sections.iter().position(|s| s.id == id), prj.sections.iter().position(|s| s.id == other));
    let (Some(a), Some(b)) = (a, b) else { return false };
    let (old_a, old_b) = (full_number(prj, id), full_number(prj, other));
    let (ids_a, ids_b) = (subtree(prj, id), subtree(prj, other));
    prj.sections.swap(a, b);
    let number = std::mem::take(&mut prj.sections[a].number);
    prj.sections[a].number = std::mem::replace(&mut prj.sections[b].number, number);
    let (new_a, new_b) = (full_number(prj, id), full_number(prj, other));
    rename_pages(prj, &ids_a, &old_a, &new_a);
    rename_pages(prj, &ids_b, &old_b, &new_b);
    sort_pages(prj);
    true
}

/// Moves a section into `parent` (0: top level) before its subsection `before`
/// (`None`: after them): [`set_parent`], then [`move_section`] steps, so the numbers
/// keep counting in tree order. The pages follow.
pub fn move_section_to(prj: &mut Project, id: u64, parent: u64, before: Option<u64>) -> Result<(), &'static str> {
    set_parent(prj, id, parent)?;
    let siblings = children(prj, parent);
    let Some(k) = siblings.iter().position(|&c| c == id) else { return Ok(()) };
    let target = match before.and_then(|b| siblings.iter().position(|&c| c == b)) {
        Some(b) if b > k => b - 1,
        Some(b) => b,
        None => siblings.len() - 1,
    };
    for _ in target..k {
        move_section(prj, id, true);
    }
    for _ in k..target {
        move_section(prj, id, false);
    }
    Ok(())
}

/// The full number a section gets from [`move_section_to`].
pub fn number_after_move(prj: &Project, id: u64, parent: u64, before: Option<u64>) -> Result<String, &'static str> {
    let mut sim = Project { sections: prj.sections.clone(), pages: Vec::new(), ..Project::default() };
    move_section_to(&mut sim, id, parent, before)?;
    Ok(full_number(&sim, id))
}

/// Moves a page into a section (0: top level), after the section's pages. It keeps
/// its number within the section if that is free there, otherwise it gets the next
/// one. Returns the page's new index.
pub fn move_page(prj: &mut Project, page: usize, section: u64) -> Result<usize, &'static str> {
    let section = if get(prj, section).is_some() { section } else { 0 };
    if prj.pages.get(page).is_some_and(|p| !p.locked) && of_page(prj, page) == section {
        return Ok(page);
    }
    move_page_to(prj, page, section, None)
}

/// The name a page gets when it moves into a section (0: top level): its number
/// within its section if that is free there, otherwise the next one; within its own
/// section it keeps its name.
pub fn moved_name(prj: &Project, page: usize, section: u64) -> String {
    let section = if get(prj, section).is_some() { section } else { 0 };
    if of_page(prj, page) == section {
        return prj.pages[page].name.clone();
    }
    let local = local_name(prj, page);
    let name = format!("{}{local}", prefix(prj, section));
    let taken = local.is_empty() || prj.pages.iter().enumerate().any(|(i, p)| i != page && p.name == name);
    if taken { next_name(prj, section, Some(page)) } else { name }
}

/// Moves a page into a section (0: top level) before the page `before` of that
/// section (`None`, or a page of another section: after the section's pages); it gets
/// [`moved_name`]. Returns the page's new index.
pub fn move_page_to(prj: &mut Project, page: usize, section: u64, before: Option<usize>) -> Result<usize, &'static str> {
    let section = if get(prj, section).is_some() { section } else { 0 };
    let pg = prj.pages.get(page).ok_or(tr_const!("No such page"))?;
    if pg.locked {
        return Err(tr_const!("Released pages cannot be moved"));
    }
    let before = before.filter(|&b| b < prj.pages.len() && b != page && of_page(prj, b) == section).map(|b| prj.pages[b].id);
    let name = moved_name(prj, page, section);
    let mut pg = prj.pages.remove(page);
    pg.name = name;
    pg.section = section;
    let id = pg.id;
    let at = before.and_then(|b| prj.pages.iter().position(|p| p.id == b)).unwrap_or(prj.pages.len());
    prj.pages.insert(at, pg);
    sort_pages(prj);
    Ok(prj.pages.iter().position(|p| p.id == id).unwrap_or(0))
}

/// Puts a new page into a section (0: top level) after the section's pages, with the
/// next page number there. Returns the page's new index.
pub fn place_new_page(prj: &mut Project, page: usize, section: u64) -> usize {
    let section = if get(prj, section).is_some() { section } else { 0 };
    if of_page(prj, page) == section {
        return page;
    }
    let mut pg = prj.pages.remove(page);
    pg.name = next_page_name(prj, section);
    pg.section = section;
    let id = pg.id;
    prj.pages.push(pg);
    sort_pages(prj);
    prj.pages.iter().position(|p| p.id == id).unwrap_or(0)
}

/// Deletes a section; its pages and subsections go to its parent (renamed like moved
/// ones). Fails while it holds released pages.
pub fn delete(prj: &mut Project, id: u64) -> Result<(), &'static str> {
    let s = get(prj, id).ok_or(tr_const!("No such section"))?;
    let parent = parent_of(prj, s);
    if prj.pages.iter().any(|p| p.section == id && p.locked) {
        return Err(tr_const!("Released pages cannot be moved"));
    }
    while let Some(i) = prj.pages.iter().position(|p| p.section == id) {
        move_page(prj, i, parent)?;
    }
    let kids: Vec<(u64, String, Vec<u64>)> = children(prj, id).into_iter().map(|c| (c, full_number(prj, c), subtree(prj, c))).collect();
    prj.sections.retain(|s| s.id != id);
    for (c, old, ids) in kids {
        let taken = get(prj, c).is_some_and(|s| number_taken(prj, parent, &s.number, c));
        let number = taken.then(|| next_number(prj, parent, c));
        if let Some(s) = prj.sections.iter_mut().find(|s| s.id == c) {
            s.parent = parent;
            if let Some(n) = number {
                s.number = n;
            }
        }
        rename_pages(prj, &ids, &old, &full_number(prj, c));
    }
    sort_pages(prj);
    Ok(())
}

/// Full numbers of the sections after [`renumber`].
pub fn renumbered(prj: &Project) -> HashMap<u64, String> {
    let mut local: HashMap<u64, String> = HashMap::new();
    for (id, _) in tree(prj) {
        let Some(s) = get(prj, id) else { continue };
        let k = children(prj, parent_of(prj, s)).iter().position(|&c| c == id).unwrap_or(0);
        local.insert(id, (k + 1).to_string());
    }
    let mut out = HashMap::new();
    for (id, _) in tree(prj) {
        let full = match get(prj, id).map(|s| parent_of(prj, s)) {
            Some(p) if p != 0 => format!("{}{SEPARATOR}{}", out.get(&p).cloned().unwrap_or_default(), local[&id]),
            _ => local[&id].clone(),
        };
        out.insert(id, full);
    }
    out
}

/// Numbers the sections 1, 2, 3 … on every level in tree order; the pages follow.
/// Returns how many sections got a new number.
pub fn renumber(prj: &mut Project) -> usize {
    let new = renumbered(prj);
    let old: Vec<(u64, String)> = tree(prj).into_iter().map(|(id, _)| (id, full_number(prj, id))).collect();
    let mut n = 0;
    for s in prj.sections.iter_mut() {
        let Some(full) = new.get(&s.id) else { continue };
        let number = full.rsplit(SEPARATOR).next().unwrap_or_default().to_string();
        if s.number != number {
            s.number = number;
            n += 1;
        }
    }
    for (id, old) in old {
        rename_pages(prj, &[id], &old, &new[&id]);
    }
    n
}

#[cfg(test)]
mod tests;
