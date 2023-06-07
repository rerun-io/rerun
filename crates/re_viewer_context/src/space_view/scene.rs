use crate::{
    SceneContext, ScenePartCollection, SceneQuery, SpaceViewClass, SpaceViewHighlights,
    SpaceViewState, ViewerContext,
};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
///
/// In practice, the only thing implementing [`Scene`] is [`TypedScene`] which in turn is defined by
/// by a concrete [`SpaceViewClassImpl`].
pub trait Scene {
    /// Populates the scene for a given query.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    );

    /// Converts itself to a mutable reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Implementation of [`Scene`] for a specific [`SpaceViewClassImpl`].
pub struct TypedScene<C: SpaceViewClass> {
    pub context: C::Context,
    pub parts: C::SceneParts,
    pub highlights: SpaceViewHighlights,

    /// All draw data gathered during the last call to [`Self::populate`].
    ///
    /// TODO(wumpf): Right now the ui methods control when and how to create [`re_renderer::ViewBuilder`]s.
    ///              In the future, we likely want to move view builder handling to `re_viewport` with
    ///              minimal configuration options exposed via [`crate::SpaceViewClass`].
    pub draw_data: Vec<re_renderer::QueueableDrawData>,
}

impl<C: SpaceViewClass> Default for TypedScene<C> {
    fn default() -> Self {
        Self {
            context: Default::default(),
            parts: Default::default(),
            highlights: Default::default(),
            draw_data: Default::default(),
        }
    }
}

impl<C: SpaceViewClass + 'static> Scene for TypedScene<C> {
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    ) {
        re_tracing::profile_function!();

        self.highlights = highlights;

        let Some(state) = space_view_state
            .as_any()
            .downcast_ref::<C::State>()
            else {
                re_log::error_once!("Unexpected space view state type. Expected {}",
                                    std::any::type_name::<C::State>());
                return;
            };

        // TODO(andreas): Both loops are great candidates for parallelization.
        for context in self.context.vec_mut() {
            // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
            context.populate(ctx, query, state);
        }
        self.draw_data = self
            .parts
            .vec_mut()
            .into_iter()
            .flat_map(|element| {
                // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
                element.populate(ctx, query, state, &self.context, &self.highlights)
            })
            .collect();
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
