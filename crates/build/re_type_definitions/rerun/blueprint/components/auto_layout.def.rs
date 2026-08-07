// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether the viewport layout is determined automatically.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct AutoLayout {
    pub auto_layout: rerun::datatypes::Bool,
}
