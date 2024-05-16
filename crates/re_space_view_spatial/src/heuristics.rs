use std::collections::BTreeSet;

use egui::NumExt as _;
use nohash_hasher::IntSet;

use re_data_ui::image_meaning_for_entity;
use re_entity_db::EditableAutoValue;
use re_log_types::EntityPath;
use re_types::{
    components::{DepthMeter, TensorData},
    tensor_data::TensorDataMeaning,
    Archetype as _, SpaceViewClassIdentifier,
};
use re_viewer_context::{IdentifiedViewSystem, PerSystemEntities, ViewerContext};

use crate::{
    query_pinhole,
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        CamerasVisualizer, ImageVisualizer, SpatialViewVisualizerData, Transform3DArrowsVisualizer,
    },
};

pub fn generate_auto_legacy_properties(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    scene_bbox_accum: &macaw::BoundingBox,
    spatial_kind: SpatialSpaceViewKind,
) -> re_entity_db::EntityPropertyMap {
    re_tracing::profile_function!();

    let mut auto_properties = re_entity_db::EntityPropertyMap::default();

    // Do pinhole properties before, since they may be used in transform3d heuristics.
    update_pinhole_property_heuristics(per_system_entities, &mut auto_properties, scene_bbox_accum);
    update_depth_cloud_property_heuristics(
        ctx,
        per_system_entities,
        &mut auto_properties,
        spatial_kind,
    );
    update_transform3d_lines_heuristics(
        ctx,
        per_system_entities,
        &mut auto_properties,
        scene_bbox_accum,
    );

    auto_properties
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
    auto_properties: &mut re_entity_db::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    for ent_path in per_system_entities
        .get(&CamerasVisualizer::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        let mut properties = auto_properties.get(ent_path);

        let scene_size = scene_bbox_accum.size().length();
        let default_image_plane_distance = if scene_size.is_finite() && scene_size > 0.0 {
            scene_size * 0.02 // Works pretty well for `examples/python/open_photogrammetry_format/open_photogrammetry_format.py --no-frames`
        } else {
            // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
            // the heuristic will change it or it will be user edited. In the case of non-defined bounds
            // this value works better with the default camera setup.
            0.3
        };
        properties.pinhole_image_plane_distance =
            EditableAutoValue::Auto(default_image_plane_distance);
        auto_properties.overwrite_properties(ent_path.clone(), properties);
    }
}

fn update_depth_cloud_property_heuristics(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    auto_properties: &mut re_entity_db::EntityPropertyMap,
    spatial_kind: SpatialSpaceViewKind,
) {
    // TODO(andreas): There should be a depth cloud system
    for ent_path in per_system_entities
        .get(&ImageVisualizer::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        // TODO(#5607): what should happen if the promise is still pending?
        let Some(tensor) = ctx
            .recording()
            .latest_at_component::<TensorData>(ent_path, &ctx.current_query())
        else {
            continue;
        };

        let meaning =
            image_meaning_for_entity(ent_path, &ctx.current_query(), ctx.recording().store());

        // TODO(#5607): what should happen if the promise is still pending?
        let meter = ctx
            .recording()
            .latest_at_component::<DepthMeter>(ent_path, &ctx.current_query())
            .map(|meter| meter.value.0);

        let mut properties = auto_properties.get(ent_path);
        properties.backproject_depth = EditableAutoValue::Auto(
            meaning == TensorDataMeaning::Depth && spatial_kind == SpatialSpaceViewKind::ThreeD,
        );

        if meaning == TensorDataMeaning::Depth {
            let auto = meter.unwrap_or_else(|| {
                if tensor.dtype().is_integer() {
                    1000.0
                } else {
                    1.0
                }
            });
            properties.depth_from_world_scale = EditableAutoValue::Auto(auto);
            properties.backproject_radius_scale = EditableAutoValue::Auto(1.0);

            auto_properties.overwrite_properties(ent_path.clone(), properties);
        }
    }
}

fn update_transform3d_lines_heuristics(
    ctx: &ViewerContext<'_>,
    per_system_entities: &PerSystemEntities,
    auto_properties: &mut re_entity_db::EntityPropertyMap,
    scene_bbox_accum: &macaw::BoundingBox,
) {
    for ent_path in per_system_entities
        .get(&Transform3DArrowsVisualizer::identifier())
        .unwrap_or(&BTreeSet::new())
    {
        fn is_pinhole_extrinsics_of<'a>(
            ent_path: &'a EntityPath,
            ctx: &'a ViewerContext<'_>,
        ) -> Option<&'a EntityPath> {
            if query_pinhole(ctx.recording(), &ctx.current_query(), ent_path).is_some() {
                return Some(ent_path);
            } else {
                // Any direct child has a pinhole camera?
                if let Some(child_tree) = ctx.recording().tree().subtree(ent_path) {
                    for child in child_tree.children.values() {
                        if query_pinhole(ctx.recording(), &ctx.current_query(), &child.path)
                            .is_some()
                        {
                            return Some(&child.path);
                        }
                    }
                }
            }

            None
        }

        let mut properties = auto_properties.get(ent_path);

        // By default show the transform if it is a camera extrinsic,
        // or if this entity only contains Transform3D components.
        let only_has_transform_components = ctx
            .recording_store()
            .all_components(&ctx.current_query().timeline(), ent_path)
            .map_or(false, |c| {
                c.iter()
                    .all(|c| re_types::archetypes::Transform3D::all_components().contains(c))
            });
        properties.transform_3d_visible = EditableAutoValue::Auto(
            only_has_transform_components || is_pinhole_extrinsics_of(ent_path, ctx).is_some(),
        );

        if let Some(pinhole_path) = is_pinhole_extrinsics_of(ent_path, ctx) {
            // If there's a pinhole, we orient ourselves on its image plane distance
            let pinhole_path_props = auto_properties.get(pinhole_path);
            properties.transform_3d_size =
                EditableAutoValue::Auto(*pinhole_path_props.pinhole_image_plane_distance * 0.25);
        } else {
            // Size should be proportional to the scene extent, here covered by its diagonal
            let diagonal_length = (scene_bbox_accum.max - scene_bbox_accum.min).length();
            properties.transform_3d_size = EditableAutoValue::Auto(diagonal_length * 0.05);
        }

        auto_properties.overwrite_properties(ent_path.clone(), properties);
    }
}

/// Returns all entities for which a visualizer of the given kind would be picked.
///
/// I.e. all entities for which at least one visualizer of the specified kind is applicable
/// *and* has a matching indicator component.
pub fn default_visualized_entities_for_visualizer_kind(
    ctx: &ViewerContext<'_>,
    space_view_class_identifier: SpaceViewClassIdentifier,
    visualizer_kind: SpatialSpaceViewKind,
) -> IntSet<EntityPath> {
    re_tracing::profile_function!();

    ctx.space_view_class_registry
        .new_visualizer_collection(space_view_class_identifier)
        .iter_with_identifiers()
        .filter_map(|(id, visualizer)| {
            let data = visualizer
                .data()?
                .downcast_ref::<SpatialViewVisualizerData>()?;

            if data.preferred_view_kind == Some(visualizer_kind) {
                let indicator_matching = ctx.indicated_entities_per_visualizer.get(&id)?;
                let applicable = ctx.applicable_entities_per_visualizer.get(&id)?;
                Some(indicator_matching.intersection(applicable))
            } else {
                None
            }
        })
        .flatten()
        .cloned()
        .collect()
}
