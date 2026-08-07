// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A bar chart view.
///
/// \example views/bar_chart title="Use a blueprint to create a BarChartView." image="https://static.rerun.io/bar_chart_view/74fa45af3c7310b51cd283c37439ed8f8ca9356d/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "BarChart")]
#[rerun(state = "unstable")]
pub struct BarChartView {
    /// Configures the legend of the plot.
    pub plot_legend: rerun::blueprint::archetypes::PlotLegend,

    /// Configures the background of the plot.
    pub background: rerun::blueprint::archetypes::PlotBackground,
}
