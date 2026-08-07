// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A position in 3D space.
#[rerun::rerun_type]
#[python(aliases = "npt.NDArray[np.float32] | Sequence[float] | Tuple[float, float, float]")]
#[python(array_aliases = "npt.NDArray[np.float32] | Sequence[float]")]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Position3D {
    pub xyz: rerun::datatypes::Vec3D,
}
