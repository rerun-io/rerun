use glam::Vec3;
use re_math::IsoTransform;

use re_log_types::EntityPath;
use re_types::archetypes::Pinhole;
use re_types::components::ViewCoordinates;

use crate::visualizers::image_view_coordinates;

/// A logged camera that connects spaces.
#[derive(Clone, PartialEq)]
pub struct SpaceCamera3D {
    /// Path to the entity which has the projection (pinhole, ortho or otherwise) transforms.
    ///
    /// We expect the camera transform to apply to this instance and every path below it.
    pub ent_path: EntityPath,

    /// The coordinate system of the pinhole entity ("view-space").
    pub pinhole_view_coordinates: ViewCoordinates,

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
        match self.pinhole_view_coordinates.from_rub_quat() {
            Ok(from_rub) => Some(self.world_from_camera * IsoTransform::from_quat(from_rub)),
            Err(err) => {
                re_log::warn_once!("Camera {:?}: {err}", self.ent_path);
                None
            }
        }
    }

    /// Returns x, y, and depth in image/pixel coordinates.
    pub fn project_onto_2d(&self, point_in_world: Vec3) -> Option<Vec3> {
        let pinhole = self.pinhole.as_ref()?;
        let point_in_cam = self.cam_from_world().transform_point3(point_in_world);

        // The pinhole view-coordinates are important here because they define how the image plane is aligned
        // with the camera coordinate system. It is not a given that a user wants the image-plane aligned with the
        // XY-plane in camera space.
        //
        // Because the [`Pinhole`] component currently assumes an input in the default `image_view_coordinates`
        // we need to pre-transform the data from the user-defined `pinhole_view_coordinates` to the required
        // `image_view_coordinates`.
        //
        // TODO(emilk): When Pinhole is an archetype instead of a component, `pinhole.project` should do this
        // internally.
        let point_in_image_unprojected =
            image_view_coordinates().from_other(&self.pinhole_view_coordinates) * point_in_cam;

        let point_in_image = pinhole.project(point_in_image_unprojected);
        Some(point_in_image)
    }
}
