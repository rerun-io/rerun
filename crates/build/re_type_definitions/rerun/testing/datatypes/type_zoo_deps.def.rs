// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Default, Eq, PartialEq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct PrimitiveComponent {
    pub value: u32,
}

#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Default, Eq, PartialEq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct StringComponent {
    pub value: String,
}
