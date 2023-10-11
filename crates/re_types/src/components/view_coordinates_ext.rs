#![allow(clippy::wrong_self_convention)] // TODO(emilk): re-enable

// ----------------------------------------------------------------------------

use crate::view_coordinates::{Axis3, Handedness, Sign, SignedAxis3, ViewDir};

use super::ViewCoordinates;

impl Default for ViewCoordinates {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ViewCoordinates {
    /// The `ViewCoordinates` used when none have been specified.
    pub const DEFAULT: Self = Self::RUB;

    /// Construct a new `ViewCoordinates` from an array of [`ViewDir`]s.
    pub const fn new(x: ViewDir, y: ViewDir, z: ViewDir) -> Self {
        Self([x as u8, y as u8, z as u8])
    }

    /// Choses a coordinate system based on just an up-axis.
    pub fn from_up_and_handedness(up: SignedAxis3, handedness: Handedness) -> Self {
        use ViewDir::{Back, Down, Forward, Right, Up};
        match handedness {
            Handedness::Right => match up {
                SignedAxis3::POSITIVE_X => Self::new(Up, Right, Forward),
                SignedAxis3::NEGATIVE_X => Self::new(Down, Right, Back),
                SignedAxis3::POSITIVE_Y => Self::new(Right, Up, Back),
                SignedAxis3::NEGATIVE_Y => Self::new(Right, Down, Forward),
                SignedAxis3::POSITIVE_Z => Self::new(Right, Forward, Up),
                SignedAxis3::NEGATIVE_Z => Self::new(Right, Back, Down),
            },
            Handedness::Left => match up {
                SignedAxis3::POSITIVE_X => Self::new(Up, Right, Back),
                SignedAxis3::NEGATIVE_X => Self::new(Down, Right, Forward),
                SignedAxis3::POSITIVE_Y => Self::new(Right, Up, Forward),
                SignedAxis3::NEGATIVE_Y => Self::new(Right, Down, Back),
                SignedAxis3::POSITIVE_Z => Self::new(Right, Back, Up),
                SignedAxis3::NEGATIVE_Z => Self::new(Right, Forward, Down),
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
    pub fn from_other(&self, other: &Self) -> glam::Mat3 {
        self.from_rdf() * other.to_rdf()
    }

    /// Returns a matrix that transforms this coordinate system to RDF.
    ///
    /// (RDF: X=Right, Y=Down, Z=Forward)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rdf(&self) -> glam::Mat3 {
        fn rdf(dir: Option<ViewDir>) -> [f32; 3] {
            match dir {
                Some(ViewDir::Right) => [1.0, 0.0, 0.0],
                Some(ViewDir::Left) => [-1.0, 0.0, 0.0],
                Some(ViewDir::Up) => [0.0, -1.0, 0.0],
                Some(ViewDir::Down) => [0.0, 1.0, 0.0],
                Some(ViewDir::Back) => [0.0, 0.0, -1.0],
                Some(ViewDir::Forward) => [0.0, 0.0, 1.0],
                // TODO(jleibs): Is there a better value to return here?
                // this means the ViewCoordinates aren't valid.
                None => [0.0, 0.0, 0.0],
            }
        }

        glam::Mat3::from_cols_array_2d(&[
            rdf(ViewDir::try_from(self.0[0]).ok()),
            rdf(ViewDir::try_from(self.0[1]).ok()),
            rdf(ViewDir::try_from(self.0[2]).ok()),
        ])
    }

    /// Returns a matrix that transforms from RDF to this coordinate system.
    ///
    /// (RDF: X=Right, Y=Down, Z=Forward)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rdf(&self) -> glam::Mat3 {
        self.to_rdf().transpose()
    }

    /// Returns a matrix that transforms this coordinate system to RUB.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rub(&self) -> glam::Mat3 {
        fn rub(dir: Option<ViewDir>) -> [f32; 3] {
            match dir {
                Some(ViewDir::Right) => [1.0, 0.0, 0.0],
                Some(ViewDir::Left) => [-1.0, 0.0, 0.0],
                Some(ViewDir::Up) => [0.0, 1.0, 0.0],
                Some(ViewDir::Down) => [0.0, -1.0, 0.0],
                Some(ViewDir::Back) => [0.0, 0.0, 1.0],
                Some(ViewDir::Forward) => [0.0, 0.0, -1.0],
                // TODO(jleibs): Is there a better value to return here?
                // this means the ViewCoordinates aren't valid.
                None => [0.0, 0.0, 0.0],
            }
        }

        glam::Mat3::from_cols_array_2d(&[
            rub(ViewDir::try_from(self.0[0]).ok()),
            rub(ViewDir::try_from(self.0[1]).ok()),
            rub(ViewDir::try_from(self.0[2]).ok()),
        ])
    }

    /// Returns a matrix that transforms from RUB to this coordinate system.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rub(&self) -> glam::Mat3 {
        self.to_rub().transpose()
    }

    /// Returns a quaternion that rotates from RUB to this coordinate system.
    ///
    /// Errors if the coordinate system is left-handed or degenerate.
    ///
    /// (RUB: X=Right, Y=Up, Z=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rub_quat(&self) -> Result<glam::Quat, String> {
        let mat3 = self.from_rub();

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
        let to_rdf = self.to_rdf();
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
                let slf = Self::new(
                    ViewDir::from_ascii_char(*x)?,
                    ViewDir::from_ascii_char(*y)?,
                    ViewDir::from_ascii_char(*z)?,
                );
                slf.sanity_check()?;
                Ok(slf)
            }
            _ => Err(format!("Expected three letters, got: {s:?}")),
        }
    }
}

// ----------------------------------------------------------------------------

macro_rules! define_coordinates {
    ($name:ident => ($x:ident, $y:ident, $z:ident) ) => {
        pub const $name: Self = Self([ViewDir::$x as u8, ViewDir::$y as u8, ViewDir::$z as u8]);
    };
}

impl ViewCoordinates {
    // <BEGIN_GENERATED:declarations>
    // This section is generated by running `scripts/generate_view_coordinate_defs.py --rust`
    define_coordinates!(ULF => (Up, Left, Forward));
    define_coordinates!(UFL => (Up, Forward, Left));
    define_coordinates!(LUF => (Left, Up, Forward));
    define_coordinates!(LFU => (Left, Forward, Up));
    define_coordinates!(FUL => (Forward, Up, Left));
    define_coordinates!(FLU => (Forward, Left, Up));
    define_coordinates!(ULB => (Up, Left, Back));
    define_coordinates!(UBL => (Up, Back, Left));
    define_coordinates!(LUB => (Left, Up, Back));
    define_coordinates!(LBU => (Left, Back, Up));
    define_coordinates!(BUL => (Back, Up, Left));
    define_coordinates!(BLU => (Back, Left, Up));
    define_coordinates!(URF => (Up, Right, Forward));
    define_coordinates!(UFR => (Up, Forward, Right));
    define_coordinates!(RUF => (Right, Up, Forward));
    define_coordinates!(RFU => (Right, Forward, Up));
    define_coordinates!(FUR => (Forward, Up, Right));
    define_coordinates!(FRU => (Forward, Right, Up));
    define_coordinates!(URB => (Up, Right, Back));
    define_coordinates!(UBR => (Up, Back, Right));
    define_coordinates!(RUB => (Right, Up, Back));
    define_coordinates!(RBU => (Right, Back, Up));
    define_coordinates!(BUR => (Back, Up, Right));
    define_coordinates!(BRU => (Back, Right, Up));
    define_coordinates!(DLF => (Down, Left, Forward));
    define_coordinates!(DFL => (Down, Forward, Left));
    define_coordinates!(LDF => (Left, Down, Forward));
    define_coordinates!(LFD => (Left, Forward, Down));
    define_coordinates!(FDL => (Forward, Down, Left));
    define_coordinates!(FLD => (Forward, Left, Down));
    define_coordinates!(DLB => (Down, Left, Back));
    define_coordinates!(DBL => (Down, Back, Left));
    define_coordinates!(LDB => (Left, Down, Back));
    define_coordinates!(LBD => (Left, Back, Down));
    define_coordinates!(BDL => (Back, Down, Left));
    define_coordinates!(BLD => (Back, Left, Down));
    define_coordinates!(DRF => (Down, Right, Forward));
    define_coordinates!(DFR => (Down, Forward, Right));
    define_coordinates!(RDF => (Right, Down, Forward));
    define_coordinates!(RFD => (Right, Forward, Down));
    define_coordinates!(FDR => (Forward, Down, Right));
    define_coordinates!(FRD => (Forward, Right, Down));
    define_coordinates!(DRB => (Down, Right, Back));
    define_coordinates!(DBR => (Down, Back, Right));
    define_coordinates!(RDB => (Right, Down, Back));
    define_coordinates!(RBD => (Right, Back, Down));
    define_coordinates!(BDR => (Back, Down, Right));
    define_coordinates!(BRD => (Back, Right, Down));
    define_coordinates!(RIGHT_HAND_X_UP => (Up, Right, Forward));
    define_coordinates!(RIGHT_HAND_X_DOWN => (Down, Right, Back));
    define_coordinates!(RIGHT_HAND_Y_UP => (Right, Up, Back));
    define_coordinates!(RIGHT_HAND_Y_DOWN => (Right, Down, Forward));
    define_coordinates!(RIGHT_HAND_Z_UP => (Right, Forward, Up));
    define_coordinates!(RIGHT_HAND_Z_DOWN => (Right, Back, Down));
    define_coordinates!(LEFT_HAND_X_UP => (Up, Right, Back));
    define_coordinates!(LEFT_HAND_X_DOWN => (Down, Right, Forward));
    define_coordinates!(LEFT_HAND_Y_UP => (Right, Up, Forward));
    define_coordinates!(LEFT_HAND_Y_DOWN => (Right, Down, Back));
    define_coordinates!(LEFT_HAND_Z_UP => (Right, Back, Up));
    define_coordinates!(LEFT_HAND_Z_DOWN => (Right, Forward, Down));
    // <END_GENERATED:declarations>
}
