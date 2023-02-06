use std::collections::{BTreeMap, BTreeSet};

use re_arrow_store::Timeline;
use re_data_store::EntityPath;
use re_log_types::component_types::{Tensor, TensorTrait};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpaceInfoCollection},
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

    for space_info in spaces_info.iter() {
        for (category, entity_paths) in
            default_queries_entities_by_category(ctx, spaces_info, space_info)
        {
            // For tensors create one space view for each tensor.
            if category == ViewCategory::Tensor {
                for entity_path in entity_paths {
                    let mut space_view = SpaceView::new(category, space_info, &[entity_path]);
                    space_view.entities_determined_by_user = true;
                    space_views.push(space_view);
                }
            } else {
                space_views.push(SpaceView::new(category, space_info, &entity_paths));
            }
        }
    }

    space_views
}

pub fn default_created_space_views(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
) -> Vec<SpaceView> {
    crate::profile_function!();

    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let timeline_query =
        re_arrow_store::LatestAtQuery::new(*timeline, re_arrow_store::TimeInt::MAX);

    let mut space_views = Vec::new();

    for space_view_candidate in all_possible_space_views(ctx, spaces_info) {
        // Skip root space for now, messes things up.
        if space_view_candidate.space_path.is_root() {
            continue;
        }

        let Some(space_info) = spaces_info.get(&space_view_candidate.space_path) else {
                // Should never happen.
                continue;
            };

        if space_view_candidate.category == ViewCategory::Spatial {
            // For every item that isn't a direct descendant of the root, skip if connection to parent is via rigid (too trivial for a new space view!)
            if space_info.path.parent() != Some(EntityPath::root()) {
                if let Some(parent_transform) = space_info.parent_transform() {
                    match parent_transform {
                        re_log_types::Transform::Rigid3(_) => {
                            continue;
                        }
                        re_log_types::Transform::Pinhole(_) | re_log_types::Transform::Unknown => {}
                    }
                }
            }

            // Gather all images that are untransformed children of the space view candidate's root.
            let images = space_info
                .descendants_without_transform
                .iter()
                .filter_map(|entity_path| {
                    if let Ok(entity_view) = re_query::query_entity_with_primary::<Tensor>(
                        &ctx.log_db.entity_db.arrow_store,
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

                    space_views.push(single_image_space_view);
                }

                // Only if all images have the same size, so we _also_ want to create the stacked version (e.g. rgb + segmentation)
                // TODO(andreas): What if there's also other entities that we want to show?
                if image_sizes.len() > 1 {
                    continue;
                }
            }
        }

        space_views.push(space_view_candidate);
    }

    space_views
}

/// List of entities a space view queries by default for a given category.
///
/// These are all entities in the given space which have the requested category and are reachable by a transform.
pub fn default_queries_entities(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
    space_info: &SpaceInfo,
    category: ViewCategory,
) -> Vec<EntityPath> {
    crate::profile_function!();

    let timeline = Timeline::log_time();
    let log_db = &ctx.log_db;

    let mut entities = Vec::new();

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        entities.extend(
            space_info
                .descendants_without_transform
                .iter()
                .filter(|entity_path| {
                    categorize_entity_path(timeline, log_db, entity_path).contains(category)
                })
                .cloned(),
        );
    });

    entities
}

/// List of entities a space view queries by default for all any possible category.
fn default_queries_entities_by_category(
    ctx: &ViewerContext<'_>,
    spaces_info: &SpaceInfoCollection,
    space_info: &SpaceInfo,
) -> BTreeMap<ViewCategory, Vec<EntityPath>> {
    crate::profile_function!();

    let timeline = Timeline::log_time();
    let log_db = &ctx.log_db;

    let mut groups: BTreeMap<ViewCategory, Vec<EntityPath>> = BTreeMap::default();

    space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
        for entity_path in &space_info.descendants_without_transform {
            for category in categorize_entity_path(timeline, log_db, entity_path) {
                groups
                    .entry(category)
                    .or_default()
                    .push(entity_path.clone());
            }
        }
    });

    groups
}
