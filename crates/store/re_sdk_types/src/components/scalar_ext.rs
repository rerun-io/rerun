use re_types_core::datatypes::Float64;

use super::Scalar;

impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for Scalar {
    #[inline]
    fn default() -> Self {
        Self(Float64(0.0))
    }
}
