// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A uint32 vector in 2D space.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[int]")]
#[python(
    array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[int]] | Sequence[int]"
)]
#[rust(derive(Default, Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct UVec2D {
    pub xy: [u32; 2],
}
