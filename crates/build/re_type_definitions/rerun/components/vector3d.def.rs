// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A vector in 3D space.
#[rerun::rerun_type]
#[rerun(state = "stable")]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
pub struct Vector3D {
    pub vector: rerun::datatypes::Vec3D,
}
