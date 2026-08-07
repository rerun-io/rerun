// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A time series view for scalars over time, for use with [archetypes.Scalars].
///
/// \example views/timeseries title="Use a blueprint to customize a TimeSeriesView." image="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "TimeSeries")]
#[rerun(state = "unstable")]
pub struct TimeSeriesView {
    /// Configures the horizontal axis of the plot.
    pub axis_x: rerun::blueprint::archetypes::TimeAxis,

    /// Configures the vertical axis of the plot.
    pub axis_y: rerun::blueprint::archetypes::ScalarAxis,

    /// Configures the legend of the plot.
    pub plot_legend: rerun::blueprint::archetypes::PlotLegend,

    /// Configures the background of the plot.
    pub background: rerun::blueprint::archetypes::PlotBackground,

    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    ///
    /// If not specified, the default is to show the entire timeline.
    /// If a timeline is specified more than once, the first entry will be used.
    pub time_ranges: rerun::blueprint::archetypes::VisibleTimeRanges,
}
