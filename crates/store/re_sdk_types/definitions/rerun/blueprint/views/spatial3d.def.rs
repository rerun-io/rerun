// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// For viewing spatial 3D data.
///
/// \example views/spatial3d title="Use a blueprint to customize a Spatial3DView." image="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "3D")]
#[rerun(state = "unstable")]
pub struct Spatial3DView {
    /// Configuration for the background of the view.
    pub background: rerun::blueprint::archetypes::Background,

    /// Configuration for the 3D line grid.
    pub line_grid: rerun::blueprint::archetypes::LineGrid3D,

    /// Configuration of debug drawing in the 3D view.
    pub spatial_information: rerun::blueprint::archetypes::SpatialInformation,

    /// Configuration for the 3D eye
    pub eye_controls: rerun::blueprint::archetypes::EyeControls3D,

    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    ///
    /// If not specified, the default is to show the latest state of each component.
    /// If a timeline is specified more than once, the first entry will be used.
    pub time_ranges: rerun::blueprint::archetypes::VisibleTimeRanges,
}
