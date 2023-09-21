use crate::datatypes::Color;

use super::Material;

impl Material {
    #[inline]
    pub fn from_albedo_factor(color: impl Into<Color>) -> Self {
        Self {
            albedo_factor: Some(color.into()),
        }
    }
}
