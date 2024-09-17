use crate::datatypes::Rgba32;

use super::AlbedoFactor;

impl Default for AlbedoFactor {
    #[inline]
    fn default() -> Self {
        Self(Rgba32::WHITE)
    }
}
