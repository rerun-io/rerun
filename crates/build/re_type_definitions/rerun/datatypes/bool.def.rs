// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A single boolean.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "bool")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(override_crate = "re_types_core")]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Bool {
    pub value: bool,
}
