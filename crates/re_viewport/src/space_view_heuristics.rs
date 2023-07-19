use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntSet;
<<<<<<< HEAD
=======
use re_arrow_store::{DataStore, LatestAtQuery, Timeline};
use re_components::{DisconnectedSpace, Pinhole, Tensor};
use re_data_store::EntityPath;
use re_types::{ComponentName, Loggable as _};
use re_viewer_context::{SpaceViewClassName, ViewerContext};
>>>>>>> 1e25abf51 (Migrating DataCell to re_types::Component)

use re_arrow_store::{LatestAtQuery, Timeline};
use re_components::{DisconnectedSpace, Pinhole, Tensor};
use re_data_store::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, SpaceViewClassName, ViewPartCollection, ViewerContext,
};

use crate::{space_info::SpaceInfoCollection, space_view::SpaceViewBlueprint};

// ---------------------------------------------------------------------------
// TODO(andreas): Figure out how we can move heuristics based on concrete space view classes into the classes themselves.

/// Returns true if a class is one of our spatial classes.
fn is_spatial_class(class: &SpaceViewClassName) -> bool {
    class.as_str() == "3D" || class.as_str() == "2D"
}

fn is_tensor_class(class: &SpaceViewClassName) -> bool {
    class.as_str() == "Tensor"
}

// ---------------------------------------------------------------------------

/// List out all space views we allow the user to create.
pub fn all_possible_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceViewBlueprint> {
    re_tracing::profile_function!();

    // Everything with a SpaceInfo is a candidate (that is root + whenever there is a transform),
    // as well as all direct descendants of the root.
    let root_children = &ctx.store_db.entity_db.tree.children;
    let candidate_space_paths = spaces_info
        .iter()
        .map(|info| &info.path)
        .chain(root_children.values().map(|sub_tree| &sub_tree.path))
        .unique();

    // For each candidate, create space views for all possible classes.
    // TODO(andreas): Could save quite a view allocations here by re-using component- and parts arrays.
    candidate_space_paths
        .flat_map(|candidate_space_path| {
            ctx.space_view_class_registry
                .iter_classes()
                .filter_map(|class| {
                    let class_name = class.name();
                    let entities = default_queried_entities(
                        ctx,
                        &class_name,
                        candidate_space_path,
                        spaces_info,
                    );
                    if entities.is_empty() {
                        None
                    } else {
                        Some(SpaceViewBlueprint::new(
                            class_name,
                            &candidate_space_path.clone(),
                            &entities,
                        ))
                    }
                })
        })
        .collect()
}

fn contains_any_image(
    entity_path: &EntityPath,
    store: &re_arrow_store::DataStore,
    query: &LatestAtQuery,
) -> bool {
    if let Some(tensor) = store.query_latest_component::<Tensor>(entity_path, query) {
        tensor.is_shaped_like_an_image()
    } else {
        false
    }
}

fn is_interesting_space_view_at_root(
    data_store: &re_arrow_store::DataStore,
    candidate: &SpaceViewBlueprint,
    query: &LatestAtQuery,
) -> bool {
    // Not interesting if it has only data blueprint groups and no direct entities.
    // -> If there In that case we want spaceviews at those groups.
    if candidate.data_blueprint.root_group().entities.is_empty() {
        return false;
    }

    // If there are any images directly under the root, don't create root space either.
    // -> For images we want more fine grained control and resort to child-of-root spaces only.
    for entity_path in &candidate.data_blueprint.root_group().entities {
        if contains_any_image(entity_path, data_store, query) {
            return false;
        }
    }

    true
}

fn is_interesting_space_view_not_at_root(
    store: &re_arrow_store::DataStore,
    candidate: &SpaceViewBlueprint,
    classes_with_interesting_roots: &[SpaceViewClassName],
    query: &LatestAtQuery,
) -> bool {
    // TODO(andreas): Can we express this with [`AutoSpawnHeuristic`] instead?

    // Consider children of the root interesting, *unless* a root with the same category was already considered interesting!
    if candidate.space_origin.len() == 1
        && !classes_with_interesting_roots.contains(candidate.class_name())
    {
        return true;
    }

    // .. otherwise, spatial views are considered only interesting if they have an interesting transform.
    // -> If there is ..
    //    .. a disconnect transform, the children can't be shown otherwise
    //    .. an pinhole transform, we'd like to see the world from this camera's pov as well!
    if is_spatial_class(candidate.class_name())
        && (store
            .query_latest_component::<Pinhole>(&candidate.space_origin, query)
            .is_some()
            || store
                .query_latest_component::<DisconnectedSpace>(&candidate.space_origin, query)
                .is_some())
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
    let candidates = all_possible_space_views(ctx, spaces_info)
        .into_iter()
        .map(|c| {
            (
                c.class(ctx.space_view_class_registry).auto_spawn_heuristic(
                    ctx,
                    &c.space_origin,
                    c.data_blueprint.entity_paths(),
                ),
                c,
            )
        })
        .collect::<Vec<_>>();

    // All queries are "right most" on the log timeline.
    let query = LatestAtQuery::latest(Timeline::log_time());

    // First pass to look for interesting roots, as their existence influences the heuristic for non-roots!
    let classes_with_interesting_roots = candidates
        .iter()
        .filter_map(|(_, space_view_candidate)| {
            (space_view_candidate.space_origin.is_root()
                && is_interesting_space_view_at_root(store, space_view_candidate, &query))
            .then_some(*space_view_candidate.class_name())
        })
        .collect::<Vec<_>>();

    let mut space_views: Vec<(SpaceViewBlueprint, AutoSpawnHeuristic)> = Vec::new();

    // Main pass through all candidates.
    // We first check if a candidate is "interesting" and then split it up/modify it further if required.
    for (spawn_heuristic, candidate) in candidates {
        if spawn_heuristic == AutoSpawnHeuristic::NeverSpawn {
            continue;
        }
        if spawn_heuristic != AutoSpawnHeuristic::AlwaysSpawn {
            if candidate.space_origin.is_root() {
                if !classes_with_interesting_roots.contains(candidate.class_name()) {
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

        // For tensors create one space view for each tensor (even though we're able to stack them in one view)
        if is_tensor_class(candidate.class_name()) {
            for entity_path in candidate.data_blueprint.entity_paths() {
                let mut space_view = SpaceViewBlueprint::new(
                    *candidate.class_name(),
                    entity_path,
                    &[entity_path.clone()],
                );
                space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                space_views.push((space_view, AutoSpawnHeuristic::AlwaysSpawn));
            }
            continue;
        }

        // Spatial views with images get extra treatment as well.
        if is_spatial_class(candidate.class_name()) {
            #[derive(Hash, PartialEq, Eq)]
            enum ImageBucketing {
                BySize((u64, u64)),
                ExplicitDrawOrder,
            }

            let mut images_by_bucket: HashMap<ImageBucketing, Vec<EntityPath>> = HashMap::default();

            // For this we're only interested in the direct children.
            for entity_path in &candidate.data_blueprint.root_group().entities {
                if let Some(tensor) = store.query_latest_component::<Tensor>(entity_path, &query) {
                    if let Some([height, width, _]) = tensor.image_height_width_channels() {
                        if store
                            .query_latest_component::<re_components::DrawOrder>(entity_path, &query)
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

            if images_by_bucket.len() > 1 {
                // If all images end up in the same bucket, proceed as normal. Otherwise stack images as instructed.
                for bucket in images_by_bucket.keys() {
                    // Ignore every image from another bucket. Keep all other entities.
                    let images_of_different_size = images_by_bucket
                        .iter()
                        .filter_map(|(other_bucket, images)| {
                            (bucket != other_bucket).then_some(images)
                        })
                        .flatten()
                        .cloned()
                        .collect::<IntSet<_>>();
                    let entities = candidate
                        .data_blueprint
                        .entity_paths()
                        .iter()
                        .filter(|path| !images_of_different_size.contains(path))
                        .cloned()
                        .collect_vec();

                    let mut space_view = SpaceViewBlueprint::new(
                        *candidate.class_name(),
                        &candidate.space_origin,
                        &entities,
                    );
                    space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                    space_views.push((space_view, AutoSpawnHeuristic::AlwaysSpawn));
                }
                continue;
            }
        }

        // TODO(andreas): Interaction of [`AutoSpawnHeuristic`] with above hardcoded heuristics is a bit wonky.

        // `AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot` means we're competing with other candidates for the same root.
        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score) = spawn_heuristic {
            let mut should_spawn_new = true;
            for (prev_candidate, prev_spawn_heuristic) in &mut space_views {
                if prev_candidate.space_origin == candidate.space_origin {
                    #[allow(clippy::match_same_arms)]
                    match prev_spawn_heuristic {
                        AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(prev_score) => {
                            // If we're competing with a candidate for the same root, we either replace a lower score, or we yield.
                            should_spawn_new = false;
                            if *prev_score < score {
                                // Replace the previous candidate with this one.
                                *prev_candidate = candidate.clone();
                                *prev_spawn_heuristic = spawn_heuristic;
                            } else {
                                // We have a lower score, so we don't spawn.
                                break;
                            }
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
            }

            if should_spawn_new {
                space_views.push((candidate, spawn_heuristic));
            }
        } else {
            space_views.push((candidate, spawn_heuristic));
        }
    }

    space_views.into_iter().map(|(s, _)| s).collect()
}

/// List of entities a space view queries by default for a given category.
///
/// These are all entities which are reachable and
/// match at least one archetypes that is processed by at least one [`re_viewer_context::ViewPartSystem`]
/// of the given [`re_viewer_context::SpaceViewClass`]
pub fn default_queried_entities(
    ctx: &ViewerContext<'_>,
    class: &SpaceViewClassName,
    space_path: &EntityPath,
    spaces_info: &SpaceInfoCollection,
) -> Vec<EntityPath> {
    re_tracing::profile_function!();

    let mut entities = Vec::new();
    let space_info = spaces_info.get_first_parent_with_info(space_path);

    let parts = ctx
        .space_view_class_registry
        .get_system_registry_or_log_error(class)
        .new_part_collection();

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        entities.extend(
            space_info
                .descendants_without_transform
                .iter()
                .filter(|ent_path| {
                    (ent_path.is_descendant_of(space_path) || ent_path == &space_path)
                        && is_entity_processed_by_part_collection(
                            ctx.store_db.store(),
                            &parts,
                            ent_path,
                        )
                })
                .cloned(),
        );
    });

    entities
}

/// Returns true if an entity is processed by any of the given [`re_viewer_context::ViewPartSystem`]s.
pub fn is_entity_processed_by_class(
    ctx: &ViewerContext<'_>,
    class: &SpaceViewClassName,
    ent_path: &EntityPath,
) -> bool {
    let parts = ctx
        .space_view_class_registry
        .get_system_registry_or_log_error(class)
        .new_part_collection();
    is_entity_processed_by_part_collection(ctx.store_db.store(), &parts, ent_path)
}

/// Returns true if an entity is processed by any of the given [`re_viewer_context::ViewPartSystem`]s.
fn is_entity_processed_by_part_collection(
    store: &re_arrow_store::DataStore,
    parts: &ViewPartCollection,
    ent_path: &EntityPath,
) -> bool {
    let timeline = Timeline::log_time();
    let components = store
        .all_components(&timeline, ent_path)
        .unwrap_or_default();
    for part in parts.iter() {
        if part.queries_any_components_of(store, ent_path, &components) {
            return true;
        }
    }

    false
}
