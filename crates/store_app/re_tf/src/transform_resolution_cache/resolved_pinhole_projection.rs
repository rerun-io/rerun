use std::ops::Deref;

use re_sdk_types::components;

use crate::TransformFrameIdHash;

#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct ResolvedPinholeProjection {
    /// All components that are updated atomically are cached.
    pub(crate) cached: ResolvedPinholeProjectionCached,

    /// View coordinates at this pinhole camera.
    ///
    /// This orients embedded 2D data in 3D and projected 3D data in 2D.
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
