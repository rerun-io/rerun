use super::Resolution2D;

impl Resolution2D {
    /// From `width` and `height`.
    #[inline]
    pub fn new(width: u32, height: u32) -> Self {
        Self(crate::datatypes::UVec2D::new(width, height))
    }

    /// Width
    #[inline]
    pub fn width(&self) -> u32 {
        self.0.x()
    }

    /// Height
    #[inline]
    pub fn height(&self) -> u32 {
        self.0.y()
    }
}
