use super::*;
use crate::model::PageKind;

fn names(prj: &Project) -> Vec<&str> {
    prj.pages.iter().map(|p| p.name.as_str()).collect()
}

/// Cover page 1, section 1 with pages 1.1 and 1.2 and subsection 1.1 with page 1.1.1.
fn project() -> (Project, u64, u64) {
    let mut prj = Project::default();
    prj.pages[0].kind = PageKind::Cover;
    let a = add(&mut prj, 0, "Supply");
    let b = add(&mut prj, a, "Main feed");
    for s in [b, a, a] {
        let id = prj.new_id();
        let mut pg = crate::model::Page::new(id, &next_page_name(&prj, s), "", PageKind::Schematic);
        pg.section = s;
        prj.pages.push(pg);
    }
    sort_pages(&mut prj);
    (prj, a, b)
}

#[test]
fn page_names_include_the_section_numbers() {
    let (prj, a, b) = project();
    assert_eq!(names(&prj), ["1", "1.1", "1.2", "1.1.1"]);
    assert_eq!(full_number(&prj, b), "1.1");
    assert_eq!(title(&prj, a), "1  Supply");
    assert_eq!(local_name(&prj, 3), "1");
    assert_eq!(tree(&prj), [(a, 0), (b, 1)]);
    assert_eq!(next_page_name(&prj, a), "1.3");
    assert_eq!(next_page_name(&prj, 0), "2");
}

#[test]
fn renumbering_a_section_renames_its_pages_and_subsections() {
    let (mut prj, a, b) = project();
    prj.pages[2].locked = true;
    set_number(&mut prj, a, "4").unwrap();
    assert_eq!(names(&prj), ["1", "4.1", "1.2", "4.1.1"]);
    assert!(set_number(&mut prj, b, "").is_err());
    assert!(set_number(&mut prj, b, "2.1").is_err());
    let c = add(&mut prj, a, "");
    assert_eq!(full_number(&prj, c), "4.2");
    assert!(set_number(&mut prj, c, "1").is_err());
}

#[test]
fn moving_pages_and_sections_keeps_the_tree_order() {
    let (mut prj, a, b) = project();
    let c = add(&mut prj, 0, "Controls");
    // Page 1.2 goes to section 2 and keeps its number there.
    let at = move_page(&mut prj, 2, c).unwrap();
    assert_eq!((at, names(&prj)), (3, vec!["1", "1.1", "1.1.1", "2.2"]));
    // Section 1.1 moves to the top level: number 1 is taken there, it becomes 3.
    set_parent(&mut prj, b, 0).unwrap();
    assert_eq!(names(&prj), ["1", "1.1", "2.2", "3.1"]);
    assert!(set_parent(&mut prj, a, a).is_err());
    // Section 3 moves up: it swaps place and number with section 2.
    assert!(move_section(&mut prj, b, true));
    assert_eq!(names(&prj), ["1", "1.1", "2.1", "3.2"]);
    assert!(!move_section(&mut prj, a, true));
}

#[test]
fn deleting_a_section_moves_its_content_to_the_parent() {
    let (mut prj, a, b) = project();
    let c = add(&mut prj, b, "");
    delete(&mut prj, b).unwrap();
    assert_eq!(names(&prj), ["1", "1.1", "1.2", "1.3"]);
    assert_eq!(get(&prj, c).map(|s| s.parent), Some(a));
    assert_eq!(full_number(&prj, c), "1.1");
    delete(&mut prj, a).unwrap();
    assert_eq!(names(&prj), ["1", "2", "3", "4"]);
    assert_eq!(full_number(&prj, c), "1");
    assert!(delete(&mut prj, a).is_err());
}

#[test]
fn renumber_counts_every_level_from_one() {
    let (mut prj, a, b) = project();
    let c = add(&mut prj, 0, "");
    set_number(&mut prj, a, "7").unwrap();
    set_number(&mut prj, b, "B").unwrap();
    set_number(&mut prj, c, "9").unwrap();
    assert_eq!(renumbered(&prj)[&b], "1.1");
    assert_eq!(renumber(&mut prj), 3);
    assert_eq!(names(&prj), ["1", "1.1", "1.2", "1.1.1"]);
    assert_eq!(full_number(&prj, c), "2");
}

#[test]
fn pages_of_unknown_sections_are_top_level_pages() {
    let (mut prj, _, _) = project();
    prj.pages[0].section = 999;
    assert_eq!(of_page(&prj, 0), 0);
    sort_pages(&mut prj);
    assert_eq!(names(&prj)[0], "1");
}

#[test]
fn section_numbers_are_letters_and_digits() {
    let mut prj = Project::new_default();
    let id = add(&mut prj, 0, "A");
    for bad in ["-1", "0", "01", "a b", "1/2"] {
        assert!(set_number(&mut prj, id, bad).is_err(), "{bad}");
    }
    assert!(set_number(&mut prj, id, "A").is_ok());
    assert!(set_number(&mut prj, id, "12").is_ok());
}

#[test]
fn a_page_moves_between_pages_of_a_section() {
    let (mut prj, a, b) = project();
    // Page 1.2 goes before page 1.1 of its own section: the order changes, the name stays.
    assert_eq!(moved_name(&prj, 2, a), "1.2");
    let at = move_page_to(&mut prj, 2, a, Some(1)).unwrap();
    assert_eq!((at, names(&prj)), (1, vec!["1", "1.2", "1.1", "1.1.1"]));
    // Page 1 goes into section 1.1 before its page 1.1.1: number 1 is taken there.
    assert_eq!(moved_name(&prj, 0, b), "1.1.2");
    let at = move_page_to(&mut prj, 0, b, Some(3)).unwrap();
    assert_eq!((at, names(&prj)), (2, vec!["1.2", "1.1", "1.1.2", "1.1.1"]));
    // A page of another section as `before` puts it after the section's pages.
    let at = move_page_to(&mut prj, 0, 0, Some(1)).unwrap();
    assert_eq!((at, names(&prj)), (0, vec!["2", "1.1", "1.1.2", "1.1.1"]));
    prj.pages[1].locked = true;
    assert!(move_page_to(&mut prj, 1, 0, None).is_err());
    assert!(move_page_to(&mut prj, 9, 0, None).is_err());
}

#[test]
fn pages_without_sections_are_reordered() {
    let mut prj = Project::default();
    for _ in 0..2 {
        let at = prj.pages.len();
        crate::ops::add_page(&mut prj, PageKind::Schematic, at);
    }
    assert_eq!(names(&prj), ["1", "2", "3"]);
    assert_eq!(move_page_to(&mut prj, 2, 0, Some(0)), Ok(0));
    assert_eq!(names(&prj), ["3", "1", "2"]);
    assert_eq!(move_page_to(&mut prj, 0, 0, None), Ok(2));
    assert_eq!(names(&prj), ["1", "2", "3"]);
}

#[test]
fn a_section_moves_before_a_sibling_and_into_another_section() {
    let (mut prj, a, b) = project();
    let c = add(&mut prj, 0, "Controls");
    let d = add(&mut prj, 0, "Field");
    // Section 3 goes before section 1: the numbers keep counting in tree order.
    assert_eq!(number_after_move(&prj, d, 0, Some(a)).as_deref(), Ok("1"));
    move_section_to(&mut prj, d, 0, Some(a)).unwrap();
    assert_eq!(children(&prj, 0), [d, a, c]);
    assert_eq!((full_number(&prj, a), full_number(&prj, b)), ("2".into(), "2.1".into()));
    assert_eq!(names(&prj), ["1", "2.1", "2.2", "2.1.1"]);
    // Section 2.1 goes into section 3 as its last subsection, then back before nothing.
    assert_eq!(number_after_move(&prj, b, c, None).as_deref(), Ok("3.1"));
    move_section_to(&mut prj, b, c, None).unwrap();
    assert_eq!(names(&prj), ["1", "2.1", "2.2", "3.1.1"]);
    // Moving a section onto its old place changes nothing; into itself fails.
    move_section_to(&mut prj, a, 0, Some(c)).unwrap();
    assert_eq!(children(&prj, 0), [d, a, c]);
    assert!(number_after_move(&prj, c, b, None).is_err());
    assert_eq!(full_number(&prj, b), "3.1");
}

#[test]
fn new_pages_continue_numeric_stems_of_imported_alphabetic_subpages() {
    let mut prj=Project::default();prj.pages[0].name="1.a".into();
    for name in ["2.b","20a","0009.c","Legend"] {
        let id=prj.new_id();prj.pages.push(crate::model::Page::new(id,name,"",PageKind::Schematic));
    }
    assert_eq!(next_page_name(&prj,0),"21");
    let before=prj.pages.clone();
    let added=crate::ops::add_page_in(&mut prj,PageKind::Schematic,0);
    assert_eq!(prj.pages[added].name,"21");assert_eq!(&prj.pages[..before.len()],&before);
    assert_eq!(next_page_name(&prj,0),"22");
}

#[test]
fn subpage_numbering_keeps_section_scope_and_global_name_collisions() {
    let (mut prj,a,b)=project();
    for page in prj.pages.iter_mut().filter(|p|p.section==a) {page.name.push_str(".a");}
    prj.pages[0].name="1.3".into();
    assert_eq!(next_page_name(&prj,a),"1.4");
    assert_eq!(next_page_name(&prj,b),"1.1.2");
    let at=crate::ops::add_page_in(&mut prj,PageKind::Schematic,a);
    assert_eq!(prj.pages[at].name,"1.4");assert_eq!(prj.pages[at].section,a);
}

#[test]
fn very_large_imported_page_numbers_do_not_overflow() {
    let mut prj=Project::default();prj.pages[0].name="18446744073709551615.a".into();
    assert_eq!(next_page_name(&prj,0),"1");
    prj.pages[0].name="4294967295.a".into();
    assert_eq!(next_page_name(&prj,0),"4294967296");
}
