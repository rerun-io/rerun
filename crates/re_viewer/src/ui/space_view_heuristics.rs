use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntSet;
use re_arrow_store::{DataStore, LatestAtQuery, Timeline};
use re_data_store::{ComponentName, EntityPath};
use re_log_types::{
    component_types::{DisconnectedSpace, Pinhole, Tensor},
    Component,
};
use re_viewer_context::ViewerContext;

use crate::{
    misc::space_info::SpaceInfoCollection,
    ui::{view_category::categorize_entity_path, ViewCategory},
};

use super::{view_category::ViewCategorySet, SpaceView};

/// List out all space views we allow the user to create.
pub fn all_possible_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceView> {
    crate::profile_function!();

    // Everything with a SpaceInfo is a candidate (that is root + whenever there is a transform),
    // as well as all direct descendants of the root.
    let root_children = &ctx.log_db.entity_db.tree.children;
    let candidate_space_paths = spaces_info
        .iter()
        .map(|info| &info.path)
        .chain(root_children.values().map(|sub_tree| &sub_tree.path))
        .unique();

    // For each candidate, create space views for all possible categories.
    candidate_space_paths
        .flat_map(|candidate_space_path| {
            default_queried_entities_by_category(ctx, candidate_space_path, spaces_info)
                .iter()
                .map(|(category, entity_paths)| {
                    SpaceView::new(*category, candidate_space_path, entity_paths)
                })
                .collect::<Vec<_>>()
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
    candidate: &SpaceView,
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
    candidate: &SpaceView,
    categories_with_interesting_roots: &ViewCategorySet,
    query: &LatestAtQuery,
) -> bool {
    // Consider children of the root interesting, *unless* a root with the same category was already considered interesting!
    if candidate.space_path.len() == 1
        && !categories_with_interesting_roots.contains(candidate.category)
    {
        return true;
    }

    // .. otherwise, spatial views are considered only interesting if they have an interesting transform.
    // -> If there is ..
    //    .. a disconnect transform, the children can't be shown otherwise
    //    .. an pinhole transform, we'd like to see the world from this camera's pov as well!
    if candidate.category == ViewCategory::Spatial
        && (store
            .query_latest_component::<Pinhole>(&candidate.space_path, query)
            .is_some()
            || store
                .query_latest_component::<DisconnectedSpace>(&candidate.space_path, query)
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
) -> Vec<SpaceView> {
    let candidates = all_possible_space_views(ctx, spaces_info);
    default_created_space_views_from_candidates(&ctx.log_db.entity_db.data_store, candidates)
}

fn default_created_space_views_from_candidates(
    store: &re_arrow_store::DataStore,
    candidates: Vec<SpaceView>,
) -> Vec<SpaceView> {
    crate::profile_function!();

    // All queries are "right most" on the log timeline.
    let query = LatestAtQuery::new(Timeline::log_time(), re_arrow_store::TimeInt::MAX);

    // First pass to look for interesting roots, as their existence influences the heuristic for non-roots!
    let categories_with_interesting_roots = candidates
        .iter()
        .filter_map(|space_view_candidate| {
            (space_view_candidate.space_path.is_root()
                && is_interesting_space_view_at_root(store, space_view_candidate, &query))
            .then_some(space_view_candidate.category)
        })
        .collect::<ViewCategorySet>();

    let mut space_views = Vec::new();

    // Main pass through all candidates.
    // We first check if a candidate is "interesting" and then split it up/modify it further if required.
    for candidate in candidates {
        if candidate.space_path.is_root() {
            if !categories_with_interesting_roots.contains(candidate.category) {
                continue;
            }
        } else if !is_interesting_space_view_not_at_root(
            store,
            &candidate,
            &categories_with_interesting_roots,
            &query,
        ) {
            continue;
        }

        // For tensors create one space view for each tensor (even though we're able to stack them in one view)
        if candidate.category == ViewCategory::Tensor {
            for entity_path in candidate.data_blueprint.entity_paths() {
                let mut space_view =
                    SpaceView::new(ViewCategory::Tensor, entity_path, &[entity_path.clone()]);
                space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                space_views.push(space_view);
            }
            continue;
        }

        // Spatial views with images get extra treatment as well.
        if candidate.category == ViewCategory::Spatial {
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
                            .query_latest_component::<re_log_types::DrawOrder>(entity_path, &query)
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
                    // Ignore every image from antoher bucket. Keep all other entities.
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

                    let mut space_view =
                        SpaceView::new(candidate.category, &candidate.space_path, &entities);
                    space_view.entities_determined_by_user = true; // Suppress auto adding of entities.
                    space_views.push(space_view);
                }
                continue;
            }
        }

        // Take the candidate as is.
        space_views.push(candidate);
    }

    space_views
}

fn has_any_component_except(
    entity_path: &EntityPath,
    data_store: &DataStore,
    timeline: Timeline,
    excluded_components: &[ComponentName],
) -> bool {
    data_store
        .all_components(&timeline, entity_path)
        .map_or(false, |all_components| {
            all_components
                .iter()
                .any(|comp| !excluded_components.contains(comp))
        })
}

/// Whether an entity should be added to a space view at a given path (independent of its category!)
fn is_default_added_to_space_view(
    entity_path: &EntityPath,
    space_path: &EntityPath,
    data_store: &DataStore,
    timeline: Timeline,
) -> bool {
    let ignored_components = [
        re_log_types::component_types::Transform3D::name(),
        re_log_types::ViewCoordinates::name(),
        re_log_types::component_types::InstanceKey::name(),
        re_log_types::component_types::KeypointId::name(),
        DataStore::insert_id_key(),
    ];

    entity_path.is_descendant_of(space_path)
        || (entity_path == space_path
            && has_any_component_except(entity_path, data_store, timeline, &ignored_components))
}

/// List of entities a space view queries by default for a given category.
///
/// These are all entities in the given space which have the requested category and are reachable by a transform.
pub fn default_queried_entities(
    ctx: &ViewerContext<'_>,
    space_path: &EntityPath,
    spaces_info: &SpaceInfoCollection,
    category: ViewCategory,
) -> Vec<EntityPath> {
    crate::profile_function!();

    let timeline = Timeline::log_time();
    let log_db = &ctx.log_db;
    let data_store = &log_db.entity_db.data_store;

    let mut entities = Vec::new();
    let space_info = spaces_info.get_first_parent_with_info(space_path);

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        entities.extend(
            space_info
                .descendants_without_transform
                .iter()
                .filter(|entity_path| {
                    is_default_added_to_space_view(entity_path, space_path, data_store, timeline)
                        && categorize_entity_path(timeline, log_db, entity_path).contains(category)
                })
                .cloned(),
        );
    });

    entities
}

/// List of entities a space view queries by default for all possible category.
fn default_queried_entities_by_category(
    ctx: &ViewerContext<'_>,
    space_path: &EntityPath,
    space_info_collection: &SpaceInfoCollection,
) -> BTreeMap<ViewCategory, Vec<EntityPath>> {
    crate::profile_function!();

    let timeline = Timeline::log_time();
    let log_db = &ctx.log_db;
    let data_store = &log_db.entity_db.data_store;

    let mut groups: BTreeMap<ViewCategory, Vec<EntityPath>> = BTreeMap::default();
    let space_info = space_info_collection.get_first_parent_with_info(space_path);

    space_info.visit_descendants_with_reachable_transform(
        space_info_collection,
        &mut |space_info| {
            for entity_path in &space_info.descendants_without_transform {
                if is_default_added_to_space_view(entity_path, space_path, data_store, timeline) {
                    for category in categorize_entity_path(timeline, log_db, entity_path) {
                        groups
                            .entry(category)
                            .or_default()
                            .push(entity_path.clone());
                    }
                }
            }
        },
    );

    groups
}
