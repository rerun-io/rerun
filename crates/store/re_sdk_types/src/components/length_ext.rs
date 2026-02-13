use re_types_core::datatypes::Float32;

use super::Length;

impl Default for Length {
    #[inline]
    fn default() -> Self {
        Self(Float32(1.0))
    }
}
