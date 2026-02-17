use re_types_core::{DeserializationResult, Loggable as _};

use super::Fisheye;
use crate::components::{PinholeProjection, Resolution, ViewCoordinates};
use crate::datatypes::Vec2D;

impl Fisheye {
    /// Camera orientation used when there's no camera orientation explicitly logged.
    ///
    /// - x pointing right
    /// - y pointing down
    /// - z pointing into the image plane
    ///   (this is convenient for reading out a depth image which has typically positive z values)
    pub const DEFAULT_CAMERA_XYZ: ViewCoordinates = ViewCoordinates::RDF;

    /// Creates a fisheye camera from the camera focal length and resolution, both specified in pixels.
    ///
    /// The focal length is the diagonal of the projection matrix.
    /// Set the same value for x & y value for symmetric cameras, or two values for anamorphic cameras.
    ///
    /// Assumes the principal point to be in the middle of the sensor.
    /// Distortion coefficients default to zero.
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

    /// Sets the distortion coefficients (k1, k2, k3, k4) on the archetype.
    #[inline]
    pub fn with_coefficients(self, k1: f32, k2: f32, k3: f32, k4: f32) -> Self {
        self.with_distortion_coefficients(crate::components::FisheyeCoefficients(
            crate::datatypes::Vec4D([k1, k2, k3, k4]),
        ))
    }

    /// Deserializes the pinhole projection from the `image_from_camera` field.
    ///
    /// Returns [`re_types_core::DeserializationError::MissingData`] if the component is not present.
    pub fn image_from_camera_from_arrow(&self) -> DeserializationResult<PinholeProjection> {
        self.image_from_camera.as_ref().map_or(
            Err(re_types_core::DeserializationError::missing_data()),
            |data| {
                let v = PinholeProjection::from_arrow(&data.array)?;
                v.first()
                    .copied()
                    .ok_or_else(re_types_core::DeserializationError::missing_data)
            },
        )
    }

    /// Deserializes the resolution from the `resolution` field.
    ///
    /// Returns [`re_types_core::DeserializationError::MissingData`] if the component is not present.
    pub fn resolution_from_arrow(&self) -> DeserializationResult<Resolution> {
        self.resolution.as_ref().map_or(
            Err(re_types_core::DeserializationError::missing_data()),
            |data| {
                let v = Resolution::from_arrow(&data.array)?;
                v.first()
                    .copied()
                    .ok_or_else(re_types_core::DeserializationError::missing_data)
            },
        )
    }
}
