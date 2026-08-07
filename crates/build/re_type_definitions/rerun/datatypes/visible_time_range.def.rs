// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Visible time range bounds for a specific timeline.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(override_crate = "re_types_core")]
#[rerun(state = "stable")]
pub struct TimeRange {
    /// Low time boundary for sequence timeline.
    // Can't call it `from` because it's a reserved keyword in Python.
    pub start: rerun::datatypes::TimeRangeBoundary,

    /// High time boundary for sequence timeline.
    pub end: rerun::datatypes::TimeRangeBoundary,
}

/// Left or right boundary of a time range.
#[rerun::rerun_type]
#[repr(i8)]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(override_crate = "re_types_core")]
#[rerun(state = "stable")]
pub enum TimeRangeBoundary {
    /// Boundary is a value relative to the time cursor.
    CursorRelative(rerun::datatypes::TimeInt) = 1,

    /// Boundary is an absolute value.
    Absolute(rerun::datatypes::TimeInt) = 2,

    /// The boundary extends to infinity.
    Infinite = 3,
}

/// Visible time range bounds for a specific timeline.
///
/// \example archetypes/line_strips3d_time_window missing="cpp,rs" title="Time-windowed trails (e.g. Trajectories)" image="https://static.rerun.io/line_strips3d_time_window/999f92d8f7f09b77e8307e6bbcaad652cf2f2c44/1200w.png"
#[rerun::rerun_type]
#[rust(derive(PartialEq, Eq))]
#[rust(override_crate = "re_types_core")]
#[rerun(state = "stable")]
pub struct VisibleTimeRange {
    /// Name of the timeline this applies to.
    pub timeline: rerun::datatypes::Utf8,

    /// Time range to use for this timeline.
    pub range: rerun::datatypes::TimeRange,
}
