use super::Angle;
use std::fmt::Formatter;

impl Angle {
    /// Angle in radians independent of the underlying representation.
    #[inline]
    pub fn radians(&self) -> f32 {
        match self {
            Self::Radians(v) => *v,
            Self::Degrees(v) => v.to_radians(),
        }
    }

    /// Angle in degrees independent of the underlying representation.
    #[inline]
    pub fn degrees(&self) -> f32 {
        match self {
            Self::Radians(v) => v.to_degrees(),
            Self::Degrees(v) => *v,
        }
    }
}

impl std::fmt::Display for Angle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Radians(v) => {
                v.fmt(f)?;
                write!(f, " rad",)
            }
            Self::Degrees(v) => {
                v.fmt(f)?;
                write!(f, " deg")
            }
        }
    }
}
