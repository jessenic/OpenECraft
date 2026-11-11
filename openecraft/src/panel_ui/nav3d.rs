//! Moving the camera of the 3D view: orbit, pan, zoom at the pointer (walking on into
//! a room once close to the orbit centre), moving with the keyboard, the standard views
//! and the camera gliding to a new view.

use openecraft_core::parts::vec3::{add, length, norm, scale, sub, Orbit, V3};
use openecraft_i18n::tr_const;
use egui::{InputState, Key, Vec2};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Nearest the camera comes to its orbit centre (mm); zooming in further walks on.
pub(super) const MIN_DIST: f32 = 150.0;
pub(super) const MAX_DIST: f32 = 200_000.0;
/// Steepest view from above or below, short of straight down.
pub(super) const MAX_PITCH: f32 = 1.5;
/// Distance walked on per unit of zoom (natural logarithm of the factor) at [`MIN_DIST`].
const WALK: f32 = 2000.0;

/// Turns the camera around its orbit centre by a drag of `dl` points.
pub(super) fn orbit(cam: &mut Orbit, dl: Vec2) {
    cam.yaw -= dl.x * 0.008;
    cam.pitch = (cam.pitch + dl.y * 0.008).clamp(-MAX_PITCH, MAX_PITCH);
}

/// Moves the camera with a drag of `dl` points; what lies at the depth of the orbit
/// centre follows the pointer. `f`: focal length in points.
pub(super) fn pan(cam: &mut Orbit, dl: Vec2, f: f32) {
    let (_, right, up, _) = cam.basis();
    let s = cam.dist / f;
    cam.target = add(cam.target, add(scale(right, -dl.x * s), scale(up, dl.y * s)));
}

/// Zooms by `factor` (above 1: closer) at the point `at` points from the centre of the
/// view: what lies there at the depth of the orbit centre stays under the pointer.
/// Closer than [`MIN_DIST`] the camera walks on along the ray through the pointer.
pub(super) fn zoom_at(cam: &mut Orbit, factor: f32, at: Vec2, f: f32) {
    if !factor.is_finite() || factor <= 0.0 || factor == 1.0 {
        return;
    }
    let (eye, right, up, _) = cam.basis();
    let s = cam.dist / f;
    let under = add(cam.target, add(scale(right, at.x * s), scale(up, -at.y * s)));
    let want = cam.dist / factor;
    let dist = want.clamp(MIN_DIST, MAX_DIST);
    cam.target = add(under, scale(sub(cam.target, under), dist / cam.dist));
    cam.dist = dist;
    if want < MIN_DIST {
        cam.target = add(cam.target, scale(norm(sub(under, eye)), WALK * factor.ln()));
    }
}

/// Pans and zooms by the wheel, a trackpad or the fingers (`centre`: middle of the
/// view); gives whether the camera moved.
pub(super) fn gesture(cam: &mut Orbit, g: &openecraft_ui::nav::Gesture, centre: egui::Pos2, f: f32) -> bool {
    if g.pans() {
        pan(cam, g.pan, f);
    }
    if g.zooms() {
        zoom_at(cam, g.zoom, g.at.map_or(Vec2::ZERO, |q| q - centre), f);
    }
    g.pans() || g.zooms()
}

/// Moves the camera with its orbit centre: `ahead` level in the direction looked at,
/// `side` to the right, `rise` up (mm).
pub(super) fn walk(cam: &mut Orbit, ahead: f32, side: f32, rise: f32) {
    let (s, c) = cam.yaw.sin_cos();
    cam.target = add(cam.target, [-s * ahead + c * side, rise, -c * ahead - s * side]);
}

/// Keys held for moving: ahead, to the right, up, closer (each -1, 0 or 1) and whether
/// Shift is held. Nothing while Ctrl, Cmd or Alt is held (shortcuts).
pub(super) fn held(i: &InputState) -> ([f32; 4], bool) {
    if i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt {
        return ([0.0; 4], false);
    }
    let axis = |plus: &[Key], minus: &[Key]| {
        let on = |keys: &[Key]| keys.iter().any(|k| i.key_down(*k));
        on(plus) as i8 as f32 - on(minus) as i8 as f32
    };
    let dir = [
        axis(&[Key::W, Key::ArrowUp], &[Key::S, Key::ArrowDown]),
        axis(&[Key::D, Key::ArrowRight], &[Key::A, Key::ArrowLeft]),
        axis(&[Key::E], &[Key::Q]),
        axis(&[Key::Plus, Key::Equals], &[Key::Minus]),
    ];
    (dir, i.modifiers.shift)
}

/// Moves the camera for `dt` seconds with the keys held (see [`held`]).
pub(super) fn keys(cam: &mut Orbit, [ahead, side, rise, closer]: [f32; 4], fast: bool, dt: f32) {
    let speed = cam.dist.clamp(1000.0, 20000.0) * if fast { 2.4 } else { 0.8 } * dt;
    walk(cam, ahead * speed, side * speed, rise * speed);
    if closer != 0.0 {
        cam.dist = (cam.dist * (-closer * dt * if fast { 3.0 } else { 1.2 }).exp()).clamp(MIN_DIST, MAX_DIST);
    }
}

/// Moves `cam` a step of `dt` seconds towards `goal`; true once it is there.
pub(super) fn glide(cam: &mut Orbit, goal: &Orbit, dt: f32) -> bool {
    let t = 1.0 - (-dt * 10.0).exp();
    cam.yaw += turn(goal.yaw - cam.yaw) * t;
    cam.pitch += (goal.pitch - cam.pitch) * t;
    cam.dist *= (goal.dist / cam.dist).powf(t);
    cam.target = add(cam.target, scale(sub(goal.target, cam.target), t));
    let there = turn(goal.yaw - cam.yaw).abs() < 1e-3
        && (goal.pitch - cam.pitch).abs() < 1e-3
        && (goal.dist / cam.dist - 1.0).abs() < 1e-3
        && length(sub(goal.target, cam.target)) < goal.dist * 1e-3;
    if there {
        *cam = *goal;
    }
    there
}

/// An angle as the shortest turn, -π … π.
fn turn(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// The camera turned from where it is to look at `at`, closer to it.
pub(super) fn look_at(cam: &Orbit, at: V3) -> Orbit {
    let (eye, ..) = cam.basis();
    let d = sub(eye, at);
    let len = length(d).max(1.0);
    Orbit {
        yaw: d[0].atan2(d[2]),
        pitch: (d[1] / len).clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH),
        dist: (len * 0.6).clamp(MIN_DIST, MAX_DIST),
        target: at,
    }
}

/// Standard directions of the 3D view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Front,
    Left,
    Right,
    Rear,
    Top,
    Iso,
}

impl Side {
    pub const ALL: [Side; 6] = [Side::Front, Side::Left, Side::Right, Side::Rear, Side::Top, Side::Iso];

    /// English title, translated where shown.
    pub fn title(self) -> &'static str {
        match self {
            Side::Front => tr_const!("Front"),
            Side::Left => tr_const!("Left"),
            Side::Right => tr_const!("Right"),
            Side::Rear => tr_const!("Rear"),
            Side::Top => tr_const!("Top"),
            Side::Iso => tr_const!("Isometric"),
        }
    }

    /// Yaw and pitch of the camera (the door faces +z).
    pub fn angles(self) -> (f32, f32) {
        match self {
            Side::Front => (0.0, 0.0),
            Side::Left => (-FRAC_PI_2, 0.0),
            Side::Right => (FRAC_PI_2, 0.0),
            Side::Rear => (PI, 0.0),
            Side::Top => (0.0, MAX_PITCH),
            Side::Iso => (0.55, 0.3),
        }
    }
}

#[cfg(test)]
mod tests;
