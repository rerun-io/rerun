use std::collections::BTreeMap;

use ahash::HashMap;
use enumset::EnumSet;
use itertools::Itertools;
use nohash_hasher::IntSet;
use re_arrow_store::{LatestAtQuery, Timeline};
use re_data_store::{query_transform, EntityPath};
use re_log_types::component_types::{Tensor, TensorTrait};

use crate::{
    misc::{space_info::SpaceInfoCollection, ViewerContext},
    ui::{view_category::categorize_entity_path, ViewCategory},
};

use super::SpaceView;

pub fn all_space_view_candidates(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceView> {
    crate::profile_function!();

    let mut space_views = Vec::new();

    // Everything is a candidate for which we have a space info (i.e. a transform!) plus direct descendants of the root.
    let candidate_space_paths = spaces_info
        .iter()
        .map(|info| &info.path)
        .chain(
            ctx.log_db
                .entity_db
                .tree
                .children
                .values()
                .map(|sub_tree| &sub_tree.path),
        )
        .unique();

    // For each candidate, create space views for all possible categories.
    for candidate_space_path in candidate_space_paths {
        for (category, entity_paths) in
            default_queried_entities_by_category(ctx, candidate_space_path, spaces_info)
        {
            space_views.push(SpaceView::new(
                category,
                candidate_space_path,
                &entity_paths,
            ));
        }
    }

    space_views
}

fn is_interesting_root(
    ctx: &ViewerContext<'_>,
    candidate: &SpaceView,
    timeline_query: &LatestAtQuery,
) -> bool {
    // Not interesting if it has only data blueprint groups and no direct entities.
    if candidate.data_blueprint.root_group().entities.is_empty() {
        return false;
    }

    // Not interesting if it has only images
    for entity_path in &candidate.data_blueprint.root_group().entities {
        if let Ok(entity_view) = re_query::query_entity_with_primary::<Tensor>(
            &ctx.log_db.entity_db.data_store,
            timeline_query,
            entity_path,
            &[],
        ) {
            if let Ok(iter) = entity_view.iter_primary() {
                for tensor in iter.flatten() {
                    if tensor.is_shaped_like_an_image() {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn is_interesting_non_root(
    ctx: &ViewerContext<'_>,
    candidate: &SpaceView,
    categories_with_interesting_roots: &EnumSet<ViewCategory>,
    timeline_query: &LatestAtQuery,
) -> bool {
    // Consider children of the root interesting, *unless* a root with the same category was already considered interesting!
    if candidate.space_path.len() == 1
        && !categories_with_interesting_roots.contains(candidate.category)
    {
        return true;
    }

    // .. otherwise, spatial views are considered only interesting if they have in interesting transform.
    if candidate.category == ViewCategory::Spatial {
        if let Some(transform) =
            query_transform(&ctx.log_db.entity_db, &candidate.space_path, timeline_query)
        {
            match transform {
                re_log_types::Transform::Rigid3(_) => {}
                re_log_types::Transform::Pinhole(_) | re_log_types::Transform::Unknown => {
                    return true;
                }
            }
        }
    }

    // Not interesting!
    false
}

pub fn default_created_space_views(
    ctx: &ViewerContext<'_>,
    candidates: Vec<SpaceView>,
) -> Vec<SpaceView> {
    crate::profile_function!();

    let timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let timeline_query = re_arrow_store::LatestAtQuery::new(timeline, re_arrow_store::TimeInt::MAX);

    // First pass to look for interesting roots.
    let categories_with_interesting_roots = candidates
        .iter()
        .filter_map(|space_view_candidate| {
            (space_view_candidate.space_path.is_root()
                && is_interesting_root(ctx, space_view_candidate, &timeline_query))
            .then_some(space_view_candidate.category)
        })
        .collect::<EnumSet<_>>();

    let timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let timeline_query = re_arrow_store::LatestAtQuery::new(timeline, re_arrow_store::TimeInt::MAX);

    let mut space_views = Vec::new();

    for candidate in candidates {
        if candidate.space_path.is_root() {
            if !categories_with_interesting_roots.contains(candidate.category) {
                continue;
            }
        } else if !is_interesting_non_root(
            ctx,
            &candidate,
            &categories_with_interesting_roots,
            &timeline_query,
        ) {
            continue;
        }

        // Now that we know this space view is "interesting", we need to consider doing some
        // changes to the selected objects and maybe create variants of it!

        if candidate.category == ViewCategory::Tensor {
            // For tensors create one space view for each tensor (even though we're able to stack them in one view)
            for entity_path in candidate.data_blueprint.entity_paths() {
                let mut space_view =
                    SpaceView::new(ViewCategory::Tensor, entity_path, &[entity_path.clone()]);
                space_view.entities_determined_by_user = true;
                space_views.push(space_view);
            }
            continue;
        } else if candidate.category == ViewCategory::Spatial {
            // Spatial views with images get extra treatment as well.
            // For this we're only interested in the direct children.
            let mut images: HashMap<(u64, u64), Vec<EntityPath>> = HashMap::default();

            for entity_path in &candidate.data_blueprint.root_group().entities {
                if let Ok(entity_view) = re_query::query_entity_with_primary::<Tensor>(
                    &ctx.log_db.entity_db.data_store,
                    &timeline_query,
                    entity_path,
                    &[],
                ) {
                    if let Ok(iter) = entity_view.iter_primary() {
                        for tensor in iter.flatten() {
                            if tensor.is_shaped_like_an_image() {
                                debug_assert!(matches!(tensor.shape.len(), 2 | 3));
                                let dim = (tensor.shape[0].size, tensor.shape[1].size);
                                images.entry(dim).or_default().push(entity_path.clone());
                            }
                        }
                    }
                }
            }

            if images.len() > 1 {
                // Stack images of the same size, but no others.
                // TODO(andreas): A possible refinement here would be to stack only with `TensorDataMeaning::ClassId`
                //                But that's not entirely straight forward to implement.
                for dim in images.keys() {
                    // New spaces views that do not contain any image but this one plus all fitting class id images.
                    let ignore_list = images
                        .iter()
                        .filter_map(|(other_dim, images)| (dim != other_dim).then_some(images))
                        .flatten()
                        .cloned()
                        .collect::<IntSet<_>>();
                    let entities = candidate
                        .data_blueprint
                        .entity_paths()
                        .iter()
                        .filter(|path| !ignore_list.contains(path))
                        .cloned()
                        .collect::<Vec<_>>();

                    let mut space_view =
                        SpaceView::new(candidate.category, &candidate.space_path, &entities);
                    space_view.entities_determined_by_user = true;
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

    let mut entities = Vec::new();
    let space_info = spaces_info.get_first_parent_with_info(space_path);

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        entities.extend(
            space_info
                .descendants_without_transform
                .iter()
                .filter(|entity_path| {
                    (entity_path == &space_path || entity_path.is_descendant_of(space_path))
                        && categorize_entity_path(timeline, log_db, entity_path).contains(category)
                })
                .cloned(),
        );
    });

    entities
}

/// List of entities a space view queries by default for all any possible category.
fn default_queried_entities_by_category(
    ctx: &ViewerContext<'_>,
    space_path: &EntityPath,
    spaces_info: &SpaceInfoCollection,
) -> BTreeMap<ViewCategory, Vec<EntityPath>> {
    crate::profile_function!();

    let timeline = Timeline::log_time();
    let log_db = &ctx.log_db;

    let mut groups: BTreeMap<ViewCategory, Vec<EntityPath>> = BTreeMap::default();
    let space_info = spaces_info.get_first_parent_with_info(space_path);

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        for entity_path in &space_info.descendants_without_transform {
            if entity_path == space_path || entity_path.is_descendant_of(space_path) {
                for category in categorize_entity_path(timeline, log_db, entity_path) {
                    groups
                        .entry(category)
                        .or_default()
                        .push(entity_path.clone());
                }
            }
        }
    });

    groups
}
