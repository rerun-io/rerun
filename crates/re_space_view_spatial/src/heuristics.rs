use std::collections::BTreeSet;

use egui::NumExt as _;

use re_data_store::EditableAutoValue;
use re_data_ui::image_meaning_for_entity;
use re_log_types::EntityPath;
use re_types::{
    components::{DepthMeter, TensorData},
    tensor_data::TensorDataMeaning,
    Archetype as _,
};
use re_viewer_context::{
    AutoSpawnHeuristic, IdentifiedViewSystem, PerSystemEntities, SpaceViewClassIdentifier,
    ViewerContext,
};

use crate::{
    parts::{CamerasPart, ImagesPart, SpatialViewPartData, Transform3DArrowsPart},
    query_pinhole,
    view_kind::SpatialSpaceViewKind,
};

pub fn auto_spawn_heuristic(
    class: &SpaceViewClassIdentifier,
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    view_kind: SpatialSpaceViewKind,
) -> AutoSpawnHeuristic {
    re_tracing::profile_function!();

    let mut score = 0.0;

    let parts = ctx
        .space_view_class_registry
        .get_system_registry_or_log_error(class)
        .new_part_collection();

    // Gather all systems that advertise a "preferred view kind" matching the passed in kind.
    let system_names_with_matching_view_kind = parts
        .iter_with_identifiers()
        .filter_map(|(name, part)| {
            part.data()
                .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
                .map_or(false, |data| data.preferred_view_kind == Some(view_kind))
                .then_some(name)
        })
        .collect::<Vec<_>>();

    // For each of the system with the matching "preferred view kind", count the entities involved.
    // We tally this up for scoring.
    for system_name in system_names_with_matching_view_kind {
        if per_system_entities
            .get(&system_name)
            .map_or(false, |c| !c.is_empty())
        {
            score += 1.0;
        }
    }

    AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score)
}

pub fn update_object_property_heuristics(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
    spatial_kind: SpatialSpaceViewKind,
) {
    re_tracing::profile_function!();

    // Do pinhole properties before, since they may be used in transform3d heuristics.
    update_pinhole_property_heuristics(per_system_entities, entity_properties, scene_bbox_accum);
    update_depth_cloud_property_heuristics(
        ctx,
        per_system_entities,
        entity_properties,
        spatial_kind,
    );
    update_transform3d_lines_heuristics(
        ctx,
        per_system_entities,
        entity_properties,
        scene_bbox_accum,
    );
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
    per_system_entities: &PerSystemEntities,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    for ent_path in per_system_entities
        .get(&CamerasPart::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        let mut properties = entity_properties.get(ent_path);
        if properties.pinhole_image_plane_distance.is_auto() {
            let scene_size = scene_bbox_accum.size().length();
            let default_image_plane_distance = if scene_size.is_finite() && scene_size > 0.0 {
                scene_size * 0.02 // Works pretty well for `examples/python/open_photogrammetry_format/main.py --no-frames`
            } else {
                // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
                // the heuristic will change it or it will be user edited. In the case of non-defined bounds
                // this value works better with the default camera setup.
                0.3
            };
            properties.pinhole_image_plane_distance =
                EditableAutoValue::Auto(default_image_plane_distance);
            entity_properties.update(ent_path.clone(), properties);
        }
    }
}

fn update_depth_cloud_property_heuristics(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    spatial_kind: SpatialSpaceViewKind,
) {
    // TODO(andreas): There should be a depth cloud system
    for ent_path in per_system_entities
        .get(&ImagesPart::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        let store = ctx.store_db.store();
        let Some(tensor) =
            store.query_latest_component::<TensorData>(ent_path, &ctx.current_query())
        else {
            continue;
        };

        let meaning = image_meaning_for_entity(ent_path, ctx);

        let meter = store
            .query_latest_component::<DepthMeter>(ent_path, &ctx.current_query())
            .map(|meter| meter.value.0);

        let mut properties = entity_properties.get(ent_path);
        if properties.backproject_depth.is_auto() {
            properties.backproject_depth = EditableAutoValue::Auto(
                meaning == TensorDataMeaning::Depth && spatial_kind == SpatialSpaceViewKind::ThreeD,
            );
        }

        if meaning == TensorDataMeaning::Depth {
            if properties.depth_from_world_scale.is_auto() {
                let auto = meter.unwrap_or_else(|| {
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

            entity_properties.update(ent_path.clone(), properties);
        }
    }
}

fn update_transform3d_lines_heuristics(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    entity_properties: &mut re_data_store::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    for ent_path in per_system_entities
        .get(&Transform3DArrowsPart::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        fn is_pinhole_extrinsics_of<'a>(
            store: &re_arrow_store::DataStore,
            ent_path: &'a EntityPath,
            ctx: &'a ViewerContext<'_>,
        ) -> Option<&'a EntityPath> {
            if query_pinhole(store, &ctx.current_query(), ent_path).is_some() {
                return Some(ent_path);
            } else {
                // Any direct child has a pinhole camera?
                if let Some(child_tree) = ctx.store_db.tree().subtree(ent_path) {
                    for child in child_tree.children.values() {
                        if query_pinhole(store, &ctx.current_query(), &child.path).is_some() {
                            return Some(&child.path);
                        }
                    }
                }
            }

            None
        }

        let mut properties = entity_properties.get(ent_path);
        if properties.transform_3d_visible.is_auto() {
            // By default show the transform if it is a camera extrinsic,
            // or if this entity only contains Transform3D components.
            let only_has_transform_components = ctx
                .store_db
                .store()
                .all_components(&ctx.current_query().timeline, ent_path)
                .map_or(false, |c| {
                    c.iter()
                        .all(|c| re_types::archetypes::Transform3D::all_components().contains(c))
                });
            properties.transform_3d_visible = EditableAutoValue::Auto(
                only_has_transform_components
                    || is_pinhole_extrinsics_of(ctx.store_db.store(), ent_path, ctx).is_some(),
            );
        }

        if properties.transform_3d_size.is_auto() {
            if let Some(pinhole_path) =
                is_pinhole_extrinsics_of(ctx.store_db.store(), ent_path, ctx)
            {
                // If there's a pinhole, we orient ourselves on its image plane distance
                let pinhole_path_props = entity_properties.get(pinhole_path);
                properties.transform_3d_size = EditableAutoValue::Auto(
                    *pinhole_path_props.pinhole_image_plane_distance * 0.25,
                );
            } else {
                // Size should be proportional to the scene extent, here covered by its diagonal
                let diagonal_length = (scene_bbox_accum.max - scene_bbox_accum.min).length();
                properties.transform_3d_size = EditableAutoValue::Auto(diagonal_length * 0.05);
            }
        }

        entity_properties.update(ent_path.clone(), properties);
    }
}
