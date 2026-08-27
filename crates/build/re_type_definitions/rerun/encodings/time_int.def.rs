// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 64-bit number describing either nanoseconds OR sequence numbers.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "int")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(override_crate = "re_types_core")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct TimeInt {
    pub value: i64,
}
