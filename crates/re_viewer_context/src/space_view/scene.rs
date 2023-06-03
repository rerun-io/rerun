use crate::{
    SceneContext, ScenePartCollection, SceneQuery, SpaceViewClassImpl, SpaceViewHighlights,
    SpaceViewState, ViewerContext,
};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
pub trait Scene: std::any::Any {
    /// Populates the scene for a given query.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Implementation of [`Scene`] for a specific [`SpaceViewClassImpl`].
pub struct TypedScene<C: SpaceViewClassImpl> {
    pub context: C::SceneContext,
    pub parts: C::ScenePartCollection,
    pub highlights: SpaceViewHighlights,
}

impl<C: SpaceViewClassImpl> Default for TypedScene<C> {
    fn default() -> Self {
        Self {
            context: Default::default(),
            parts: Default::default(),
            highlights: Default::default(),
        }
    }
}

impl<C: SpaceViewClassImpl + 'static> Scene for TypedScene<C> {
    /// Populates the scene for a given query.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_function!();

        self.highlights = highlights;

        let Some(state) = space_view_state
            .as_any()
            .downcast_ref::<C::SpaceViewState>()
            else {
                re_log::error_once!("Unexpected space view state type. Expected {}",
                                    std::any::type_name::<C::SpaceViewState>());
                return Vec::new();
            };

        // TODO(andreas): Both loops are great candidates for parallelization.
        for context in self.context.vec_mut() {
            // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
            context.populate(ctx, query, state);
        }
        self.parts
            .vec_mut()
            .into_iter()
            .flat_map(|element| {
                // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
                element.populate(ctx, query, state, &self.context, &self.highlights)
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
