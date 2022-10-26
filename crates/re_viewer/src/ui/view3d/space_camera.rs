use glam::*;
use macaw::{IsoTransform, Ray3};

use re_data_store::ObjPath;
use re_log_types::{CoordinateSystem, IndexHash};

/// A logged camera that connects spaces.
pub struct SpaceCamera {
    pub obj_path: ObjPath,
    pub instance_index_hash: IndexHash,

    /// The coordinate system of the camera.
    pub view_space: Option<CoordinateSystem>,

    pub extrinsics: re_log_types::Extrinsics,
    pub intrinsics: Option<re_log_types::Intrinsics>,

    /// The child 2D space we project into.
    pub target_space: Option<ObjPath>,
}

impl SpaceCamera {
    /// Scene-space from Rerun view-space (RUB).
    pub fn world_from_view(&self) -> Option<IsoTransform> {
        match user_view_from_rerun_view(self.view_space) {
            Ok(user_view_from_rerun_view) => {
                let rotation = Quat::from_slice(&self.extrinsics.rotation);
                let translation = Vec3::from_slice(&self.extrinsics.position);
                let extrinsics = IsoTransform::from_rotation_translation(rotation, translation);
                Some(extrinsics * IsoTransform::from_quat(user_view_from_rerun_view))
            }
            Err(err) => {
                re_log::warn_once!("Camera {:?}: {}", self.obj_path, err);
                None
            }
        }
    }

    /// Rerun view-space (RUB) from scene-space
    pub fn view_from_world(&self) -> Option<macaw::IsoTransform> {
        self.world_from_view().map(|t| t.inverse())
    }

    /// Projects image coordinates into world coordinates
    pub fn world_from_image(&self) -> Option<Affine3A> {
        let intrinsics = self.intrinsics?;
        let world_from_view = self.world_from_view()?;

        let intrinsics_matrix = Mat3::from_cols_array_2d(&intrinsics.intrinsics_matrix);
        Some(
            world_from_view
                * Affine3A::from_scale([1.0, -1.0, -1.0].into()) // negate Y and Z here here because image space and view space are different. TODO(emilk): use the `CoordinateSystem` of the image space
                * Affine3A::from_mat3(intrinsics_matrix.inverse()),
        )
    }

    /// Projects world coordinates onto 2D image coordinates
    pub fn image_from_world(&self) -> Option<Affine3A> {
        let intrinsics = self.intrinsics?;
        let view_from_world = self.view_from_world()?;

        let intrinsics_matrix = Mat3::from_cols_array_2d(&intrinsics.intrinsics_matrix);
        Some(
            Affine3A::from_mat3(intrinsics_matrix)
            * Affine3A::from_scale([1.0, -1.0, -1.0].into()) // negate Y and Z here here because image space and view space are different. TODO(emilk): use the `CoordinateSystem` of the image space
            * view_from_world,
        )
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
            let origin = Vec3::from_slice(&self.extrinsics.position);
            let stop = world_from_pixel.transform_point3(pos2d.extend(1.0));
            let dir = (stop - origin).normalize();
            Ray3::from_origin_dir(origin, dir)
        })
    }
}

/// Rerun uses RUB view coordinates.
fn user_view_from_rerun_view(system: Option<CoordinateSystem>) -> Result<Quat, String> {
    match system {
        None => Err("lacks a coordinate system".to_owned()),
        Some(CoordinateSystem::World(_)) => {
            Err("has a world coordinate system but needs a _relative_ coordinate system".to_owned())
        }
        Some(CoordinateSystem::Relative(system)) => {
            let mat3 = system.from_rub();
            let det = mat3.determinant();
            if det < -0.5 {
                Err(
                    "has a left-handed coordinate system - Rerun does not yet support this!"
                        .to_owned(),
                )
            } else if det < 0.5 {
                Err(format!(
                    "has a degenerate coordinate system: {}",
                    system.describe()
                ))
            } else {
                Ok(Quat::from_mat3(&mat3))
            }
        }
    }
}
