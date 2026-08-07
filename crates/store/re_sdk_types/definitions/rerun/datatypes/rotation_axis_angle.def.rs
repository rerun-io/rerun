// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D rotation represented by a rotation around a given axis.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq))]
#[rerun(state = "stable")]
pub struct RotationAxisAngle {
    /// Axis to rotate around.
    ///
    /// This is not required to be normalized.
    /// However, if normalization of the rotation axis fails (typically due to a zero vector)
    /// the rotation is treated as an invalid transform, unless the angle is zero in which case
    /// it is treated as an identity.
    pub axis: rerun::datatypes::Vec3D,

    /// How much to rotate around the axis.
    pub angle: rerun::datatypes::Angle,
}
