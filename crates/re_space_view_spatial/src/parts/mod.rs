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
mod spatial_scene_part_data;

pub use images::Image;
pub use spatial_scene_part_data::SpatialViewPartSystemData;

use ahash::HashMap;
use std::sync::Arc;

use re_components::{ClassId, ColorRGBA, KeypointId, Radius};
use re_data_store::{EntityPath, InstancePathHash};
use re_viewer_context::{
    auto_color, Annotations, DefaultColor, ResolvedAnnotationInfo, TypedScene,
    ViewPartSystemCollection, ViewQuery,
};

use crate::{
    ui::{SpatialNavigationMode, SpatialSpaceViewState},
    SpatialSpaceView,
};

use super::contexts::SpatialSceneEntityContext;

/// Collection of keypoints for annotation context.
pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
pub const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

#[derive(Default)]
pub struct SpatialViewPartSystemCollection {
    pub points2d: points2d::Points2DPart,
    pub points3d: points3d::Points3DPart,
    pub arrows3d: arrows3d::Arrows3DPart,
    pub boxes2d: boxes2d::Boxes2DPart,
    pub boxes3d: boxes3d::Boxes3DPart,
    pub cameras: cameras::CamerasPart,
    pub lines2d: lines2d::Lines2DPart,
    pub lines3d: lines3d::Lines3DPart,
    pub meshes: meshes::MeshPart,
    pub images: images::ImagesPart,
}

impl ViewPartSystemCollection<SpatialSpaceView> for SpatialViewPartSystemCollection {
    fn vec_mut(&mut self) -> Vec<&mut dyn re_viewer_context::ViewPartSystem<SpatialSpaceView>> {
        let Self {
            points2d,
            points3d,
            arrows3d,
            boxes2d,
            boxes3d,
            cameras,
            lines2d,
            lines3d,
            meshes,
            images,
        } = self;
        vec![
            points2d, points3d, arrows3d, boxes2d, boxes3d, cameras, lines2d, lines3d, meshes,
            images,
        ]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SpatialViewPartSystemCollection {
    fn vec(&self) -> Vec<&dyn re_viewer_context::ViewPartSystem<SpatialSpaceView>> {
        let Self {
            points2d,
            points3d,
            arrows3d,
            boxes2d,
            boxes3d,
            cameras,
            lines2d,
            lines3d,
            meshes,
            images,
        } = self;
        vec![
            points2d, points3d, arrows3d, boxes2d, boxes3d, cameras, lines2d, lines3d, meshes,
            images,
        ]
    }

    pub fn calculate_bounding_box(&self) -> macaw::BoundingBox {
        let mut bounding_box = macaw::BoundingBox::nothing();
        for scene_part in self.vec() {
            if let Some(data) = scene_part.data() {
                bounding_box = bounding_box.union(data.bounding_box);
            }
        }
        bounding_box
    }

    pub fn collect_ui_labels(&self) -> Vec<UiLabel> {
        let mut ui_labels = Vec::new();
        for scene_part in self.vec() {
            if let Some(data) = scene_part.data() {
                ui_labels.extend(data.ui_labels.iter().cloned());
            }
        }
        ui_labels
    }
}

pub fn picking_id_from_instance_key(
    instance_key: re_log_types::InstanceKey,
) -> re_renderer::PickingLayerInstanceId {
    re_renderer::PickingLayerInstanceId(instance_key.0)
}

/// Process [`ColorRGBA`] components using annotations and default colors.
pub fn process_colors<'a, Primary>(
    entity_view: &'a re_query::EntityView<Primary>,
    ent_path: &'a EntityPath,
    annotation_infos: &'a [ResolvedAnnotationInfo],
) -> Result<impl Iterator<Item = egui::Color32> + 'a, re_query::QueryError>
where
    Primary: re_log_types::SerializableComponent + re_log_types::DeserializableComponent,
    for<'b> &'b Primary::ArrayType: IntoIterator,
{
    re_tracing::profile_function!();
    let default_color = DefaultColor::EntityPath(ent_path);

    Ok(itertools::izip!(
        annotation_infos.iter(),
        entity_view.iter_component::<ColorRGBA>()?,
    )
    .map(move |(annotation_info, color)| {
        annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color)
    }))
}

/// Process [`Radius`] components to [`re_renderer::Size`] using auto size where no radius is specified.
pub fn process_radii<'a, Primary>(
    ent_path: &EntityPath,
    entity_view: &'a re_query::EntityView<Primary>,
) -> Result<impl Iterator<Item = re_renderer::Size> + 'a, re_query::QueryError>
where
    Primary: re_log_types::SerializableComponent + re_log_types::DeserializableComponent,
    for<'b> &'b Primary::ArrayType: IntoIterator,
{
    re_tracing::profile_function!();
    let ent_path = ent_path.clone();
    Ok(entity_view.iter_component::<Radius>()?.map(move |radius| {
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
fn process_annotations_and_keypoints<Primary>(
    query: &ViewQuery<'_>,
    entity_view: &re_query::EntityView<Primary>,
    annotations: &Arc<Annotations>,
) -> Result<(Vec<ResolvedAnnotationInfo>, Keypoints), re_query::QueryError>
where
    Primary: re_log_types::SerializableComponent + re_log_types::DeserializableComponent,
    for<'b> &'b Primary::ArrayType: IntoIterator,
    glam::Vec3: std::convert::From<Primary>,
{
    re_tracing::profile_function!();

    let mut keypoints: Keypoints = HashMap::default();

    // No need to process annotations if we don't have keypoints or class-ids
    if !entity_view.has_component::<KeypointId>() && !entity_view.has_component::<ClassId>() {
        let resolved_annotation = annotations.class_description(None).annotation_info();
        return Ok((
            vec![resolved_annotation; entity_view.num_instances()],
            keypoints,
        ));
    }

    let annotation_info = itertools::izip!(
        entity_view.iter_primary()?,
        entity_view.iter_component::<KeypointId>()?,
        entity_view.iter_component::<ClassId>()?,
    )
    .map(|(position, keypoint_id, class_id)| {
        let class_description = annotations.class_description(class_id);

        if let (Some(keypoint_id), Some(class_id), Some(position)) =
            (keypoint_id, class_id, position)
        {
            keypoints
                .entry((class_id, query.latest_at.as_i64()))
                .or_insert_with(Default::default)
                .insert(keypoint_id, position.into());
            class_description.annotation_info_with_keypoint(keypoint_id)
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

/// Type alias for spatial scenes.
pub type SceneSpatial = TypedScene<SpatialSpaceView>;

/// Heuristic whether the default way of looking at this scene should be 2d or 3d.
pub fn preferred_navigation_mode(
    scene: &SceneSpatial,
    space_info_path: &EntityPath,
) -> SpatialNavigationMode {
    // If there's any space cameras that are not the root, we need to go 3D, otherwise we can't display them.
    if scene
        .parts
        .cameras
        .space_cameras
        .iter()
        .any(|camera| &camera.ent_path != space_info_path)
    {
        return SpatialNavigationMode::ThreeD;
    }

    if !scene.parts.images.images.is_empty() {
        return SpatialNavigationMode::TwoD;
    }

    if scene
        .context
        .num_3d_primitives
        .load(std::sync::atomic::Ordering::Relaxed)
        == 0
    {
        return SpatialNavigationMode::TwoD;
    }

    SpatialNavigationMode::ThreeD
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
        let Some(class_description) = ent_context.annotations.context.class_map.get(&class_id) else {
            continue;
        };

        let color = class_description.info.color.map_or_else(
            || auto_color(class_description.info.id),
            |color| color.into(),
        );

        for (a, b) in &class_description.keypoint_connections {
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
                .picking_instance_id(re_renderer::PickingLayerInstanceId(
                    re_log_types::InstanceKey::SPLAT.0,
                ));
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
