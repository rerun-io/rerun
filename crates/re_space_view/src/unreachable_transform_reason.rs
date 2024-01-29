#[derive(Clone, Copy)]
pub enum UnreachableTransformReason {
    /// More than one pinhole camera between this and the reference space.
    NestedPinholeCameras,

    /// Exiting out of a space with a pinhole camera that doesn't have a resolution is not supported.
    InversePinholeCameraWithoutResolution,

    /// Unknown transform between this and the reference space.
    DisconnectedSpace,

    /// View coordinates contained an invalid value
    InvalidViewCoordinates,
}

impl std::fmt::Display for UnreachableTransformReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NestedPinholeCameras =>
                "Can't display entities under nested pinhole cameras.",
            Self::DisconnectedSpace =>
                "Can't display entities that are in an explicitly disconnected space.",
            Self::InversePinholeCameraWithoutResolution =>
                "Can't display entities that would require inverting a pinhole camera without a specified resolution.",
            Self::InvalidViewCoordinates =>
                "Can't display entities that have invalid view coordinates."
        })
    }
}
