// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Integer index of a voxel in a sparse 3D voxel grid.
///
/// The voxel center in local grid coordinates is `(index + 0.5) * voxel_size`.
#[rerun::rerun_type]
#[python(aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[int]")]
#[python(
    array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[int]] | Sequence[int]"
)]
#[rust(derive(Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct VoxelIndex {
    pub index: rerun::datatypes::IVec3D,
}
