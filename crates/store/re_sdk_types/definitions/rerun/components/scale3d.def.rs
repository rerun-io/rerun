// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 3D scale factor.
///
/// A scale of 1.0 means no scaling.
/// A scale of 2.0 means doubling the size.
/// Each component scales along the corresponding axis.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Scale3D {
    pub scale: rerun::datatypes::Vec3D,
}
