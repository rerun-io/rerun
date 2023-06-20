use glam::{Affine3A, Mat3, Quat, Vec2, Vec3};
use macaw::{IsoTransform, Ray3};

use re_components::{Pinhole, ViewCoordinates};
use re_log_types::EntityPath;

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
        match from_rub_quat(self.view_coordinates) {
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
        let point_in_image = pinhole.project(point_in_cam);
        Some(point_in_image)
    }

    /// Unproject a 2D image coordinate as a ray in 3D space
    pub fn unproject_as_ray(&self, pos2d: Vec2) -> Option<Ray3> {
        let pinhole = self.pinhole?;

        let depth = 1.0; // whatever will do
        let stop = pinhole.unproject(pos2d.extend(depth));
        let ray_in_camera = Ray3::from_origin_dir(Vec3::ZERO, stop);
        Some(self.world_from_camera * ray_in_camera)
    }
}

fn from_rub_quat(system: ViewCoordinates) -> Result<Quat, String> {
    let mat3 = system.from_rub();

    let det = mat3.determinant();
    if det == 1.0 {
        Ok(Quat::from_mat3(&mat3))
    } else if det == -1.0 {
        Err("has a left-handed coordinate system - Rerun does not yet support this!".to_owned())
    } else {
        Err(format!(
            "has a degenerate coordinate system: {}",
            system.describe()
        ))
    }
}
