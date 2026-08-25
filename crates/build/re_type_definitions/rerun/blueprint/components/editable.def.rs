// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether a table column's values can be edited.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct Editable {
    pub editable: rerun::encodings::Bool,
}
