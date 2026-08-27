// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Angle in radians.
#[rerun::rerun_type]
#[arrow(transparent)]
#[cpp(no_field_ctors)]
#[python(aliases = "float | int")]
#[python(array_aliases = "npt.ArrayLike | Sequence[float] | Sequence[int]")]
#[rust(derive(
    Copy,
    Default,
    PartialEq,
    PartialOrd,
    bytemuck::Pod,
    bytemuck::Zeroable
))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Angle {
    /// Angle in radians. One turn is equal to 2π (or τ) radians.
    #[cpp(rename_field = "angle_radians")]
    pub radians: f32,
}
