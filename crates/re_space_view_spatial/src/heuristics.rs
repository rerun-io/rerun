use egui::NumExt as _;
use nohash_hasher::IntSet;

use re_components::{Pinhole, Tensor, TensorDataMeaning};
use re_data_store::EditableAutoValue;
use re_log_types::{EntityPath, Timeline};
use re_types::components::Transform3D;
use re_viewer_context::{AutoSpawnHeuristic, SpaceViewClassName, ViewerContext};

use crate::{parts::SpatialViewPartData, view_kind::SpatialSpaceViewKind};

pub fn auto_spawn_heuristic(
    class: &SpaceViewClassName,
    ctx: &ViewerContext<'_>,
    ent_paths: &IntSet<EntityPath>,
    view_kind: SpatialSpaceViewKind,
) -> AutoSpawnHeuristic {
    re_tracing::profile_function!();

    let store = ctx.store_db.store();
    let timeline = Timeline::log_time();

    let mut score = 0.0;

    let parts = ctx
        .space_view_class_registry
        .get_system_registry_or_log_error(class)
        .new_part_collection();
    let parts_with_view_kind = parts
        .iter()
        .filter(|part| {
            part.data()
                .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
                .map_or(false, |data| data.preferred_view_kind == Some(view_kind))
        })
        .collect::<Vec<_>>();

    for ent_path in ent_paths {
        let Some(components) = store.all_components(&timeline, ent_path) else {
            continue;
        };

        for part in &parts_with_view_kind {
            if part.queries_any_components_of(store, ent_path, &components) {
                score += 1.0;
                break;
            }
        }
    }

    if view_kind == SpatialSpaceViewKind::TwoD {
        // Prefer 2D views over 3D views.
        score += 0.1;
    }

    AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score)
}

pub fn update_object_property_heuristics(
    ctx: &mut ViewerContext<'_>,
    ent_paths: &IntSet<EntityPath>,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
    spatial_kind: SpatialSpaceViewKind,
) {
    re_tracing::profile_function!();

    for entity_path in ent_paths {
        // Do pinhole properties before, since they may be used in transform3d heuristics.
        update_pinhole_property_heuristics(ctx, entity_path, entity_properties, scene_bbox_accum);
        update_depth_cloud_property_heuristics(ctx, entity_path, entity_properties, spatial_kind);
        update_transform3d_lines_heuristics(ctx, entity_path, entity_properties, scene_bbox_accum);
    }
}

pub fn auto_size_world_heuristic(
    scene_bbox_accum: &macaw::BoundingBox,
    scene_num_primitives: usize,
) -> f32 {
    if scene_bbox_accum.is_nothing() || scene_bbox_accum.is_nan() {
        return 0.01;
    }

    // Motivation: Size should be proportional to the scene extent, here covered by its diagonal
    let diagonal_length = (scene_bbox_accum.max - scene_bbox_accum.min).length();
    let heuristic0 = diagonal_length * 0.0025;

    // Motivation: A lot of times we look at the entire scene and expect to see everything on a flat screen with some gaps between.
    let size = scene_bbox_accum.size();
    let mut sorted_components = size.to_array();
    sorted_components.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Median is more robust against outlier in one direction (as such pretty poor still though)
    let median_extent = sorted_components[1];
    // sqrt would make more sense but using a smaller root works better in practice.
    let heuristic1 =
        (median_extent / (scene_num_primitives.at_least(1) as f32).powf(1.0 / 1.7)) * 0.25;

    heuristic0.min(heuristic1)
}

fn update_pinhole_property_heuristics(
    ctx: &mut ViewerContext<'_>,
    entity_path: &EntityPath,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    let store = &ctx.store_db.entity_db.data_store;
    if store
        .query_latest_component::<Pinhole>(entity_path, &ctx.current_query())
        .is_some()
    {
        let mut properties = entity_properties.get(entity_path);
        if properties.pinhole_image_plane_distance.is_auto() {
            let scene_size = scene_bbox_accum.size().length();
            let default_image_plane_distance = if scene_size.is_finite() && scene_size > 0.0 {
                scene_size * 0.02 // Works pretty well for `examples/python/open_photogrammetry_format/main.py --no-frames`
            } else {
                1.0
            };
            properties.pinhole_image_plane_distance =
                EditableAutoValue::Auto(default_image_plane_distance);
            entity_properties.set(entity_path.clone(), properties);
        }
    }
}

fn update_depth_cloud_property_heuristics(
    ctx: &mut ViewerContext<'_>,
    entity_path: &EntityPath,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    spatial_kind: SpatialSpaceViewKind,
) -> Option<()> {
    let store = &ctx.store_db.entity_db.data_store;
    let tensor = store.query_latest_component::<Tensor>(entity_path, &ctx.current_query())?;

    let mut properties = entity_properties.get(entity_path);
    if properties.backproject_depth.is_auto() {
        properties.backproject_depth = EditableAutoValue::Auto(
            tensor.meaning == TensorDataMeaning::Depth
                && spatial_kind == SpatialSpaceViewKind::ThreeD,
        );
    }

    if tensor.meaning == TensorDataMeaning::Depth {
        if properties.depth_from_world_scale.is_auto() {
            let auto = tensor.meter.unwrap_or_else(|| {
                if tensor.dtype().is_integer() {
                    1000.0
                } else {
                    1.0
                }
            });
            properties.depth_from_world_scale = EditableAutoValue::Auto(auto);
        }

        if properties.backproject_radius_scale.is_auto() {
            properties.backproject_radius_scale = EditableAutoValue::Auto(1.0);
        }

        entity_properties.set(entity_path.clone(), properties);
    }

    Some(())
}

fn update_transform3d_lines_heuristics(
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    if ctx
        .store_db
        .store()
        .query_latest_component::<Transform3D>(ent_path, &ctx.current_query())
        .is_none()
    {
        return;
    }

    fn is_pinhole_extrinsics_of<'a>(
        store: &re_arrow_store::DataStore,
        ent_path: &'a EntityPath,
        ctx: &'a ViewerContext<'_>,
    ) -> Option<&'a EntityPath> {
        if store
            .query_latest_component::<Pinhole>(ent_path, &ctx.current_query())
            .is_some()
        {
            return Some(ent_path);
        } else {
            // Any direct child has a pinhole camera?
            if let Some(child_tree) = ctx.store_db.entity_db.tree.subtree(ent_path) {
                for child in child_tree.children.values() {
                    if store
                        .query_latest_component::<Pinhole>(&child.path, &ctx.current_query())
                        .is_some()
                    {
                        return Some(&child.path);
                    }
                }
            }
        }

        None
    }

    let mut properties = entity_properties.get(ent_path);
    if properties.transform_3d_visible.is_auto() {
        // By default show the transform if it is a camera extrinsic or if it's the only component on this entity path.
        let single_component = ctx
            .store_db
            .store()
            .all_components(&ctx.current_query().timeline, ent_path)
            .map_or(false, |c| c.len() == 1);
        properties.transform_3d_visible = EditableAutoValue::Auto(
            single_component
                || is_pinhole_extrinsics_of(ctx.store_db.store(), ent_path, ctx).is_some(),
        );
    }

    if properties.transform_3d_size.is_auto() {
        if let Some(pinhole_path) = is_pinhole_extrinsics_of(ctx.store_db.store(), ent_path, ctx) {
            // If there's a pinhole, we orient ourselves on its image plane distance
            let pinhole_path_props = entity_properties.get(pinhole_path);
            properties.transform_3d_size =
                EditableAutoValue::Auto(*pinhole_path_props.pinhole_image_plane_distance * 0.25);
        } else {
            // Size should be proportional to the scene extent, here covered by its diagonal
            let diagonal_length = (scene_bbox_accum.max - scene_bbox_accum.min).length();
            properties.transform_3d_size = EditableAutoValue::Auto(diagonal_length * 0.05);
        }
    }

    entity_properties.set(ent_path.clone(), properties);
}
