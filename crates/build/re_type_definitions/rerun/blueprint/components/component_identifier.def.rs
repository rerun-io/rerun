// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Identifies a component by fully qualified name.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ComponentIdentifier {
    pub value: rerun::encodings::Utf8,
}
