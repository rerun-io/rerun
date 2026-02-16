use super::MagnificationFilter;

impl MagnificationFilter {
    /// Instantiate a new [`MagnificationFilter`] from a u8 value.
    ///
    /// Returns `None` if the value doesn't match any of the enum's arms.
    pub fn from_u8(value: u8) -> Option<Self> {
        // NOTE: This code will be optimized out, it's only here to make sure this method fails to
        // compile if the enum is modified.
        match Self::default() {
            Self::Nearest | Self::Linear => {}
        }

        match value {
            v if v == Self::Nearest as u8 => Some(Self::Nearest),
            v if v == Self::Linear as u8 => Some(Self::Linear),
            _ => None,
        }
    }
}
