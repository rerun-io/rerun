// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Presentation timestamp within a [archetypes.AssetVideo].
///
/// Specified in nanoseconds.
/// Presentation timestamps are typically measured as time since video start.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "int")]
#[python(array_aliases = "npt.NDArray[np.int64]")]
#[rust(derive(Default, Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct VideoTimestamp {
    /// Presentation timestamp value in nanoseconds.
    pub timestamp_ns: i64,
    // Implementation note:
    // Keeping this to nanoseconds makes the timestamp more consistent to our other timestamp values!
}
