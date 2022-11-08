use glam::*;
use macaw::{IsoTransform, Ray3};

use re_data_store::ObjPath;
use re_log_types::{IndexHash, ViewCoordinates};

/// A logged camera that connects spaces.
#[derive(Clone)]
pub struct SpaceCamera {
    /// Path to the object which has the rigid [Self::world_from_camera`] transforms.
    pub camera_obj_path: ObjPath,

    /// The hash of the instance index (if any) of the object at [`Self::camera_obj_path`].
    pub instance_index_hash: IndexHash,

    /// The coordinate system of the camera ("view-space").
    pub camera_view_coordinates: Option<ViewCoordinates>,

    /// Camera "Extrinsics", i.e. the pose of the camera.
    pub world_from_camera: IsoTransform,

    // -------------------------
    // Optional projection-related things:
    /// The projection transform of a child-object.
    pub pinhole: Option<re_log_types::Pinhole>,

    /// The child 2D space we project into.
    pub target_space: Option<ObjPath>,
}

impl SpaceCamera {
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
        match from_rub_quat(self.camera_view_coordinates) {
            Ok(from_rub) => Some(self.world_from_camera * IsoTransform::from_quat(from_rub)),
            Err(err) => {
                re_log::warn_once!("Camera {:?}: {}", self.camera_obj_path, err);
                None
            }
        }
    }

    /// Projects image coordinates into world coordinates
    pub fn world_from_image(&self) -> Option<Affine3A> {
        let pinhole = self.pinhole?;
        let world_from_cam = self.world_from_cam();
        let image_from_cam = Mat3::from_cols_array_2d(&pinhole.image_from_cam);
        let cam_from_image = Affine3A::from_mat3(image_from_cam.inverse());
        Some(world_from_cam * cam_from_image)
    }

    /// Projects world coordinates onto 2D image coordinates
    pub fn image_from_world(&self) -> Option<Affine3A> {
        let pinhole = self.pinhole?;
        let cam_from_world = self.cam_from_world();

        let image_from_cam = Mat3::from_cols_array_2d(&pinhole.image_from_cam);
        let image_from_cam = Affine3A::from_mat3(image_from_cam);
        Some(image_from_cam * cam_from_world)
    }

    /// Returns x, y, and depth in image coordinates.
    pub fn project_onto_2d(&self, pos3d: Vec3) -> Option<Vec3> {
        self.image_from_world().map(|pixel_from_world| {
            let point = pixel_from_world.transform_point3(pos3d);
            vec3(point.x / point.z, point.y / point.z, point.z)
        })
    }

    /// Unproject a 2D image coordinate as a ray in 3D space
    pub fn unproject_as_ray(&self, pos2d: Vec2) -> Option<Ray3> {
        self.world_from_image().map(|world_from_pixel| {
            let origin = self.position();
            let stop = world_from_pixel.transform_point3(pos2d.extend(1.0));
            let dir = (stop - origin).normalize();
            Ray3::from_origin_dir(origin, dir)
        })
    }
}

/// Rerun uses RUB (X=Right Y=Up Z=Back) view coordinates.
fn from_rub_mat3(system: Option<ViewCoordinates>) -> Result<Mat3, String> {
    match system {
        None => Err("lacks a coordinate system".to_owned()),
        Some(system) => Ok(system.from_rub()),
    }
}

fn from_rub_quat(system: Option<ViewCoordinates>) -> Result<Quat, String> {
    let mat3 = from_rub_mat3(system)?;
    let system = system.unwrap(); // Safe, or `from_rub_mat3` would have returned an `Err`.

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
