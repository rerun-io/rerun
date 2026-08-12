// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A double-precision vector in 2D space.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[float]")]
#[python(
    array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[float]] | Sequence[float]"
)]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct DVec2D {
    pub xy: [f64; 2],
}
