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

#[allow(clippy::derivable_impls)] // Soon no longer be derivable, also wanted to comment on choices here.
impl Default for Material {
    #[inline]
    fn default() -> Self {
        Self {
            // TODO(andreas): Would be nicer to not make this optional and just use white as default factor.
            albedo_factor: None,
        }
    }
}
