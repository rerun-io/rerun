//! Storage for the state of each `View`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use crate::{ViewClass, ViewId, ViewState};

/// State for the `View`s that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<ViewId, Box<dyn ViewState>>,
}

impl ViewStates {
    pub fn get(&self, space_view_id: ViewId) -> Option<&dyn ViewState> {
        self.states.get(&space_view_id).map(|s| s.as_ref())
    }

    pub fn get_mut_or_create(
        &mut self,
        view_id: ViewId,
        view_class: &dyn ViewClass,
    ) -> &mut dyn ViewState {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state())
            .as_mut()
    }

    pub fn ensure_state_exists(&mut self, view_id: ViewId, view_class: &dyn ViewClass) {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state());
    }
}
