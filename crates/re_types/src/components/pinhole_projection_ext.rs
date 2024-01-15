use crate::datatypes::Vec2D;

use super::PinholeProjection;

impl PinholeProjection {
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
