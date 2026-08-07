// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Define the style and mapping for state values in a state timeline view.
///
/// This archetype provides configuration for how state values are displayed.
/// It maps raw state values to display labels, colors, and visibility.
///
/// `values`, `labels`, `colors`, and `visible` are parallel arrays: the entry
/// at index `i` of each describes the same state value, and only the
/// per-index pairing is meaningful. The four arrays should have matching
/// length; any secondary array (`labels`, `colors`, `visible`) that is shorter
/// than `values` falls back to defaults for the missing entries.
///
/// It's generally recommended to log this type as static.
///
/// The underlying data needs to be logged to the same entity path using [archetypes.StateChange].
///
/// \example archetypes/state_configuration title="State changes with a custom style"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "StateTimelineView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "StateVisualizer")]
#[rust(derive(PartialEq))]
pub struct StateConfiguration {
    /// The raw state values that this configuration applies to.
    ///
    /// Each entry defines a known state value. The order determines the mapping to
    /// `labels`, `colors`, and `visible` (by index).
    #[rerun(optional)]
    pub values: Option<Vec<rerun::components::Text>>,

    /// Display labels for each state value.
    ///
    /// If provided, the label at index `i` is shown instead of the raw value at index `i`.
    /// If not provided or shorter than `values`, the raw value is used as the label.
    #[rerun(optional)]
    pub labels: Option<Vec<rerun::components::Text>>,

    /// Colors for each state value.
    ///
    /// If provided, the color at index `i` is used for the state at index `i`.
    /// If not provided, colors are assigned automatically from a built-in palette.
    #[rerun(optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Visibility for each state value.
    ///
    /// If provided, the visibility at index `i` controls whether the state at index `i` is shown.
    /// If not provided, all state values are visible.
    #[rerun(optional)]
    pub visible: Option<Vec<rerun::components::Visible>>,
}
