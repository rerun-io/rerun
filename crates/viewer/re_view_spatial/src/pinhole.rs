/// A pinhole camera model.
///
/// Corresponds roughly to the [`re_sdk_types::archetypes::Pinhole`] archetype, but uses render-friendly types.
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
