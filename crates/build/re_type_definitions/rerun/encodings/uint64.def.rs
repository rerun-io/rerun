// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 64bit unsigned integer.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "int")]
#[python(array_aliases = "int | npt.NDArray[np.uint64]")]
#[rust(derive(Default, Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(override_crate = "re_types_core")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct UInt64 {
    pub value: u64,
}
