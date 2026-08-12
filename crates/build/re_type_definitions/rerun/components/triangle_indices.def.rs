// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The three indices of a triangle in a triangle mesh.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TriangleIndices {
    pub indices: rerun::datatypes::UVec3D,
}
