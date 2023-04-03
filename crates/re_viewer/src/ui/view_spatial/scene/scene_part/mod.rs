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

use super::SceneSpatial;
use crate::{
    misc::{SpaceViewHighlights, TransformCache, ViewerContext},
    ui::scene::SceneQuery,
};
use re_data_store::{EntityPath, EntityProperties, InstancePathHash};

pub trait ScenePart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
    );
}

/// Computes the instance hash that should be used for picking (in turn for selecting/hover)
///
/// TODO(andreas): Resolve the picking upon picking instead of doing it ahead of time here. This will speed up things.
/// Right now this is a bit hard to do since number of instances depends on the Primary. This is expected to change soon.
pub fn instance_path_hash_for_picking<T: re_log_types::Component>(
    ent_path: &EntityPath,
    instance_key: re_log_types::component_types::InstanceKey,
    entity_view: &re_query::EntityView<T>,
    props: &EntityProperties,
    any_part_selected: bool,
) -> InstancePathHash {
    if props.interactive {
        InstancePathHash::instance(
            ent_path,
            instance_key_for_picking(instance_key, entity_view, any_part_selected),
        )
    } else {
        InstancePathHash::NONE
    }
}

/// Computes the instance key that should be used for picking (in turn for selecting/hover)
///
/// Assumes the entity is interactive.
///
/// TODO(andreas): Resolve the picking upon picking instead of doing it ahead of time here. This will speed up things.
/// Right now this is a bit hard to do since number of instances depends on the Primary. This is expected to change soon.
pub fn instance_key_for_picking<T: re_log_types::Component>(
    instance_key: re_log_types::component_types::InstanceKey,
    entity_view: &re_query::EntityView<T>,
    any_part_selected: bool,
) -> re_log_types::component_types::InstanceKey {
    if entity_view.num_instances() == 1 || !any_part_selected {
        re_log_types::component_types::InstanceKey::SPLAT
    } else {
        instance_key
    }
}

/// See [`instance_key_for_picking`]
pub fn instance_key_to_picking_id<T: re_log_types::Component>(
    instance_key: re_log_types::component_types::InstanceKey,
    entity_view: &re_query::EntityView<T>,
    any_part_selected: bool,
) -> re_renderer::PickingLayerInstanceId {
    re_renderer::PickingLayerInstanceId(
        instance_key_for_picking(instance_key, entity_view, any_part_selected).0,
    )
}
