//! Responsible for populating `SceneSpatialPrimitives` and `SceneSpatialUiData`

use crate::{
    misc::{SpaceViewHighlights, ViewerContext},
    ui::{scene::SceneQuery, transform_cache::TransformCache},
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
mod segments2d;

pub(crate) use arrows3d::{Arrows3DPart, Arrows3DPartClassic};
pub(crate) use boxes2d::{Boxes2DPart, Boxes2DPartClassic};
pub(crate) use boxes3d::{Boxes3DPart, Boxes3DPartClassic};
pub(crate) use cameras::{CamerasPart, CamerasPartClassic};
pub(crate) use images::{ImagesPart, ImagesPartClassic};
pub(crate) use lines2d::Lines2DPart;
pub(crate) use lines3d::{Lines3DPart, Lines3DPartClassic};
pub(crate) use meshes::{MeshPart, MeshPartClassic};
pub(crate) use points2d::{Points2DPart, Points2DPartClassic};
pub(crate) use points3d::{Points3DPart, Points3DPartClassic};
pub(crate) use segments2d::LineSegments2DPartClassic;

pub trait ScenePart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        view_spatial_state: &crate::ui::view_spatial::ViewSpatialState,
    );
}
