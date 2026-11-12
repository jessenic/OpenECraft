//! Navigation of the 2D and 3D views: mouse wheel, trackpad and touch screen.
//!
//! A view asks [`read`] once a frame for the zoom and pan of the wheel, a trackpad
//! (two fingers scroll, pinch zooms) and two or more fingers on a touch screen (they
//! pan and zoom together, rotation is ignored). While fingers rest on a touch screen
//! after a gesture began, [`Gesture::touch`] is set: the view then leaves out what the
//! first finger would do as a pointer (selecting, moving, orbiting).

use egui::{Context, Event, Id, InputState, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, Response, Ui, Vec2};

/// A modifier key the user can choose for a function; `Ctrl` is Cmd on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Mod {
    Shift,
    Ctrl,
    Alt,
}

impl Mod {
    pub const ALL: [Mod; 3] = [Mod::Shift, Mod::Ctrl, Mod::Alt];

    /// Key name, as in the shortcuts (not translated).
    pub fn name(self) -> &'static str {
        match self {
            Mod::Shift => "Shift",
            Mod::Ctrl => "Ctrl",
            Mod::Alt => "Alt",
        }
    }

    pub fn parse(s: &str) -> Option<Mod> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shift" => Some(Mod::Shift),
            "ctrl" | "cmd" | "command" => Some(Mod::Ctrl),
            "alt" | "option" => Some(Mod::Alt),
            _ => None,
        }
    }

    pub fn held(self, m: Modifiers) -> bool {
        match self {
            Mod::Shift => m.shift,
            Mod::Ctrl => m.ctrl || m.command,
            Mod::Alt => m.alt,
        }
    }

    /// The first modifier held (Ctrl, Shift, Alt), as recorded for a gesture.
    pub fn of(m: Modifiers) -> Option<Mod> {
        [Mod::Ctrl, Mod::Shift, Mod::Alt].into_iter().find(|k| k.held(m))
    }
}

/// What scrolling (wheel, two fingers on a trackpad) does in a view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scroll {
    /// Trackpads pan and mouse wheels zoom; unknown sources follow
    /// [`Settings::wheel_source_policy`].
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

/// How wheel events with an unknown source are interpreted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WheelSourcePolicy {
    /// Treat unknown-source wheel events as trackpad input (useful for Windows precision touchpads).
    #[default]
    AssumeWindowsTrackpad,
    /// Only treat events explicitly identified by the platform as trackpad input.
    Strict,
}

impl WheelSourcePolicy {
    pub const ALL: [WheelSourcePolicy; 2] = [
        WheelSourcePolicy::AssumeWindowsTrackpad,
        WheelSourcePolicy::Strict,
    ];

    /// Name shown in settings (English, translate with `tr`).
    pub fn title(self) -> &'static str {
        match self {
            WheelSourcePolicy::Strict => openecraft_i18n::tr_const!("Strict"),
            WheelSourcePolicy::AssumeWindowsTrackpad => {
                openecraft_i18n::tr_const!("Assume Windows trackpad")
            }
        }
    }

    fn is_trackpad(self, source: MouseWheelSource, unit: MouseWheelUnit, delta: Vec2) -> bool {
        match source {
            MouseWheelSource::Trackpad | MouseWheelSource::Momentum => true,
            MouseWheelSource::Unknown if self == WheelSourcePolicy::AssumeWindowsTrackpad => {
                match unit {
                    MouseWheelUnit::Point => true,
                    MouseWheelUnit::Line => {
                        delta.x.fract().abs() > 1e-4 || delta.y.fract().abs() > 1e-4
                    }
                    MouseWheelUnit::Page => false,
                }
            }
            MouseWheelSource::Unknown => false,
            MouseWheelSource::Wheel => false,
        }
    }
}

/// What fingers do: two on a touch screen or a trackpad, three on a touch screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Fingers {
    /// Pan and zoom in one movement.
    #[default]
    PanAndZoom,
    Zoom,
    Pan,
}

impl Fingers {
    pub const ALL: [Fingers; 3] = [Fingers::PanAndZoom, Fingers::Zoom, Fingers::Pan];

    /// Name shown in the settings (English, translate with `tr`).
    pub fn title(self) -> &'static str {
        match self {
            Fingers::PanAndZoom => openecraft_i18n::tr_const!("Pan and zoom together"),
            Fingers::Zoom => openecraft_i18n::tr_const!("Zoom only"),
            Fingers::Pan => openecraft_i18n::tr_const!("Pan only"),
        }
    }

    pub fn zooms(self) -> bool {
        self != Fingers::Pan
    }

    pub fn pans(self) -> bool {
        self != Fingers::Zoom
    }
}

/// The user's choice, kept by the application and handed to the views with [`set`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub scroll: Scroll,
    /// How an event whose source the platform does not identify is classified.
    pub wheel_source_policy: WheelSourcePolicy,
    /// Scrolling pans the other way round (the content moves against the fingers).
    pub invert: bool,
    /// Two fingers on a touch screen; on a trackpad pinching (not with [`Fingers::Pan`])
    /// and scrolling the platform names a trackpad's (zooms with [`Fingers::Zoom`]).
    pub two_fingers: Fingers,
    /// Three or more fingers on a touch screen (none: like two). Trackpads leave them
    /// to the system.
    pub three_fingers: Option<Fingers>,
    /// Held while scrolling: scrolling up and down pans sideways (none: off). Kept by
    /// the application with its other modifiers, so not stored here.
    #[serde(skip)]
    pub sideways: Option<Mod>,
    /// Held while scrolling: scrolling zooms, also where it pans (none: off).
    #[serde(skip)]
    pub zoom: Option<Mod>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scroll: Scroll::Auto,
            wheel_source_policy: WheelSourcePolicy::AssumeWindowsTrackpad,
            invert: false,
            two_fingers: Fingers::PanAndZoom,
            three_fingers: None,
            sideways: Some(Mod::Shift),
            zoom: Some(Mod::Ctrl),
        }
    }
}

impl Settings {
    /// What `n` fingers on a touch screen do.
    pub fn fingers(&self, n: usize) -> Fingers {
        match self.three_fingers {
            Some(f) if n >= 3 => f,
            _ => self.two_fingers,
        }
    }
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
            let f = s.fingers(t.num_touches);
            let zoom = if f.zooms() { t.zoom_delta } else { 1.0 };
            let pan = if f.pans() { t.translation_delta } else { Vec2::ZERO };
            return Gesture { zoom, at: Some(t.center_pos), pan, touch };
        }
        if touch || !resp.hovered() {
            return Gesture { touch, ..Gesture::NONE };
        }
        let (wheel, pan) = wheel(&i.events, s);
        Gesture { zoom: pinch(&i.events, s) * (wheel * 0.0025).exp(), at: i.pointer.hover_pos(), pan, touch: false }
    })
}

/// Zoom of the pinches on a trackpad in a frame (1: none). Ctrl + wheel, which egui
/// adds to `InputState::zoom_delta`, is read by [`wheel`] with the chosen modifier.
fn pinch(events: &[Event], s: Settings) -> f32 {
    if !s.two_fingers.zooms() {
        return 1.0;
    }
    events.iter().filter_map(|e| if let Event::Zoom(f) = e { Some(*f) } else { None }).product()
}

/// Wheel events of a frame: the movement that zooms (points up) and the pan.
fn wheel(events: &[Event], s: Settings) -> (f32, Vec2) {
    let mut zoom = 0.0;
    let mut pan = Vec2::ZERO;
    for e in events {
        let Event::MouseWheel { unit, delta, modifiers, source, .. } = e else { continue };
        let mut d = *delta
            * match unit {
                MouseWheelUnit::Point => 1.0,
                MouseWheelUnit::Line => 40.0,
                MouseWheelUnit::Page => 400.0,
            };
        // As fast as Ctrl + wheel in egui.
        if s.zoom.is_some_and(|m| m.held(*modifiers)) {
            zoom += 2.0 * (d.x + d.y);
            continue;
        }
        // The modifier turns the wheel sideways (macOS does that itself for Shift).
        let sideways = s.sideways.is_some_and(|m| m.held(*modifiers));
        if sideways && d.x == 0.0 {
            d = Vec2::new(d.y, 0.0);
        }
        let trackpad = s.wheel_source_policy.is_trackpad(*source, *unit, *delta);
        let pans = match s.scroll {
            _ if trackpad && !s.two_fingers.pans() => false,
            Scroll::Pan => true,
            Scroll::Zoom => false,
            Scroll::Auto => trackpad,
        };
        let moved = if pans || sideways {
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
