// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A state change, representing a transition of an entity into a new state.
///
/// Useful for representing discrete state machines, mode transitions, or
/// state changes over time. Each logged [archetypes.StateChange] marks a new state
/// at the given time. A `null` state resets the state, showing a gap in the state timeline view.
///
/// The state timeline view displays these as horizontal colored lanes over time.
///
/// \example archetypes/state_change title="State changes over time" image="https://static.rerun.io/state_change/6654a13e984702b96547750469c368ce6e900c0f/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Plotting")]
#[docs(view_types = "StateTimelineView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "StateVisualizer")]
#[rust(derive(PartialEq))]
pub struct StateChange {
    /// The new state values; each instance gets its own lane in the state timeline view.
    ///
    /// A reset ends the previous state and shows a gap in the state timeline view until the
    /// next state. An empty string, a null array entry, and an empty state array (e.g. from
    /// clearing the field) all act as resets.
    ///
    /// The length of the state array should not change over time.
    #[rerun(required)]
    pub state: Option<Vec<rerun::components::Text>>,
}
