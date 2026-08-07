// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Optional scalar occupancy or value associated with a voxel.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct VoxelValue {
    pub value: rerun::datatypes::Float32,
}
