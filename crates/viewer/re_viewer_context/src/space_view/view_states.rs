//! Storage for the state of each `SpaceView`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use crate::{SpaceViewClass, SpaceViewId, SpaceViewState};

/// State for the `SpaceView`s that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<SpaceViewId, Box<dyn SpaceViewState>>,
}

impl ViewStates {
    pub fn get(&self, space_view_id: SpaceViewId) -> Option<&dyn SpaceViewState> {
        self.states.get(&space_view_id).map(|s| s.as_ref())
    }

    pub fn get_mut_or_create(
        &mut self,
        view_id: SpaceViewId,
        view_class: &dyn SpaceViewClass,
    ) -> &mut dyn SpaceViewState {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state())
            .as_mut()
    }

    pub fn ensure_state_exists(&mut self, view_id: SpaceViewId, view_class: &dyn SpaceViewClass) {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state());
    }
}
