use crate::datatypes::Color;

use super::Material;

impl Material {
    #[inline]
    pub fn from_albedo_factor(color: impl Into<Color>) -> Self {
        Self(crate::datatypes::Material::from_albedo_factor(color))
    }
}
