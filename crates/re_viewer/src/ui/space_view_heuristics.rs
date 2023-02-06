use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use re_arrow_store::Timeline;
use re_data_store::{query_transform, EntityPath};
use re_log_types::component_types::{Tensor, TensorTrait};

use crate::{
    misc::{
        space_info::{query_view_coordinates, SpaceInfoCollection},
        ViewerContext,
    },
    ui::{view_category::categorize_entity_path, ViewCategory},
};

use super::SpaceView;

pub fn all_possible_space_views(
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

fn has_direct_children(space_view: &SpaceView) -> bool {
    space_view
        .data_blueprint
        .entity_paths()
        .iter()
        .any(|path| path.is_child_of(&space_view.space_path))
}

pub fn default_created_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceView> {
    crate::profile_function!();

    let all_possible_space_views = all_possible_space_views(ctx, spaces_info);
    let is_spatial_view_at_root = all_possible_space_views.iter().any(|view| {
        view.space_path.is_root()
            && view.category == ViewCategory::Spatial
            && has_direct_children(view)
    });

    fn is_interesting(
        ctx: &ViewerContext<'_>,
        space_view_candidate: &SpaceView,
        is_spatial_view_at_root: bool,
    ) -> bool {
        let entity_path = &space_view_candidate.space_path;

        // // Disqualify any spaceview that doesn't have direct children to its space_path.
        // if !space_view_candidate
        //     .data_blueprint
        //     .entity_paths()
        //     .iter()
        //     .any(|path| path.is_child_of(entity_path))
        // {
        //     return false;
        // }

        // After that, anything that isn't a spatial view is considered interesting.
        if space_view_candidate.category != ViewCategory::Spatial {
            return true;
        }

        let timeline = *ctx.rec_cfg.time_ctrl.timeline();
        let timeline_query =
            re_arrow_store::LatestAtQuery::new(timeline, re_arrow_store::TimeInt::MAX);

        // Spatial views that came this far, are considered only interesting if they EITHER...
        if entity_path.is_root() {
            return true;
        }
        if entity_path.len() == 1 && !is_spatial_view_at_root {
            return true;
        }
        if let Some(transform) =
            query_transform(&ctx.log_db.entity_db, entity_path, &timeline_query)
        {
            // An "interesting transform"?
            match transform {
                re_log_types::Transform::Rigid3(_) => {}
                re_log_types::Transform::Pinhole(_) | re_log_types::Transform::Unknown => {
                    return true;
                }
            }
        }
        if query_view_coordinates(&ctx.log_db.entity_db, entity_path, &timeline_query).is_some() {
            return true;
        }

        // Not interesting!
        false
    }

    let mut space_views = Vec::new();
    let timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let timeline_query = re_arrow_store::LatestAtQuery::new(timeline, re_arrow_store::TimeInt::MAX);

    for space_view_candidate in all_possible_space_views {
        if !is_interesting(ctx, &space_view_candidate, is_spatial_view_at_root) {
            continue;
        }

        // In some cases we want to do some extra modifications.

        // For tensors create one space view for each tensor.
        if space_view_candidate.category == ViewCategory::Tensor {
            for entity_path in space_view_candidate.data_blueprint.entity_paths() {
                let mut space_view =
                    SpaceView::new(ViewCategory::Tensor, entity_path, &[entity_path.clone()]);
                space_view.entities_determined_by_user = true;
                space_views.push(space_view);
            }
            continue;
        } else if space_view_candidate.category == ViewCategory::Spatial {
            let images = space_view_candidate
                .data_blueprint
                .entity_paths()
                .iter()
                .filter_map(|entity_path| {
                    // Only interested in direct children of the space path.
                    if entity_path.is_child_of(&space_view_candidate.space_path) {
                        if let Ok(entity_view) = re_query::query_entity_with_primary::<Tensor>(
                            &ctx.log_db.entity_db.data_store,
                            &timeline_query,
                            entity_path,
                            &[],
                        ) {
                            if let Ok(iter) = entity_view.iter_primary() {
                                for tensor in iter.flatten() {
                                    if tensor.is_shaped_like_an_image() {
                                        return Some((entity_path.clone(), tensor.shape));
                                    }
                                }
                            }
                        }
                    }
                    None
                })
                .collect::<Vec<_>>();

            if images.len() > 1 {
                // Multiple images (e.g. depth and rgb, or rgb and segmentation) in the same 2D scene.
                // Stacking them on top of each other works, but is often confusing.
                // Let's create one space view for each image, where the other images are disabled:

                let mut image_sizes = BTreeSet::default();
                for (entity_path, shape) in &images {
                    debug_assert!(matches!(shape.len(), 2 | 3));
                    let image_size = (shape[0].size, shape[1].size);
                    image_sizes.insert(image_size);

                    // Space view with everything but the other images.
                    // (note that other entities stay!)
                    let mut single_image_space_view = space_view_candidate.clone();
                    for (other_entity_path, _) in &images {
                        if other_entity_path != entity_path {
                            single_image_space_view
                                .data_blueprint
                                .remove_entity(other_entity_path);
                        }
                    }

                    single_image_space_view.entities_determined_by_user = true;
                }

                // Only if all images have the same size, so we _also_ want to create the stacked version (e.g. rgb + segmentation)
                // TODO(andreas): What if there's also other entities that we want to show?
                if image_sizes.len() > 1 {
                    continue;
                }
            }
        }

        // Take the candidate as is.
        space_views.push(space_view_candidate);
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
