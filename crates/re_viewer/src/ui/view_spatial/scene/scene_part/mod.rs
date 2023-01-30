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
    misc::{OptionalSpaceViewEntityHighlight, SpaceViewHighlights, TransformCache, ViewerContext},
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
/// Takes into account the currently the object properties, currently highlighted objects, and number of instances.
pub fn instance_hash_for_picking<T: re_log_types::msg_bundle::Component>(
    ent_path: &EntityPath,
    instance: re_log_types::component_types::Instance,
    entity_view: &re_query::EntityView<T>,
    props: &EntityProperties,
    entity_highlight: OptionalSpaceViewEntityHighlight<'_>,
) -> InstancePathHash {
    if props.interactive {
        if entity_view.num_instances() == 1 || !entity_highlight.any_selection_highlight() {
            InstancePathHash::entity_splat(ent_path)
        } else {
            InstancePathHash::instance(ent_path, instance)
        }
    } else {
        InstancePathHash::NONE
    }
}
