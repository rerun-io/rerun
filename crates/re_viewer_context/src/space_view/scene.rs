use crate::{
    ArchetypeDefinition, SceneContext, ScenePartCollection, SceneQuery, SpaceViewHighlights,
    SpaceViewState, ViewerContext,
};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
pub struct Scene {
    pub context: Box<dyn SceneContext>,
    pub elements: ScenePartCollection,
    pub highlights: SpaceViewHighlights, // TODO(wumpf): Consider making this a scene context - problem: populate can't create it.
}

impl Scene {
    /// List of all archetypes this scene queries for its parts.
    pub fn part_archetypes(&self) -> Vec<ArchetypeDefinition> {
        self.elements.iter().map(|e| e.archetype()).collect()
    }

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
            // TODO(andreas): Restrict the query with the archetype somehow, ideally making it trivial to do the correct thing.
            context.populate(ctx, query, space_view_state);
        }
        self.elements
            .iter_mut()
            .flat_map(|element| {
                // TODO(andreas): Restrict the query with the archetype somehow, ideally making it trivial to do the correct thing.
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
