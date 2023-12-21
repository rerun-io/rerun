use ahash::HashMap;
use itertools::Itertools;

use re_arrow_store::{LatestAtQuery, Timeline};
use re_data_store::EntityPath;
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
    // TODO(emilk): query the actual [`ViewPartSystem`] instead.
    class == "Tensor" || class == "Text Document"
}

// ---------------------------------------------------------------------------

fn candidate_space_view_paths<'a>(
    ctx: &ViewerContext<'a>,
    spaces_info: &'a SpaceInfoCollection,
) -> impl Iterator<Item = &'a EntityPath> {
    // Everything with a SpaceInfo is a candidate (that is root + whenever there is a transform),
    // as well as all direct descendants of the root.
    let root_children = &ctx.store_db.tree().children;
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
            let reachable_entities =
                reachable_entities_from_root(candidate_space_path, spaces_info);
            if reachable_entities.is_empty() {
                return Vec::new();
            }

            // TODO: only running on applicable is too weak of a filter
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

                    // TODO(#4377): The need to run a query-per-candidate for all possible candidates
                    // is way too expensive. This needs to be optimized significantly.
                    let candidate_query =
                        DataQueryBlueprint::new(class_identifier, entity_path_filter);

                    let visualizable_entities = determine_visualizable_entities(
                        ctx.applicable_entities_per_visualizer,
                        ctx.store_db,
                        &ctx.space_view_class_registry
                            .new_part_collection(class_identifier),
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
                                ctx.space_view_class_registry
                                    .display_name(&class_identifier),
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
    _data_store: &re_arrow_store::DataStore,
    query_results: &DataQueryResult,
) -> bool {
    if let Some(root) = query_results.tree.root_node() {
        // Not interesting if it has only data blueprint groups and no direct entities.
        // -> If there In that case we want spaceviews at those groups.
        if root.children.iter().all(|child| {
            query_results
                .tree
                .lookup_node(*child)
                .map_or(true, |child| child.data_result.view_parts.is_empty())
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
                    child.data_result.view_parts.contains(&"Images".into()) // TODO(jleibs): Refer to `ImagesPart`
                })
        }) {
            return false;
        }
    } else {
        return false;
    }

    true
}

fn is_interesting_space_view_not_at_root(
    store: &re_arrow_store::DataStore,
    candidate: &SpaceViewBlueprint,
    classes_with_interesting_roots: &[SpaceViewClassIdentifier],
    query: &LatestAtQuery,
) -> bool {
    // TODO(andreas): Can we express this with [`AutoSpawnHeuristic`] instead?

    // Consider children of the root interesting, *unless* a root with the same category was already considered interesting!
    if candidate.space_origin.len() == 1
        && !classes_with_interesting_roots.contains(candidate.class_identifier())
    {
        return true;
    }

    // .. otherwise, spatial views are considered only interesting if they have an interesting transform.
    // -> If there is ..
    //    .. a disconnect transform, the children can't be shown otherwise
    //    .. an pinhole transform, we'd like to see the world from this camera's pov as well!
    if is_spatial_class(candidate.class_identifier())
        && (query_pinhole(store, query, &candidate.space_origin).is_some()
            || store
                .query_latest_component::<DisconnectedSpace>(&candidate.space_origin, query)
                .map_or(false, |dp| dp.0))
    {
        return true;
    }

    // Not interesting!
    false
}

/// List out all space views we generate by default for the available data.
pub fn default_created_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceViewBlueprint> {
    re_tracing::profile_function!();

    let store = ctx.store_db.store();
    let candidates = all_possible_space_views(ctx, spaces_info);

    // All queries are "right most" on the log timeline.
    let query = LatestAtQuery::latest(Timeline::log_time());

    // First pass to look for interesting roots, as their existence influences the heuristic for non-roots!
    let classes_with_interesting_roots = candidates
        .iter()
        .filter_map(|(space_view_candidate, query_results)| {
            (space_view_candidate.space_origin.is_root()
                && is_interesting_space_view_at_root(store, query_results))
            .then_some(*space_view_candidate.class_identifier())
        })
        .collect::<Vec<_>>();

    let mut space_views: Vec<(SpaceViewBlueprint, AutoSpawnHeuristic)> = Vec::new();

    // Main pass through all candidates.
    // We first check if a candidate is "interesting" and then split it up/modify it further if required.
    for (candidate, query_result) in candidates {
        // TODO(#4377): Can spawn heuristics consume the query_result directly?
        let mut per_system_entities = PerSystemEntities::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
                    for system in &result.view_parts {
                        per_system_entities
                            .entry(*system)
                            .or_default()
                            .insert(result.entity_path.clone());
                    }
                }
            });
        }

        let spawn_heuristic = candidate
            .class(ctx.space_view_class_registry)
            .auto_spawn_heuristic(ctx, &candidate.space_origin, &per_system_entities);

        if spawn_heuristic == AutoSpawnHeuristic::NeverSpawn {
            continue;
        }
        if spawn_heuristic != AutoSpawnHeuristic::AlwaysSpawn {
            if candidate.space_origin.is_root() {
                if !classes_with_interesting_roots.contains(candidate.class_identifier()) {
                    continue;
                }
            } else if !is_interesting_space_view_not_at_root(
                store,
                &candidate,
                &classes_with_interesting_roots,
                &query,
            ) {
                continue;
            }
        }

        if spawn_one_space_view_per_entity(candidate.class_identifier()) {
            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
                    if !result.view_parts.is_empty() {
                        let mut entity_path_filter = EntityPathFilter::default();
                        entity_path_filter.add_exact(result.entity_path.clone());
                        let query = DataQueryBlueprint::new(
                            *candidate.class_identifier(),
                            entity_path_filter,
                        );
                        let mut space_view = SpaceViewBlueprint::new(
                            *candidate.class_identifier(),
                            ctx.space_view_class_registry
                                .display_name(candidate.class_identifier()),
                            &result.entity_path,
                            query,
                        );
                        space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                        space_views.push((space_view, AutoSpawnHeuristic::AlwaysSpawn));
                    }
                }
            });
            continue;
        }

        // TODO(andreas): Interaction of [`AutoSpawnHeuristic`] with above hardcoded heuristics is a bit wonky.

        // `AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot` means we're competing with other candidates for the same root.
        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score) = spawn_heuristic {
            // [`SpaceViewBlueprint`]s don't implement clone so wrap in an option so we can
            // track whether or not we've consumed it.
            let mut candidate_still_considered = Some(candidate);

            for (prev_candidate, prev_spawn_heuristic) in &mut space_views {
                if let Some(candidate) = candidate_still_considered.take() {
                    if prev_candidate.space_origin == candidate.space_origin {
                        #[allow(clippy::match_same_arms)]
                        match prev_spawn_heuristic {
                            AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(prev_score) => {
                                // If we're competing with a candidate for the same root, we either replace a lower score, or we yield.
                                if *prev_score < score {
                                    // Replace the previous candidate with this one.
                                    *prev_candidate = candidate;
                                    *prev_spawn_heuristic = spawn_heuristic;
                                }

                                // Either way we're done with this candidate.
                                break;
                            }
                            AutoSpawnHeuristic::AlwaysSpawn => {
                                // We can live side by side with always-spawn candidates.
                            }
                            AutoSpawnHeuristic::NeverSpawn => {
                                // Never spawn candidates should not be in the list, this is weird!
                                // But let's not fail on this since our heuristics are not perfect anyways.
                            }
                        }
                    }

                    // If we didn't hit the break condition, continue to consider the candidate
                    candidate_still_considered = Some(candidate);
                }
            }

            if let Some(candidate) = candidate_still_considered {
                // Spatial views with images get extra treatment as well.
                if is_spatial_2d_class(candidate.class_identifier()) {
                    #[derive(Hash, PartialEq, Eq)]
                    enum ImageBucketing {
                        BySize((u64, u64)),
                        ExplicitDrawOrder,
                    }

                    let mut images_by_bucket: HashMap<ImageBucketing, Vec<EntityPath>> =
                        HashMap::default();

                    if let Some(root) = query_result.tree.root_node() {
                        // For this we're only interested in the direct children.
                        for child in &root.children {
                            if let Some(node) = query_result.tree.lookup_node(*child) {
                                if !node.data_result.view_parts.is_empty() {
                                    let entity_path = &node.data_result.entity_path;
                                    if let Some(tensor) = store
                                        .query_latest_component::<TensorData>(entity_path, &query)
                                    {
                                        if let Some([height, width, _]) =
                                            tensor.image_height_width_channels()
                                        {
                                            if store
                                                .query_latest_component::<re_types::components::DrawOrder>(
                                                    entity_path,
                                                    &query,
                                                )
                                                .is_some()
                                            {
                                                // Put everything in the same bucket if it has a draw order.
                                                images_by_bucket
                                                    .entry(ImageBucketing::ExplicitDrawOrder)
                                                    .or_default()
                                                    .push(entity_path.clone());
                                            } else {
                                                // Otherwise, distinguish buckets by image size.
                                                images_by_bucket
                                                    .entry(ImageBucketing::BySize((height, width)))
                                                    .or_default()
                                                    .push(entity_path.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if images_by_bucket.len() > 1 {
                        // If all images end up in the same bucket, proceed as normal. Otherwise stack images as instructed.
                        for bucket in images_by_bucket.values() {
                            let mut entity_path_filter = EntityPathFilter::default();
                            for path in bucket {
                                entity_path_filter.add_exact(path.clone());
                            }

                            let query = DataQueryBlueprint::new(
                                *candidate.class_identifier(),
                                entity_path_filter,
                            );

                            let mut space_view = SpaceViewBlueprint::new(
                                *candidate.class_identifier(),
                                ctx.space_view_class_registry
                                    .display_name(candidate.class_identifier()),
                                &candidate.space_origin,
                                query,
                            );
                            space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                            space_views.push((space_view, AutoSpawnHeuristic::AlwaysSpawn));
                        }
                        continue;
                    }
                }

                space_views.push((candidate, spawn_heuristic));
            }
        } else {
            space_views.push((candidate, spawn_heuristic));
        }
    }

    space_views.into_iter().map(|(s, _)| s).collect()
}

pub fn reachable_entities_from_root(
    root: &EntityPath,
    spaces_info: &SpaceInfoCollection,
) -> Vec<EntityPath> {
    re_tracing::profile_function!();

    let mut entities = Vec::new();
    let space_info = spaces_info.get_first_parent_with_info(root);

    if &space_info.path == root {
        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            entities.extend(space_info.descendants_without_transform.iter().cloned());
        });
    } else {
        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            entities.extend(
                space_info
                    .descendants_without_transform
                    .iter()
                    .filter(|ent_path| ent_path.starts_with(root))
                    .cloned(),
            );
        });
    }

    entities
}
