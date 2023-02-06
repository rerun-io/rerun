use re_data_store::LogDb;

use crate::misc::ItemCollection;

use super::Blueprint;

/// A `Selection` and its index into the historical stack.
#[derive(Debug, Clone)]
pub struct HistoricalSelection {
    pub index: usize,
    pub selection: ItemCollection,
}

impl From<(usize, ItemCollection)> for HistoricalSelection {
    fn from((index, selection): (usize, ItemCollection)) -> Self {
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
    pub(crate) stack: Vec<ItemCollection>,
}

impl SelectionHistory {
    pub(crate) fn on_frame_start(&mut self, log_db: &LogDb, blueprint: &Blueprint) {
        crate::profile_function!();

        let mut i = 0;
        self.stack.retain_mut(|selection| {
            selection.purge_invalid(log_db, blueprint);
            let retain = !selection.is_empty();
            if !retain && i <= self.current {
                self.current = self.current.saturating_sub(1);
            }
            i += 1;
            retain
        });

        // In case `self.current` was bad going in to this function:
        self.current = self.current.min(self.stack.len().saturating_sub(1));
    }

    pub fn current(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current)
            .cloned()
            .map(|s| (self.current, s).into())
    }

    pub fn previous(&self) -> Option<HistoricalSelection> {
        let prev_index = self.current.checked_sub(1)?;
        let prev = self.stack.get(prev_index)?;
        Some((prev_index, prev.clone()).into())
    }

    pub fn next(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current + 1)
            .map(|sel| (self.current + 1, sel.clone()).into())
    }

    pub fn update_selection(&mut self, item_collection: &ItemCollection) {
        // Selecting nothing is irrelevant from a history standpoint.
        if item_collection.is_empty() {
            return;
        }

        // Do not grow the history if the thing being selected is equal to the value that the
        // current history cursor points to.
        if self.current().as_ref().map(|c| &c.selection) == Some(item_collection) {
            return;
        }

        // Make sure to clear the entire redo history past this point: we are engaging in a
        // diverging timeline!
        self.stack.truncate(self.current + 1);

        self.stack.push(item_collection.clone());

        // Keep size under a certain maximum.
        if self.stack.len() > MAX_SELECTION_HISTORY_LENGTH {
            self.stack
                .drain((self.stack.len() - MAX_SELECTION_HISTORY_LENGTH)..self.stack.len());
        }

        // Update current index last so it points to something valid!
        self.current = self.stack.len() - 1;
    }
}
