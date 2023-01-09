//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

use re_data_store::{InstanceIdHash, ObjectsProperties};

use crate::{
    misc::ViewerContext,
    ui::{scene::SceneQuery, transform_cache::TransformCache},
};

use super::SceneSpatial;

mod arrows3d;
mod boxes2d;
mod boxes3d;
mod images;
mod lines3d;
mod meshes;
mod points2d;
mod points3d;
mod segments2d;

pub use arrows3d::Arrows3DPart;
pub use boxes2d::{Boxes2DPart, Boxes2DPartClassic};
pub use boxes3d::Boxes3DPart;
pub use images::ImagesPart;
pub use lines3d::Lines3DPart;
pub use meshes::MeshPart;
pub use points2d::{Points2DPart, Points2DPartClassic};
pub use points3d::{Points3DPart, Points3DPartClassic};
pub use segments2d::LineSegments2DPart;

pub trait ScenePart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    );
}
