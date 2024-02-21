use crate::datatypes::Vec2D;

use super::Pinhole;

impl Pinhole {
    /// Creates a pinhole from the camera focal length and resolution, both specified in pixels.
    ///
    /// The focal length is the diagonal of the projection matrix.
    /// Set the same value for x & y value for symmetric cameras, or two values for anamorphic cameras.
    ///
    /// Assumes the principal point to be in the middle of the sensor.
    pub fn from_focal_length_and_resolution(
        focal_length: impl Into<Vec2D>,
        resolution: impl Into<Vec2D>,
    ) -> Self {
        let resolution = resolution.into();
        let focal_length = focal_length.into();

        let u_cen = resolution.x() / 2.0;
        let v_cen = resolution.y() / 2.0;

        Self::new([
            [focal_length.x(), 0.0, 0.0],
            [0.0, focal_length.y(), 0.0],
            [u_cen, v_cen, 1.0],
        ])
        .with_resolution(resolution)
    }

    /// Creates a pinhole from the camera vertical field of view (in radians) and aspect ratio (width/height).
    ///
    /// Assumes the principal point to be in the middle of the sensor.
    #[inline]
    pub fn from_fov_and_aspect_ratio(fov_y: f32, aspect_ratio: f32) -> Self {
        let focal_length_y = 0.5 / (fov_y * 0.5).max(f32::EPSILON).tan();
        let focal_length = [focal_length_y, focal_length_y];
        Self::from_focal_length_and_resolution(focal_length, [aspect_ratio, 1.0])
    }

    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    #[inline]
    pub fn fov_y(&self) -> Option<f32> {
        self.resolution
            .map(|resolution| 2.0 * (0.5 * resolution[1] / self.image_from_camera.col(1)[1]).atan())
    }

    #[inline]
    #[cfg(feature = "glam")]
    pub fn resolution(&self) -> Option<glam::Vec2> {
        self.resolution.map(|r| (*r).into())
    }

    #[inline]
    pub fn aspect_ratio(&self) -> Option<f32> {
        self.resolution.map(|r| r[0] / r[1])
    }

    // ------------------------------------------------------------------------
    // Forwarding calls to `PinholeProjection`:

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn focal_length_in_pixels(&self) -> Vec2D {
        self.image_from_camera.focal_length_in_pixels()
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn principal_point(&self) -> glam::Vec2 {
        self.image_from_camera.principal_point()
    }

    /// Project camera-space coordinates into pixel coordinates,
    /// returning the same z/depth.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn project(&self, pixel: glam::Vec3) -> glam::Vec3 {
        self.image_from_camera.project(pixel)
    }

    /// Given pixel coordinates and a world-space depth,
    /// return a position in the camera space.
    ///
    /// The returned z is the same as the input z (depth).
    #[cfg(feature = "glam")]
    #[inline]
    pub fn unproject(&self, pixel: glam::Vec3) -> glam::Vec3 {
        self.image_from_camera.unproject(pixel)
    }
}
