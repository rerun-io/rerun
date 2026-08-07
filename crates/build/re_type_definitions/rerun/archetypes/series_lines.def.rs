// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Define the style properties for one or more line series in a chart.
///
/// This archetype only provides styling information.
/// Changes over time are supported for most but not all its fields (see respective fields for details),
/// it's generally recommended to log this type as static.
///
/// The underlying data needs to be logged to the same entity-path using [archetypes.Scalars].
/// Dimensionality of the scalar arrays logged at each time point is assumed to be the same over time.
///
/// \example archetypes/series_lines_style title="Line series" image="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "TimeSeriesView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "SeriesLines")]
pub struct SeriesLines {
    // TODO(#8368): Once it's trivial to override how scalars for a plot are sourced,
    // we should make it explicit that the `SeriesLines`/`SeriesPoints` visualizers require
    // scalars as an input.
    // Doing so right now would break the model of how time series logging works too much:
    // This is a case where we want to encourage data <-> styling separation more than elsewhere,
    // so it's important to make keeping it separate easy.
    //pub scalars: Vec<rerun::components::Scalar>,
    /// Color for the corresponding series.
    ///
    /// May change over time, but can cause discontinuities in the line.
    #[rerun(optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Stroke width for the corresponding series.
    ///
    /// May change over time, but can cause discontinuities in the line.
    #[rerun(optional)]
    pub widths: Option<Vec<rerun::components::StrokeWidth>>,

    /// Display name of the series.
    ///
    /// Used in the legend. Expected to be unchanging over time.
    #[rerun(optional)]
    pub names: Option<Vec<rerun::components::Name>>,

    /// Which lines are visible.
    ///
    /// If not set, all line series on this entity are visible.
    /// Unlike with the regular visibility property of the entire entity, any series that is hidden
    /// via this property will still be visible in the legend.
    ///
    /// May change over time, but can cause discontinuities in the line.
    #[rerun(optional)]
    pub visible_series: Option<Vec<rerun::components::Visible>>,

    /// Configures the zoom-dependent scalar aggregation.
    ///
    /// This is done only if steps on the X axis go below a single pixel,
    /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
    /// (and readability) in such situations as it prevents overdraw.
    ///
    /// Expected to be unchanging over time.
    #[rerun(optional)]
    pub aggregation_policy: Option<rerun::components::AggregationPolicy>,

    /// Specifies how values between data points are interpolated.
    ///
    /// Defaults to linear interpolation. Use one of the `Step*` variants for a stepped (staircase) line.
    ///
    /// Expected to be unchanging over time.
    #[rerun(optional)]
    pub interpolation_mode: Option<rerun::components::InterpolationMode>,
}
