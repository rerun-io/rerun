/// The six cardinal directions for 3D view-space and image-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewDir {
    Up = 1,
    Down = 2,
    Right = 3,
    Left = 4,
    Forward = 5,
    Back = 6,
}

impl TryFrom<u8> for ViewDir {
    type Error = String;

    #[inline]
    fn try_from(i: u8) -> Result<Self, String> {
        match i {
            1 => Ok(Self::Up),
            2 => Ok(Self::Down),
            3 => Ok(Self::Right),
            4 => Ok(Self::Left),
            5 => Ok(Self::Forward),
            6 => Ok(Self::Back),
            _ => Err("Could not interpret {i} as ViewDir.".to_owned()),
        }
    }
}

impl ViewDir {
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
