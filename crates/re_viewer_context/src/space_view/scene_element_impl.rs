use crate::{
    ArchetypeDefinition, SceneElement, SceneQuery, SpaceViewHighlights, SpaceViewState,
    ViewerContext,
};

use super::scene::SceneContextCollection;

/// Element of a scene derived from a single archetype query.
pub trait SceneElementImpl {
    type State: SpaceViewState + Default + 'static;

    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &Self::State,
        contexts: &SceneContextCollection,
        highlights: &SpaceViewHighlights,
    );
}

impl<T: SceneElementImpl + 'static> SceneElement for T {
    fn archetype(&self) -> ArchetypeDefinition {
        self.archetype()
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &crate::SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        contexts: &SceneContextCollection,
        highlights: &SpaceViewHighlights,
    ) {
        if let Some(state) = space_view_state.as_any().downcast_ref() {
            self.populate(ctx, query, state, contexts, highlights);
        } else {
            re_log::error_once!("Incorrect type of space view state.");
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
