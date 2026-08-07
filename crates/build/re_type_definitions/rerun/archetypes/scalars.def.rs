// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// One or more double-precision scalar values, e.g. for use for time-series plots.
///
/// The current timeline value will be used for the time/X-axis, hence scalars
/// should not be static.
/// Number of scalars per timestamp is expected to be the same over time.
///
/// When used to produce a plot, this archetype is used to provide the data that
/// is referenced by [archetypes.SeriesLines] or [archetypes.SeriesPoints]. You can do
/// this by logging both archetypes to the same path, or alternatively configuring
/// the plot-specific archetypes through the blueprint.
///
/// \example archetypes/scalars_simple !api title="Simple line plot" image="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/1200w.png"
/// \example archetypes/scalars_multiple_plots !api title="Multiple time series plots" image="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/1200w.png"
/// \example archetypes/scalars_row_updates title="Update a scalar over time" image="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1200w.png"
/// \example archetypes/scalars_column_updates title="Update a scalar over time, in a single operation" image="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "TimeSeriesView")]
#[rerun(state = "stable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct Scalars {
    /// The scalar values to log.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub scalars: Vec<rerun::components::Scalar>,
    // TODO(#1289): Support labeling points.
}
