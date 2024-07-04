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
                write!(f, "{} rad", re_format::format_f32(*v))
            }
            Self::Degrees(v) => {
                // TODO(andreas): Convert to arc minutes/seconds for very small angles.
                // That code should be in re_format!
                write!(f, "{} °", re_format::format_f32(*v))
            }
        }
    }
}
