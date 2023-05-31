use crate::{ArchetypeDefinition, SceneElement, SceneQuery, SpaceViewState, ViewerContext};

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
    );

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
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
    ) {
        if let Some(state) = space_view_state.as_any().downcast_ref() {
            self.populate(ctx, query, state);
        } else {
            re_log::error_once!("Incorrect type of space view state.");
        }
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}
