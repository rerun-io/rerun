use re_types_core::datatypes::Float32;

use super::ClippingPlane;

impl Default for ClippingPlane {
    #[inline]
    fn default() -> Self {
        // Default clipping plane set at a reasonable distance for common cameras
        Self(Float32(0.1))
    }
}
