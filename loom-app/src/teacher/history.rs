//! The shared undo for every button that rewrites the whole editor: VIBE,
//! GÉNÉRER, DÉVELOPPE, HUM, a lesson's « Essayer ».
//!
//! Before replacing the text, the app calls [`History::record`] with the old
//! text. [`History::undo`] brings it back and keeps the text it replaces for
//! [`History::redo`], so the user's code is never lost: whatever is in the
//! editor when undoing (hand edits included) can always come back.

/// How many steps back are kept.
pub const DEPTH: usize = 30;

/// One version of the song, and the action that replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// `VIBE`, `GÉNÉRER`, `DÉVELOPPE`, `HUM`, `leçon`…
    pub label: String,
    pub code: String,
}

/// Undo and redo stacks.
#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    /// `label` is about to replace `before` in the editor.
    pub fn record(&mut self, label: &str, before: &str) {
        self.undo.push(Snapshot { label: label.to_string(), code: before.to_string() });
        if self.undo.len() > DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The code to put back in place of `current` (which becomes redoable), with the action undone.
    pub fn undo(&mut self, current: &str) -> Option<Snapshot> {
        let back = self.undo.pop()?;
        self.redo.push(Snapshot { label: back.label.clone(), code: current.to_string() });
        Some(back)
    }

    /// The code the last undo took away, in place of `current` (which becomes undoable again).
    pub fn redo(&mut self, current: &str) -> Option<Snapshot> {
        let again = self.redo.pop()?;
        self.undo.push(Snapshot { label: again.label.clone(), code: current.to_string() });
        Some(again)
    }

    /// The action an undo would take back, e.g. `GÉNÉRER`.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    /// The action a redo would bring back.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    /// Forget everything (another song was opened).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_walk_back_and_forth() {
        let mut h = History::default();
        assert_eq!(h.undo("a"), None);
        h.record("GÉNÉRER", "mon morceau");
        h.record("DÉVELOPPE", "généré");
        assert_eq!(h.undo_label(), Some("DÉVELOPPE"));
        // the developed text, edited by hand, is kept for redo
        let back = h.undo("développé + retouche").unwrap();
        assert_eq!((back.label.as_str(), back.code.as_str()), ("DÉVELOPPE", "généré"));
        assert_eq!(h.redo_label(), Some("DÉVELOPPE"));
        let back = h.undo("généré").unwrap();
        assert_eq!(back.code, "mon morceau");
        assert_eq!(h.undo("mon morceau"), None);
        assert_eq!(h.redo("mon morceau").unwrap().code, "généré");
        assert_eq!(h.redo("généré").unwrap().code, "développé + retouche");
        assert_eq!(h.redo("x"), None);
        assert_eq!(h.undo("développé + retouche").unwrap().code, "généré");
    }

    #[test]
    fn a_new_action_forgets_redo_and_depth_is_bounded() {
        let mut h = History::default();
        h.record("VIBE", "1");
        h.undo("2");
        assert!(h.redo_label().is_some());
        h.record("HUM", "1");
        assert_eq!(h.redo_label(), None);
        for i in 0..DEPTH + 5 {
            h.record("GÉNÉRER", &i.to_string());
        }
        let mut count = 0;
        let mut current = "x".to_string();
        while let Some(s) = h.undo(&current) {
            current = s.code;
            count += 1;
        }
        assert_eq!(count, DEPTH);
        assert_eq!(current, "5", "the oldest steps are dropped first");
        h.clear();
        assert!(h.undo_label().is_none() && h.redo_label().is_none());
    }
}
