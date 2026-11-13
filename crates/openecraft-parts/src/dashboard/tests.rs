use super::*;
use crate::portal::{Attachment, AttachmentKind};

const NOV_2026: u64 = 1_794_268_800; // 2026-11-10

fn part(number: &str, maker: &str, modified: u64) -> Part {
    Part { number: number.into(), manufacturer: maker.into(), group: "Relays, contactors".into(), modified, ..Default::default() }
}

#[test]
fn months_run_up_to_now() {
    assert_eq!(month(NOV_2026), "2026-11");
    let months = last_months(NOV_2026);
    assert_eq!(months.len(), MONTHS);
    assert_eq!(months[0], "2025-12");
    assert_eq!(months[MONTHS - 1], "2026-11");
}

#[test]
fn long_distributions_end_with_the_others() {
    let counts: BTreeMap<String, usize> = (0..15).map(|i| (format!("M{i}"), i + 1)).collect();
    let list = ranked(counts);
    assert_eq!(list.len(), MAX_VALUES);
    assert_eq!((list[0].value.as_str(), list[0].count), ("M14", 15));
    let others = list.last().unwrap();
    assert_eq!(others.value, OTHERS);
    // M0 … M5 with 1 … 6 parts.
    assert_eq!(others.count, 21);
    assert_eq!(list.iter().map(|v| v.count).sum::<usize>(), (1..=15).sum::<usize>());
}

#[test]
fn parts_are_counted() {
    let mut a = part("A", "Siemens", NOV_2026);
    a.lifecycle = Lifecycle::Discontinued;
    a.price = 3.5;
    a.attachments = vec![Attachment { id: "f".into(), name: "a.pdf".into(), kind: AttachmentKind::Datasheet, size: 1000 }];
    let b = part("B", "Siemens", NOV_2026 - 40 * 86_400);
    let mut c = part("C", "ABB", 0);
    c.catalog = "abb".into();
    let d = dashboard(&[a, b, c], NOV_2026);
    assert_eq!((d.parts, d.manufacturers, d.files, d.file_size), (3, 2, 1, 1000));
    assert_eq!(d.by_manufacturer[0], FacetValue { value: "Siemens".into(), count: 2 });
    assert_eq!(d.by_catalog.iter().find(|v| v.value.is_empty()).unwrap().count, 2);
    assert_eq!(d.by_lifecycle.iter().map(|v| (v.value.as_str(), v.count)).collect::<Vec<_>>(), [("Active", 2), ("PhaseOut", 0), ("Discontinued", 1)]);
    assert_eq!(d.changes.last().unwrap().count, 1);
    assert_eq!(d.changes[MONTHS - 2], FacetValue { value: "2026-10".into(), count: 1 });
    assert_eq!((d.coverage.price, d.coverage.files, d.coverage.datasheet, d.coverage.description), (1, 1, 1, 0));
    // Parts never changed on the server are not among the latest changes.
    assert_eq!(d.recent.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["A", "B"]);
}
