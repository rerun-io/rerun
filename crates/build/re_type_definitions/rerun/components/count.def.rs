// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A generic count value.
///
/// Used for counting various entities like messages, schemas, channels, etc.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(array_aliases = "int | npt.NDArray[np.uint64]")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Count {
    pub value: rerun::datatypes::UInt64,
}
