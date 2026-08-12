// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The origin of a view.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ViewOrigin {
    pub value: rerun::datatypes::EntityPath,
}
