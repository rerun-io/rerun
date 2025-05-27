use re_types_core::{DeserializationResult, Loggable as _};

use crate::{
    components::{PinholeProjection, Resolution, ViewCoordinates},
    datatypes::Vec2D,
};

use super::Pinhole;

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

    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    ///
    /// Only returns a result if both projection & resolution are set.
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::image_from_camera_from_arrow` & `Pinhole::resolution_from_arrow` to deserialize the components back,
                or better use the components prior to passing it to the archetype, and then call `PinholeProjection::fov_y`",
        since = "0.22.0"
    )]
    pub fn fov_y(&self) -> Option<f32> {
        self.image_from_camera_from_arrow()
            .ok()
            .and_then(|projection| {
                self.resolution_from_arrow()
                    .ok()
                    .map(|r| projection.fov_y(r))
            })
    }

    /// The resolution of the camera sensor in pixels.
    #[inline]
    #[cfg(feature = "glam")]
    #[deprecated(
        note = "Use `Pinhole::resolution_from_arrow` to deserialize back to a `Resolution` component,
                or better use the `Resolution` prior to passing it to the archetype, and then use `Resolution::into` for the conversion",
        since = "0.22.0"
    )]
    pub fn resolution(&self) -> Option<glam::Vec2> {
        self.resolution_from_arrow().ok().map(|r| r.into())
    }

    /// Width/height ratio of the camera sensor.
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::resolution_from_arrow` to deserialize back to a `Resolution` component,
                or better use the `Resolution` prior to passing it to the archetype, and then use `Resolution::aspect_ratio`",
        since = "0.22.0"
    )]
    pub fn aspect_ratio(&self) -> Option<f32> {
        self.resolution_from_arrow().ok().map(|r| r.aspect_ratio())
    }

    /// Deserializes the pinhole projection from the `image_from_camera` field.
    ///
    /// Returns [`re_types_core::DeserializationError::MissingData`] if the component is not present.
    pub fn image_from_camera_from_arrow(&self) -> DeserializationResult<PinholeProjection> {
        self.image_from_camera.as_ref().map_or(
            Err(re_types_core::DeserializationError::missing_data()),
            |data| {
                PinholeProjection::from_arrow(&data.array).and_then(|v| {
                    v.first()
                        .copied()
                        .ok_or(re_types_core::DeserializationError::missing_data())
                })
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
                Resolution::from_arrow(&data.array).and_then(|v| {
                    v.first()
                        .copied()
                        .ok_or(re_types_core::DeserializationError::missing_data())
                })
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

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::image_from_camera_from_arrow` instead to deserialize back to a `PinholeProjection` component,
                or better use the `PinholeProjection` component prior to passing it to the archetype, and then call PinholeProjection::focal_length_in_pixels",
        since = "0.22.0"
    )]
    pub fn focal_length_in_pixels(&self) -> Vec2D {
        self.image_from_camera_from_arrow()
            .unwrap_or_default()
            .focal_length_in_pixels()
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::image_from_camera_from_arrow` instead to deserialize back to a `PinholeProjection` component,
                or better use the `PinholeProjection` component prior to passing it to the archetype, and then call PinholeProjection::principal_point",
        since = "0.22.0"
    )]
    pub fn principal_point(&self) -> glam::Vec2 {
        self.image_from_camera_from_arrow()
            .unwrap_or_default()
            .principal_point()
    }

    /// Project camera-space coordinates into pixel coordinates,
    /// returning the same z/depth.
    #[cfg(feature = "glam")]
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::image_from_camera_from_arrow` instead to deserialize back to a `PinholeProjection` component,
                or better use the `PinholeProjection` component prior to passing it to the archetype, and then call PinholeProjection::project",
        since = "0.22.0"
    )]
    pub fn project(&self, pixel: glam::Vec3) -> glam::Vec3 {
        self.image_from_camera_from_arrow()
            .unwrap_or_default()
            .project(pixel)
    }

    /// Given pixel coordinates and a world-space depth,
    /// return a position in the camera space.
    ///
    /// The returned z is the same as the input z (depth).
    #[cfg(feature = "glam")]
    #[inline]
    #[deprecated(
        note = "Use `Pinhole::image_from_camera_from_arrow` instead to deserialize back to a `PinholeProjection` component,
                or better use the `PinholeProjection` component prior to passing it to the archetype, and then call PinholeProjection::unproject",
        since = "0.22.0"
    )]
    pub fn unproject(&self, pixel: glam::Vec3) -> glam::Vec3 {
        self.image_from_camera_from_arrow()
            .unwrap_or_default()
            .unproject(pixel)
    }
}
