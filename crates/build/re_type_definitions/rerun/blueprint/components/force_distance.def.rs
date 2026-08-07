// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The target distance between two nodes.
///
/// This is helpful to scale the layout, for example if long labels are involved.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, Copy, PartialEq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ForceDistance {
    pub distance: rerun::datatypes::Float64,
}
