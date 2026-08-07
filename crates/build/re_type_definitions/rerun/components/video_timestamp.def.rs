// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Timestamp inside a [archetypes.AssetVideo].
#[rerun::rerun_type]
#[python(array_aliases = "npt.NDArray[np.int64]")]
#[rerun(state = "stable")]
#[rust(derive(Copy, PartialEq, Eq, Default))]
#[rust(repr = "transparent")]
pub struct VideoTimestamp {
    pub timestamp: rerun::datatypes::VideoTimestamp,
}
