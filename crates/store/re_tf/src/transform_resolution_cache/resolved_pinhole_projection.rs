use std::ops::Deref;

use re_sdk_types::components;

use crate::TransformFrameIdHash;

#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct ResolvedPinholeProjection {
    /// All components that are updated atomically are cached.
    pub(crate) cached: ResolvedPinholeProjectionCached,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`re_sdk_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ`].
    pub view_coordinates: components::ViewCoordinates,
}

impl Deref for ResolvedPinholeProjection {
    type Target = ResolvedPinholeProjectionCached;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.cached
    }
}

#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct ResolvedPinholeProjectionCached {
    /// The parent frame of the pinhole projection.
    pub parent: TransformFrameIdHash,

    pub image_from_camera: components::PinholeProjection,

    pub resolution: Option<components::Resolution>,
}
