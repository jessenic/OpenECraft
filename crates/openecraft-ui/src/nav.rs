//! Navigation of the 2D and 3D views: mouse wheel, trackpad and touch screen.
//!
//! A view asks [`read`] once a frame for the zoom and pan of the wheel, a trackpad
//! (two fingers scroll, pinch zooms) and two or more fingers on a touch screen (they
//! pan and zoom together, rotation is ignored). While fingers rest on a touch screen
//! after a gesture began, [`Gesture::touch`] is set: the view then leaves out what the
//! first finger would do as a pointer (selecting, moving, orbiting).

use egui::{Context, Event, Id, InputState, MouseWheelSource, MouseWheelUnit, Pos2, Response, Ui, Vec2};

/// What scrolling (wheel, two fingers on a trackpad) does in a view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scroll {
    /// Trackpads pan, mouse wheels zoom; where the platform does not tell them apart
    /// (everywhere but macOS) like [`Scroll::Zoom`].
    #[default]
    Auto,
    /// Scrolling up and down zooms at the pointer, sideways pans (CAD style).
    Zoom,
    /// Scrolling pans in both directions; pinching or Ctrl + scrolling zooms.
    Pan,
}

impl Scroll {
    pub const ALL: [Scroll; 3] = [Scroll::Auto, Scroll::Zoom, Scroll::Pan];

    /// Name shown in the settings (English, translate with `tr`).
    pub fn title(self) -> &'static str {
        match self {
            Scroll::Auto => openecraft_i18n::tr_const!("Automatic"),
            Scroll::Zoom => openecraft_i18n::tr_const!("Zooms (mouse wheel)"),
            Scroll::Pan => openecraft_i18n::tr_const!("Pans (trackpad)"),
        }
    }
}

/// The user's choice, kept by the application and handed to the views with [`set`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub scroll: Scroll,
    /// Scrolling pans the other way round (the content moves against the fingers).
    pub invert: bool,
}

fn settings_id() -> Id {
    Id::unique("openecraft_nav_settings")
}

/// Hands the settings to the views (every frame or when they change).
pub fn set(ctx: &Context, s: Settings) {
    ctx.data_mut(|d| d.insert_temp(settings_id(), s));
}

pub fn settings(ctx: &Context) -> Settings {
    ctx.data(|d| d.get_temp(settings_id())).unwrap_or_default()
}

/// Zoom and pan of a view in this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gesture {
    /// Zoom factor (1: none, above 1: in).
    pub zoom: f32,
    /// Where the zoom is centred: the pointer or the middle of the fingers.
    pub at: Option<Pos2>,
    /// How far the content moves, in points.
    pub pan: Vec2,
    /// Two or more fingers are, or were during this touch, on the screen.
    pub touch: bool,
}

impl Gesture {
    pub const NONE: Gesture = Gesture { zoom: 1.0, at: None, pan: Vec2::ZERO, touch: false };

    pub fn zooms(&self) -> bool {
        (self.zoom - 1.0).abs() > 1e-4
    }

    pub fn pans(&self) -> bool {
        self.pan != Vec2::ZERO
    }
}

/// The navigation of the view of `resp`: the wheel and trackpad while the pointer is
/// over it, a touch gesture that began on it.
pub fn read(ui: &Ui, resp: &Response) -> Gesture {
    let s = settings(ui.ctx());
    let touch = touching(ui.ctx());
    ui.input(|i| {
        if let Some(t) = i.multi_touch() {
            if !resp.rect.contains(t.start_pos) {
                return Gesture { touch, ..Gesture::NONE };
            }
            return Gesture { zoom: t.zoom_delta, at: Some(t.center_pos), pan: t.translation_delta, touch };
        }
        if touch || !resp.hovered() {
            return Gesture { touch, ..Gesture::NONE };
        }
        let (wheel, pan) = wheel(&i.events, s);
        Gesture { zoom: i.zoom_delta() * (wheel * 0.0025).exp(), at: i.pointer.hover_pos(), pan, touch: false }
    })
}

/// Wheel events of a frame: the movement that zooms (points up) and the pan. Ctrl /
/// Command + wheel and pinching are left to `InputState::zoom_delta`.
fn wheel(events: &[Event], s: Settings) -> (f32, Vec2) {
    let mut zoom = 0.0;
    let mut pan = Vec2::ZERO;
    for e in events {
        let Event::MouseWheel { unit, delta, modifiers, source, .. } = e else { continue };
        if modifiers.ctrl || modifiers.command {
            continue;
        }
        let mut d = *delta
            * match unit {
                MouseWheelUnit::Point => 1.0,
                MouseWheelUnit::Line => 40.0,
                MouseWheelUnit::Page => 400.0,
            };
        // Shift turns the wheel sideways (macOS does that itself).
        if modifiers.shift && d.x == 0.0 {
            d = Vec2::new(d.y, 0.0);
        }
        let pans = match s.scroll {
            Scroll::Pan => true,
            Scroll::Zoom => false,
            Scroll::Auto => matches!(source, MouseWheelSource::Trackpad | MouseWheelSource::Momentum),
        };
        let moved = if pans || modifiers.shift {
            d
        } else {
            zoom += d.y;
            Vec2::new(d.x, 0.0)
        };
        pan += if s.invert { -moved } else { moved };
    }
    (zoom, pan)
}

#[derive(Clone, Copy, Default)]
struct TouchState {
    pass: Option<u64>,
    now: bool,
    carry: bool,
}

/// A touch gesture is going on: two or more fingers since the first one came down,
/// until the frame after the last one left (that frame still has its release).
fn touching(ctx: &Context) -> bool {
    let id = Id::unique("openecraft_nav_touch");
    let pass = ctx.cumulative_pass_nr();
    let (multi, any) = ctx.input(|i: &InputState| (i.multi_touch().is_some(), i.any_touches()));
    ctx.data_mut(|d| {
        let s: &mut TouchState = d.get_temp_mut_or_default(id);
        if s.pass != Some(pass) {
            (s.now, s.carry) = sticky(s.carry, multi, any);
            s.pass = Some(pass);
        }
        s.now
    })
}

/// (gesture now, gesture in the next frame) from the one of this frame, two or more
/// fingers and any finger.
fn sticky(carry: bool, multi: bool, any: bool) -> (bool, bool) {
    let now = carry || multi;
    (now, now && any)
}

#[cfg(test)]
mod tests;
