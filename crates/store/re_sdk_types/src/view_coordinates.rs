#![expect(missing_docs)]

pub use crate::encodings::ViewDir;

/// Failed to interpret a value as a [`ViewDir`].
#[derive(thiserror::Error, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewDirError {
    /// The character is not one of `UDRLFB`.
    #[error("Expected one of U/D/R/L/F/B (Up Down Right Left Forward Back), got {0:?}")]
    InvalidChar(char),

    /// The integer does not correspond to any [`ViewDir`].
    #[error("Could not interpret {0} as ViewDir.")]
    InvalidInteger(u8),
}

/// An invalid or unsupported [`crate::components::ViewCoordinates`].
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum ViewCoordinatesError {
    /// The coordinate system does not span all three dimensions.
    #[error(
        "Coordinate system does not cover all three cardinal directions: {}",
        .0.describe()
    )]
    MissingDimension(crate::components::ViewCoordinates),

    /// The coordinate system is degenerate.
    #[error("Found a degenerate coordinate system: {}", .0.describe())]
    Degenerate(crate::components::ViewCoordinates),

    /// The coordinate system is left-handed, which is not yet supported.
    #[error(
        "Rerun does not yet support left-handed coordinate systems (found {})",
        .0.describe()
    )]
    LeftHanded(crate::components::ViewCoordinates),

    /// A textual description did not have exactly three letters.
    #[error("Expected three letters, got: {0:?}")]
    WrongLength(String),

    /// One of the letters is not a valid [`ViewDir`].
    #[error(transparent)]
    InvalidViewDir(#[from] ViewDirError),
}

/// Failed to parse a [`SignedAxis3`] from a string.
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
#[error("Expected one of: +X -X +Y -Y +Z -Z, got {0:?}")]
pub struct SignedAxis3ParseError(pub String);

/// One of `X`, `Y`, `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Axis3 {
    X,
    Y,
    Z,
}

impl Axis3 {
    /// Convert a dimension index to an axis.
    ///
    /// * 0 => [`Self::X`]
    /// * 1 => [`Self::Y`]
    /// * 2 => [`Self::Z`]
    #[inline]
    pub fn from_dim(dim: usize) -> Self {
        match dim {
            0 => Self::X,
            1 => Self::Y,
            2 => Self::Z,
            _ => panic!("Expected a 3D axis, got {dim}"),
        }
    }
}

impl std::fmt::Display for Axis3 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => "X".fmt(f),
            Self::Y => "Y".fmt(f),
            Self::Z => "Z".fmt(f),
        }
    }
}

// ----------------------------------------------------------------------------

/// Positive (`+`) or Negative (`-`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Positive,
    Negative,
}

// ----------------------------------------------------------------------------

/// One of: `+X`, `-X`, `+Y`, `-Y`, `+Z`, `-Z`,
/// i.e. one of the six cardinal direction in 3D space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedAxis3 {
    /// Positive or negative.
    pub sign: Sign,

    /// One of `X`, `Y`, `Z`.
    pub axis: Axis3,
}

impl SignedAxis3 {
    /// +X
    pub const POSITIVE_X: Self = Self::new(Sign::Positive, Axis3::X);

    /// -X
    pub const NEGATIVE_X: Self = Self::new(Sign::Negative, Axis3::X);

    /// +Y
    pub const POSITIVE_Y: Self = Self::new(Sign::Positive, Axis3::Y);

    /// -Y
    pub const NEGATIVE_Y: Self = Self::new(Sign::Negative, Axis3::Y);

    /// +Z
    pub const POSITIVE_Z: Self = Self::new(Sign::Positive, Axis3::Z);

    /// -Z
    pub const NEGATIVE_Z: Self = Self::new(Sign::Negative, Axis3::Z);

    #[inline]
    pub const fn new(sign: Sign, axis: Axis3) -> Self {
        Self { sign, axis }
    }

    /// Convert to a unit-length 3D vector.
    #[inline]
    pub fn as_vec3(&self) -> [f32; 3] {
        match (self.sign, self.axis) {
            (Sign::Positive, Axis3::X) => [1.0, 0.0, 0.0],
            (Sign::Negative, Axis3::X) => [-1.0, 0.0, 0.0],
            (Sign::Positive, Axis3::Y) => [0.0, 1.0, 0.0],
            (Sign::Negative, Axis3::Y) => [0.0, -1.0, 0.0],
            (Sign::Positive, Axis3::Z) => [0.0, 0.0, 1.0],
            (Sign::Negative, Axis3::Z) => [0.0, 0.0, -1.0],
        }
    }
}

impl std::fmt::Display for SignedAxis3 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = match self.sign {
            Sign::Positive => "+",
            Sign::Negative => "-",
        };
        write!(f, "{}{}", sign, self.axis)
    }
}

impl std::str::FromStr for SignedAxis3 {
    type Err = SignedAxis3ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+X" => Ok(Self::new(Sign::Positive, Axis3::X)),
            "-X" => Ok(Self::new(Sign::Negative, Axis3::X)),
            "+Y" => Ok(Self::new(Sign::Positive, Axis3::Y)),
            "-Y" => Ok(Self::new(Sign::Negative, Axis3::Y)),
            "+Z" => Ok(Self::new(Sign::Positive, Axis3::Z)),
            "-Z" => Ok(Self::new(Sign::Negative, Axis3::Z)),
            _ => Err(SignedAxis3ParseError(s.to_owned())),
        }
    }
}

#[cfg(feature = "glam")]
impl From<SignedAxis3> for glam::Vec3 {
    #[inline]
    fn from(signed_axis: SignedAxis3) -> Self {
        Self::from(signed_axis.as_vec3())
    }
}

// ----------------------------------------------------------------------------

/// Left or right handedness. Used to describe a coordinate system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handedness {
    /// Right-handed coordinate system.
    Right,

    /// Left-handed coordinate system.
    ///
    /// Rerun does not yet support this,
    /// see <https://github.com/rerun-io/rerun/issues/5032>.
    Left, // TODO(#5032): Support left-handed coordinate systems.
}

impl Handedness {
    /// Create a `Handedness` from a boolean.
    ///
    /// If `true`, returns `Right`, otherwise `Left`.
    #[inline]
    pub const fn from_right_handed(right_handed: bool) -> Self {
        if right_handed {
            Self::Right
        } else {
            Self::Left
        }
    }
}
