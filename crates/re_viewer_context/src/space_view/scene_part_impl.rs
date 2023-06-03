use crate::{
    ArchetypeDefinition, SceneContext, ScenePart, SceneQuery, SpaceViewHighlights, SpaceViewState,
    ViewerContext,
};

/// Implementation utility for [`crate::ScenePart`]
pub trait ScenePartImpl {
    type SpaceViewState: SpaceViewState + Default + 'static;
    type SceneContext: SceneContext + 'static;

    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    ///
    /// TODO(andreas): don't pass in `ViewerContext` if we want to restrict the queries here.
    /// If we want to make this restriction, then the trait-contract should be that something external
    /// to the `ScenePartImpl` does the query and then passes an `ArchetypeQueryResult` into populate.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &Self::SpaceViewState,
        scene_context: &Self::SceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is a useful for retrieving a data struct that may be common for all scene elements
    /// of a particular [`crate::SpaceViewClass`].
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

impl<T: ScenePartImpl + 'static> ScenePart for T {
    #[inline]
    fn archetype(&self) -> ArchetypeDefinition {
        self.archetype()
    }

    #[inline]
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &crate::SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        scene_context: &dyn SceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        let Some(state) = space_view_state.as_any().downcast_ref() else {
            re_log::error_once!("Incorrect type of space view state.");
            return Vec::new();
        };
        let Some(context) = scene_context.as_any().downcast_ref() else {
            re_log::error_once!("Incorrect type of space view context.");
            return Vec::new();
        };
        self.populate(ctx, query, state, context, highlights)
    }

    #[inline]
    fn data(&self) -> Option<&dyn std::any::Any> {
        self.data()
    }
}
