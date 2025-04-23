use re_log_types::ResolvedEntityPathFilter;
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, RecommendedView, ViewClass,
    ViewSpawnHeuristics, ViewerContext, VisualizerSystem,
};

/// Spawns a view for each single entity which is visualizable & indicator-matching for a given visualizer.
///
/// This is used as utility by *some* view types that want
/// to spawn a view for every single entity that is visualizable with a given visualizer.
pub fn suggest_view_for_each_entity<TVisualizer>(
    ctx: &ViewerContext<'_>,
    view: &dyn ViewClass,
    excluded_entities: &ResolvedEntityPathFilter,
) -> ViewSpawnHeuristics
where
    TVisualizer: VisualizerSystem + IdentifiedViewSystem + Default,
{
    re_tracing::profile_function!();

    let Some(indicator_matching_entities) = ctx
        .indicated_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return Default::default();
    };
    let Some(maybe_visualizable_entities) = ctx
        .maybe_visualizable_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return Default::default();
    };

    let visualizer = TVisualizer::default();
    let recommended_views = maybe_visualizable_entities
        .intersection(indicator_matching_entities)
        .filter_map(|entity| {
            let context = view.visualizable_filter_context(entity, ctx.recording());
            if !visualizer
                .filter_visualizable_entities(
                    MaybeVisualizableEntities(std::iter::once(entity.clone()).collect()),
                    context.as_ref(),
                )
                .is_empty()
                && !excluded_entities.matches(entity)
            {
                Some(RecommendedView::new_single_entity(entity.clone()))
            } else {
                None
            }
        });

    re_viewer_context::ViewSpawnHeuristics::new(recommended_views)
}
