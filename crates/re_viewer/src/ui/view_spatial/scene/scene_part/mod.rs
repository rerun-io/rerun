//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

use crate::{
    misc::{SpaceViewHighlights, TransformCache, ViewerContext},
    ui::scene::SceneQuery,
};

use super::SceneSpatial;

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
