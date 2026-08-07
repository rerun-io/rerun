// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Camera projection, from image coordinates to view coordinates.
///
/// Child from parent.
/// Image coordinates from camera view coordinates.
///
/// Example:
/// ```text
/// 1496.1     0.0  980.5
///    0.0  1496.1  744.5
///    0.0     0.0    1.0
/// ```
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rerun(state = "stable")]
pub struct PinholeProjection {
    pub image_from_camera: rerun::datatypes::Mat3x3,
}
