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
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectionHistory {
    pub(crate) current: usize, // index into `self.stack`
    pub(crate) stack: Vec<MultiSelection>,
}

impl SelectionHistory {
    pub(crate) fn on_frame_start(&mut self, log_db: &LogDb, blueprint: &Blueprint) {
        crate::profile_function!();

        // Remove all invalid elements from each multiselection.
        for stack_element in &mut self.stack {
            if stack_element
                .selected()
                .iter()
                .any(|s| !s.is_valid(log_db, blueprint))
            {
                *stack_element = MultiSelection::new(
                    stack_element
                        .selected()
                        .iter()
                        .filter(|s| s.is_valid(log_db, blueprint))
                        .cloned(),
                );
            }
        }

        // .. and then remove all empty elements!
        self.stack.retain(|stack_element| !stack_element.is_empty());
    }

    pub fn current(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current)
            .cloned()
            .map(|s| (self.current, s).into())
    }

    pub fn previous(&self) -> Option<HistoricalSelection> {
        (self.current > 0).then(|| (self.current - 1, self.stack[self.current - 1].clone()).into())
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
        self.current = self.stack.len() - 1;

        // Keep size under a certain maximum.
        if self.stack.len() > MAX_SELECTION_HISTORY_LENGTH {
            self.stack
                .drain((self.stack.len() - MAX_SELECTION_HISTORY_LENGTH)..self.stack.len());
        }
    }
}
