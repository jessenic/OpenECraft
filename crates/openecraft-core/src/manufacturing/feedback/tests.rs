use super::*;
use crate::manufacturing::handoff::release;
use crate::sample::demo_project;

fn released() -> Project {
    let mut prj = demo_project();
    release(&mut prj, 0, "2026-10-24").unwrap();
    prj
}

#[test]
fn feedback_from_csv_and_json() {
    let csv = "Auftrag;File;Op;BMK;Result;dev_x;dev_y;Duration;Note\nCAB1-1;CAB1_plate.nc;0;;OK;;;120;\nCAB1-1;CAB1_door.nc;3;-S1;NOK;0,8;0;;burr\n";
    let v = parse(csv).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!((v[0].status, v[0].time), (Status::Done, 120.0));
    assert_eq!((v[1].status, v[1].seq, v[1].item.as_str(), v[1].dx, v[1].message.as_str()), (Status::Failed, 3, "-S1", 0.8, "burr"));
    let json = r#"{ "feedback": [{ "job": "CAB1-1", "status": "rework", "message": "paint", "time": 30 }] }"#;
    let j = parse(json).unwrap();
    assert_eq!((j[0].status, j[0].time), (Status::Rework, 30.0));
    assert!(parse("job;status\nCAB1-1;maybe").is_err());
    assert!(parse("status\ndone").is_err());
    assert!(parse("").is_err());
    assert!(Status::ALL.iter().all(|s| Status::parse(s.key()) == Some(*s)));
}

#[test]
fn progress_problems_and_calibration() {
    let mut prj = released();
    assert!(import(&mut prj, "job;status\nCAB9-1;done").is_err());
    let plate = prj.layout.manufacturing.jobs[0].programs[0].clone();
    let st = status(&prj);
    assert_eq!((st[0].done, st[0].title()), (0, "Released"));
    let csv = format!("job;program;status;time;dx;item\nCAB1-1;{};done;{};0.2;\nCAB1-1;CAB1_door.nc;started;;;\nCAB1-1;CAB1_door.nc;rework;;1.5;-S2\n", plate.name, plate.time * 2.0);
    assert_eq!(import(&mut prj, &csv).unwrap(), 3);
    let st = status(&prj);
    // Plate, door and bottom (the hole of the gland): one of three done.
    assert_eq!((st[0].programs, st[0].done, st[0].rework), (3, 1, 1));
    assert!((st[0].progress() - 100.0 / 3.0).abs() < 0.01, "{}", st[0].progress());
    assert_eq!(st[0].title(), "In production");
    let p = problems(&prj);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(p.iter().any(|m| m.contains("reworked")) && p.iter().any(|m| m.contains("1.5 mm off")));
    let cal = calibrate(&mut prj, false);
    assert_eq!(cal.len(), 1);
    assert!((cal[0].2 - 2.0).abs() < 1e-3, "{cal:?}");
    assert!(prj.layout.manufacturing.machines.is_empty());
    calibrate(&mut prj, true);
    assert_eq!(crate::manufacturing::machines::find(&prj, &plate.machine).unwrap().time_factor, cal[0].2);
    // A failed whole job.
    import(&mut prj, "job;status;message\nCAB1-1;failed;power cut").unwrap();
    assert_eq!(status(&prj)[0].title(), "Problem");
    assert!(problems(&prj).iter().any(|m| m.contains("power cut")));
}
