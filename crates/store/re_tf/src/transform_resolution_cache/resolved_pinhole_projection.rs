use re_byte_size::SizeBytes;
use re_sdk_types::components;

use crate::TransformFrameIdHash;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPinholeProjection {
    /// The parent frame of the pinhole projection.
    pub parent: TransformFrameIdHash,

    pub image_from_camera: components::PinholeProjection,

    pub resolution: Option<components::Resolution>,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`re_sdk_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ`].
    pub view_coordinates: components::ViewCoordinates,
}

impl SizeBytes for ResolvedPinholeProjection {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            parent,
            image_from_camera,
            resolution,
            view_coordinates,
        } = self;

        parent.heap_size_bytes()
            + image_from_camera.heap_size_bytes()
            + resolution.heap_size_bytes()
            + view_coordinates.heap_size_bytes()
    }
}
