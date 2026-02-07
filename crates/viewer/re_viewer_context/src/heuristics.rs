use re_log_types::EntityPath;

use crate::{
    IdentifiedViewSystem, RecommendedView, ViewSpawnHeuristics, ViewerContext, VisualizerSystem,
};

/// Spawns a view for each single entity which is visualizable & indicator-matching for a given visualizer.
///
/// This is used as utility by *some* view types that want
/// to spawn a view for every single entity that is visualizable with a given visualizer.
pub fn suggest_view_for_each_entity<TVisualizer>(
    ctx: &ViewerContext<'_>,
    include_entity: &dyn Fn(&EntityPath) -> bool,
) -> ViewSpawnHeuristics
where
    TVisualizer: VisualizerSystem + IdentifiedViewSystem + Default,
{
    re_tracing::profile_function!();

    let Some(indicator_matching_entities) = ctx
        .indicated_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return ViewSpawnHeuristics::empty();
    };
    let Some(visualizable_entities) = ctx
        .visualizable_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return ViewSpawnHeuristics::empty();
    };

    let recommended_views = indicator_matching_entities
        .iter()
        .filter(|entity| visualizable_entities.contains_key(entity))
        .filter_map(|entity| {
            if include_entity(entity) {
                Some(RecommendedView::new_single_entity(entity.clone()))
            } else {
                None
            }
        });

    ViewSpawnHeuristics::new(recommended_views)
}
