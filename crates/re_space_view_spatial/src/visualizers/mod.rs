//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

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

pub use cameras::CamerasPart;
pub use images::ImagesPart;
pub use images::ViewerImage;
use re_viewer_context::ApplicableEntities;
pub use spatial_view_visualizer::SpatialViewVisualizerData;
pub use transform3d_arrows::{add_axis_arrows, Transform3DArrowsPart};

#[doc(hidden)] // Public for benchmarks
pub use points3d::LoadedPoints;

use ahash::HashMap;

use re_data_store::{EntityPath, InstancePathHash};
use re_types::components::{Color, InstanceKey, Text};
use re_types::datatypes::{KeypointId, KeypointPair};
use re_types::Archetype;
use re_viewer_context::{
    auto_color, Annotations, DefaultColor, ResolvedAnnotationInfos, SpaceViewClassRegistryError,
    SpaceViewSystemRegistrator, ViewQuery, VisualizableEntities, VisualizerCollection,
};

use crate::space_view_3d::VisualizableFilterContext3D;

use super::contexts::SpatialSceneEntityContext;

/// Collection of keypoints for annotation context.
pub type Keypoints = HashMap<(re_types::components::ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
pub const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

pub fn register_2d_spatial_parts(
    system_registry: &mut SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    // Note: 2D spatial systems don't include cameras as this
    // part only shows a 2D projection WITHIN a 3D view.
    system_registry.register_visualizer::<arrows3d::Arrows3DPart>()?;
    system_registry.register_visualizer::<assets3d::Asset3DPart>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DPart>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DPart>()?;
    system_registry.register_visualizer::<images::ImagesPart>()?;
    system_registry.register_visualizer::<lines2d::Lines2DPart>()?;
    system_registry.register_visualizer::<lines3d::Lines3DPart>()?;
    system_registry.register_visualizer::<meshes::Mesh3DPart>()?;
    system_registry.register_visualizer::<points2d::Points2DPart>()?;
    system_registry.register_visualizer::<points3d::Points3DPart>()?;
    system_registry.register_visualizer::<transform3d_arrows::Transform3DArrowsPart>()?;
    Ok(())
}

pub fn register_3d_spatial_parts(
    system_registry: &mut SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    system_registry.register_visualizer::<arrows3d::Arrows3DPart>()?;
    system_registry.register_visualizer::<assets3d::Asset3DPart>()?;
    system_registry.register_visualizer::<boxes2d::Boxes2DPart>()?;
    system_registry.register_visualizer::<boxes3d::Boxes3DPart>()?;
    system_registry.register_visualizer::<cameras::CamerasPart>()?;
    system_registry.register_visualizer::<images::ImagesPart>()?;
    system_registry.register_visualizer::<lines2d::Lines2DPart>()?;
    system_registry.register_visualizer::<lines3d::Lines3DPart>()?;
    system_registry.register_visualizer::<meshes::Mesh3DPart>()?;
    system_registry.register_visualizer::<points2d::Points2DPart>()?;
    system_registry.register_visualizer::<points3d::Points3DPart>()?;
    system_registry.register_visualizer::<transform3d_arrows::Transform3DArrowsPart>()?;
    Ok(())
}

pub fn calculate_bounding_box(
    visualizers: &VisualizerCollection,
    bounding_box_accum: &mut macaw::BoundingBox,
) -> macaw::BoundingBox {
    let mut bounding_box = macaw::BoundingBox::nothing();
    for part in visualizers.iter() {
        if let Some(data) = part
            .data()
            .and_then(|d| d.downcast_ref::<SpatialViewVisualizerData>())
        {
            bounding_box = bounding_box.union(data.bounding_box);
        }
    }

    if bounding_box_accum.is_nothing() || !bounding_box_accum.size().is_finite() {
        *bounding_box_accum = bounding_box;
    } else {
        *bounding_box_accum = bounding_box_accum.union(bounding_box);
    }

    bounding_box
}

pub fn collect_ui_labels(visualizers: &VisualizerCollection) -> Vec<UiLabel> {
    let mut ui_labels = Vec::new();
    for part in visualizers.iter() {
        if let Some(data) = part
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
#[allow(dead_code)]
pub fn process_colors<'a, A: Archetype>(
    arch_view: &'a re_query::ArchetypeView<A>,
    ent_path: &'a EntityPath,
    annotation_infos: &'a ResolvedAnnotationInfos,
) -> Result<impl Iterator<Item = egui::Color32> + 'a, re_query::QueryError> {
    re_tracing::profile_function!();
    let default_color = DefaultColor::EntityPath(ent_path);

    Ok(itertools::izip!(
        annotation_infos.iter(),
        arch_view.iter_optional_component::<Color>()?,
    )
    .map(move |(annotation_info, color)| {
        annotation_info.color(color.map(|c| c.to_array()), default_color)
    }))
}

/// Process [`Text`] components using annotations.
#[allow(dead_code)]
pub fn process_labels<'a, A: Archetype>(
    arch_view: &'a re_query::ArchetypeView<A>,
    annotation_infos: &'a ResolvedAnnotationInfos,
) -> Result<impl Iterator<Item = Option<String>> + 'a, re_query::QueryError> {
    re_tracing::profile_function!();

    Ok(itertools::izip!(
        annotation_infos.iter(),
        arch_view.iter_optional_component::<Text>()?,
    )
    .map(move |(annotation_info, text)| annotation_info.label(text.as_ref().map(|t| t.as_str()))))
}

/// Process [`re_types::components::Radius`] components to [`re_renderer::Size`] using auto size
/// where no radius is specified.
pub fn process_radii<'a, A: Archetype>(
    arch_view: &'a re_query::ArchetypeView<A>,
    ent_path: &EntityPath,
) -> Result<impl Iterator<Item = re_renderer::Size> + 'a, re_query::QueryError> {
    re_tracing::profile_function!();
    let ent_path = ent_path.clone();
    Ok(arch_view
        .iter_optional_component::<re_types::components::Radius>()?
        .map(move |radius| {
            radius.map_or(re_renderer::Size::AUTO, |r| {
                if 0.0 <= r.0 && r.0.is_finite() {
                    re_renderer::Size::new_scene(r.0)
                } else {
                    if r.0 < 0.0 {
                        re_log::warn_once!("Found negative radius in entity {ent_path}");
                    } else if r.0.is_infinite() {
                        re_log::warn_once!("Found infinite radius in entity {ent_path}");
                    } else {
                        re_log::warn_once!("Found NaN radius in entity {ent_path}");
                    }
                    re_renderer::Size::AUTO
                }
            })
        }))
}

/// Resolves all annotations for the given entity view.
fn process_annotations<Primary, A: Archetype>(
    query: &ViewQuery<'_>,
    arch_view: &re_query::ArchetypeView<A>,
    annotations: &Annotations,
) -> Result<ResolvedAnnotationInfos, re_query::QueryError>
where
    Primary: re_types::Component + Clone,
{
    process_annotations_and_keypoints(query.latest_at, arch_view, annotations, |_: &Primary| {
        glam::Vec3::ZERO
    })
    .map(|(a, _)| a)
}

/// Resolves all annotations and keypoints for the given entity view.
fn process_annotations_and_keypoints<Primary, A: Archetype>(
    latest_at: re_log_types::TimeInt,
    arch_view: &re_query::ArchetypeView<A>,
    annotations: &Annotations,
    mut primary_into_position: impl FnMut(&Primary) -> glam::Vec3,
) -> Result<(ResolvedAnnotationInfos, Keypoints), re_query::QueryError>
where
    Primary: re_types::Component + Clone,
{
    re_tracing::profile_function!();

    let mut keypoints: Keypoints = HashMap::default();

    // No need to process annotations if we don't have keypoints or class-ids
    if !arch_view.has_component::<re_types::components::KeypointId>()
        && !arch_view.has_component::<re_types::components::ClassId>()
    {
        let resolved_annotation = annotations
            .resolved_class_description(None)
            .annotation_info();

        return Ok((
            ResolvedAnnotationInfos::Same(arch_view.num_instances(), resolved_annotation),
            keypoints,
        ));
    }

    let annotation_info = itertools::izip!(
        arch_view.iter_required_component::<Primary>()?,
        arch_view.iter_optional_component::<re_types::components::KeypointId>()?,
        arch_view.iter_optional_component::<re_types::components::ClassId>()?,
    )
    .map(|(primary, keypoint_id, class_id)| {
        let class_description = annotations.resolved_class_description(class_id);

        if let (Some(keypoint_id), Some(class_id), primary) = (keypoint_id, class_id, primary) {
            keypoints
                .entry((class_id, latest_at.as_i64()))
                .or_default()
                .insert(keypoint_id.0, primary_into_position(&primary));
            class_description.annotation_info_with_keypoint(keypoint_id.0)
        } else {
            class_description.annotation_info()
        }
    })
    .collect();

    Ok((ResolvedAnnotationInfos::Many(annotation_info), keypoints))
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
    ent_context: &SpatialSceneEntityContext<'_>,
    ent_path: &re_data_store::EntityPath,
    keypoints: &Keypoints,
) {
    if keypoints.is_empty() {
        return;
    }

    re_tracing::profile_function!();

    // Generate keypoint connections if any.
    let mut line_builder = ent_context.shared_render_builders.lines();
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
            let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b)) else {
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

/// If 2d object is shown in a 3d space view, it is only then visualizable, if it is under a pinhole camera.
fn filter_visualizable_2d_entities(
    entities: ApplicableEntities,
    context: &dyn std::any::Any,
) -> VisualizableEntities {
    // `VisualizableFilterContext3D` will only be available if we're in a 3D space view.
    if let Some(context) = context.downcast_ref::<VisualizableFilterContext3D>() {
        VisualizableEntities(
            entities
                .0
                .into_iter()
                .filter(|ent_path| context.entities_under_pinhole.contains(&ent_path.hash()))
                .collect(),
        )
    } else {
        VisualizableEntities(entities.0)
    }
}
