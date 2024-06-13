//! Storage for the state of each `SpaceView`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use re_log_types::external::re_types_core::SpaceViewClassIdentifier;

use crate::{SpaceViewClassRegistry, SpaceViewId, SpaceViewState};

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

    pub fn get_mut(
        &mut self,
        view_class_registry: &SpaceViewClassRegistry,
        view_id: SpaceViewId,
        view_class: SpaceViewClassIdentifier,
    ) -> &mut dyn SpaceViewState {
        self.states
            .entry(view_id)
            .or_insert_with(|| {
                view_class_registry
                    .get_class_or_log_error(view_class)
                    .new_state()
            })
            .as_mut()
    }
}
