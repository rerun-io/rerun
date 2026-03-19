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
}
