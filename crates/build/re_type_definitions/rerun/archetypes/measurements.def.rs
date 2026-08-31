// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// One or more scalar measurements, each with a variance and a unit.
///
/// Use this for sensors that report a value together with its uncertainty:
/// pressure, temperature, illuminance, relative humidity, range, and so on.
///
/// In a [`rerun::blueprint::views::TimeSeriesView`] each series is drawn as a line with a translucent
/// band around it, one standard deviation (the square root of the variance) wide in each direction.
/// Leave `variances` unset if the uncertainty is unknown.
///
/// The current timeline value is used for the time/X-axis, so measurements should not be
/// static. Number of values per timestamp is expected to be the same over time.
///
/// Unlike [`rerun::archetypes::Scalars`], this archetype carries its own styling, so values and style
/// are logged together.
/// Changes over time are supported for most but not all styling fields (see respective fields for details).
///
/// \example archetypes/measurements_simple title="Pressure with variance" image="https://static.rerun.io/measurements/2388490ab2b487bb6c47a2be3e7d5e7aa17c08f3/1024w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(unreleased)]
#[docs(view_types = "TimeSeriesView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "MeasurementsSeries")]
#[rust(derive(PartialEq))]
pub struct Measurements {
    /// The measured scalar values.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub values: Vec<rerun::components::Scalar>,

    /// Variances of the measurements (σ², in the units of `values` squared).
    ///
    /// When set, length is expected to match `values`.
    #[rerun(recommended)]
    pub variances: Option<Vec<rerun::components::Variance>>,

    /// Units of the measurements, shown in the legend and in tooltips.
    ///
    /// When set, length is expected to match `values`.
    /// Expected to be unchanging over time.
    #[rerun(optional)]
    pub units: Option<Vec<rerun::components::Unit>>,

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

    /// Which series are visible.
    ///
    /// If not set, all series on this entity are visible.
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
