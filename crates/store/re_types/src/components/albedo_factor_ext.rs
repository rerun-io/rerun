use super::AlbedoFactor;
use crate::datatypes::Rgba32;

impl Default for AlbedoFactor {
    #[inline]
    fn default() -> Self {
        Self(Rgba32::WHITE)
    }
}
