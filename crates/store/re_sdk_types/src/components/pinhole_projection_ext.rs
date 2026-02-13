use super::PinholeProjection;
use crate::datatypes::Vec2D;

impl PinholeProjection {
    /// Create a new pinhole projection matrix from a focal length and principal point.
    #[inline]
    pub fn from_focal_length_and_principal_point(
        focal_length: impl Into<Vec2D>,
        principal_point: impl Into<Vec2D>,
    ) -> Self {
        let fl = focal_length.into();
        let pp = principal_point.into();
        Self::from([
            [fl.x(), 0.0, 0.0],
            [0.0, fl.y(), 0.0],
            [pp.x(), pp.y(), 1.0],
        ])
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn with_principal_point(mut self, principal_point: impl Into<Vec2D>) -> Self {
        let pp = principal_point.into();
        let col = 2;
        self.0.set(0, col, pp.x());
        self.0.set(1, col, pp.y());
        self
    }

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn focal_length_in_pixels(&self) -> Vec2D {
        [self.col(0)[0], self.col(1)[1]].into()
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn principal_point(&self) -> glam::Vec2 {
        glam::vec2(self.col(2)[0], self.col(2)[1])
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

    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    pub fn fov_y(&self, resolution: impl Into<super::Resolution>) -> f32 {
        let resolution = resolution.into();
        2.0 * (0.5 * resolution[1] / self.col(1)[1]).atan()
    }
}

impl Default for PinholeProjection {
    #[inline]
    fn default() -> Self {
        // There's no good default for this, but we need a fallback for the viewer
        // so center at 100x100 with a focal length of 100.
        Self::from_focal_length_and_principal_point([100.0, 100.0], [100.0, 100.0])
    }
}

#[test]
#[cfg(feature = "glam")]
fn test_pinhole() {
    let fl = Vec2D::from([600.0, 600.0]);
    let pp = glam::Vec2::from([300.0, 240.0]);
    let pinhole = PinholeProjection::from_focal_length_and_principal_point(fl, pp);
    assert_eq!(pinhole.focal_length_in_pixels(), fl);
    assert_eq!(pinhole.principal_point(), pp);
}
