use glam::Vec3;
use macaw::IsoTransform;

use re_components::{Pinhole, ViewCoordinates};
use re_log_types::EntityPath;

use crate::scene::image_view_coordinates;

/// A logged camera that connects spaces.
#[derive(Clone, PartialEq)]
pub struct SpaceCamera3D {
    /// Path to the entity which has the projection (pinhole, ortho or otherwise) transforms.
    ///
    /// We expect the camera transform to apply to this instance and every path below it.
    pub ent_path: EntityPath,

    /// The coordinate system of the camera ("view-space").
    pub view_coordinates: ViewCoordinates,

    /// Camera "Extrinsics", i.e. the pose of the camera.
    pub world_from_camera: IsoTransform,

    // -------------------------
    // Optional projection-related things:
    /// The projection transform of a child-entity.
    pub pinhole: Option<Pinhole>,

    /// Distance of a picture plane from the camera.
    pub picture_plane_distance: f32,
}

impl SpaceCamera3D {
    /// Where in scene-space is the camera origin?
    pub fn position(&self) -> Vec3 {
        self.world_from_camera.translation()
    }

    pub fn world_from_cam(&self) -> IsoTransform {
        self.world_from_camera
    }

    pub fn cam_from_world(&self) -> IsoTransform {
        self.world_from_cam().inverse()
    }

    /// Scene-space from Rerun view-space (RUB).
    pub fn world_from_rub_view(&self) -> Option<IsoTransform> {
        match self.view_coordinates.from_rub_quat() {
            Ok(from_rub) => Some(self.world_from_camera * IsoTransform::from_quat(from_rub)),
            Err(err) => {
                re_log::warn_once!("Camera {:?}: {err}", self.ent_path);
                None
            }
        }
    }

    /// Returns x, y, and depth in image/pixel coordinates.
    pub fn project_onto_2d(&self, point_in_world: Vec3) -> Option<Vec3> {
        let pinhole = self.pinhole?;
        let point_in_cam = self.cam_from_world().transform_point3(point_in_world);

        // View-coordinates are relevant here because without them we have no notion of what the image plane is.
        // (it's not a given that e.g. XY is the camera image plane!)
        // First transform to the "standard RUB" 3D camera and then from there to the image plane coordinate system.
        let point_in_image_unprojected =
            image_view_coordinates().from_rub() * self.view_coordinates.to_rub() * point_in_cam;

        let point_in_image = pinhole.project(point_in_image_unprojected);
        Some(point_in_image)
    }
}
