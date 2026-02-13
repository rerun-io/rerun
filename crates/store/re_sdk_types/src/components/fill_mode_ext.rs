use super::FillMode;

impl FillMode {
    /// Instantiate a new [`FillMode`] from a u8 value.
    ///
    /// Returns `None` if the value doesn't match any of the enum's arms.
    pub fn from_u8(value: u8) -> Option<Self> {
        // NOTE: This code will be optimized out, it's only here to make sure this method fails to
        // compile if the enum is modified.
        match Self::default() {
            Self::MajorWireframe | Self::DenseWireframe | Self::Solid => {}
        }

        match value {
            v if v == Self::MajorWireframe as u8 => Some(Self::MajorWireframe),
            v if v == Self::DenseWireframe as u8 => Some(Self::DenseWireframe),
            v if v == Self::Solid as u8 => Some(Self::Solid),
            _ => None,
        }
    }
}
