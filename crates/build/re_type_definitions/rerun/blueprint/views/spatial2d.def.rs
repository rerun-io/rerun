// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// For viewing spatial 2D data.
///
/// \example views/spatial2d title="Use a blueprint to customize a Spatial2DView." image="https://static.rerun.io/Spatial2DVIew/824a075e0c50ea4110eb6ddd60257f087cb2264d/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "2D")]
#[rerun(state = "unstable")]
pub struct Spatial2DView {
    /// Configuration for the background of the view.
    pub background: rerun::blueprint::archetypes::Background,

    /// The visible parts of the scene, in the coordinate space of the scene.
    ///
    /// Everything within these bounds are guaranteed to be visible.
    /// Somethings outside of these bounds may also be visible due to letterboxing.
    pub visual_bounds: rerun::blueprint::archetypes::VisualBounds2D,

    /// Configuration of spatial information shown in the view.
    pub spatial_information: rerun::blueprint::archetypes::SpatialInformation,

    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    ///
    /// If not specified, the default is to show the latest state of each component.
    /// If a timeline is specified more than once, the first entry will be used.
    pub time_ranges: rerun::blueprint::archetypes::VisibleTimeRanges,
}
