use re_types_core::datatypes::Float32;

use super::Length;

impl Default for Length {
    #[inline]
    fn default() -> Self {
        Length(Float32(1.0))
    }
}
