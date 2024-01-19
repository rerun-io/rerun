use itertools::Itertools;

use re_entity_db::EntityPath;
use re_log_types::EntityPathFilter;
use re_space_view::{DataQuery as _, DataQueryBlueprint};
use re_viewer_context::{DataQueryResult, ViewerContext};

use crate::{
    determine_visualizable_entities, space_info::SpaceInfoCollection,
    space_view::SpaceViewBlueprint,
};

fn candidate_space_view_paths<'a>(
    ctx: &ViewerContext<'a>,
    spaces_info: &'a SpaceInfoCollection,
) -> impl Iterator<Item = &'a EntityPath> {
    // Everything with a SpaceInfo is a candidate (that is root + whenever there is a transform),
    // as well as all direct descendants of the root.
    let root_children = &ctx.entity_db.tree().children;
    spaces_info
        .iter()
        .map(|info| &info.path)
        .chain(root_children.values().map(|sub_tree| &sub_tree.path))
        .unique()
}

/// List out all space views we allow the user to create.
pub fn all_possible_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<(SpaceViewBlueprint, DataQueryResult)> {
    re_tracing::profile_function!();

    // For each candidate, create space views for all possible classes.
    candidate_space_view_paths(ctx, spaces_info)
        .flat_map(|candidate_space_path| {
            ctx.space_view_class_registry
                .iter_registry()
                .filter_map(|entry| {
                    // We only want to run the query if there's at least one applicable entity under `candidate_space_path`.
                    if !entry.visualizer_system_ids.iter().any(|visualizer| {
                        let Some(entities) = ctx.applicable_entities_per_visualizer.get(visualizer)
                        else {
                            return false;
                        };
                        entities
                            .iter()
                            .any(|entity| entity.starts_with(candidate_space_path))
                    }) {
                        return None;
                    }

                    let class_identifier = entry.class.identifier();

                    let mut entity_path_filter = EntityPathFilter::default();
                    entity_path_filter.add_subtree(candidate_space_path.clone());

                    let candidate_query =
                        DataQueryBlueprint::new(class_identifier, entity_path_filter);

                    let visualizable_entities = determine_visualizable_entities(
                        ctx.applicable_entities_per_visualizer,
                        ctx.entity_db,
                        &ctx.space_view_class_registry
                            .new_visualizer_collection(class_identifier),
                        entry.class.as_ref(),
                        candidate_space_path,
                    );

                    let results = candidate_query.execute_query(
                        ctx.store_context,
                        &visualizable_entities,
                        ctx.indicator_matching_entities_per_visualizer,
                    );

                    if !results.is_empty() {
                        Some((
                            SpaceViewBlueprint::new(
                                entry.class.identifier(),
                                candidate_space_path,
                                candidate_query,
                            ),
                            results,
                        ))
                    } else {
                        None
                    }
                })
                .collect_vec()
        })
        .collect_vec()
}

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
                        DataQueryBlueprint::new(class_id, recommendation.query_filter),
                    )
                })
        })
        .collect()
}
