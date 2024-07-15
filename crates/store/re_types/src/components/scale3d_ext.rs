use super::Scale3D;

impl Scale3D {
    /// Scale the same amount along all axis.
    #[inline]
    pub fn uniform(value: f32) -> Self {
        Self(crate::datatypes::Vec3D([value, value, value]))
    }
}

impl From<f32> for Scale3D {
    #[inline]
    fn from(value: f32) -> Self {
        Self(crate::datatypes::Vec3D([value, value, value]))
    }
}
