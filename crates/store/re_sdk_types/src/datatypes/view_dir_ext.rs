use crate::datatypes::ViewDir;
use re_types_core::reflection::Enum as _;

impl ViewDir {
    /// Convert an upper case letter to one of the six cardinal directions.
    ///
    /// * 'U' => [`Self::Up`]
    /// * 'D' => [`Self::Down`]
    /// * 'R' => [`Self::Right`]
    /// * 'L' => [`Self::Left`]
    /// * 'F' => [`Self::Forward`]
    /// * 'B' => [`Self::Back`]
    #[inline]
    pub fn from_ascii_char(c: u8) -> Result<Self, String> {
        match c {
            b'U' => Ok(Self::Up),
            b'D' => Ok(Self::Down),
            b'R' => Ok(Self::Right),
            b'L' => Ok(Self::Left),
            b'F' => Ok(Self::Forward),
            b'B' => Ok(Self::Back),
            _ => Err("Expected one of UDRLFB (Up Down Right Left Forward Back)".to_owned()),
        }
    }

    /// Represent this direction as the first letter of the direction's name, in uppercase.
    ///
    /// * [`Self::Up`] => 'U'
    /// * [`Self::Down`] => 'D'
    /// * [`Self::Right`] => 'R'
    /// * [`Self::Left`] => 'L'
    /// * [`Self::Forward`] => 'F'
    /// * [`Self::Back`] => 'B'
    #[inline]
    pub fn short(&self) -> &'static str {
        match self {
            Self::Up => "U",
            Self::Down => "D",
            Self::Right => "R",
            Self::Left => "L",
            Self::Forward => "F",
            Self::Back => "B",
        }
    }

    /// Long description of the direction, e.g. "Up", "Down", "Right", "Left", "Forward", "Back".
    #[inline]
    pub fn long(&self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Right => "Right",
            Self::Left => "Left",
            Self::Forward => "Forward",
            Self::Back => "Back",
        }
    }
}

impl TryFrom<u8> for ViewDir {
    type Error = String;

    #[inline]
    fn try_from(i: u8) -> Result<Self, String> {
        Self::try_from_integer(i).ok_or_else(|| format!("Could not interpret {i} as ViewDir."))
    }
}
