use super::MeshFaceRendering;

impl MeshFaceRendering {
    /// Instantiate a new [`MeshFaceRendering`] from a u8 value.
    ///
    /// Returns `None` if the value doesn't match any of the enum's arms.
    pub fn from_u8(value: u8) -> Option<Self> {
        // NOTE: This code will be optimized out, it's only here to make sure this method fails to
        // compile if the enum is modified.
        match Self::default() {
            Self::DoubleSided | Self::Front | Self::Back => {}
        }

        match value {
            v if v == Self::DoubleSided as u8 => Some(Self::DoubleSided),
            v if v == Self::Front as u8 => Some(Self::Front),
            v if v == Self::Back as u8 => Some(Self::Back),
            _ => None,
        }
    }
}
