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

    /// width * height
    #[inline]
    pub fn area(&self) -> usize {
        self.width() as usize * self.height() as usize
    }
}

impl std::fmt::Display for Resolution2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width(), self.height())
    }
}
