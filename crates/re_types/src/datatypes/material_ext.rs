use crate::datatypes::Rgba32;

use super::Material;

impl Material {
    #[inline]
    pub fn from_albedo_factor(color: impl Into<Rgba32>) -> Self {
        Self {
            albedo_factor: Some(color.into()),
            albedo_texture: None,
        }
    }
}
