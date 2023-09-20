#![allow(clippy::wrong_self_convention)] // TODO(emilk): re-enable
                                         // ----------------------------------------------------------------------------

use crate::view_dir::ViewDir;

use super::ViewCoordinates;

impl ViewCoordinates {
    /// Construct a new `ViewCoordinates` from a [`ViewDir`]
    pub const fn new(coordinates: [ViewDir; 3]) -> Self {
        Self([
            coordinates[0] as u8,
            coordinates[1] as u8,
            coordinates[2] as u8,
        ])
    }

    /// Choses a coordinate system based on just an up-axis.
    pub fn from_up_and_handedness(up: SignedAxis3, handedness: Handedness) -> Self {
        use ViewDir::{Back, Down, Forward, Right, Up};
        match handedness {
            Handedness::Right => match up {
                SignedAxis3::POSITIVE_X => Self::new([Up, Right, Forward]),
                SignedAxis3::NEGATIVE_X => Self::new([Down, Right, Back]),
                SignedAxis3::POSITIVE_Y => Self::new([Right, Up, Back]),
                SignedAxis3::NEGATIVE_Y => Self::new([Right, Down, Forward]),
                SignedAxis3::POSITIVE_Z => Self::new([Right, Forward, Up]),
                SignedAxis3::NEGATIVE_Z => Self::new([Right, Back, Down]),
            },
            Handedness::Left => match up {
                SignedAxis3::POSITIVE_X => Self::new([Up, Right, Back]),
                SignedAxis3::NEGATIVE_X => Self::new([Down, Right, Forward]),
                SignedAxis3::POSITIVE_Y => Self::new([Right, Up, Forward]),
                SignedAxis3::NEGATIVE_Y => Self::new([Right, Down, Back]),
                SignedAxis3::POSITIVE_Z => Self::new([Right, Back, Up]),
                SignedAxis3::NEGATIVE_Z => Self::new([Right, Forward, Down]),
            },
        }
    }

    /// Returns an error if this does not span all three dimensions.
    pub fn sanity_check(&self) -> Result<(), String> {
        let mut dims = [false; 3];
        for dir in self.0 {
            let dim = match ViewDir::try_from(dir)? {
                ViewDir::Up | ViewDir::Down => 0,
                ViewDir::Right | ViewDir::Left => 1,
                ViewDir::Forward | ViewDir::Back => 2,
            };
            dims[dim] = true;
        }
        if dims == [true; 3] {
            Ok(())
        } else {
            Err(format!(
                "Coordinate system does not cover all three cardinal directions: {}",
                self.describe()
            ))
        }
    }

    #[inline]
    pub fn up(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == ViewDir::Up as u8 {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Down as u8 {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

    #[inline]
    pub fn right(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == ViewDir::Right as u8 {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Left as u8 {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

    #[inline]
    pub fn forward(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == ViewDir::Forward as u8 {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Back as u8 {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

    pub fn describe_short(&self) -> String {
        let [x, y, z] = self.0;
        let x = ViewDir::try_from(x).map(|x| x.short()).unwrap_or("?");
        let y = ViewDir::try_from(y).map(|y| y.short()).unwrap_or("?");
        let z = ViewDir::try_from(z).map(|z| z.short()).unwrap_or("?");
        format!("{x}{y}{z}")
    }

    pub fn describe(&self) -> String {
        let [x, y, z] = self.0;
        let x_short = ViewDir::try_from(x).map(|x| x.short()).unwrap_or("?");
        let y_short = ViewDir::try_from(y).map(|y| y.short()).unwrap_or("?");
        let z_short = ViewDir::try_from(z).map(|z| z.short()).unwrap_or("?");
        let x_long = ViewDir::try_from(x).map(|x| x.long()).unwrap_or("?");
        let y_long = ViewDir::try_from(y).map(|y| y.long()).unwrap_or("?");
        let z_long = ViewDir::try_from(z).map(|z| z.long()).unwrap_or("?");
        format!("{x_short}{y_short}{z_short} (X={x_long}, Y={y_long}, Z={z_long})",)
    }

    /// Returns a matrix that transforms from another coordinate system to this (self) one.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn try_from_other(&self, other: &Self) -> Result<glam::Mat3, String> {
        Ok(self.from_rdf()? * other.to_rdf()?)
    }

    /// Returns a matrix that transforms this coordinate system to RDF.
    ///
    /// (RDF: X=Right, Y=Down, Z=Forward)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rdf(&self) -> Result<glam::Mat3, String> {
        fn rdf(dir: ViewDir) -> [f32; 3] {
            match dir {
                ViewDir::Right => [1.0, 0.0, 0.0],
                ViewDir::Left => [-1.0, 0.0, 0.0],
                ViewDir::Up => [0.0, -1.0, 0.0],
                ViewDir::Down => [0.0, 1.0, 0.0],
                ViewDir::Back => [0.0, 0.0, -1.0],
                ViewDir::Forward => [0.0, 0.0, 1.0],
            }
        }

        Ok(glam::Mat3::from_cols_array_2d(&[
            rdf(ViewDir::try_from(self.0[0])?),
            rdf(ViewDir::try_from(self.0[1])?),
            rdf(ViewDir::try_from(self.0[2])?),
        ]))
    }

    /// Returns a matrix that transforms from RDF to this coordinate system.
    ///
    /// (RDF: X=Right, Y=Down, Z=Forward)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rdf(&self) -> Result<glam::Mat3, String> {
        Ok(self.to_rdf()?.transpose())
    }

    /// Returns a matrix that transforms this coordinate system to RUB.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rub(&self) -> Result<glam::Mat3, String> {
        fn rub(dir: ViewDir) -> [f32; 3] {
            match dir {
                ViewDir::Right => [1.0, 0.0, 0.0],
                ViewDir::Left => [-1.0, 0.0, 0.0],
                ViewDir::Up => [0.0, 1.0, 0.0],
                ViewDir::Down => [0.0, -1.0, 0.0],
                ViewDir::Back => [0.0, 0.0, 1.0],
                ViewDir::Forward => [0.0, 0.0, -1.0],
            }
        }

        Ok(glam::Mat3::from_cols_array_2d(&[
            rub(ViewDir::try_from(self.0[0])?),
            rub(ViewDir::try_from(self.0[1])?),
            rub(ViewDir::try_from(self.0[2])?),
        ]))
    }

    /// Returns a matrix that transforms from RUB to this coordinate system.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rub(&self) -> Result<glam::Mat3, String> {
        Ok(self.to_rub()?.transpose())
    }

    /// Returns a quaternion that rotates from RUB to this coordinate system.
    ///
    /// Errors if the coordinate system is left-handed or degenerate.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rub_quat(&self) -> Result<glam::Quat, String> {
        let mat3 = self.from_rub()?;

        let det = mat3.determinant();
        if det == 1.0 {
            Ok(glam::Quat::from_mat3(&mat3))
        } else if det == -1.0 {
            Err(format!(
                "Rerun does not yet support left-handed coordinate systems (found {})",
                self.describe()
            ))
        } else {
            Err(format!(
                "Found a degenerate coordinate system: {}",
                self.describe()
            ))
        }
    }

    #[cfg(feature = "glam")]
    #[inline]
    pub fn handedness(&self) -> Result<Handedness, String> {
        let to_rdf = self.to_rdf()?;
        let det = to_rdf.determinant();
        if det == -1.0 {
            Ok(Handedness::Left)
        } else if det == 0.0 {
            Err(format!("Invalid ViewCoordinate: {}", self.describe()))
        } else {
            Ok(Handedness::Right)
        }
    }
}

impl std::str::FromStr for ViewCoordinates {
    type Err = String;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            [x, y, z] => {
                let slf = Self::new([
                    ViewDir::from_ascii_char(*x)?,
                    ViewDir::from_ascii_char(*y)?,
                    ViewDir::from_ascii_char(*z)?,
                ]);
                slf.sanity_check()?;
                Ok(slf)
            }
            _ => Err(format!("Expected three letters, got: {s:?}")),
        }
    }
}

// ----------------------------------------------------------------------------

/// One of `X`, `Y`, `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Axis3 {
    X,
    Y,
    Z,
}

impl Axis3 {
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Sign {
    Positive,
    Negative,
}

// ----------------------------------------------------------------------------

/// One of: `+X`, `-X`, `+Y`, `-Y`, `+Z`, `-Z`,
/// i.e. one of the six cardinal direction in 3D space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SignedAxis3 {
    pub sign: Sign,
    pub axis: Axis3,
}

impl SignedAxis3 {
    pub const POSITIVE_X: Self = Self::new(Sign::Positive, Axis3::X);
    pub const NEGATIVE_X: Self = Self::new(Sign::Negative, Axis3::X);
    pub const POSITIVE_Y: Self = Self::new(Sign::Positive, Axis3::Y);
    pub const NEGATIVE_Y: Self = Self::new(Sign::Negative, Axis3::Y);
    pub const POSITIVE_Z: Self = Self::new(Sign::Positive, Axis3::Z);
    pub const NEGATIVE_Z: Self = Self::new(Sign::Negative, Axis3::Z);

    #[inline]
    pub const fn new(sign: Sign, axis: Axis3) -> Self {
        Self { sign, axis }
    }

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
        glam::Vec3::from(signed_axis.as_vec3())
    }
}

// ----------------------------------------------------------------------------

/// Left or right handedness. Used to describe a coordinate system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Handedness {
    Right,
    Left,
}

impl Handedness {
    #[inline]
    pub const fn from_right_handed(right_handed: bool) -> Self {
        if right_handed {
            Handedness::Right
        } else {
            Handedness::Left
        }
    }

    #[inline]
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Left => "left handed",
            Self::Right => "right handed",
        }
    }
}
