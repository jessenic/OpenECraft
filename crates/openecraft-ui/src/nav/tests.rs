use super::*;
use egui::{pos2, vec2, Modifiers, Rect, RawInput, Sense, TouchDeviceId, TouchId, TouchPhase};

fn scroll(unit: MouseWheelUnit, delta: Vec2, source: MouseWheelSource, modifiers: Modifiers) -> Event {
    Event::MouseWheel { unit, delta, phase: TouchPhase::Move, source, modifiers }
}

fn setting(scroll: Scroll, invert: bool) -> Settings {
    Settings { scroll, invert }
}

#[test]
fn a_mouse_wheel_zooms_and_pans_sideways() {
    let up = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Wheel, Modifiers::NONE);
    assert_eq!(wheel(std::slice::from_ref(&up), setting(Scroll::Zoom, false)), (40.0, Vec2::ZERO));
    // Automatic: a wheel the platform names, or does not, zooms.
    assert_eq!(wheel(&[up], setting(Scroll::Auto, false)), (40.0, Vec2::ZERO));
    let unknown = scroll(MouseWheelUnit::Line, vec2(0.0, -2.0), MouseWheelSource::Unknown, Modifiers::NONE);
    assert_eq!(wheel(&[unknown], setting(Scroll::Auto, false)), (-80.0, Vec2::ZERO));
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
    // Chosen: every scroll pans, also a wheel or a trackpad the platform does not name.
    let unknown = scroll(MouseWheelUnit::Line, vec2(0.0, 1.0), MouseWheelSource::Unknown, Modifiers::NONE);
    assert_eq!(wheel(&[unknown], setting(Scroll::Pan, false)), (0.0, vec2(0.0, 40.0)));
    // Chosen: trackpads zoom like a wheel.
    assert_eq!(wheel(std::slice::from_ref(&fingers), setting(Scroll::Zoom, false)), (-5.0, vec2(3.0, 0.0)));
    // Inverted: the content moves against the fingers.
    assert_eq!(wheel(&[fingers], setting(Scroll::Auto, true)), (0.0, vec2(-3.0, 5.0)));
}

#[test]
fn ctrl_scrolling_is_left_to_the_zoom_of_egui() {
    for m in [Modifiers::CTRL, Modifiers::COMMAND] {
        let pinch = scroll(MouseWheelUnit::Point, vec2(0.0, 4.0), MouseWheelSource::Trackpad, m);
        assert_eq!(wheel(&[pinch], setting(Scroll::Pan, false)), (0.0, Vec2::ZERO));
    }
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
