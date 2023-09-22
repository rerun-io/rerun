#[derive(Clone, Copy)]
pub enum UnreachableTransformReason {
    /// `SpaceInfoCollection` is outdated and can't find a corresponding space info for the given path.
    ///
    /// If at all, this should only happen for a single frame until space infos are rebuilt.
    UnknownSpaceInfo,

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
            Self::UnknownSpaceInfo =>
                "Can't determine transform because internal data structures are not in a valid state. Please file an issue on https://github.com/rerun-io/rerun/",
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
