//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

mod arrows2d;
mod arrows3d;
mod assets3d;
mod boxes2d;
mod boxes3d;
mod cameras;
mod entity_iterator;
mod images;
mod lines2d;
mod lines3d;
mod meshes;
mod points2d;
mod points3d;
mod spatial_view_visualizer;
mod transform3d_arrows;

pub use cameras::CamerasVisualizer;
pub use images::{ImageVisualizer, ViewerImage};
pub use spatial_view_visualizer::SpatialViewVisualizerData;
pub use transform3d_arrows::{add_axis_arrows, Transform3DArrowsVisualizer};

#[doc(hidden)] // Public for benchmarks
pub use points3d::{LoadedPoints, Points3DComponentData};

use ahash::HashMap;

use re_entity_db::{EntityPath, InstancePathHash};
use re_renderer::{LineDrawableBuilder, QueueableDrawData};
use re_types::{
    components::{Color, InstanceKey},
    datatypes::{KeypointId, KeypointPair},
};
use re_viewer_context::{
    auto_color, Annotations, ApplicableEntities, DefaultColor, ResolvedAnnotationInfos,
    SpaceViewClassRegistryError, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
    VisualizableEntities, VisualizableFilterContext, VisualizerCollection,
};

use crate::space_view_2d::VisualizableFilterContext2D;
use crate::space_view_3d::VisualizableFilterContext3D;

use super::contexts::SpatialSceneEntityContext;

/// Collection of keypoints for annotation context.
pub type Keypoints = HashMap<(re_types::components::ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

// TODO(andreas): It would be nice if these wouldn't need to be set on every single line/point builder.
pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
pub const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

pub fn register_2d_spatial_visualizers(
    system_registry: &mut SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    // Note: 2D spatial systems don't include cameras as this
    // visualizer only shows a 2D projection WITHIN a 3D view.
    system_registry.register_visualizer::<arrows3d::Arrows3DVisualizer>()?;
    system_registry.register_visualizer::<arrows2d::Arrows2DVisualizer>()?;
    system_registry.register_visualizer::<assets3d::Asset3DVisualizer>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DVisualizer>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DVisualizer>()?;
    system_registry.register_visualizer::<images::ImageVisualizer>()?;
    system_registry.register_visualizer::<lines2d::Lines2DVisualizer>()?;
    system_registry.register_visualizer::<lines3d::Lines3DVisualizer>()?;
    system_registry.register_visualizer::<meshes::Mesh3DVisualizer>()?;
    system_registry.register_visualizer::<points2d::Points2DVisualizer>()?;
    system_registry.register_visualizer::<points3d::Points3DVisualizer>()?;
    system_registry.register_visualizer::<transform3d_arrows::Transform3DArrowsVisualizer>()?;
    Ok(())
}

pub fn register_3d_spatial_visualizers(
    system_registry: &mut SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    system_registry.register_visualizer::<arrows3d::Arrows3DVisualizer>()?;
    system_registry.register_visualizer::<arrows2d::Arrows2DVisualizer>()?;
    system_registry.register_visualizer::<assets3d::Asset3DVisualizer>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DVisualizer>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DVisualizer>()?;
    system_registry.register_visualizer::<cameras::CamerasVisualizer>()?;
    system_registry.register_visualizer::<images::ImageVisualizer>()?;
    system_registry.register_visualizer::<lines2d::Lines2DVisualizer>()?;
    system_registry.register_visualizer::<lines3d::Lines3DVisualizer>()?;
    system_registry.register_visualizer::<meshes::Mesh3DVisualizer>()?;
    system_registry.register_visualizer::<points2d::Points2DVisualizer>()?;
    system_registry.register_visualizer::<points3d::Points3DVisualizer>()?;
    system_registry.register_visualizer::<transform3d_arrows::Transform3DArrowsVisualizer>()?;
    Ok(())
}

pub fn collect_ui_labels(visualizers: &VisualizerCollection) -> Vec<UiLabel> {
    let mut ui_labels = Vec::new();
    for visualizer in visualizers.iter() {
        if let Some(data) = visualizer
            .data()
            .and_then(|d| d.downcast_ref::<SpatialViewVisualizerData>())
        {
            ui_labels.extend(data.ui_labels.iter().cloned());
        }
    }
    ui_labels
}

pub fn picking_id_from_instance_key(
    instance_key: InstanceKey,
) -> re_renderer::PickingLayerInstanceId {
    re_renderer::PickingLayerInstanceId(instance_key.0)
}

/// Process [`Color`] components using annotations and default colors.
pub fn process_color_slice<'a>(
    colors: Option<&'a [Option<Color>]>,
    ent_path: &'a EntityPath,
    annotation_infos: &'a ResolvedAnnotationInfos,
) -> Vec<egui::Color32> {
    // This can be rather slow for colors with transparency, since we need to pre-multiply the alpha.
    re_tracing::profile_function!();

    let default_color = DefaultColor::EntityPath(ent_path);

    match (colors, annotation_infos) {
        (None, ResolvedAnnotationInfos::Same(count, annotation_info)) => {
            re_tracing::profile_scope!("no colors, same annotation");
            let color = annotation_info.color(None, default_color);
            vec![color; *count]
        }

        (None, ResolvedAnnotationInfos::Many(annotation_infos)) => {
            re_tracing::profile_scope!("no-colors, many annotations");
            annotation_infos
                .iter()
                .map(|annotation_info| annotation_info.color(None, default_color))
                .collect()
        }

        (Some(colors), ResolvedAnnotationInfos::Same(count, annotation_info)) => {
            re_tracing::profile_scope!("many-colors, same annotation");
            debug_assert_eq!(colors.len(), *count);
            colors
                .iter()
                .map(|color| annotation_info.color(color.map(|c| c.to_array()), default_color))
                .collect()
        }

        (Some(colors), ResolvedAnnotationInfos::Many(annotation_infos)) => {
            re_tracing::profile_scope!("many-colors, many annotations");
            colors
                .iter()
                .zip(annotation_infos.iter())
                .map(move |(color, annotation_info)| {
                    annotation_info.color(color.map(|c| c.to_array()), default_color)
                })
                .collect()
        }
    }
}

/// Process `Text` components using annotations.
pub fn process_label_slice(
    labels: Option<&[Option<re_types::components::Text>]>,
    default_len: usize,
    annotation_infos: &ResolvedAnnotationInfos,
) -> Vec<Option<String>> {
    re_tracing::profile_function!();

    match labels {
        None => vec![None; default_len],
        Some(labels) => itertools::izip!(annotation_infos.iter(), labels)
            .map(move |(annotation_info, text)| {
                annotation_info.label(text.as_ref().map(|t| t.as_str()))
            })
            .collect(),
    }
}

/// Process [`re_types::components::Radius`] components to [`re_renderer::Size`] using auto size
/// where no radius is specified.
pub fn process_radius_slice(
    radii: Option<&[Option<re_types::components::Radius>]>,
    default_len: usize,
    ent_path: &EntityPath,
) -> Vec<re_renderer::Size> {
    re_tracing::profile_function!();
    let ent_path = ent_path.clone();

    match radii {
        None => {
            vec![re_renderer::Size::AUTO; default_len]
        }
        Some(radii) => radii
            .iter()
            .map(|radius| process_radius(&ent_path, radius))
            .collect(),
    }
}

fn process_radius(
    entity_path: &EntityPath,
    radius: &Option<re_types::components::Radius>,
) -> re_renderer::Size {
    radius.map_or(re_renderer::Size::AUTO, |r| {
        if 0.0 <= r.0 && r.0.is_finite() {
            re_renderer::Size::new_scene(r.0)
        } else {
            if r.0 < 0.0 {
                re_log::warn_once!("Found negative radius in entity {entity_path}");
            } else if r.0.is_infinite() {
                re_log::warn_once!("Found infinite radius in entity {entity_path}");
            } else {
                re_log::warn_once!("Found NaN radius in entity {entity_path}");
            }
            re_renderer::Size::AUTO
        }
    })
}

/// Resolves all annotations and keypoints for the given entity view.
fn process_annotation_and_keypoint_slices(
    latest_at: re_log_types::TimeInt,
    instance_keys: &[InstanceKey],
    keypoint_ids: Option<&[Option<re_types::components::KeypointId>]>,
    class_ids: Option<&[Option<re_types::components::ClassId>]>,
    positions: impl Iterator<Item = glam::Vec3>,
    annotations: &Annotations,
) -> (ResolvedAnnotationInfos, Keypoints) {
    re_tracing::profile_function!();

    let mut keypoints: Keypoints = HashMap::default();

    // No need to process annotations if we don't have class-ids
    let Some(class_ids) = class_ids else {
        let resolved_annotation = annotations
            .resolved_class_description(None)
            .annotation_info();

        return (
            ResolvedAnnotationInfos::Same(instance_keys.len(), resolved_annotation),
            keypoints,
        );
    };

    if let Some(keypoint_ids) = keypoint_ids {
        let annotation_info = itertools::izip!(positions, keypoint_ids, class_ids)
            .map(|(positions, &keypoint_id, &class_id)| {
                let class_description = annotations.resolved_class_description(class_id);

                if let (Some(keypoint_id), Some(class_id), position) =
                    (keypoint_id, class_id, positions)
                {
                    keypoints
                        .entry((class_id, latest_at.as_i64()))
                        .or_default()
                        .insert(keypoint_id.0, position);
                    class_description.annotation_info_with_keypoint(keypoint_id.0)
                } else {
                    class_description.annotation_info()
                }
            })
            .collect();

        (ResolvedAnnotationInfos::Many(annotation_info), keypoints)
    } else {
        let annotation_info = class_ids
            .iter()
            .map(|&class_id| {
                let class_description = annotations.resolved_class_description(class_id);
                class_description.annotation_info()
            })
            .collect();

        (
            ResolvedAnnotationInfos::Many(annotation_info),
            Default::default(),
        )
    }
}

#[derive(Clone)]
pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
}

#[derive(Clone)]
pub struct UiLabel {
    pub text: String,
    pub color: egui::Color32,

    /// The shape/position being labeled.
    pub target: UiLabelTarget,

    /// What is hovered if this label is hovered.
    pub labeled_instance: InstancePathHash,
}

pub fn load_keypoint_connections(
    render_ctx: &re_renderer::RenderContext,
    ent_context: &SpatialSceneEntityContext<'_>,
    ent_path: &re_entity_db::EntityPath,
    keypoints: &Keypoints,
) -> Result<Option<QueueableDrawData>, SpaceViewSystemExecutionError> {
    re_tracing::profile_function!();

    // TODO(andreas): We should be able to compute this already when we load the keypoints
    // in `process_annotation_and_keypoint_slices`
    let max_num_connections = keypoints
        .iter()
        .map(|((class_id, _time), _keypoints_in_class)| {
            ent_context
                .annotations
                .resolved_class_description(Some(*class_id))
                .class_description
                .map_or(0, |d| d.keypoint_annotations.len() as u32)
        })
        .sum();
    if max_num_connections == 0 {
        return Ok(None);
    }

    // Generate keypoint connections if any.
    let mut line_builder =
        LineDrawableBuilder::new(render_ctx, max_num_connections, max_num_connections * 2);
    {
        let mut line_batch = line_builder
            .batch("keypoint connections")
            .world_from_obj(ent_context.world_from_entity)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        for ((class_id, _time), keypoints_in_class) in keypoints {
            let resolved_class_description = ent_context
                .annotations
                .resolved_class_description(Some(*class_id));

            let Some(class_description) = resolved_class_description.class_description else {
                continue;
            };

            let color = class_description.info.color.map_or_else(
                || auto_color(class_description.info.id),
                |color| color.into(),
            );

            for KeypointPair {
                keypoint0: a,
                keypoint1: b,
            } in &class_description.keypoint_connections
            {
                let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b))
                else {
                    re_log::warn_once!(
                    "Keypoint connection from index {:?} to {:?} could not be resolved in object {:?}",
                    a, b, ent_path
                );
                    continue;
                };
                line_batch
                    .add_segment(*a, *b)
                    .radius(re_renderer::Size::AUTO)
                    .color(color)
                    .flags(re_renderer::renderer::LineStripFlags::FLAG_COLOR_GRADIENT)
                    // Select the entire object when clicking any of the lines.
                    .picking_instance_id(re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0));
            }
        }
    }

    Ok(Some(line_builder.into_draw_data(render_ctx)?.into()))
}

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_types::components::ViewCoordinates {
    // Typical image spaces have
    // - x pointing right
    // - y pointing down
    // - z pointing into the image plane (this is convenient for reading out a depth image which has typically positive z values)
    re_types::components::ViewCoordinates::RDF
}

fn filter_visualizable_2d_entities(
    entities: ApplicableEntities,
    context: &dyn VisualizableFilterContext,
) -> VisualizableEntities {
    if let Some(context) = context
        .as_any()
        .downcast_ref::<VisualizableFilterContext2D>()
    {
        VisualizableEntities(
            context
                .entities_in_main_2d_space
                .intersection(&entities.0)
                .cloned()
                .collect(),
        )
    } else if let Some(context) = context
        .as_any()
        .downcast_ref::<VisualizableFilterContext3D>()
    {
        VisualizableEntities(
            context
                .entities_under_pinholes
                .intersection(&entities.0)
                .cloned()
                .collect(),
        )
    } else {
        VisualizableEntities(entities.0)
    }
}

fn filter_visualizable_3d_entities(
    entities: ApplicableEntities,
    context: &dyn VisualizableFilterContext,
) -> VisualizableEntities {
    if let Some(context) = context
        .as_any()
        .downcast_ref::<VisualizableFilterContext2D>()
    {
        VisualizableEntities(
            context
                .reprojectable_3d_entities
                .intersection(&entities.0)
                .cloned()
                .collect(),
        )
    } else if let Some(context) = context
        .as_any()
        .downcast_ref::<VisualizableFilterContext3D>()
    {
        VisualizableEntities(
            context
                .entities_in_main_3d_space
                .intersection(&entities.0)
                .cloned()
                .collect(),
        )
    } else {
        VisualizableEntities(entities.0)
    }
}
