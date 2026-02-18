//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

mod arrows2d;
mod arrows3d;
mod assets3d;
mod boxes2d;
mod boxes3d;
mod cameras;
mod capsules3d;
mod cylinders3d;
mod depth_images;
mod ellipsoids;
mod encoded_depth_image;
mod encoded_image;
mod images;
mod lines2d;
mod lines3d;
mod meshes;
mod points2d;
mod points3d;
mod segmentation_images;
mod transform_axes_3d;
pub mod utilities;
mod video;

pub use cameras::CamerasVisualizer;
pub use depth_images::{DepthImageProcessResult, DepthImageVisualizer};
pub use encoded_depth_image::EncodedDepthImageVisualizer;
use re_sdk_types::{ComponentDescriptor, archetypes};
pub use transform_axes_3d::{TransformAxes3DVisualizer, add_axis_arrows};
pub use utilities::{
    SpatialViewVisualizerData, TexturedRectParams, UiLabel, UiLabelStyle, UiLabelTarget,
    entity_iterator, process_labels_3d, textured_rect_from_image,
};

/// Shows a loading animation in a spatial view.
///
/// Represents a 2D rectangle, oriented somewhere in scene coordinates.
#[derive(Clone, Debug)]
pub struct LoadingIndicator {
    pub center: glam::Vec3,

    /// The "radius" along one local axis.
    pub half_extent_u: glam::Vec3,

    /// The "radius" along the other local axis.
    pub half_extent_v: glam::Vec3,

    /// Why we are loading. Shown on hover in debug builds.
    pub reason: String,
}

// ---

use ahash::HashMap;
use re_entity_db::EntityPath;
use re_sdk_types::datatypes::{KeypointId, KeypointPair};
use re_view::clamped_or_nothing;
use re_viewer_context::{
    Annotations, IdentifiedViewSystem as _, ViewClassRegistryError, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewSystemRegistrator, VisualizerCollection, auto_color_egui,
};

/// Collection of keypoints for annotation context.
pub type Keypoints =
    HashMap<(re_sdk_types::components::ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

pub fn register_2d_spatial_visualizers(
    system_registry: &mut ViewSystemRegistrator<'_>,
) -> Result<(), ViewClassRegistryError> {
    // Note: 2D spatial systems don't include cameras as this
    // visualizer only shows a 2D projection WITHIN a 3D view.
    system_registry.register_visualizer::<arrows2d::Arrows2DVisualizer>()?;
    system_registry.register_visualizer::<arrows3d::Arrows3DVisualizer>()?;
    system_registry.register_visualizer::<assets3d::Asset3DVisualizer>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DVisualizer>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DVisualizer>()?;
    system_registry.register_visualizer::<depth_images::DepthImageVisualizer>()?;
    system_registry.register_visualizer::<ellipsoids::Ellipsoids3DVisualizer>()?;
    system_registry.register_visualizer::<encoded_depth_image::EncodedDepthImageVisualizer>()?;
    system_registry.register_visualizer::<encoded_image::EncodedImageVisualizer>()?;
    system_registry.register_visualizer::<images::ImageVisualizer>()?;
    system_registry.register_visualizer::<lines2d::Lines2DVisualizer>()?;
    system_registry.register_visualizer::<lines3d::Lines3DVisualizer>()?;
    system_registry.register_visualizer::<meshes::Mesh3DVisualizer>()?;
    system_registry.register_visualizer::<points2d::Points2DVisualizer>()?;
    system_registry.register_visualizer::<points3d::Points3DVisualizer>()?;
    system_registry.register_visualizer::<segmentation_images::SegmentationImageVisualizer>()?;
    system_registry.register_visualizer::<transform_axes_3d::TransformAxes3DVisualizer>()?;
    system_registry.register_visualizer::<video::VideoFrameReferenceVisualizer>()?;
    system_registry.register_visualizer::<video::VideoStreamVisualizer>()?;
    Ok(())
}

pub fn register_3d_spatial_visualizers(
    system_registry: &mut ViewSystemRegistrator<'_>,
) -> Result<(), ViewClassRegistryError> {
    system_registry.register_visualizer::<arrows2d::Arrows2DVisualizer>()?;
    system_registry.register_visualizer::<arrows3d::Arrows3DVisualizer>()?;
    system_registry.register_visualizer::<assets3d::Asset3DVisualizer>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DVisualizer>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DVisualizer>()?;
    system_registry.register_visualizer::<cameras::CamerasVisualizer>()?;
    system_registry.register_visualizer::<capsules3d::Capsules3DVisualizer>()?;
    system_registry.register_visualizer::<cylinders3d::Cylinders3DVisualizer>()?;
    system_registry.register_visualizer::<depth_images::DepthImageVisualizer>()?;
    system_registry.register_visualizer::<ellipsoids::Ellipsoids3DVisualizer>()?;
    system_registry.register_visualizer::<encoded_depth_image::EncodedDepthImageVisualizer>()?;
    system_registry.register_visualizer::<encoded_image::EncodedImageVisualizer>()?;
    system_registry.register_visualizer::<images::ImageVisualizer>()?;
    system_registry.register_visualizer::<lines2d::Lines2DVisualizer>()?;
    system_registry.register_visualizer::<lines3d::Lines3DVisualizer>()?;
    system_registry.register_visualizer::<meshes::Mesh3DVisualizer>()?;
    system_registry.register_visualizer::<points2d::Points2DVisualizer>()?;
    system_registry.register_visualizer::<points3d::Points3DVisualizer>()?;
    system_registry.register_visualizer::<segmentation_images::SegmentationImageVisualizer>()?;
    system_registry.register_visualizer::<transform_axes_3d::TransformAxes3DVisualizer>()?;
    system_registry.register_visualizer::<video::VideoFrameReferenceVisualizer>()?;
    system_registry.register_visualizer::<video::VideoStreamVisualizer>()?;
    Ok(())
}

/// List of all visualizers that read [`re_sdk_types::components::DrawOrder`] and the exact draw order component descriptor they use.
// TODO(jan, andreas): consider adding DrawOrder to video
pub fn visualizers_processing_draw_order()
-> impl Iterator<Item = (ViewSystemIdentifier, ComponentDescriptor)> {
    [
        (
            arrows2d::Arrows2DVisualizer::identifier(),
            archetypes::Arrows2D::descriptor_draw_order(),
        ),
        (
            boxes2d::Boxes2DVisualizer::identifier(),
            archetypes::Boxes2D::descriptor_draw_order(),
        ),
        (
            depth_images::DepthImageVisualizer::identifier(),
            archetypes::DepthImage::descriptor_draw_order(),
        ),
        (
            encoded_depth_image::EncodedDepthImageVisualizer::identifier(),
            archetypes::EncodedDepthImage::descriptor_draw_order(),
        ),
        (
            encoded_image::EncodedImageVisualizer::identifier(),
            archetypes::EncodedImage::descriptor_draw_order(),
        ),
        (
            images::ImageVisualizer::identifier(),
            archetypes::Image::descriptor_draw_order(),
        ),
        (
            lines2d::Lines2DVisualizer::identifier(),
            archetypes::LineStrips2D::descriptor_draw_order(),
        ),
        (
            points2d::Points2DVisualizer::identifier(),
            archetypes::Points2D::descriptor_draw_order(),
        ),
        (
            segmentation_images::SegmentationImageVisualizer::identifier(),
            archetypes::SegmentationImage::descriptor_draw_order(),
        ),
        (
            video::VideoFrameReferenceVisualizer::identifier(),
            archetypes::VideoFrameReference::descriptor_draw_order(),
        ),
        (
            video::VideoStreamVisualizer::identifier(),
            archetypes::VideoStream::descriptor_draw_order(),
        ),
    ]
    .into_iter()
}

pub fn collect_ui_labels(visualizers: &VisualizerCollection) -> Vec<UiLabel> {
    visualizers
        .iter_visualizer_data::<SpatialViewVisualizerData>()
        .flat_map(|data| data.ui_labels.iter().cloned())
        .collect()
}

/// Process [`re_sdk_types::components::Radius`] components to [`re_renderer::Size`] using auto size
/// where no radius is specified.
pub fn process_radius_slice(
    entity_path: &EntityPath,
    num_instances: usize,
    radii: &[re_sdk_types::components::Radius],
    fallback_radius: re_sdk_types::components::Radius,
) -> Vec<re_renderer::Size> {
    re_tracing::profile_function!();

    if let Some(last_radius) = radii.last() {
        if radii.len() == num_instances {
            // Common happy path
            radii
                .iter()
                .map(|radius| process_radius(entity_path, *radius))
                .collect()
        } else if radii.len() == 1 {
            // Common happy path
            let last_radius = process_radius(entity_path, *last_radius);
            vec![last_radius; num_instances]
        } else {
            clamped_or_nothing(radii, num_instances)
                .map(|radius| process_radius(entity_path, *radius))
                .collect()
        }
    } else {
        vec![re_renderer::Size(*fallback_radius.0); num_instances]
    }
}

fn process_radius(
    entity_path: &EntityPath,
    radius: re_sdk_types::components::Radius,
) -> re_renderer::Size {
    if radius.0.is_infinite() {
        re_log::warn_once!("Found infinite radius in entity {entity_path}");
    } else if radius.0.is_nan() {
        re_log::warn_once!("Found NaN radius in entity {entity_path}");
    }

    re_renderer::Size(*radius.0)
}

pub fn load_keypoint_connections(
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    annotations: &Annotations,
    world_from_obj: glam::Affine3A,
    ent_path: &re_entity_db::EntityPath,
    keypoints: &Keypoints,
) -> Result<(), ViewSystemExecutionError> {
    re_tracing::profile_function!();

    // TODO(andreas): We should be able to compute this already when we load the keypoints
    // in `process_annotation_and_keypoint_slices`
    let max_num_connections = keypoints
        .iter()
        .map(|((class_id, _time), _keypoints_in_class)| {
            annotations
                .resolved_class_description(Some(*class_id))
                .class_description
                .map_or(0, |d| d.keypoint_connections.len())
        })
        .sum();
    if max_num_connections == 0 {
        return Ok(());
    }

    // Generate keypoint connections if any.
    line_builder.reserve_strips(max_num_connections)?;
    line_builder.reserve_vertices(max_num_connections * 2)?;

    // The calling visualizer has the same issue of not knowing what do with per instance transforms
    // and should have warned already if there are multiple transforms.
    let mut line_batch = line_builder
        .batch("keypoint connections")
        .world_from_obj(world_from_obj)
        .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

    // TODO(andreas): Make configurable. Should we pick up the point's radius and make this proportional?
    let line_radius = re_renderer::Size(*re_sdk_types::components::Radius::default().0);

    for ((class_id, _time), keypoints_in_class) in keypoints {
        let resolved_class_description = annotations.resolved_class_description(Some(*class_id));

        let Some(class_description) = resolved_class_description.class_description else {
            continue;
        };

        let color = class_description.info.color.map_or_else(
            || auto_color_egui(class_description.info.id),
            |color| color.into(),
        );

        for KeypointPair {
            keypoint0: a,
            keypoint1: b,
        } in &class_description.keypoint_connections
        {
            let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b)) else {
                re_log::warn_once!(
                    "Keypoint connection from index {a:?} to {b:?} could not be resolved in entity {ent_path:?}"
                );
                continue;
            };
            line_batch
                .add_segment(*a, *b)
                .radius(line_radius)
                .color(color)
                .flags(re_renderer::renderer::LineStripFlags::FLAG_COLOR_GRADIENT)
                // Select the entire object when clicking any of the lines.
                .picking_instance_id(re_renderer::PickingLayerInstanceId(
                    re_log_types::Instance::ALL.get(),
                ));
        }
    }

    Ok(())
}

pub fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
