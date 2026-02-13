use re_types_core::datatypes::Float32;

use super::NearClipPlane;

impl Default for NearClipPlane {
    #[inline]
    fn default() -> Self {
        // Default near clip plane to reasonable distance for common cameras
        Self(Float32(0.1))
    }
}
