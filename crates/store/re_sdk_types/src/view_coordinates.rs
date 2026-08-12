#![expect(missing_docs)]

pub use crate::datatypes::ViewDir;

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
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+X" => Ok(Self::new(Sign::Positive, Axis3::X)),
            "-X" => Ok(Self::new(Sign::Negative, Axis3::X)),
            "+Y" => Ok(Self::new(Sign::Positive, Axis3::Y)),
            "-Y" => Ok(Self::new(Sign::Negative, Axis3::Y)),
            "+Z" => Ok(Self::new(Sign::Positive, Axis3::Z)),
            "-Z" => Ok(Self::new(Sign::Negative, Axis3::Z)),
            _ => Err("Expected one of: +X -X +Y -Y +Z -Z".to_owned()),
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
