use crate::datatypes::Rgba32;

use super::Material;

impl Material {
    #[inline]
    pub fn from_albedo_factor(color: impl Into<Rgba32>) -> Self {
        Self(crate::datatypes::Material::from_albedo_factor(color))
    }
}
