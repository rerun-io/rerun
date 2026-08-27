use super::SphericalHarmonicsDegree;

impl Default for SphericalHarmonicsDegree {
    /// Use every coefficient the data has.
    #[inline]
    fn default() -> Self {
        Self(Self::MAX.into())
    }
}

impl SphericalHarmonicsDegree {
    /// The highest degree [`super::SphericalHarmonics3Rgb`] can express.
    pub const MAX: u32 = 3;

    /// How many coefficients this degree needs: `(degree + 1)² - 1`, i.e. 0, 3, 8 or 15.
    ///
    /// The degree-0 (DC) term is the gaussian's color and not counted here.
    #[inline]
    pub fn num_coefficients(self) -> usize {
        let degree = u64::from(self.0.0);
        let num_coefficients = (degree + 1).saturating_mul(degree + 1) - 1;
        usize::try_from(num_coefficients).unwrap_or(usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::SphericalHarmonicsDegree;

    #[test]
    fn num_coefficients_per_degree() {
        // `(degree + 1)² - 1`, excluding the DC term.
        assert_eq!(SphericalHarmonicsDegree(0.into()).num_coefficients(), 0);
        assert_eq!(SphericalHarmonicsDegree(1.into()).num_coefficients(), 3);
        assert_eq!(SphericalHarmonicsDegree(2.into()).num_coefficients(), 8);
        assert_eq!(SphericalHarmonicsDegree(3.into()).num_coefficients(), 15);
    }

    #[test]
    fn default_is_max() {
        assert_eq!(
            SphericalHarmonicsDegree::default(),
            SphericalHarmonicsDegree(SphericalHarmonicsDegree::MAX.into())
        );
    }

    #[test]
    fn degrees_above_max_dont_overflow() {
        // Degrees above `MAX` are the caller's problem (the renderer only uploads 15
        // coefficients), but the count must not overflow on the way there.
        assert_eq!(SphericalHarmonicsDegree(4.into()).num_coefficients(), 24);
        assert_eq!(
            SphericalHarmonicsDegree(65_535.into()).num_coefficients(),
            65_536 * 65_536 - 1
        );
        // Saturates instead of overflowing, on 32-bit `usize` targets too:
        assert!(15 < SphericalHarmonicsDegree(u32::MAX.into()).num_coefficients());
    }
}
