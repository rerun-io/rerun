use ahash::HashMap;
use itertools::Itertools;

use rayon::spawn;
use re_data_store::{LatestAtQuery, Timeline};
use re_entity_db::EntityPath;
use re_log_types::EntityPathFilter;
use re_space_view::{DataQuery as _, DataQueryBlueprint};
use re_types::components::{DisconnectedSpace, TensorData};
use re_viewer_context::{
    AutoSpawnHeuristic, DataQueryResult, PerSystemEntities, SpaceViewClassIdentifier, ViewerContext,
};

use crate::{
    determine_visualizable_entities, query_pinhole, space_info::SpaceInfoCollection,
    space_view::SpaceViewBlueprint,
};

// ---------------------------------------------------------------------------
// TODO(#3079): Knowledge of specific space view classes should not leak here.

/// Returns true if a class is one of our spatial classes.
fn is_spatial_class(class: &SpaceViewClassIdentifier) -> bool {
    class.as_str() == "3D" || class.as_str() == "2D"
}

fn is_spatial_2d_class(class: &SpaceViewClassIdentifier) -> bool {
    class.as_str() == "2D"
}

fn spawn_one_space_view_per_entity(class: &SpaceViewClassIdentifier) -> bool {
    // For tensors create one space view for each tensor (even though we're able to stack them in one view)
    // TODO(emilk): query the actual [`VisualizerSystem`] instead.
    class == "Tensor" || class == "Text Document"
}

// ---------------------------------------------------------------------------

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

fn is_interesting_space_view_at_root(
    _data_store: &re_data_store::DataStore,
    query_results: &DataQueryResult,
) -> bool {
    if let Some(root) = query_results.tree.root_node() {
        // Not interesting if it has only data blueprint groups and no direct entities.
        // -> If there In that case we want spaceviews at those groups.
        if root.children.iter().all(|child| {
            query_results
                .tree
                .lookup_node(*child)
                .map_or(true, |child| child.data_result.visualizers.is_empty())
        }) {
            return false;
        }

        // TODO(andreas): We have to figure out how to do this kind of heuristic in a more generic way without deep knowledge of re_types.
        //
        // If there are any images directly under the root, don't create root space either.
        // -> For images we want more fine grained control and resort to child-of-root spaces only.
        if root.children.iter().any(|child| {
            query_results
                .tree
                .lookup_node(*child)
                .map_or(false, |child| {
                    child.data_result.visualizers.contains(&"Images".into()) // TODO(jleibs): Refer to `ImagesPart`
                })
        }) {
            return false;
        }
    } else {
        return false;
    }

    true
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
