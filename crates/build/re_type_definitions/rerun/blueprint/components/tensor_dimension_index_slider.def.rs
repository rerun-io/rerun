// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Show a slider for the index of some dimension of a slider.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Hash, PartialEq, Eq, Default))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TensorDimensionIndexSlider {
    pub selection: rerun::blueprint::datatypes::TensorDimensionIndexSlider,
}
