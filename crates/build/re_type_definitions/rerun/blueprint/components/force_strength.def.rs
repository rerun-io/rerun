// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The strength of a given force.
///
/// Allows to assign different weights to the individual forces, prioritizing one over the other.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, Copy, PartialEq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ForceStrength {
    pub distance: rerun::datatypes::Float64,
}
