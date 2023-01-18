use crate::misc::Selection;

/// A `Selection` and its index into the historical stack.
#[derive(Debug, Clone)]
pub struct HistoricalSelection {
    pub index: usize,
    pub selection: Selection,
}

impl From<(usize, Selection)> for HistoricalSelection {
    fn from((index, selection): (usize, Selection)) -> Self {
        Self { index, selection }
    }
}

// ---

/// A stack of `Selection`s, used to implement "undo/redo"-like semantics for selections.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectionHistory {
    pub(crate) current: usize, // index into `self.stack`
    pub(crate) stack: Vec<Selection>,
}

impl SelectionHistory {
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

    pub fn update_selection(&mut self, selection: &Selection) {
        // Selecting nothing is irrelevant from a history standpoint.
        if *selection == Selection::None {
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
    }
}
