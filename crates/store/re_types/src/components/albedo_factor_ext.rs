use crate::datatypes::Rgba32;

use super::AlbedoFactor;

impl Default for AlbedoFactor {
    #[inline]
    fn default() -> Self {
        AlbedoFactor(Rgba32::WHITE)
    }
}
