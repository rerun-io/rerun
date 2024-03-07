use re_viewer_context::ViewerContext;

use re_space_view::SpaceViewBlueprint;

/// List out all space views we generate by default for the available data.
///
/// TODO(andreas): This is transitional. We want to pass on the space view spawn heuristics
/// directly and make more high level decisions with it.
pub fn default_created_space_views(ctx: &ViewerContext<'_>) -> Vec<SpaceViewBlueprint> {
    re_tracing::profile_function!();

    ctx.space_view_class_registry
        .iter_registry()
        .flat_map(|entry| {
            let class_id = entry.class.identifier();
            let spawn_heuristics = entry.class.spawn_heuristics(ctx);
            spawn_heuristics
                .recommended_space_views
                .into_iter()
                .map(move |recommendation| {
                    SpaceViewBlueprint::new(
                        class_id,
                        &recommendation.root,
                        recommendation.query_filter,
                    )
                })
        })
        .collect()
}
