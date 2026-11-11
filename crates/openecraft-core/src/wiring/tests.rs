use super::*;
use crate::analysis::Analysis;
use crate::sample::demo_project;

#[test]
fn steps_cover_the_routed_wires_in_order() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let all = steps(&prj, &an.ends, &an.wiring, None, Order::Device);
    assert_eq!(all.len(), an.wiring.len());
    assert!(all.iter().all(|s| s.status == Status::Open));
    for pair in all.windows(2) {
        let (a, b) = (&an.wiring[pair[0].wire], &an.wiring[pair[1].wire]);
        assert_ne!(natural_cmp(&a.from, &b.from), std::cmp::Ordering::Greater);
    }
    let thick = steps(&prj, &an.ends, &an.wiring, Some(0), Order::CrossSection);
    let cs: Vec<f32> = thick.iter().map(|s| cross_section_mm2(&an.wiring[s.wire].cross_section).unwrap_or(0.0)).collect();
    assert!(cs.windows(2).all(|w| w[0] >= w[1]), "{cs:?}");
    assert!(thick.iter().all(|s| s.ends.iter().all(|e| e.is_some())));
}

#[test]
fn marks_track_progress_and_changes() {
    let mut prj = demo_project();
    let an = Analysis::checked(&prj);
    let w = an.wiring[0].clone();
    assert_eq!(progress(&prj, &an.wiring, None).done, 0);
    set_mark(&mut prj, &w, Some(Mark::Done), "");
    let p = progress(&prj, &an.wiring, None);
    assert_eq!((p.done, p.total), (1, an.wiring.len()));
    assert!((p.length_done - w.length).abs() < 0.01);
    assert!(p.fraction() > 0.0);
    assert_eq!(status(&prj, &w), Status::Done);

    // The wire gets another colour in the schematic: it has to be checked again.
    let mut changed = w.clone();
    changed.color = "VT".into();
    assert_eq!(status(&prj, &changed), Status::Changed);
    assert_eq!(progress(&prj, std::slice::from_ref(&changed), None).fraction(), 0.0);

    set_mark(&mut prj, &w, Some(Mark::Problem), "too short");
    assert_eq!(prj.wiring_marks.len(), 1);
    assert_eq!(status(&prj, &w), Status::Problem);
    let an2 = Analysis::checked(&prj);
    assert!(an2.issues.iter().any(|i| i.message().contains("too short")));

    set_mark(&mut prj, &w, None, "");
    assert!(prj.wiring_marks.is_empty());
}

#[test]
fn marks_of_removed_wires_are_stale() {
    let mut prj = demo_project();
    let an = Analysis::checked(&prj);
    let mut gone = an.wiring[0].clone();
    gone.from = "-X99:1".into();
    set_mark(&mut prj, &gone, Some(Mark::Done), "");
    set_mark(&mut prj, &an.wiring[1], Some(Mark::Done), "");
    assert_eq!(stale(&prj, &an.wiring).len(), 1);
    assert_eq!(remove_stale(&mut prj, &an.wiring), 1);
    assert_eq!(prj.wiring_marks.len(), 1);
}

#[test]
fn next_open_skips_wired_steps_and_wraps() {
    let step = |status| Step { wire: 0, status, note: String::new(), ends: [None; 2], twins: [None, None] };
    let v = vec![step(Status::Open), step(Status::Done), step(Status::Changed), step(Status::Done)];
    assert_eq!(next_open(&v, None), Some(0));
    assert_eq!(next_open(&v, Some(0)), Some(2));
    assert_eq!(next_open(&v, Some(2)), Some(0));
    assert_eq!(next_open(&[step(Status::Done)], Some(0)), None);
    assert_eq!(next_open(&[], None), None);
}

#[test]
fn csv_lists_every_wire_with_its_state() {
    let mut prj = demo_project();
    let an = Analysis::checked(&prj);
    set_mark(&mut prj, &an.wiring[0], Some(Mark::Problem), "a;b");
    let text = csv(&prj, &an.ends, &an.wiring, Order::Number);
    assert_eq!(text.lines().count(), an.wiring.len() + 1);
    assert!(text.lines().any(|l| l.ends_with(";problem;a b")));
    assert!(text.starts_with("Enclosure;Wire no.;From;To"));
}
