use half::f16;

use super::SphericalHarmonics3;

/// Keeps [`SphericalHarmonics3::NUM_COEFFICIENTS`] in sync with the array length declared in
/// `spherical_harmonics3.fbs`, which is what the wrapped type is generated from.
const _: () = assert!(
    size_of::<SphericalHarmonics3>()
        == SphericalHarmonics3::NUM_COEFFICIENTS * 3 * size_of::<f16>()
);

impl SphericalHarmonics3 {
    /// The number of coefficients of degrees 1 through 3, i.e. the number of RGB triples.
    pub const NUM_COEFFICIENTS: usize = 15;

    /// Create from 15 RGB coefficient triples of `f32`, converting each to half-precision.
    #[inline]
    pub fn from_f32(coefficients: [[f32; 3]; Self::NUM_COEFFICIENTS]) -> Self {
        Self(coefficients.map(|rgb| rgb.map(f16::from_f32)))
    }
}

impl From<[[f32; 3]; Self::NUM_COEFFICIENTS]> for SphericalHarmonics3 {
    #[inline]
    fn from(coefficients: [[f32; 3]; Self::NUM_COEFFICIENTS]) -> Self {
        Self::from_f32(coefficients)
    }
}
