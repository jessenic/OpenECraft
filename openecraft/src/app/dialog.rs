//! The dialogs of the editor (settings, properties, page and project properties, the
//! small dialogs of the tools) behave alike: they open centred over the editor (or
//! beside a point), keep the place and size the user gave them, and Esc closes the one
//! on top. `Dialogs` decides which one that is without drawing anything.

/// The dialogs shown and the one Esc closes.
#[derive(Default)]
pub(super) struct Dialogs {
    /// Dialogs shown in the last frame.
    open: Vec<egui::Id>,
    /// Dialogs shown in this frame.
    shown: Vec<egui::Id>,
    /// The dialog Esc closes in this frame.
    close: Option<egui::Id>,
}

/// The dialog on top: the last of `open` in the window order (back to front), or the
/// one opened last when none of them is ordered yet.
pub(super) fn topmost(order: &[egui::Id], open: &[egui::Id]) -> Option<egui::Id> {
    order.iter().rev().find(|id| open.contains(id)).or_else(|| open.last()).copied()
}

impl Dialogs {
    /// Start of a frame: `esc` pressed (not taken by anything else) closes the dialog on
    /// top of `order` (window layers, back to front). Returns whether a dialog takes it.
    pub fn begin(&mut self, esc: bool, order: &[egui::Id]) -> bool {
        self.open = std::mem::take(&mut self.shown);
        self.close = if esc { topmost(order, &self.open) } else { None };
        self.close.is_some()
    }

    /// Dialog `id` is shown in this frame; true when Esc closes it (it is gone then).
    pub fn shown(&mut self, id: egui::Id) -> bool {
        if self.close == Some(id) {
            return true;
        }
        if !self.shown.contains(&id) {
            self.shown.push(id);
        }
        false
    }

    /// Dialog `id` opens in this frame (it was not shown in the last one).
    pub fn opens(&self, id: egui::Id) -> bool {
        !self.open.contains(&id)
    }

    /// The window of the dialog `salt` titled `title`: centred over the editor the first
    /// time, then where the user left it; a dialog with `at` opens with its top left
    /// corner there every time. The bool is true when Esc closes the dialog in this frame.
    pub fn window(&mut self, ctx: &egui::Context, title: impl Into<egui::WidgetText>, salt: &str, at: Option<egui::Pos2>) -> (egui::Window<'static>, bool) {
        let id = egui::Id::unique(("dialog", salt));
        // Recover dialogs collapsed by a command in older versions: these windows
        // have no collapse button with which to reopen their contents.
        if let Some(mut state) = egui::collapsing_header::CollapsingState::load(ctx, id.with("collapsing")) {
            state.set_open(true);
            state.store(ctx);
        }
        let opens = self.opens(id);
        let esc = self.shown(id);
        let w = egui::Window::new(title).id(id).collapsible(false).constrain(true);
        let w = match at {
            Some(at) if opens => w.pivot(egui::Align2::LEFT_TOP).current_pos(at),
            Some(at) => w.pivot(egui::Align2::LEFT_TOP).default_pos(at),
            None => w.pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()),
        };
        (w, esc)
    }
}

#[cfg(test)]
mod tests;
