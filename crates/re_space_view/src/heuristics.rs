use re_log_types::EntityPathFilter;
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, RecommendedSpaceView, SpaceViewClass,
    SpaceViewSpawnHeuristics, ViewerContext, VisualizerSystem,
};

/// Spawns a space view each only containing a single entity for each entity that is visualizable to a given visualizer
/// in the context of a space view with that entity as a root.
pub fn recommend_space_view_for_each_visualizable_entity<TVisualizer>(
    ctx: &ViewerContext<'_>,
    space_view: &impl SpaceViewClass,
) -> SpaceViewSpawnHeuristics
where
    TVisualizer: VisualizerSystem + IdentifiedViewSystem + Default,
{
    re_tracing::profile_function!();

    let visualizer = TVisualizer::default();

    let recommended_space_views = if let Some(applicable_entities) = ctx
        .applicable_entities_per_visualizer
        .get(&TVisualizer::identifier())
    {
        applicable_entities
            .iter()
            .filter_map(|entity| {
                let context = space_view.visualizable_filter_context(entity, ctx.entity_db);
                if visualizer
                    .filter_visualizable_entities(
                        ApplicableEntities(std::iter::once(entity.clone()).collect()),
                        context.as_ref(),
                    )
                    .is_empty()
                {
                    None
                } else {
                    Some(RecommendedSpaceView {
                        root: entity.clone(),
                        query_filter: EntityPathFilter::single_entity_filter(entity),
                    })
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    re_viewer_context::SpaceViewSpawnHeuristics {
        recommended_space_views,
    }
}
