// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Range of expected or valid values, specifying a lower and upper bound.
#[rerun::rerun_type]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
pub struct ValueRange {
    pub range: rerun::datatypes::Range1D,
}
