use re_types::{archetypes, components};

use crate::resolution_of_image_at;

/// A pinhole camera model.
///
/// Corresponds roughly to the [`re_types::archetypes::Pinhole`] archetype, but uses `glam` types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinhole {
    pub image_from_camera: glam::Mat3,
    pub resolution: glam::Vec2,
}

impl Pinhole {
    /// Width/height ratio of the camera sensor.
    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        self.resolution.x / self.resolution.y
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn principal_point(&self) -> glam::Vec2 {
        glam::vec2(
            self.image_from_camera.col(2)[0],
            self.image_from_camera.col(2)[1],
        )
    }

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn focal_length_in_pixels(&self) -> glam::Vec2 {
        glam::vec2(
            self.image_from_camera.col(0)[0],
            self.image_from_camera.col(1)[1],
        )
    }

    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    #[inline]
    pub fn fov_y(&self) -> f32 {
        2.0 * (0.5 * self.resolution[1] / self.image_from_camera.col(1)[1]).atan()
    }

    /// The pinhole sensor rectangle: [0, 0] - [width, height],
    /// ignoring principal point.
    #[inline]
    pub fn resolution_rect(&self) -> egui::Rect {
        egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(self.resolution.x, self.resolution.y),
        )
    }

    /// Project camera-space coordinates into pixel coordinates,
    /// returning the same z/depth.
    #[inline]
    pub fn project(&self, pixel: glam::Vec3) -> glam::Vec3 {
        ((pixel.truncate() * self.focal_length_in_pixels()) / pixel.z + self.principal_point())
            .extend(pixel.z)
    }

    /// Given pixel coordinates and a world-space depth,
    /// return a position in the camera space.
    ///
    /// The returned z is the same as the input z (depth).
    #[inline]
    pub fn unproject(&self, pixel: glam::Vec3) -> glam::Vec3 {
        ((pixel.truncate() - self.principal_point()) * pixel.z / self.focal_length_in_pixels())
            .extend(pixel.z)
    }
}

/// Utility for querying the pinhole from the store.
///
/// Fallback provider will be used for everything but the projection itself.
/// Does NOT take into account blueprint overrides, defaults and fallbacks.
/// However, it will use the resolution of the image at the entity path if available.
///
/// If the projection isn't present, returns `None`.
// TODO(andreas): Give this another pass and think about how we can remove this.
// Being disconnected from the blueprint & fallbacks makes this a weird snowflake with unexpected behavior.
// Also, figure out how this might actually relate to the transform cache.
pub fn query_pinhole_and_view_coordinates_from_store_without_blueprint(
    ctx: &re_viewer_context::ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<(Pinhole, components::ViewCoordinates)> {
    let entity_db = ctx.recording();

    let query_results = entity_db.latest_at(
        query,
        entity_path,
        [
            &archetypes::Pinhole::descriptor_image_from_camera(),
            &archetypes::Pinhole::descriptor_resolution(),
            &archetypes::Pinhole::descriptor_camera_xyz(),
        ],
    );

    let pinhole_projection =
        query_results.component_mono_quiet::<components::PinholeProjection>()?;

    let resolution = query_results
        .component_mono_quiet::<components::Resolution>()
        .unwrap_or_else(|| {
            resolution_of_image_at(ctx, query, entity_path).unwrap_or([100.0, 100.0].into())
        });
    let camera_xyz = query_results
        .component_mono_quiet::<components::ViewCoordinates>()
        .unwrap_or(archetypes::Pinhole::DEFAULT_CAMERA_XYZ);

    Some((
        Pinhole {
            image_from_camera: pinhole_projection.0.into(),
            resolution: resolution.into(),
        },
        camera_xyz,
    ))
}
