//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

mod arrows3d;
mod boxes2d;
mod boxes3d;
mod cameras;
mod images;
mod lines2d;
mod lines3d;
mod meshes;
mod points2d;
mod points3d;

use std::sync::Arc;

use ahash::HashMap;
pub(crate) use arrows3d::Arrows3DPart;
pub(crate) use boxes2d::Boxes2DPart;
pub(crate) use boxes3d::Boxes3DPart;
pub(crate) use cameras::CamerasPart;
pub(crate) use images::ImagesPart;
pub(crate) use lines2d::Lines2DPart;
pub(crate) use lines3d::Lines3DPart;
pub(crate) use meshes::MeshPart;

pub(crate) use points2d::Points2DPart;
pub(crate) use points3d::Points3DPart;
use re_log_types::component_types::{ClassId, ColorRGBA, KeypointId, Radius};

use super::{EntityDepthOffsets, SceneSpatial};
use crate::{
    misc::{SpaceViewHighlights, TransformCache},
    ui::view_spatial::scene::Keypoints,
};
use re_data_store::{EntityPath, InstancePathHash};
use re_viewer_context::{
    Annotations, DefaultColor, ResolvedAnnotationInfo, SceneQuery, ViewerContext,
};

pub trait ScenePart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        depth_offsets: &EntityDepthOffsets,
    );
}

/// Computes the instance hash that should be used for picking (in turn for selecting/hover)
///
/// TODO(andreas): Resolve the hash-for-picking when retrieving the picking result instead of doing it ahead of time here to speed up things.
///                 (gpu picking would always get the "most fine grained hash" which we could then resolve to groups etc. depending on selection state)
/// Right now this is a bit hard to do since number of instances depends on the Primary. This is expected to change soon.
pub fn instance_path_hash_for_picking(
    ent_path: &EntityPath,
    instance_key: re_log_types::component_types::InstanceKey,
    num_instances: usize,
    any_part_selected: bool,
) -> InstancePathHash {
    InstancePathHash::instance(
        ent_path,
        instance_key_for_picking(instance_key, num_instances, any_part_selected),
    )
}

/// Computes the instance key that should be used for picking (in turn for selecting/hover)
///
/// Assumes the entity is interactive.
///
/// TODO(andreas): Resolve the hash-for-picking when retrieving the picking result instead of doing it ahead of time here to speed up things.
///                 (gpu picking would always get the "most fine grained hash" which we could then resolve to groups etc. depending on selection state)
/// Right now this is a bit hard to do since number of instances depends on the Primary. This is expected to change soon.
pub fn instance_key_for_picking(
    instance_key: re_log_types::component_types::InstanceKey,
    num_instances: usize,
    any_part_selected: bool,
) -> re_log_types::component_types::InstanceKey {
    // If no part of the entity is selected or if there is only one instance, selecting
    // should select the entire entity, not the specific instance.
    // (the splat key means that no particular instance is selected but all at once instead)
    if num_instances == 1 || !any_part_selected {
        re_log_types::component_types::InstanceKey::SPLAT
    } else {
        instance_key
    }
}

/// See [`instance_key_for_picking`]
pub fn instance_key_to_picking_id(
    instance_key: re_log_types::component_types::InstanceKey,
    num_instances: usize,
    any_part_selected: bool,
) -> re_renderer::PickingLayerInstanceId {
    re_renderer::PickingLayerInstanceId(
        instance_key_for_picking(instance_key, num_instances, any_part_selected).0,
    )
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
    crate::profile_function!();
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
    crate::profile_function!();
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
    crate::profile_function!();

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
