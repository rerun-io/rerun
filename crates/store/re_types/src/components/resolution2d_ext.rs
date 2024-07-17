use super::Resolution2D;

impl Resolution2D {
    /// From `width` and `height`.
    #[inline]
    pub fn new(width: u32, height: u32) -> Self {
        Self(crate::datatypes::UVec2D::new(width, height))
    }
}
