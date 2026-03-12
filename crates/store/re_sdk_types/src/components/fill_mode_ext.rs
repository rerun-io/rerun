use super::FillMode;

impl FillMode {
    /// Does this fill mode include wireframe lines?
    pub fn has_wireframe(self) -> bool {
        match self {
            Self::MajorWireframe | Self::DenseWireframe | Self::TransparentFillMajorWireframe => {
                true
            }
            Self::Solid => false,
        }
    }

    /// Does this fill mode include a solid fill?
    pub fn has_solid(self) -> bool {
        match self {
            Self::Solid | Self::TransparentFillMajorWireframe => true,
            Self::MajorWireframe | Self::DenseWireframe => false,
        }
    }

    /// Should we only draw the major axes, or the full mesh?
    pub fn axes_only(self) -> bool {
        match self {
            Self::MajorWireframe | Self::TransparentFillMajorWireframe => true,
            Self::DenseWireframe | Self::Solid => false,
        }
    }

    /// Instantiate a new [`FillMode`] from a u8 value.
    ///
    /// Returns `None` if the value doesn't match any of the enum's arms.
    pub fn from_u8(value: u8) -> Option<Self> {
        // NOTE: This code will be optimized out, it's only here to make sure this method fails to
        // compile if the enum is modified.
        match Self::default() {
            Self::MajorWireframe
            | Self::DenseWireframe
            | Self::Solid
            | Self::TransparentFillMajorWireframe => {}
        }

        match value {
            v if v == Self::MajorWireframe as u8 => Some(Self::MajorWireframe),
            v if v == Self::DenseWireframe as u8 => Some(Self::DenseWireframe),
            v if v == Self::Solid as u8 => Some(Self::Solid),
            v if v == Self::TransparentFillMajorWireframe as u8 => {
                Some(Self::TransparentFillMajorWireframe)
            }
            _ => None,
        }
    }
}
