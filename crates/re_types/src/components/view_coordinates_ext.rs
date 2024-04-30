#![allow(clippy::wrong_self_convention)] // TODO(emilk): re-enable

// ----------------------------------------------------------------------------

use crate::view_coordinates::{Axis3, Handedness, Sign, SignedAxis3, ViewDir};

use super::ViewCoordinates;

impl ViewCoordinates {
    /// Construct a new `ViewCoordinates` from an array of [`ViewDir`]s.
    pub const fn new(x: ViewDir, y: ViewDir, z: ViewDir) -> Self {
        Self([x as u8, y as u8, z as u8])
    }

    /// Chooses a coordinate system based on just an up-axis.
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
            if let Some(dim) = match ViewDir::try_from(dir)? {
                ViewDir::Up | ViewDir::Down => Some(0),
                ViewDir::Right | ViewDir::Left => Some(1),
                ViewDir::Forward | ViewDir::Back => Some(2),
                ViewDir::Unused => None,
            } {
                dims[dim] = true;
            }
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

    /// The up-axis.
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

    /// The right-axis.
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

    /// The forward-axis.
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

    /// Describe using three letters, e.g. `RDF` for X=Right, Y=Down, Z=Forward.
    pub fn describe_short(&self) -> String {
        let [x, y, z] = self.0;
        let x = ViewDir::try_from(x).map(|x| x.short()).unwrap_or("?");
        let y = ViewDir::try_from(y).map(|y| y.short()).unwrap_or("?");
        let z = ViewDir::try_from(z).map(|z| z.short()).unwrap_or("?");
        format!("{x}{y}{z}")
    }

    /// A long description of the coordinate system, explicitly writing out all directions.
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
                _ => [0.0, 0.0, 0.0],
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
                _ => [0.0, 0.0, 0.0],
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

    /// Returns whether or not this coordinate system is left or right handed.
    ///
    /// If the coordinate system is degenerate, an error is returned.
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
    ($docstring:literal, $name:ident => ($x:ident, $y:ident, $z:ident) ) => {
        #[doc = $docstring]
        pub const $name: Self = Self([ViewDir::$x as u8, ViewDir::$y as u8, ViewDir::$z as u8]);
    };
}

impl ViewCoordinates {
    // <BEGIN_GENERATED:declarations>
    // This section is generated by running `scripts/generate_view_coordinate_defs.py --rust`
    define_coordinates!("X=Up, Y=Left, Z=Forward", ULF => (Up, Left, Forward));
    define_coordinates!("X=Up, Y=Forward, Z=Left", UFL => (Up, Forward, Left));
    define_coordinates!("X=Left, Y=Up, Z=Forward", LUF => (Left, Up, Forward));
    define_coordinates!("X=Left, Y=Forward, Z=Up", LFU => (Left, Forward, Up));
    define_coordinates!("X=Forward, Y=Up, Z=Left", FUL => (Forward, Up, Left));
    define_coordinates!("X=Forward, Y=Left, Z=Up", FLU => (Forward, Left, Up));
    define_coordinates!("X=Up, Y=Left, Z=Back", ULB => (Up, Left, Back));
    define_coordinates!("X=Up, Y=Back, Z=Left", UBL => (Up, Back, Left));
    define_coordinates!("X=Left, Y=Up, Z=Back", LUB => (Left, Up, Back));
    define_coordinates!("X=Left, Y=Back, Z=Up", LBU => (Left, Back, Up));
    define_coordinates!("X=Back, Y=Up, Z=Left", BUL => (Back, Up, Left));
    define_coordinates!("X=Back, Y=Left, Z=Up", BLU => (Back, Left, Up));
    define_coordinates!("X=Up, Y=Right, Z=Forward", URF => (Up, Right, Forward));
    define_coordinates!("X=Up, Y=Forward, Z=Right", UFR => (Up, Forward, Right));
    define_coordinates!("X=Right, Y=Up, Z=Forward", RUF => (Right, Up, Forward));
    define_coordinates!("X=Right, Y=Forward, Z=Up", RFU => (Right, Forward, Up));
    define_coordinates!("X=Forward, Y=Up, Z=Right", FUR => (Forward, Up, Right));
    define_coordinates!("X=Forward, Y=Right, Z=Up", FRU => (Forward, Right, Up));
    define_coordinates!("X=Up, Y=Right, Z=Back", URB => (Up, Right, Back));
    define_coordinates!("X=Up, Y=Back, Z=Right", UBR => (Up, Back, Right));
    define_coordinates!("X=Right, Y=Up, Z=Back", RUB => (Right, Up, Back));
    define_coordinates!("X=Right, Y=Back, Z=Up", RBU => (Right, Back, Up));
    define_coordinates!("X=Back, Y=Up, Z=Right", BUR => (Back, Up, Right));
    define_coordinates!("X=Back, Y=Right, Z=Up", BRU => (Back, Right, Up));
    define_coordinates!("X=Down, Y=Left, Z=Forward", DLF => (Down, Left, Forward));
    define_coordinates!("X=Down, Y=Forward, Z=Left", DFL => (Down, Forward, Left));
    define_coordinates!("X=Left, Y=Down, Z=Forward", LDF => (Left, Down, Forward));
    define_coordinates!("X=Left, Y=Forward, Z=Down", LFD => (Left, Forward, Down));
    define_coordinates!("X=Forward, Y=Down, Z=Left", FDL => (Forward, Down, Left));
    define_coordinates!("X=Forward, Y=Left, Z=Down", FLD => (Forward, Left, Down));
    define_coordinates!("X=Down, Y=Left, Z=Back", DLB => (Down, Left, Back));
    define_coordinates!("X=Down, Y=Back, Z=Left", DBL => (Down, Back, Left));
    define_coordinates!("X=Left, Y=Down, Z=Back", LDB => (Left, Down, Back));
    define_coordinates!("X=Left, Y=Back, Z=Down", LBD => (Left, Back, Down));
    define_coordinates!("X=Back, Y=Down, Z=Left", BDL => (Back, Down, Left));
    define_coordinates!("X=Back, Y=Left, Z=Down", BLD => (Back, Left, Down));
    define_coordinates!("X=Down, Y=Right, Z=Forward", DRF => (Down, Right, Forward));
    define_coordinates!("X=Down, Y=Forward, Z=Right", DFR => (Down, Forward, Right));
    define_coordinates!("X=Right, Y=Down, Z=Forward", RDF => (Right, Down, Forward));
    define_coordinates!("X=Right, Y=Forward, Z=Down", RFD => (Right, Forward, Down));
    define_coordinates!("X=Forward, Y=Down, Z=Right", FDR => (Forward, Down, Right));
    define_coordinates!("X=Forward, Y=Right, Z=Down", FRD => (Forward, Right, Down));
    define_coordinates!("X=Down, Y=Right, Z=Back", DRB => (Down, Right, Back));
    define_coordinates!("X=Down, Y=Back, Z=Right", DBR => (Down, Back, Right));
    define_coordinates!("X=Right, Y=Down, Z=Back", RDB => (Right, Down, Back));
    define_coordinates!("X=Right, Y=Back, Z=Down", RBD => (Right, Back, Down));
    define_coordinates!("X=Back, Y=Down, Z=Right", BDR => (Back, Down, Right));
    define_coordinates!("X=Back, Y=Right, Z=Down", BRD => (Back, Right, Down));
    define_coordinates!("X=Up, Y=Left, Z=Unused", UL => (Up, Left, Unused));
    define_coordinates!("X=Left, Y=Up, Z=Unused", LU => (Left, Up, Unused));
    define_coordinates!("X=Up, Y=Right, Z=Unused", UR => (Up, Right, Unused));
    define_coordinates!("X=Right, Y=Up, Z=Unused", RU => (Right, Up, Unused));
    define_coordinates!("X=Down, Y=Left, Z=Unused", DL => (Down, Left, Unused));
    define_coordinates!("X=Left, Y=Down, Z=Unused", LD => (Left, Down, Unused));
    define_coordinates!("X=Down, Y=Right, Z=Unused", DR => (Down, Right, Unused));
    define_coordinates!("X=Right, Y=Down, Z=Unused", RD => (Right, Down, Unused));
    define_coordinates!("X=Up, Y=Right, Z=Forward", RIGHT_HAND_X_UP => (Up, Right, Forward));
    define_coordinates!("X=Down, Y=Right, Z=Back", RIGHT_HAND_X_DOWN => (Down, Right, Back));
    define_coordinates!("X=Right, Y=Up, Z=Back", RIGHT_HAND_Y_UP => (Right, Up, Back));
    define_coordinates!("X=Right, Y=Down, Z=Forward", RIGHT_HAND_Y_DOWN => (Right, Down, Forward));
    define_coordinates!("X=Right, Y=Forward, Z=Up", RIGHT_HAND_Z_UP => (Right, Forward, Up));
    define_coordinates!("X=Right, Y=Back, Z=Down", RIGHT_HAND_Z_DOWN => (Right, Back, Down));
    define_coordinates!("X=Up, Y=Right, Z=Back", LEFT_HAND_X_UP => (Up, Right, Back));
    define_coordinates!("X=Down, Y=Right, Z=Forward", LEFT_HAND_X_DOWN => (Down, Right, Forward));
    define_coordinates!("X=Right, Y=Up, Z=Forward", LEFT_HAND_Y_UP => (Right, Up, Forward));
    define_coordinates!("X=Right, Y=Down, Z=Back", LEFT_HAND_Y_DOWN => (Right, Down, Back));
    define_coordinates!("X=Right, Y=Back, Z=Up", LEFT_HAND_Z_UP => (Right, Back, Up));
    define_coordinates!("X=Right, Y=Forward, Z=Down", LEFT_HAND_Z_DOWN => (Right, Forward, Down));
    // <END_GENERATED:declarations>
}
