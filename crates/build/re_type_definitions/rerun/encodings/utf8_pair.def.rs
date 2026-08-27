// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Stores a tuple of UTF-8 strings.
#[rerun::rerun_type]
#[python(aliases = "Tuple[encodings.Utf8Like, encodings.Utf8Like]")]
#[python(array_aliases = "npt.NDArray[np.str_]")]
#[rust(arrow_opt)]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rerun(state = "stable")]
pub struct Utf8Pair {
    /// The first string.
    pub first: rerun::encodings::Utf8,

    /// The second string.
    pub second: rerun::encodings::Utf8,
}
