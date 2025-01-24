use super::Resolution;

impl Default for Resolution {
    #[inline]
    fn default() -> Self {
        // Picking anything specific seems more arbitrary than just 0.
        [0.0, 0.0].into()
    }
}

impl Resolution {
    /// Width/height ratio.
    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        self[0] / self[1]
    }
}

#[cfg(feature = "glam")]
impl From<Resolution> for glam::Vec2 {
    #[inline]
    fn from(resolution: Resolution) -> Self {
        glam::vec2(resolution[0], resolution[1])
    }
}
