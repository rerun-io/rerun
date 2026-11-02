// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A boolean value.
///
/// The Viewer shows it as a checkbox.
/// Boolean table columns without a component type are shown with this component.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Bool {
    pub value: rerun::encodings::Bool,
}
