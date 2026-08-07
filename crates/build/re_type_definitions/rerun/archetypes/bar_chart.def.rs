// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A bar chart.
///
/// The bar heights will be the provided values, and the x coordinates of the bars will be the provided abscissa or default to the index of the provided values.
///
/// \example archetypes/bar_chart title="Simple bar chart" image="https://static.rerun.io/bar_chart/ba274527813ccb9049f6760d82f36c8da6a6f2ff/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "BarChartView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "BarChart")]
#[rust(derive(PartialEq))]
pub struct BarChart {
    /// The values. Should always be a 1-dimensional tensor (i.e. a vector).
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub values: rerun::components::TensorData,

    /// The color of the bar chart
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,

    /// The abscissa corresponding to each value. Should be a 1-dimensional tensor (i.e. a vector) in same length as values.
    #[rerun(optional)]
    pub abscissa: Option<rerun::components::TensorData>,

    /// The width of the bins, defined in x-axis units and defaults to 1. Should be a 1-dimensional tensor (i.e. a vector) in same length as values.
    #[rerun(optional)]
    pub widths: Option<Vec<rerun::components::Length>>,
}
