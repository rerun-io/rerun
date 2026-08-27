use re_viewer_context::ItemCollection;

const MAX_SELECTION_HISTORY_LENGTH: usize = 100;

/// A stack of [`ItemCollection`]s used to implement back/forward navigation for selections.
#[derive(Clone, Default, Debug)]
pub struct SelectionHistory {
    /// Index of the current position in [`Self::stack`].
    pub current: usize,

    /// The history stack, oldest entry first.
    pub stack: Vec<ItemCollection>,
}

impl SelectionHistory {
    pub fn current(&self) -> Option<&ItemCollection> {
        self.stack.get(self.current)
    }

    pub fn previous(&self) -> Option<(usize, &ItemCollection)> {
        let prev = self.current.checked_sub(1)?;
        Some((prev, self.stack.get(prev)?))
    }

    pub fn next(&self) -> Option<(usize, &ItemCollection)> {
        let next = self.current + 1;
        Some((next, self.stack.get(next)?))
    }

    /// Navigate to the previous entry and return it, or `None` if already at the oldest.
    #[must_use]
    pub fn select_previous(&mut self) -> Option<ItemCollection> {
        let (prev_index, _) = self.previous()?;
        self.current = prev_index;
        self.current().cloned()
    }

    /// Navigate to the next entry and return it, or `None` if already at the most recent.
    #[must_use]
    pub fn select_next(&mut self) -> Option<ItemCollection> {
        let (next_index, _) = self.next()?;
        self.current = next_index;
        self.current().cloned()
    }

    /// Record a new selection, clearing any forward ("redo") history beyond the current position.
    ///
    /// No-ops if the new selection is identical to the current one.
    pub fn update_selection(&mut self, selection: &ItemCollection) {
        if selection.is_empty() {
            return;
        }

        if self.current().map(|c| c == selection).unwrap_or(false) {
            return;
        }

        // Clear forward history — we're diverging from it.
        self.stack.truncate(self.current + 1);

        self.stack.push(selection.clone());

        // Keep total length bounded.
        if self.stack.len() > MAX_SELECTION_HISTORY_LENGTH {
            let excess = self.stack.len() - MAX_SELECTION_HISTORY_LENGTH;
            self.stack.drain(0..excess);
        }

        self.current = self.stack.len() - 1;
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::ApplicationId;
    use re_viewer_context::Item;

    use super::*;

    fn selection(name: &str) -> ItemCollection {
        ItemCollection::from(Item::AppId(ApplicationId::new_or_unknown(name)))
    }

    #[test]
    fn record_then_navigate_back_and_forward() {
        let mut history = SelectionHistory::default();
        history.update_selection(&selection("a"));
        history.update_selection(&selection("b"));
        history.update_selection(&selection("c"));

        assert_eq!(history.current(), Some(&selection("c")));
        assert_eq!(history.select_previous(), Some(selection("b")));
        assert_eq!(history.select_previous(), Some(selection("a")));
        assert_eq!(history.select_previous(), None); // already at the oldest
        assert_eq!(history.select_next(), Some(selection("b")));
        assert_eq!(history.select_next(), Some(selection("c")));
        assert_eq!(history.select_next(), None); // already at the most recent
    }

    #[test]
    fn identical_and_empty_selections_are_not_recorded() {
        let mut history = SelectionHistory::default();
        history.update_selection(&selection("a"));
        history.update_selection(&selection("a"));
        history.update_selection(&ItemCollection::default());
        assert_eq!(history.stack.len(), 1);
    }

    #[test]
    fn diverging_clears_forward_history() {
        let mut history = SelectionHistory::default();
        history.update_selection(&selection("a"));
        history.update_selection(&selection("b"));
        history.update_selection(&selection("c"));
        history.select_previous().unwrap(); // back to "b"

        history.update_selection(&selection("d")); // diverge: "c" is dropped

        assert_eq!(history.current(), Some(&selection("d")));
        assert_eq!(history.stack.len(), 3); // a, b, d
        assert_eq!(history.next(), None);
    }

    #[test]
    fn length_is_bounded() {
        let mut history = SelectionHistory::default();
        for i in 0..(2 * MAX_SELECTION_HISTORY_LENGTH) {
            history.update_selection(&selection(&i.to_string()));
        }
        assert_eq!(history.stack.len(), MAX_SELECTION_HISTORY_LENGTH);
        assert_eq!(history.current, MAX_SELECTION_HISTORY_LENGTH - 1);

        // The oldest entries are the ones dropped.
        assert_eq!(
            history.stack.first(),
            Some(&selection(&MAX_SELECTION_HISTORY_LENGTH.to_string()))
        );
    }
}
