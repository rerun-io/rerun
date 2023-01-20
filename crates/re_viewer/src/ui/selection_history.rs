use re_data_store::LogDb;

use crate::misc::MultiSelection;

use super::Blueprint;

/// A `Selection` and its index into the historical stack.
#[derive(Debug, Clone)]
pub struct HistoricalSelection {
    pub index: usize,
    pub selection: MultiSelection,
}

impl From<(usize, MultiSelection)> for HistoricalSelection {
    fn from((index, selection): (usize, MultiSelection)) -> Self {
        Self { index, selection }
    }
}

const MAX_SELECTION_HISTORY_LENGTH: usize = 100;

// ---

/// A stack of `Selection`s, used to implement "undo/redo"-like semantics for selections.
#[derive(Clone, Default, Debug)]
pub struct SelectionHistory {
    /// Index into [`Self::stack`].
    pub(crate) current: usize,

    /// Oldest first.
    pub(crate) stack: Vec<MultiSelection>,
}

impl SelectionHistory {
    pub(crate) fn on_frame_start(&mut self, log_db: &LogDb, blueprint: &Blueprint) {
        crate::profile_function!();

        self.stack.retain_mut(|selection| {
            selection.purge_invalid(log_db, blueprint);
            !selection.is_empty()
        });

        self.current = self.current.min(self.stack.len().saturating_sub(1));
    }

    pub fn current(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current)
            .cloned()
            .map(|s| (self.current, s).into())
    }

    pub fn previous(&self) -> Option<HistoricalSelection> {
        (0 < self.current && self.current < self.stack.len())
            .then(|| (self.current - 1, self.stack[self.current - 1].clone()).into())
    }

    pub fn next(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current + 1)
            .map(|sel| (self.current + 1, sel.clone()).into())
    }

    pub fn update_selection(&mut self, selection: &MultiSelection) {
        // Selecting nothing is irrelevant from a history standpoint.
        if selection.is_empty() {
            return;
        }

        // Do not grow the history if the thing being selected is equal to the value that the
        // current history cursor points to.
        if self.current().as_ref().map(|c| &c.selection) == Some(selection) {
            return;
        }

        // Make sure to clear the entire redo history past this point: we are engaging in a
        // diverging timeline!
        self.stack.truncate(self.current + 1);

        self.stack.push(selection.clone());

        // Keep size under a certain maximum.
        if self.stack.len() > MAX_SELECTION_HISTORY_LENGTH {
            self.stack
                .drain((self.stack.len() - MAX_SELECTION_HISTORY_LENGTH)..self.stack.len());
        }

        // Update current index last so it points to something valid!
        self.current = self.stack.len() - 1;
    }
}
