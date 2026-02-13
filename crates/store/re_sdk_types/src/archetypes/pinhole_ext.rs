use re_types_core::{DeserializationResult, Loggable as _};

use super::Pinhole;
use crate::components::{PinholeProjection, Resolution, ViewCoordinates};
use crate::datatypes::Vec2D;

impl Pinhole {
    /// Camera orientation used when there's no camera orientation explicitly logged.
    ///
    /// - x pointing right
    /// - y pointing down
    /// - z pointing into the image plane
    ///   (this is convenient for reading out a depth image which has typically positive z values)
    pub const DEFAULT_CAMERA_XYZ: ViewCoordinates = ViewCoordinates::RDF;

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

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    ///
    /// <div class="warning">
    /// This method can be relatively costly since it has to deserialize & reserialize the [`PinholeProjection`] component
    /// from/to its arrow representation.
    /// On performance critical paths, prefer first setting up the [`PinholeProjection`] component fully
    /// before passing it to the archetype.
    /// </div>
    #[cfg(feature = "glam")]
    #[inline]
    pub fn with_principal_point(mut self, principal_point: impl Into<Vec2D>) -> Self {
        use re_types_core::ComponentBatch as _;

        let image_from_camera = self.image_from_camera_from_arrow().unwrap_or_default();
        self.image_from_camera = image_from_camera
            .with_principal_point(principal_point)
            .serialized(Self::descriptor_image_from_camera());
        self
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

    // ------------------------------------------------------------------------
    // Forwarding calls to `PinholeProjection`:
    //
    // Starting with 0.22 all component data is stored serialized in the archetype,
    // therefore it's recommended to instead use the `PinholeProjection` component before passing it to the archetype.
    // Using `Pinhole::image_from_camera_from_arrow()` it is possible to deserialize the `PinholeProjection` component
    // if it has been already serialized/stored in the archetype struct.
}
