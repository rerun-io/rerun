use super::FisheyeCoefficients;

impl FisheyeCoefficients {
    /// Returns the k1 distortion coefficient.
    #[inline]
    pub fn k1(&self) -> f32 {
        self.0.x()
    }

    /// Returns the k2 distortion coefficient.
    #[inline]
    pub fn k2(&self) -> f32 {
        self.0.y()
    }

    /// Returns the k3 distortion coefficient.
    #[inline]
    pub fn k3(&self) -> f32 {
        self.0.z()
    }

    /// Returns the k4 distortion coefficient.
    #[inline]
    pub fn k4(&self) -> f32 {
        self.0.w()
    }
}

impl Default for FisheyeCoefficients {
    #[inline]
    fn default() -> Self {
        Self(crate::datatypes::Vec4D([0.0, 0.0, 0.0, 0.0]))
    }
}
