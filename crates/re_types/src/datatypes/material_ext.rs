use crate::datatypes::Rgba32;

use super::Material;

impl Material {
    /// A new material using a given color multiplier.
    #[inline]
    pub fn from_albedo_factor(color: impl Into<Rgba32>) -> Self {
        Self {
            albedo_factor: Some(color.into()),
        }
    }
}
