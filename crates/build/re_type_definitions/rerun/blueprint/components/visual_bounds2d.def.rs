// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Visual bounds in 2D space used for `Spatial2DView`.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct VisualBounds2D {
    /// X and y ranges that should be visible.
    pub range2d: rerun::datatypes::Range2D,
}
