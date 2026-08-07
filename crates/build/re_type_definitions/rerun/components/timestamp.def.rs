// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// When the recording started.
///
/// Should be an absolute time, i.e. relative to Unix Epoch.
#[rerun::rerun_type]
#[python(array_aliases = "npt.NDArray[np.int64]")]
#[rerun(state = "stable")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Timestamp {
    pub timestamp: rerun::datatypes::TimeInt,
}
