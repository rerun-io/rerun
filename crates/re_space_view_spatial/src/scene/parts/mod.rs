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
pub use spatial_scene_part_data::SpatialScenePartData;

use ahash::HashMap;
use std::sync::Arc;

use re_components::{ClassId, ColorRGBA, KeypointId, Radius};
use re_data_store::{EntityPath, InstancePathHash};
use re_viewer_context::{
    Annotations, DefaultColor, ResolvedAnnotationInfo, ScenePartCollection, SceneQuery,
};

use crate::{scene::Keypoints, ui::SpatialSpaceViewState, SpatialSpaceView};

use super::UiLabel;

#[derive(Default)]
pub struct SpatialScenePartCollection {
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

impl ScenePartCollection<SpatialSpaceView> for SpatialScenePartCollection {
    fn vec_mut(&mut self) -> Vec<&mut dyn re_viewer_context::ScenePart<SpatialSpaceView>> {
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

impl SpatialScenePartCollection {
    fn vec(&self) -> Vec<&dyn re_viewer_context::ScenePart<SpatialSpaceView>> {
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

/// Computes the instance hash that should be used for picking (in turn for selecting/hover)
///
/// TODO(andreas): Resolve the hash-for-picking when retrieving the picking result instead of doing it ahead of time here to speed up things.
///                 (gpu picking would always get the "most fine grained hash" which we could then resolve to groups etc. depending on selection state)
/// Right now this is a bit hard to do since number of instances depends on the Primary. This is expected to change soon.
pub fn instance_path_hash_for_picking(
    ent_path: &EntityPath,
    instance_key: re_log_types::InstanceKey,
) -> InstancePathHash {
    InstancePathHash::instance(ent_path, instance_key)
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
    query: &SceneQuery<'_>,
    entity_view: &re_query::EntityView<Primary>,
    annotations: &Arc<Annotations>,
) -> Result<(Vec<ResolvedAnnotationInfo>, super::Keypoints), re_query::QueryError>
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
