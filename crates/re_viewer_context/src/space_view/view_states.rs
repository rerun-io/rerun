//! Storage for the state of each `SpaceView`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use re_log_types::external::re_types_core::SpaceViewClassIdentifier;

use crate::{SpaceViewClassRegistry, SpaceViewId, SpaceViewState};

// State for each `SpaceView` including both the auto properties and
// the internal state of the space view itself.
// TODO: simplify further
pub struct PerViewState {
    pub view_state: Box<dyn SpaceViewState>,
}

// ----------------------------------------------------------------------------
/// State for the `SpaceView`s that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<SpaceViewId, PerViewState>,
}

impl ViewStates {
    pub fn get(&self, space_view_id: SpaceViewId) -> Option<&PerViewState> {
        self.states.get(&space_view_id)
    }

    pub fn get_mut(
        &mut self,
        view_class_registry: &SpaceViewClassRegistry,
        view_id: SpaceViewId,
        view_class: SpaceViewClassIdentifier,
    ) -> &mut PerViewState {
        self.states.entry(view_id).or_insert_with(|| PerViewState {
            view_state: view_class_registry
                .get_class_or_log_error(view_class)
                .new_state(),
        })
    }
}
