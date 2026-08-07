// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D rotation represented by a rotation around a given axis.
///
/// If normalization of the rotation axis fails the rotation is treated as an invalid transform, unless the
/// angle is zero in which case it is treated as an identity.
#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct RotationAxisAngle {
    pub rotation: rerun::datatypes::RotationAxisAngle,
}
