use crate::datatypes::Vec2D;

use super::Pinhole;

impl Pinhole {
    /// Creates a pinhole from the focal length of the camera in pixels & a resolution in pixel.
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

    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    #[inline]
    pub fn fov_y(&self) -> Option<f32> {
        self.resolution
            .map(|resolution| 2.0 * (0.5 * resolution[1] / self.image_from_camera.col(1)[1]).atan())
    }

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn focal_length_in_pixels(&self) -> Vec2D {
        [
            self.image_from_camera.col(0)[0],
            self.image_from_camera.col(1)[1],
        ]
        .into()
    }

    /// Focal length.
    #[inline]
    pub fn focal_length(&self) -> Option<f32> {
        // Use only the first element of the focal length vector, as we don't support non-square pixels.
        self.resolution
            .map(|r| self.image_from_camera.col(0)[0] / r[0])
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn principal_point(&self) -> glam::Vec2 {
        glam::vec2(
            self.image_from_camera.col(2)[0],
            self.image_from_camera.col(2)[1],
        )
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

    /// Project camera-space coordinates into pixel coordinates,
    /// returning the same z/depth.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn project(&self, pixel: glam::Vec3) -> glam::Vec3 {
        ((pixel.truncate() * glam::Vec2::from(self.focal_length_in_pixels())) / pixel.z
            + self.principal_point())
        .extend(pixel.z)
    }

    /// Given pixel coordinates and a world-space depth,
    /// return a position in the camera space.
    ///
    /// The returned z is the same as the input z (depth).
    #[cfg(feature = "glam")]
    #[inline]
    pub fn unproject(&self, pixel: glam::Vec3) -> glam::Vec3 {
        ((pixel.truncate() - self.principal_point()) * pixel.z
            / glam::Vec2::from(self.focal_length_in_pixels()))
        .extend(pixel.z)
    }
}
