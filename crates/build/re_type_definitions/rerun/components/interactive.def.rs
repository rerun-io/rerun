// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether the entity can be interacted with.
///
/// Non interactive components are still visible, but mouse interactions in the view are disabled.
#[rerun::rerun_type]
#[rerun(state = "stable")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Interactive {
    pub interactive: rerun::datatypes::Bool,
}
