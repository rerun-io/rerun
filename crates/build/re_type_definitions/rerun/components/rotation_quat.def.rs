// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 3D rotation expressed as a quaternion.
///
/// Note: although the x,y,z,w components of the quaternion will be passed through to the
/// datastore as provided, when used in the Viewer, quaternions will always be normalized.
/// If normalization fails the rotation is treated as an invalid transform.
#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct RotationQuat {
    pub quaternion: rerun::datatypes::Quaternion,
}
