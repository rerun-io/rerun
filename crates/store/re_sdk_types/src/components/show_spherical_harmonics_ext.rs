use super::ShowSphericalHarmonics;

impl Default for ShowSphericalHarmonics {
    /// Gaussians render with view-dependent color by default.
    #[inline]
    fn default() -> Self {
        Self(true.into())
    }
}
