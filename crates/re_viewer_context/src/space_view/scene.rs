use crate::{
    SceneContext, ScenePartCollection, SceneQuery, SpaceViewHighlights, SpaceViewState,
    ViewerContext,
};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
pub struct Scene {
    pub context: Box<dyn SceneContext>,
    pub parts: Box<dyn ScenePartCollection>,
    pub highlights: SpaceViewHighlights, // TODO(wumpf): Consider making this a scene context - problem: populate can't create it.
}

impl Scene {
    /// Populates the scene for a given query.
    pub fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_function!();

        self.highlights = highlights;

        // TODO(andreas): Both loops are great candidates for parallelization.
        for context in self.context.vec_mut() {
            // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
            context.populate(ctx, query, space_view_state);
        }
        self.parts
            .vec_mut()
            .into_iter()
            .flat_map(|element| {
                // TODO(andreas): Ideally, we'd pass in the result for an archetype query here.
                element.populate(
                    ctx,
                    query,
                    space_view_state,
                    self.context.as_ref(),
                    &self.highlights,
                )
            })
            .collect()
    }
}
