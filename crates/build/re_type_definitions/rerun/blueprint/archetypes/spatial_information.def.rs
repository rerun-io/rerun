// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configures spatial view properties.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct SpatialInformation {
    /// The target reference frame for all transformations.
    ///
    /// Defaults to the coordinate frame used by the space origin entity.
    #[rerun(optional)]
    pub target_frame: rerun::components::TransformFrameId,

    /// Whether the bounding box should be shown.
    // TODO(andreas): Make this an enum so the user can choose between showing bounding boxes,
    // regions of interest, or per-entity bounding boxes.
    #[rerun(optional)]
    pub show_bounding_box: Option<rerun::blueprint::components::Enabled>,

    /// Whether axes should be shown at the origin.
    #[rerun(optional)]
    pub show_axes: Option<rerun::blueprint::components::Enabled>,

    /// Controls the orientation of the axes in a 3D view; it has no effect in a 2D view.
    ///
    /// This determines the 3D eye orientation, navigation, and default grid plane.
    ///
    /// The three directions are always ordered as [x, y, z] and specify where each positive axis points.
    /// For example, [Right, Down, Forward] means that +X points right, +Y points down, and +Z points forward.
    ///
    /// When this property is unset, a 3D view first uses [archetypes.ViewCoordinates] logged at its origin entity or the closest ancestor.
    /// If none is found, it uses the camera orientation from the closest ancestor [archetypes.Pinhole].
    /// If neither is found, the fallback is RFU.
    ///
    /// This property is hidden from the selection panel for 2D views.
    ///
    /// ⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).
    // TODO(#1387): This property has no effect in 2D views and is hidden from its selection panel.
    #[rerun(optional)]
    pub axes: Option<rerun::components::ViewCoordinates>,
}
