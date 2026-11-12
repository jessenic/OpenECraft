use super::*;
use egui::{pos2, vec2, Modifiers, Rect, RawInput, Sense, TouchDeviceId, TouchId, TouchPhase};

fn scroll(unit: MouseWheelUnit, delta: Vec2, source: MouseWheelSource, modifiers: Modifiers) -> Event {
    Event::MouseWheel { unit, delta, phase: TouchPhase::Move, source, modifiers }
}

fn setting(scroll: Scroll, invert: bool) -> Settings {
    Settings { scroll, invert, ..Settings::default() }
}

#[test]
fn a_mouse_wheel_zooms_and_pans_sideways() {
    let up = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::NONE);
    assert_eq!(wheel(std::slice::from_ref(&up), setting(Scroll::Zoom, false)), (40.0, Vec2::ZERO));
    // Unknown line-based wheel events zoom, including on platforms that do not report the source.
    assert_eq!(wheel(&[up], setting(Scroll::Auto, false)), (40.0, Vec2::ZERO));
    let unknown = scroll(MouseWheelUnit::Line, vec2(0.0, -2.0), MouseWheelSource::Unknown, Modifiers::NONE);
    assert_eq!(wheel(std::slice::from_ref(&unknown), setting(Scroll::Auto, false)), (-80.0, Vec2::ZERO));
    let strict = Settings { wheel_source_policy: WheelSourcePolicy::Strict, ..setting(Scroll::Auto, false) };
    assert_eq!(wheel(&[unknown], strict), (-80.0, Vec2::ZERO));
    // The horizontal wheel pans, also when the vertical one zooms.
    let side = scroll(MouseWheelUnit::Line, vec2(1.0, 0.0), MouseWheelSource::Wheel, Modifiers::NONE);
    assert_eq!(wheel(&[side], setting(Scroll::Zoom, false)), (0.0, vec2(40.0, 0.0)));
    // Shift turns the wheel sideways.
    let shifted = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::SHIFT);
    assert_eq!(wheel(&[shifted], setting(Scroll::Zoom, false)), (0.0, vec2(40.0, 0.0)));
}

#[test]
fn trackpads_pan_in_both_directions() {
    let fingers = scroll(MouseWheelUnit::Point, vec2(3.0, -5.0), MouseWheelSource::Trackpad, Modifiers::NONE);
    assert_eq!(wheel(std::slice::from_ref(&fingers), setting(Scroll::Auto, false)), (0.0, vec2(3.0, -5.0)));
    let momentum = scroll(MouseWheelUnit::Point, vec2(0.0, 2.0), MouseWheelSource::Momentum, Modifiers::NONE);
    assert_eq!(wheel(&[fingers.clone(), momentum], setting(Scroll::Auto, false)), (0.0, vec2(3.0, -3.0)));
    // Explicit Pan mode pans even when the source is unknown.
    let unknown = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Unknown, Modifiers::NONE);
    assert_eq!(wheel(&[unknown], setting(Scroll::Pan, false)), (0.0, vec2(0.0, 40.0)));
    // Chosen: trackpads zoom like a wheel.
    assert_eq!(wheel(std::slice::from_ref(&fingers), setting(Scroll::Zoom, false)), (-5.0, vec2(3.0, 0.0)));
    // Inverted: the content moves against the fingers.
    assert_eq!(wheel(&[fingers], setting(Scroll::Auto, true)), (0.0, vec2(-3.0, 5.0)));
}

#[test]
fn wheel_source_policy_maps_unknown_events_only_when_enabled() {
    let precise_unknown = scroll(
        MouseWheelUnit::Point,
        vec2(3.0, -5.0),
        MouseWheelSource::Unknown,
        Modifiers::NONE,
    );
    let line_unknown = scroll(
        MouseWheelUnit::Line,
        vec2(0.0, -1.0),
        MouseWheelSource::Unknown,
        Modifiers::NONE,
    );
    let smooth_line_unknown = scroll(
        MouseWheelUnit::Line,
        vec2(0.0, -0.5),
        MouseWheelSource::Unknown,
        Modifiers::NONE,
    );
    let page_unknown = scroll(
        MouseWheelUnit::Page,
        vec2(0.0, -0.5),
        MouseWheelSource::Unknown,
        Modifiers::NONE,
    );
    let strict = Settings {
        wheel_source_policy: WheelSourcePolicy::Strict,
        ..setting(Scroll::Auto, false)
    };
    assert_eq!(
        wheel(std::slice::from_ref(&precise_unknown), strict),
        (-5.0, vec2(3.0, 0.0)),
        "strict policy keeps unknown vertical input as zoom"
    );

    let assume_touchpad = Settings {
        wheel_source_policy: WheelSourcePolicy::AssumeWindowsTrackpad,
        ..strict
    };
    assert_eq!(
        wheel(std::slice::from_ref(&precise_unknown), assume_touchpad),
        (0.0, vec2(3.0, -5.0)),
        "assumed precise input pans both axes"
    );
    assert_eq!(
        wheel(std::slice::from_ref(&line_unknown), assume_touchpad),
        (-40.0, Vec2::ZERO),
        "whole-line wheel steps still zoom"
    );
    assert_eq!(
        wheel(std::slice::from_ref(&smooth_line_unknown), assume_touchpad),
        (0.0, vec2(0.0, -20.0)),
        "fractional line deltas are treated as smooth trackpad input"
    );
    assert_eq!(
        wheel(std::slice::from_ref(&page_unknown), assume_touchpad),
        (-200.0, Vec2::ZERO),
        "page-based deltas are treated as wheel input"
    );

    let mouse = scroll(
        MouseWheelUnit::Point,
        vec2(3.0, -5.0),
        MouseWheelSource::Wheel,
        Modifiers::NONE,
    );
    assert_eq!(
        wheel(&[mouse], assume_touchpad),
        (-5.0, vec2(3.0, 0.0)),
        "an explicitly identified mouse wheel is unchanged"
    );

    for source in [MouseWheelSource::Trackpad, MouseWheelSource::Momentum] {
        let event = scroll(
            MouseWheelUnit::Point,
            vec2(3.0, -5.0),
            source,
            Modifiers::NONE,
        );
        assert_eq!(
            wheel(&[event], strict),
            (0.0, vec2(3.0, -5.0)),
            "{source:?} behavior remains unchanged"
        );
    }
}

#[test]
fn wheel_source_policy_defaults_and_round_trips() {
    let old_settings: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old_settings.wheel_source_policy, WheelSourcePolicy::AssumeWindowsTrackpad);

    let settings = Settings {
        wheel_source_policy: WheelSourcePolicy::AssumeWindowsTrackpad,
        ..Settings::default()
    };
    let stored = serde_json::to_string(&settings).unwrap();
    let restored: Settings = serde_json::from_str(&stored).unwrap();
    assert_eq!(restored, settings);
}

#[test]
fn the_zoom_modifier_zooms_also_where_scrolling_pans() {
    for m in [Modifiers::CTRL, Modifiers::COMMAND] {
        let pinch = scroll(MouseWheelUnit::Point, vec2(0.0, 4.0), MouseWheelSource::Trackpad, m);
        assert_eq!(wheel(&[pinch], setting(Scroll::Pan, false)), (8.0, Vec2::ZERO));
    }
    // Another modifier chosen: Ctrl + wheel is plain scrolling, Alt zooms.
    let s = Settings { zoom: Some(Mod::Alt), ..setting(Scroll::Pan, false) };
    let ctrl = scroll(MouseWheelUnit::Point, vec2(0.0, 4.0), MouseWheelSource::Trackpad, Modifiers::CTRL);
    assert_eq!(wheel(&[ctrl], s), (0.0, vec2(0.0, 4.0)));
    let alt = scroll(MouseWheelUnit::Point, vec2(0.0, 4.0), MouseWheelSource::Trackpad, Modifiers::ALT);
    assert_eq!(wheel(std::slice::from_ref(&alt), s), (8.0, Vec2::ZERO));
    // Off: no modifier zooms.
    assert_eq!(wheel(&[alt], Settings { zoom: None, ..s }), (0.0, vec2(0.0, 4.0)));
}

#[test]
fn the_sideways_modifier_can_be_changed_or_switched_off() {
    let alt = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::ALT);
    let shift = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::SHIFT);
    let s = Settings { sideways: Some(Mod::Alt), ..setting(Scroll::Zoom, false) };
    assert_eq!(wheel(std::slice::from_ref(&alt), s), (0.0, vec2(40.0, 0.0)));
    assert_eq!(wheel(&[shift], s), (40.0, Vec2::ZERO), "Shift zooms like the plain wheel");
    assert_eq!(wheel(&[alt], Settings { sideways: None, ..s }), (40.0, Vec2::ZERO));
}

#[test]
fn trackpad_pinches_zoom_unless_switched_off() {
    let events = [Event::Zoom(1.5), Event::Zoom(2.0)];
    assert!((pinch(&events, Settings::default()) - 3.0).abs() < 1e-6);
    assert_eq!(pinch(&events, Settings { two_fingers: Fingers::Pan, ..Settings::default() }), 1.0);
    assert!((pinch(&events, Settings { two_fingers: Fingers::Zoom, ..Settings::default() }) - 3.0).abs() < 1e-6);
}

#[test]
fn modifiers_are_named_and_recognised() {
    for m in Mod::ALL {
        assert_eq!(Mod::parse(m.name()), Some(m));
    }
    assert_eq!(Mod::parse("Cmd"), Some(Mod::Ctrl));
    assert_eq!(Mod::parse("Hyper"), None);
    assert!(Mod::Ctrl.held(Modifiers::COMMAND) && Mod::Ctrl.held(Modifiers::CTRL));
    assert_eq!(Mod::of(Modifiers::SHIFT | Modifiers::CTRL), Some(Mod::Ctrl));
    assert_eq!(Mod::of(Modifiers::NONE), None);
}

#[test]
fn a_touch_gesture_lasts_until_the_frame_after_the_last_finger() {
    // One finger: no gesture.
    assert_eq!(sticky(false, false, true), (false, false));
    // A second finger starts it; it stays while one is left …
    assert_eq!(sticky(false, true, true), (true, true));
    assert_eq!(sticky(true, false, true), (true, true));
    // … and in the frame of the release, then it is over.
    assert_eq!(sticky(true, false, false), (true, false));
    assert_eq!(sticky(false, false, false), (false, false));
}

fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None }
}

/// One frame with the view filling the screen; gives its gesture.
fn frame(ctx: &Context, time: f64, events: Vec<Event>) -> Gesture {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), time: Some(time), events, ..Default::default() };
    let mut g = Gesture::NONE;
    let out = ctx.run_ui(input, |ui| {
        let (_, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        g = read(ui, &resp);
    });
    out.drop_without_applying_deltas();
    g
}

#[test]
fn two_fingers_pinch_and_pan_together() {
    let ctx = Context::default();
    frame(&ctx, 0.0, vec![Event::PointerMoved(pos2(100.0, 100.0))]);
    // The first finger is the pointer too (as the integrations emulate it).
    let first = frame(&ctx, 0.1, vec![Event::PointerMoved(pos2(100.0, 100.0)), touch(1, TouchPhase::Start, pos2(100.0, 100.0))]);
    assert!(!first.touch);
    let start = frame(&ctx, 0.2, vec![touch(2, TouchPhase::Start, pos2(200.0, 100.0))]);
    assert!(start.touch && !start.zooms());
    // Spread to twice the distance while the middle moves 50 to the right.
    let g = frame(&ctx, 0.3, vec![touch(2, TouchPhase::Move, pos2(300.0, 100.0))]);
    assert!(g.touch);
    assert!((g.zoom - 2.0).abs() < 1e-3, "{g:?}");
    assert!((g.pan - vec2(50.0, 0.0)).length() < 1e-3, "{g:?}");
    assert_eq!(g.at, Some(pos2(200.0, 100.0)));
    // One finger left: still the gesture, without movement.
    let one = frame(&ctx, 0.4, vec![touch(2, TouchPhase::End, pos2(300.0, 100.0)), touch(1, TouchPhase::Move, pos2(90.0, 100.0))]);
    assert!(one.touch && !one.zooms() && !one.pans());
    let release = frame(&ctx, 0.5, vec![touch(1, TouchPhase::End, pos2(90.0, 100.0))]);
    assert!(release.touch);
    assert!(!frame(&ctx, 0.6, Vec::new()).touch);
}

/// Fingers 1 … `n` come down along y = 100 (100, 200, …), then the last one moves 100
/// to the right: the fingers spread and their middle moves. Gives the gesture then.
fn spread(s: Settings, n: u64) -> Gesture {
    let ctx = Context::default();
    set(&ctx, s);
    frame(&ctx, 0.0, vec![Event::PointerMoved(pos2(100.0, 100.0))]);
    frame(&ctx, 0.1, vec![Event::PointerMoved(pos2(100.0, 100.0)), touch(1, TouchPhase::Start, pos2(100.0, 100.0))]);
    for k in 2..=n {
        frame(&ctx, 0.1 * k as f64, vec![touch(k, TouchPhase::Start, pos2(100.0 * k as f32, 100.0))]);
    }
    frame(&ctx, 0.1 * (n + 1) as f64, vec![touch(n, TouchPhase::Move, pos2(100.0 * n as f32 + 100.0, 100.0))])
}

#[test]
fn two_fingers_only_zoom_or_only_pan_when_chosen() {
    for f in Fingers::ALL {
        let g = spread(Settings { two_fingers: f, ..Settings::default() }, 2);
        assert!(g.touch);
        assert_eq!((g.zooms(), g.pans()), (f.zooms(), f.pans()), "{f:?} {g:?}");
    }
}

#[test]
fn three_fingers_do_what_is_chosen_for_them() {
    // Like two fingers unless chosen.
    let g = spread(Settings { two_fingers: Fingers::Zoom, ..Settings::default() }, 3);
    assert!(g.zooms() && !g.pans(), "{g:?}");
    for f in [Fingers::Pan, Fingers::Zoom] {
        let s = Settings { three_fingers: Some(f), ..Settings::default() };
        let g = spread(s, 3);
        assert_eq!((g.zooms(), g.pans()), (f.zooms(), f.pans()), "{f:?} {g:?}");
        // Two fingers keep their own choice.
        let two = spread(s, 2);
        assert!(two.zooms() && two.pans(), "{two:?}");
    }
    assert_eq!(Settings { three_fingers: Some(Fingers::Pan), ..Settings::default() }.fingers(4), Fingers::Pan);
}

#[test]
fn trackpad_scrolling_zooms_when_two_fingers_only_zoom() {
    let fingers = scroll(MouseWheelUnit::Point, vec2(3.0, -5.0), MouseWheelSource::Trackpad, Modifiers::NONE);
    let s = Settings { two_fingers: Fingers::Zoom, ..setting(Scroll::Pan, false) };
    assert_eq!(wheel(std::slice::from_ref(&fingers), s), (-5.0, vec2(3.0, 0.0)));
    // A wheel still follows the scrolling setting.
    let wheel_ev = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::NONE);
    assert_eq!(wheel(&[wheel_ev], s), (0.0, vec2(0.0, 40.0)));
}

#[test]
fn the_wheel_moves_only_the_view_under_the_pointer() {
    let ctx = Context::default();
    set(&ctx, setting(Scroll::Pan, false));
    assert_eq!(settings(&ctx), setting(Scroll::Pan, false));
    let at = pos2(400.0, 300.0);
    frame(&ctx, 0.0, vec![Event::PointerMoved(at)]);
    let g = frame(&ctx, 0.1, vec![Event::PointerMoved(at), scroll(MouseWheelUnit::Point, vec2(0.0, 7.0), MouseWheelSource::Unknown, Modifiers::NONE)]);
    assert_eq!(g, Gesture { zoom: 1.0, at: Some(at), pan: vec2(0.0, 7.0), touch: false });
    frame(&ctx, 0.2, vec![Event::PointerGone]);
    let away = frame(&ctx, 0.3, vec![scroll(MouseWheelUnit::Point, vec2(0.0, 7.0), MouseWheelSource::Unknown, Modifiers::NONE)]);
    assert_eq!(away, Gesture::NONE);
}
