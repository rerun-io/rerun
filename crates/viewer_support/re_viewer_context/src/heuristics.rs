use crate::{
    IdentifiedViewSystem, RecommendedView, ViewSpawnContext, ViewSpawnHeuristics,
    ViewSystemIdentifier, ViewerContext, VisualizerSystem,
};

/// Recommends one root-subtree view if any visualizer has an eligible indicated entity.
///
/// Automatic spawning excludes property entities; explicit creation includes them.
pub fn suggest_root_view_if_indicated_for(
    ctx: &ViewerContext<'_>,
    spawn_context: ViewSpawnContext,
    visualizer_ids: impl IntoIterator<Item = ViewSystemIdentifier>,
) -> ViewSpawnHeuristics {
    re_tracing::profile_function!();

    if visualizer_ids.into_iter().any(|id| {
        ctx.indicated_entities_per_visualizer
            .get(&id)
            .is_some_and(|entities| {
                entities.iter().any(|entity| {
                    spawn_context == ViewSpawnContext::Explicit || !entity.is_property()
                })
            })
    }) {
        ViewSpawnHeuristics::root()
    } else {
        ViewSpawnHeuristics::empty()
    }
}

/// Spawns a view for each single entity which is visualizable & indicator-matching for a given visualizer.
///
/// This is used as utility by *some* view types that want
/// to spawn a view for every single entity that is visualizable with a given visualizer.
pub fn suggest_view_for_each_entity<TVisualizer>(
    ctx: &ViewerContext<'_>,
    spawn_context: ViewSpawnContext,
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
        .filter(|entity| spawn_context == ViewSpawnContext::Explicit || !entity.is_property())
        .map(|entity| RecommendedView::new_single_entity(entity.clone()));

    ViewSpawnHeuristics::new(recommended_views)
}
