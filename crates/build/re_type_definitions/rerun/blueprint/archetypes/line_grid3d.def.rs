// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the 3D line grid.
#[rerun::rerun_type]
#[python(aliases = "datatypes.BoolLike")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct LineGrid3D {
    /// Whether the grid is visible.
    ///
    /// Defaults to true.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,

    /// Space between grid lines spacing of one line to the next in scene units.
    ///
    /// As you zoom out, successively only every tenth line is shown.
    /// This controls the closest zoom level.
    #[rerun(optional)]
    pub spacing: Option<rerun::blueprint::components::GridSpacing>,

    /// In what plane the grid is drawn.
    ///
    /// Defaults to the plane at zero units along the up/down axis defined by [archetypes.SpatialInformation]'s axes property.
    #[rerun(optional)]
    pub plane: Option<rerun::components::Plane3D>,

    /// How thick the lines should be in ui units.
    ///
    /// Default is 1.0 ui unit.
    #[rerun(optional)]
    pub stroke_width: Option<rerun::components::StrokeWidth>,

    /// Color used for the grid.
    ///
    /// Transparency via alpha channel is supported.
    /// Defaults to a slightly transparent light gray.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,
}
