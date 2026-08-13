// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view for displaying state transitions over time, for use with [`rerun::archetypes::StateChange`].
// TODO(RR-4240): Add a proper snippet and update screenshot.
///
/// \example views/state_timeline title="Use a blueprint to show a StateTimelineView." image="https://static.rerun.io/status_view/997ff1c16765374651ba662812a78e53803aba75/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "StateTimeline")]
#[rerun(state = "unstable")]
pub struct StateTimelineView {
    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    ///
    /// If not specified, the default is to show the entire timeline.
    /// If a timeline is specified more than once, the first entry will be used.
    pub time_ranges: rerun::blueprint::archetypes::VisibleTimeRanges,
}
