//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

mod arrows3d;
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
mod spatial_view_part;
mod transform3d_arrows;

pub use cameras::CamerasPart;
pub use images::Image;
pub use images::ImagesPart;
pub use spatial_view_part::SpatialViewPartData;
pub use transform3d_arrows::add_axis_arrows;

use ahash::HashMap;
use std::sync::Arc;

use re_data_store::{EntityPath, InstancePathHash};
use re_types::components::{Color, InstanceKey};
use re_types::datatypes::{KeypointId, KeypointPair};
use re_types::Archetype;
use re_viewer_context::SpaceViewClassRegistryError;
use re_viewer_context::{
    auto_color, Annotations, DefaultColor, ResolvedAnnotationInfo, SpaceViewSystemRegistry,
    ViewPartCollection, ViewQuery,
};

use super::contexts::SpatialSceneEntityContext;

/// Collection of keypoints for annotation context.
pub type Keypoints = HashMap<(re_types::components::ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
pub const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

pub fn register_parts(
    system_registry: &mut SpaceViewSystemRegistry,
) -> Result<(), SpaceViewClassRegistryError> {
    system_registry.register_part_system::<arrows3d::Arrows3DPart>()?;
    system_registry.register_part_system::<boxes2d::Boxes2DPart>()?;
    system_registry.register_part_system::<boxes3d::Boxes3DPart>()?;
    system_registry.register_part_system::<cameras::CamerasPart>()?;
    system_registry.register_part_system::<images::ImagesPart>()?;
    system_registry.register_part_system::<lines2d::Lines2DPart>()?;
    system_registry.register_part_system::<lines3d::Lines3DPart>()?;
    system_registry.register_part_system::<meshes::MeshPart>()?;
    system_registry.register_part_system::<points2d::Points2DPart>()?;
    system_registry.register_part_system::<points3d::Points3DPart>()?;
    system_registry.register_part_system::<transform3d_arrows::Transform3DArrowsPart>()?;
    Ok(())
}

pub fn calculate_bounding_box(
    parts: &ViewPartCollection,
    bounding_box_accum: &mut macaw::BoundingBox,
) -> macaw::BoundingBox {
    let mut bounding_box = macaw::BoundingBox::nothing();
    for part in parts.iter() {
        if let Some(data) = part
            .data()
            .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
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

pub fn collect_ui_labels(parts: &ViewPartCollection) -> Vec<UiLabel> {
    let mut ui_labels = Vec::new();
    for part in parts.iter() {
        if let Some(data) = part
            .data()
            .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
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
    annotation_infos: &'a [ResolvedAnnotationInfo],
) -> Result<impl Iterator<Item = egui::Color32> + 'a, re_query::QueryError> {
    re_tracing::profile_function!();
    let default_color = DefaultColor::EntityPath(ent_path);

    Ok(itertools::izip!(
        annotation_infos.iter(),
        arch_view.iter_optional_component::<Color>()?,
    )
    .map(move |(annotation_info, color)| {
        annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color)
    }))
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

/// Resolves all annotations and keypoints for the given entity view.
fn process_annotations_and_keypoints<Primary, A: Archetype>(
    query: &ViewQuery<'_>,
    arch_view: &re_query::ArchetypeView<A>,
    annotations: &Arc<Annotations>,
    mut primary_into_position: impl FnMut(&Primary) -> glam::Vec3,
) -> Result<(Vec<ResolvedAnnotationInfo>, Keypoints), re_query::QueryError>
where
    Primary: re_types::Component + Clone + Default,
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
            vec![resolved_annotation; arch_view.num_instances()],
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
                .entry((class_id, query.latest_at.as_i64()))
                .or_insert_with(Default::default)
                .insert(keypoint_id.into(), primary_into_position(&primary));
            class_description.annotation_info_with_keypoint(keypoint_id.into())
        } else {
            class_description.annotation_info()
        }
    })
    .collect();

    Ok((annotation_info, keypoints))
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
    keypoints: Keypoints,
) {
    if keypoints.is_empty() {
        return;
    }

    // Generate keypoint connections if any.
    let mut line_builder = ent_context.shared_render_builders.lines();
    let mut line_batch = line_builder
        .batch("keypoint connections")
        .world_from_obj(ent_context.world_from_obj)
        .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

    for ((class_id, _time), keypoints_in_class) in keypoints {
        let resolved_class_description = ent_context
            .annotations
            .resolved_class_description(Some(class_id));

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
pub fn image_view_coordinates() -> re_components::ViewCoordinates {
    // Typical image spaces have
    // - x pointing right
    // - y pointing down
    // - z pointing into the image plane (this is convenient for reading out a depth image which has typically positive z values)
    re_components::ViewCoordinates::RDF
}
