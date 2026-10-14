//! Undo/redo over task-list snapshots.
//!
//! Changes are committed once the user stops editing (no focused text field,
//! no mouse button held), so typing a title is a single undo step.

use crate::model::{Task, TaskId};

const LIMIT: usize = 100;

#[derive(Clone, PartialEq)]
pub struct Snapshot {
    pub tasks: Vec<Task>,
    pub next_id: TaskId,
}

pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    committed: Snapshot,
}

impl History {
    pub fn new(initial: Snapshot) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), committed: initial }
    }

    /// Call every frame with the current state.
    pub fn observe(&mut self, tasks: &[Task], next_id: TaskId, editing: bool) {
        if editing || (self.committed.tasks.as_slice() == tasks && self.committed.next_id == next_id) {
            return;
        }
        self.commit(Snapshot { tasks: tasks.to_vec(), next_id });
    }

    fn commit(&mut self, current: Snapshot) {
        if current == self.committed {
            return;
        }
        let prev = std::mem::replace(&mut self.committed, current);
        self.undo.push(prev);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Returns the state to restore.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        self.commit(current);
        let prev = self.undo.pop()?;
        let cur = std::mem::replace(&mut self.committed, prev.clone());
        self.redo.push(cur);
        Some(prev)
    }

    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        self.commit(current);
        let next = self.redo.pop()?;
        let cur = std::mem::replace(&mut self.committed, next.clone());
        self.undo.push(cur);
        Some(next)
    }

    /// After restoring, record what was actually applied as the baseline.
    pub fn set_committed(&mut self, s: Snapshot) {
        self.committed = s;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(titles: &[&str]) -> Snapshot {
        Snapshot {
            tasks: titles
                .iter()
                .enumerate()
                .map(|(i, t)| Task { id: i as u64 + 1, title: t.to_string(), ..Default::default() })
                .collect(),
            next_id: titles.len() as u64 + 1,
        }
    }

    #[test]
    fn undo_redo_cycle() {
        let mut h = History::new(snap(&[]));
        let a = snap(&["a"]);
        h.observe(&a.tasks, a.next_id, false);
        let ab = snap(&["a", "b"]);
        h.observe(&ab.tasks, ab.next_id, false);

        assert!(h.undo(ab.clone()).unwrap() == a);
        assert!(h.undo(a.clone()).unwrap() == snap(&[]));
        assert!(h.undo(snap(&[])).is_none());
        assert!(h.redo(snap(&[])).unwrap() == a);
        assert!(h.redo(a.clone()).unwrap() == ab);
    }

    #[test]
    fn edits_while_typing_are_one_step() {
        let mut h = History::new(snap(&["x"]));
        for t in ["xy", "xyz"] {
            let s = snap(&[t]);
            h.observe(&s.tasks, s.next_id, true);
        }
        let done = snap(&["xyz"]);
        h.observe(&done.tasks, done.next_id, false);
        assert!(h.undo(done).unwrap() == snap(&["x"]));
    }

    #[test]
    fn new_change_clears_redo() {
        let mut h = History::new(snap(&[]));
        let a = snap(&["a"]);
        h.observe(&a.tasks, a.next_id, false);
        h.undo(a).unwrap();
        let c = snap(&["c"]);
        h.observe(&c.tasks, c.next_id, false);
        assert!(!h.can_redo());
    }
}
