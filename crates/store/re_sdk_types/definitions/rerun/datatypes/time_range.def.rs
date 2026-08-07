// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Two [datatypes.TimeInt] describing a range of time.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(override_crate = "re_types_core")]
#[rerun(state = "stable")]
pub struct AbsoluteTimeRange {
    /// Start of the range.
    pub min: rerun::datatypes::TimeInt,

    /// End of the range (inclusive).
    pub max: rerun::datatypes::TimeInt,
}
