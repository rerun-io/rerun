// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Define the style properties for one or more point series (scatter plot) in a chart.
///
/// This archetype only provides styling information.
/// Changes over time are supported for most but not all its fields (see respective fields for details),
/// it's generally recommended to log this type as static.
///
/// The underlying data needs to be logged to the same entity-path using [archetypes.Scalars].
/// Dimensionality of the scalar arrays logged at each time point is assumed to be the same over time.
///
/// \example archetypes/series_points_style title="Point series" image="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "TimeSeriesView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "SeriesPoints")]
pub struct SeriesPoints {
    /// Color for the corresponding series.
    ///
    /// May change over time, but can cause discontinuities in the line.
    #[rerun(component_optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// What shape to use to represent the point
    ///
    /// May change over time.
    #[rerun(component_required)]
    pub markers: Option<Vec<rerun::components::MarkerShape>>,

    /// Display name of the series.
    ///
    /// Used in the legend. Expected to be unchanging over time.
    #[rerun(component_optional)]
    pub names: Option<Vec<rerun::components::Name>>,

    /// Which lines are visible.
    ///
    /// If not set, all line series on this entity are visible.
    /// Unlike with the regular visibility property of the entire entity, any series that is hidden
    /// via this property will still be visible in the legend.
    ///
    /// May change over time.
    #[rerun(component_optional)]
    pub visible_series: Option<Vec<rerun::components::Visible>>,

    /// Sizes of the markers.
    ///
    /// May change over time.
    #[rerun(component_optional)]
    pub marker_sizes: Option<Vec<rerun::components::MarkerSize>>,
}
