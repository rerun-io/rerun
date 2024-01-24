use re_log_types::EntityPathFilter;
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, RecommendedSpaceView, SpaceViewClass,
    SpaceViewSpawnHeuristics, ViewerContext, VisualizerSystem,
};

/// Spawns a space view for each single entity which is visualizable & indicator-matching for a given visualizer.
pub fn recommend_space_views_for_all_default_visualized_entities_for<TVisualizer>(
    ctx: &ViewerContext<'_>,
    space_view: &impl SpaceViewClass,
) -> SpaceViewSpawnHeuristics
where
    TVisualizer: VisualizerSystem + IdentifiedViewSystem + Default,
{
    re_tracing::profile_function!();

    let Some(indicator_matching_entities) = ctx
        .indicator_matching_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return Default::default();
    };
    let Some(applicable_entities) = ctx
        .applicable_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return Default::default();
    };

    let visualizer = TVisualizer::default();
    let recommended_space_views = applicable_entities
        .intersection(indicator_matching_entities)
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
        .collect();

    re_viewer_context::SpaceViewSpawnHeuristics {
        recommended_space_views,
    }
}
