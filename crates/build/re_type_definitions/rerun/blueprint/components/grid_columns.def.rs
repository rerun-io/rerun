// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How many columns a grid container should have.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rerun(state = "unstable")]
pub struct GridColumns {
    /// The number of columns.
    pub columns: rerun::datatypes::UInt32,
}
