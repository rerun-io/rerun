use super::Angle;
use std::fmt::Formatter;

impl Angle {
    /// Zero angle, often used for representing no rotation.
    pub const ZERO: Self = Self { radians: 0.0 };

    /// Angle in radians.
    #[inline]
    pub fn radians(&self) -> f32 {
        self.radians
    }

    /// Angle in degrees (converts from radians).
    #[inline]
    pub fn degrees(&self) -> f32 {
        self.radians.to_degrees()
    }

    /// Create a new angle from degrees.
    ///
    /// Converts the value to radians.
    ///
    /// Deprecated method to mimic previous enum variant.
    #[allow(non_snake_case)]
    #[deprecated(since = "0.18.0", note = "Use `Angle::from_degrees` instead.")]
    #[inline]
    pub fn Degrees(degrees: f32) -> Self {
        Self::from_degrees(degrees)
    }

    /// Create a new angle from radians.
    ///
    /// Deprecated method to mimic previous enum variant.
    #[allow(non_snake_case)]
    #[deprecated(since = "0.18.0", note = "Use `Angle::from_radians` instead.")]
    #[inline]
    pub fn Radians(radians: f32) -> Self {
        Self { radians }
    }

    /// Create a new angle from degrees.
    ///
    /// Converts the value to radians.
    #[inline]
    pub fn from_degrees(degrees: f32) -> Self {
        Self {
            radians: degrees.to_radians(),
        }
    }

    /// Create a new angle from radians.
    #[inline]
    pub fn from_radians(radians: f32) -> Self {
        Self { radians }
    }
}

impl std::fmt::Display for Angle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(crate::DEFAULT_DISPLAY_DECIMALS);
        write!(f, "{:.prec$} rad", re_format::format_f32(self.radians))
    }
}
