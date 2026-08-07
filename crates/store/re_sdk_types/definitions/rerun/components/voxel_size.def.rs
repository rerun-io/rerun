// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The scene-unit dimensions of one voxel in a sparse 3D voxel grid.
///
/// Each component is the size of a voxel along the corresponding local grid axis.
/// All components must be finite and positive.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct VoxelSize {
    pub xyz: rerun::datatypes::Vec3D,
}
