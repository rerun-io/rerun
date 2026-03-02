#![expect(clippy::wrong_self_convention)]

use super::ViewCoordinates2D;
use crate::datatypes;
use crate::view_coordinates::ViewDir;

impl ViewCoordinates2D {
    /// X=Right, Y=Down (default, image/screen convention).
    pub const RD: Self = Self(datatypes::ViewCoordinates2D([
        ViewDir::Right as u8,
        ViewDir::Down as u8,
    ]));

    /// X=Right, Y=Up (math/plot convention).
    pub const RU: Self = Self(datatypes::ViewCoordinates2D([
        ViewDir::Right as u8,
        ViewDir::Up as u8,
    ]));

    /// X=Left, Y=Down (horizontally mirrored image).
    pub const LD: Self = Self(datatypes::ViewCoordinates2D([
        ViewDir::Left as u8,
        ViewDir::Down as u8,
    ]));

    /// X=Left, Y=Up (both axes flipped).
    pub const LU: Self = Self(datatypes::ViewCoordinates2D([
        ViewDir::Left as u8,
        ViewDir::Up as u8,
    ]));

    /// Construct a new `ViewCoordinates2D` from two [`ViewDir`]s.
    pub const fn new(x: ViewDir, y: ViewDir) -> Self {
        Self(datatypes::ViewCoordinates2D([x as u8, y as u8]))
    }

    /// Returns an error if this does not represent a valid 2D coordinate system.
    ///
    /// Requires one horizontal axis (Left/Right) and one vertical axis (Up/Down).
    /// Forward/Back are not valid for 2D coordinate systems.
    #[track_caller]
    pub fn sanity_check(&self) -> Result<(), String> {
        let mut has_horizontal = false;
        let mut has_vertical = false;

        for &dir in self.0.iter() {
            match ViewDir::try_from(dir)? {
                ViewDir::Up | ViewDir::Down => has_vertical = true,
                ViewDir::Right | ViewDir::Left => has_horizontal = true,
                ViewDir::Forward | ViewDir::Back => {
                    return Err(format!(
                        "Forward/Back are not valid for 2D coordinate systems: {}",
                        self.describe()
                    ));
                }
            }
        }

        if has_horizontal && has_vertical {
            Ok(())
        } else {
            Err(format!(
                "2D coordinate system must have one horizontal and one vertical axis: {}",
                self.describe()
            ))
        }
    }

    /// Describe using two letters, e.g. `RD` for X=Right, Y=Down.
    pub fn describe_short(&self) -> String {
        let [x, y] = *self.0;
        let x = ViewDir::try_from(x).map(|x| x.short()).unwrap_or("?");
        let y = ViewDir::try_from(y).map(|y| y.short()).unwrap_or("?");
        format!("{x}{y}")
    }

    /// A long description of the coordinate system, explicitly writing out all directions.
    pub fn describe(&self) -> String {
        let [x, y] = *self.0;
        let x_short = ViewDir::try_from(x).map(|x| x.short()).unwrap_or("?");
        let y_short = ViewDir::try_from(y).map(|y| y.short()).unwrap_or("?");
        let x_long = ViewDir::try_from(x).map(|x| x.long()).unwrap_or("?");
        let y_long = ViewDir::try_from(y).map(|y| y.long()).unwrap_or("?");
        format!("{x_short}{y_short} (X={x_long}, Y={y_long})")
    }

    /// Whether the X axis needs to be flipped relative to the default RD convention.
    ///
    /// Returns `true` if X points Left.
    #[inline]
    pub fn flip_x(&self) -> bool {
        self.0[0] == ViewDir::Left as u8
    }

    /// Whether the Y axis needs to be flipped relative to the default RD convention.
    ///
    /// Returns `true` if Y points Up.
    #[inline]
    pub fn flip_y(&self) -> bool {
        self.0[1] == ViewDir::Up as u8
    }

    /// Returns a 2x2 matrix that transforms from this coordinate system to RD (Right, Down).
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rd(&self) -> glam::Mat2 {
        let sx = if self.flip_x() { -1.0 } else { 1.0 };
        let sy = if self.flip_y() { -1.0 } else { 1.0 };
        glam::Mat2::from_diagonal(glam::vec2(sx, sy))
    }

    /// Returns a 2x2 matrix that transforms from RD (Right, Down) to this coordinate system.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rd(&self) -> glam::Mat2 {
        // For diagonal sign-flip matrices, the inverse is the same matrix.
        self.to_rd()
    }

    /// Returns a matrix that transforms from another coordinate system to this (self) one.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_other(&self, other: &Self) -> glam::Mat2 {
        self.from_rd() * other.to_rd()
    }
}

impl std::str::FromStr for ViewCoordinates2D {
    type Err = String;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            [x, y] => {
                let slf = Self::new(ViewDir::from_ascii_char(*x)?, ViewDir::from_ascii_char(*y)?);
                slf.sanity_check()?;
                Ok(slf)
            }
            _ => Err(format!("Expected two letters, got: {s:?}")),
        }
    }
}

impl Default for ViewCoordinates2D {
    #[inline]
    fn default() -> Self {
        Self::RD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(ViewCoordinates2D::RD.describe_short(), "RD");
        assert_eq!(ViewCoordinates2D::RU.describe_short(), "RU");
        assert_eq!(ViewCoordinates2D::LD.describe_short(), "LD");
        assert_eq!(ViewCoordinates2D::LU.describe_short(), "LU");
    }

    #[test]
    fn test_sanity_check() {
        assert!(ViewCoordinates2D::RD.sanity_check().is_ok());
        assert!(ViewCoordinates2D::RU.sanity_check().is_ok());
        assert!(ViewCoordinates2D::LD.sanity_check().is_ok());
        assert!(ViewCoordinates2D::LU.sanity_check().is_ok());

        // Two horizontal axes should fail.
        assert!(
            ViewCoordinates2D::new(ViewDir::Right, ViewDir::Left)
                .sanity_check()
                .is_err()
        );
        // Forward/Back should fail.
        assert!(
            ViewCoordinates2D::new(ViewDir::Right, ViewDir::Forward)
                .sanity_check()
                .is_err()
        );
    }

    #[test]
    fn test_flips() {
        assert!(!ViewCoordinates2D::RD.flip_x());
        assert!(!ViewCoordinates2D::RD.flip_y());

        assert!(!ViewCoordinates2D::RU.flip_x());
        assert!(ViewCoordinates2D::RU.flip_y());

        assert!(ViewCoordinates2D::LD.flip_x());
        assert!(!ViewCoordinates2D::LD.flip_y());

        assert!(ViewCoordinates2D::LU.flip_x());
        assert!(ViewCoordinates2D::LU.flip_y());
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "RD".parse::<ViewCoordinates2D>().unwrap(),
            ViewCoordinates2D::RD
        );
        assert_eq!(
            "RU".parse::<ViewCoordinates2D>().unwrap(),
            ViewCoordinates2D::RU
        );
        assert_eq!(
            "LD".parse::<ViewCoordinates2D>().unwrap(),
            ViewCoordinates2D::LD
        );
        assert_eq!(
            "LU".parse::<ViewCoordinates2D>().unwrap(),
            ViewCoordinates2D::LU
        );

        assert!("RF".parse::<ViewCoordinates2D>().is_err()); // Forward not allowed
        assert!("RR".parse::<ViewCoordinates2D>().is_err()); // Two horizontal
        assert!("ABC".parse::<ViewCoordinates2D>().is_err()); // Too many chars
    }
}
