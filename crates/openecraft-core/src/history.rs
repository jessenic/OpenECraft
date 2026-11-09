//! Undo / redo by snapshot comparison: every committed change of the project
//! (from the canvas, the property panel, dialogs, ...) becomes one undo step.

use crate::model::Project;

const MAX_STEPS: usize = 200;

pub struct History {
    /// Earlier and later states, each with its id.
    undo: Vec<(Project, u64)>,
    redo: Vec<(Project, u64)>,
    committed: Project,
    /// Id of the committed state: undo and redo bring back the id of the state they
    /// restore, so a state can be recognised (e.g. the saved one).
    state: u64,
    next_state: u64,
    /// Incremented on every change; used to invalidate caches.
    pub rev: u64,
}

impl History {
    pub fn new(prj: &Project) -> History {
        History { undo: Vec::new(), redo: Vec::new(), committed: prj.clone(), state: 1, next_state: 2, rev: 1 }
    }

    /// Call once per frame. `can_commit` is false while the user is still dragging
    /// or typing, so that a whole gesture becomes one undo step.
    /// Returns whether a change was committed.
    pub fn track(&mut self, prj: &Project, can_commit: bool) -> bool {
        if !can_commit || *prj == self.committed {
            return false;
        }
        let prev = std::mem::replace(&mut self.committed, prj.clone());
        let state = self.new_state();
        self.undo.push((prev, std::mem::replace(&mut self.state, state)));
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rev += 1;
        true
    }

    fn new_state(&mut self) -> u64 {
        self.next_state += 1;
        self.next_state - 1
    }

    /// The project as last committed.
    pub fn committed(&self) -> &Project {
        &self.committed
    }

    /// Id of the committed state.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Applies `f` to the project and all undo / redo steps (changes of other users
    /// of a shared project, which undo must not take back).
    pub fn rebase(&mut self, prj: &mut Project, f: impl Fn(&Project) -> Project) {
        // Every state changes: none of them is a state seen before.
        let mut next = self.next_state;
        for (p, id) in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            *p = f(p);
            *id = next;
            next += 1;
        }
        self.next_state = next;
        self.committed = f(&self.committed);
        self.state = self.new_state();
        *prj = f(prj);
        self.rev += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Steps that can be undone (the undo list of the quick access row).
    pub fn undo_steps(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_steps(&self) -> usize {
        self.redo.len()
    }

    pub fn undo(&mut self, prj: &mut Project) {
        if let Some((prev, id)) = self.undo.pop() {
            self.redo.push((std::mem::replace(&mut self.committed, prev), std::mem::replace(&mut self.state, id)));
            *prj = self.committed.clone();
            self.rev += 1;
        }
    }

    pub fn redo(&mut self, prj: &mut Project) {
        if let Some((next, id)) = self.redo.pop() {
            self.undo.push((std::mem::replace(&mut self.committed, next), std::mem::replace(&mut self.state, id)));
            *prj = self.committed.clone();
            self.rev += 1;
        }
    }
}

#[cfg(test)]
mod tests;
