//! Storage for the state of each `SpaceView`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;
use once_cell::sync::Lazy;

use re_entity_db::EntityPropertyMap;
use re_log_types::external::re_types_core::SpaceViewClassIdentifier;

use crate::{SpaceViewClassRegistry, SpaceViewId, SpaceViewState};

// State for each `SpaceView` including both the auto properties and
// the internal state of the space view itself.
pub struct PerViewState {
    pub auto_properties: EntityPropertyMap,
    pub view_state: Box<dyn SpaceViewState>,
}

// ----------------------------------------------------------------------------
/// State for the [`SpaceViews`] that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<SpaceViewId, PerViewState>,
}

static DEFAULT_PROPS: Lazy<EntityPropertyMap> = Lazy::<EntityPropertyMap>::new(Default::default);

impl ViewStates {
    pub fn view_state_mut(
        &mut self,
        space_view_class_registry: &SpaceViewClassRegistry,
        space_view_id: SpaceViewId,
        space_view_class: &SpaceViewClassIdentifier,
    ) -> &mut PerViewState {
        self.states
            .entry(space_view_id)
            .or_insert_with(|| PerViewState {
                auto_properties: Default::default(),
                view_state: space_view_class_registry
                    .get_class_or_log_error(space_view_class)
                    .new_state(),
            })
    }

    pub fn legacy_auto_properties(&self, space_view_id: SpaceViewId) -> &EntityPropertyMap {
        self.states
            .get(&space_view_id)
            .map_or(&DEFAULT_PROPS, |state| &state.auto_properties)
    }
}
