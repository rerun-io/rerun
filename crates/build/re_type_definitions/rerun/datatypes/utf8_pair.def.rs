// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Stores a tuple of UTF-8 strings.
#[rerun::rerun_type]
#[python(aliases = "Tuple[datatypes.Utf8Like, datatypes.Utf8Like]")]
#[python(array_aliases = "npt.NDArray[np.str_]")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rerun(state = "stable")]
pub struct Utf8Pair {
    /// The first string.
    pub first: rerun::datatypes::Utf8,

    /// The second string.
    pub second: rerun::datatypes::Utf8,
}
